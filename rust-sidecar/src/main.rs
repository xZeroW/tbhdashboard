use anyhow::{Context, Result, anyhow};
use http_body_util::BodyExt;
use hudsucker::{
    Body, HttpContext, HttpHandler, Proxy, RequestOrResponse,
    certificate_authority::RcgenAuthority,
    decode_response,
    hyper::{Request, Response, StatusCode, header::HOST},
    rcgen::{
        BasicConstraints, CertificateParams, DistinguishedName, DnType, IsCa, Issuer, KeyPair,
        KeyUsagePurpose,
    },
    rustls::crypto::aws_lc_rs,
};
use serde_json::{Value, json};
use std::{
    collections::HashMap,
    fs,
    io::{self, Write},
    net::SocketAddr,
    path::PathBuf,
    sync::atomic::{AtomicUsize, Ordering},
};

const DEFAULT_PORT: u16 = 8080;
const TARGET_HOST: &str = "thebackend.io";
const MAX_DIAGNOSTIC_LOGS: usize = 50;

static INTERCEPT_LOGS: AtomicUsize = AtomicUsize::new(0);
static TARGET_REQUEST_LOGS: AtomicUsize = AtomicUsize::new(0);
static PROXY_REQUEST_LOGS: AtomicUsize = AtomicUsize::new(0);

#[derive(Clone, Debug, Default)]
struct TbhHandler {
    source: Option<String>,
    path: Option<String>,
}

impl HttpHandler for TbhHandler {
    async fn handle_request(
        &mut self,
        _ctx: &HttpContext,
        req: Request<Body>,
    ) -> RequestOrResponse {
        let Some(info) = RequestInfo::from_request(&req) else {
            self.source = None;
            self.path = None;
            return req.into();
        };

        log_limited(
            &PROXY_REQUEST_LOGS,
            format_args!("proxied request: {}", info.source),
        );

        if info.host.contains(TARGET_HOST) {
            log_limited(
                &TARGET_REQUEST_LOGS,
                format_args!("target request: {}", info.source),
            );
        }

        if !is_interesting(&info.host, &info.path) {
            self.source = None;
            self.path = None;
            return req.into();
        }

        self.source = Some(info.source.clone());
        self.path = Some(info.path.clone());

        let (parts, body) = req.into_parts();
        let body_bytes = match body.collect().await {
            Ok(collected) => collected.to_bytes(),
            Err(err) => {
                eprintln!("[TBH-sidecar] failed to read request body: {err}");
                return bad_gateway();
            }
        };

        if info.path.contains("/backend-function/base/v1")
            && let Ok(req_json) = serde_json::from_slice::<Value>(&body_bytes)
        {
            let claimed_keys = mark_claimed_from_backend_request(&req_json);
            if !claimed_keys.is_empty() {
                emit(json!({
                    "type": "claimed",
                    "count": claimed_keys.len(),
                    "source": info.source,
                    "keys": claimed_keys,
                }));
            }

            if let Some(pb_info) = get_processbox_info(&req_json)
                && let Some(description) = describe_processbox_request(&pb_info)
            {
                emit(json!({
                    "type": "process_box",
                    "info": pb_info,
                    "description": description,
                }));
            }
        }

        Request::from_parts(parts, Body::from(body_bytes)).into()
    }

    async fn handle_response(&mut self, _ctx: &HttpContext, res: Response<Body>) -> Response<Body> {
        let Some(source) = self.source.clone() else {
            return res;
        };
        let path = self.path.clone().unwrap_or_default();

        let res = match decode_response(res) {
            Ok(res) => res,
            Err(err) => {
                eprintln!("[TBH-sidecar] failed to decode response body: {err}");
                return Response::builder()
                    .status(StatusCode::BAD_GATEWAY)
                    .body(Body::empty())
                    .unwrap();
            }
        };

        let (parts, body) = res.into_parts();
        let body_bytes = match body.collect().await {
            Ok(collected) => collected.to_bytes(),
            Err(err) => {
                eprintln!("[TBH-sidecar] failed to read response body: {err}");
                return Response::from_parts(parts, Body::empty());
            }
        };

        if let Ok(obj) = serde_json::from_slice::<Value>(&body_bytes) {
            let added_items = extract_added_from_any_json(&obj);
            if !added_items.is_empty() {
                emit(json!({
                    "type": "added_items",
                    "count": added_items.len(),
                    "source": source,
                    "items": added_items,
                }));
            }

            let chests = extract_chests_from_any_json(&obj);
            if !chests.is_empty() {
                if chests.len() >= 40 || path.contains("UserInventory") {
                    emit(json!({
                        "type": "chests_synced",
                        "count": chests.len(),
                        "old": 0,
                        "source": source,
                        "chests": chests,
                    }));
                } else {
                    emit(json!({
                        "type": "chests_upserted",
                        "added": chests.len(),
                        "updated": 0,
                        "source": source,
                        "chests": chests,
                    }));
                }
            }
        }

        Response::from_parts(parts, Body::from(body_bytes))
    }

    async fn should_intercept(&mut self, _ctx: &HttpContext, req: &Request<Body>) -> bool {
        let Some(authority) = req.uri().authority().map(|authority| authority.as_str()) else {
            return false;
        };
        log_limited(
            &PROXY_REQUEST_LOGS,
            format_args!("proxied CONNECT authority: {authority}"),
        );

        let should_intercept = authority.contains(TARGET_HOST);
        if should_intercept {
            log_limited(
                &INTERCEPT_LOGS,
                format_args!("intercepting HTTPS authority: {authority}"),
            );
        }
        should_intercept
    }
}

fn log_limited(counter: &AtomicUsize, args: std::fmt::Arguments<'_>) {
    if counter.fetch_add(1, Ordering::Relaxed) < MAX_DIAGNOSTIC_LOGS {
        eprintln!("[TBH-sidecar] {args}");
    }
}

#[derive(Debug)]
struct RequestInfo {
    host: String,
    path: String,
    source: String,
}

impl RequestInfo {
    fn from_request(req: &Request<Body>) -> Option<Self> {
        let host = req.uri().host().map(str::to_string).or_else(|| {
            req.headers()
                .get(HOST)
                .and_then(|h| h.to_str().ok())
                .map(|h| h.split(':').next().unwrap_or(h).to_string())
        })?;
        let path = req
            .uri()
            .path_and_query()
            .map(|p| p.as_str().to_string())
            .unwrap_or_else(|| "/".to_string());
        let path_no_query = path.split('?').next().unwrap_or("/").to_string();
        let source = format!("{} {}{}", req.method(), host, path_no_query);

        Some(Self {
            host,
            path: path_no_query,
            source,
        })
    }
}

fn bad_gateway() -> RequestOrResponse {
    Response::builder()
        .status(StatusCode::BAD_GATEWAY)
        .body(Body::empty())
        .unwrap()
        .into()
}

fn is_interesting(host: &str, path: &str) -> bool {
    host.contains(TARGET_HOST)
        && (path.contains("/backend-function/base/v1")
            || path.contains("UserInventory")
            || path.contains("SteamItemInfo"))
}

fn emit(event: Value) {
    let mut stdout = io::stdout().lock();
    if let Err(err) = serde_json::to_writer(&mut stdout, &event)
        .and_then(|_| writeln!(stdout).map_err(serde_json::Error::io))
        .and_then(|_| stdout.flush().map_err(serde_json::Error::io))
    {
        eprintln!("[TBH-sidecar] failed to emit event: {err}");
    }
}

fn safe_int(value: Option<&Value>) -> Option<i64> {
    match value {
        Some(Value::Number(n)) => n.as_i64().or_else(|| n.as_u64().map(|n| n as i64)),
        Some(Value::String(s)) => s.parse::<i64>().ok(),
        _ => None,
    }
}

fn parse_jsonish_list(value: Option<&Value>) -> Vec<String> {
    match value {
        None | Some(Value::Null) => Vec::new(),
        Some(Value::Array(items)) => items.iter().map(json_value_to_string).collect(),
        Some(Value::Number(_)) | Some(Value::Bool(_)) => {
            value.map(json_value_to_string).into_iter().collect()
        }
        Some(Value::String(s)) => {
            let s = s.trim();
            if s.is_empty() {
                return Vec::new();
            }
            match serde_json::from_str::<Value>(s) {
                Ok(Value::Array(items)) => items.iter().map(json_value_to_string).collect(),
                Ok(parsed) => vec![json_value_to_string(&parsed)],
                Err(_) => vec![s.to_string()],
            }
        }
        Some(other) => vec![json_value_to_string(other)],
    }
}

fn json_value_to_string(value: &Value) -> String {
    match value {
        Value::String(s) => s.clone(),
        Value::Null => String::new(),
        other => other.to_string(),
    }
}

fn box_label(box_id: Option<i64>) -> String {
    match box_id {
        Some(910651) => "Common Treasure Chest".to_string(),
        Some(920651) => "Stage Treasure Chest".to_string(),
        Some(id) if id.to_string().starts_with("910") => format!("Common Treasure Chest ({id})"),
        Some(id) if id.to_string().starts_with("920") => format!("Stage Treasure Chest ({id})"),
        Some(id) => format!("Box {id}"),
        None => "Unknown Chest".to_string(),
    }
}

fn extract_chests_from_any_json(obj: &Value) -> Vec<Value> {
    let mut found = Vec::new();

    match obj {
        Value::Object(map) => {
            if let Some(Value::String(result)) = map.get("result")
                && let Ok(parsed) = serde_json::from_str::<Value>(result)
            {
                found.extend(extract_chests_from_any_json(&parsed));
            }

            if let Some(Value::Object(data)) = map.get("data")
                && let Some(Value::Array(boxes)) = data.get("boxes")
            {
                found.extend(boxes.iter().filter(|b| b.is_object()).cloned());
            }

            for (key, value) in map {
                if key == "items" {
                    let parsed;
                    let value = if let Value::String(s) = value {
                        parsed = serde_json::from_str::<Value>(s).ok();
                        parsed.as_ref().unwrap_or(value)
                    } else {
                        value
                    };

                    if let Value::Array(items) = value {
                        for item in items {
                            if let Value::Object(item_map) = item
                                && (item_map.contains_key("claimableAt")
                                    || item_map.contains_key("rewardItemId"))
                            {
                                found.push(item.clone());
                            }
                        }
                    }
                } else {
                    found.extend(extract_chests_from_any_json(value));
                }
            }
        }
        Value::Array(items) => {
            for item in items {
                found.extend(extract_chests_from_any_json(item));
            }
        }
        _ => {}
    }

    found
}

fn extract_added_from_any_json(obj: &Value) -> Vec<Value> {
    let mut found = Vec::new();
    extract_added_inner(obj, &mut found);

    let mut dedup = HashMap::<String, Value>::new();
    for (idx, item) in found.into_iter().enumerate() {
        let key = item
            .as_object()
            .and_then(|m| {
                ["itemKey", "inDate", "uuid", "itemId"]
                    .iter()
                    .find_map(|field| m.get(*field).map(json_value_to_string))
            })
            .unwrap_or_else(|| format!("idx-{idx}"));
        dedup.insert(key, item);
    }

    dedup.into_values().collect()
}

fn extract_added_inner(obj: &Value, found: &mut Vec<Value>) {
    match obj {
        Value::Object(map) => {
            if let Some(Value::String(result)) = map.get("result")
                && let Ok(parsed) = serde_json::from_str::<Value>(result)
            {
                extract_added_inner(&parsed, found);
            }

            if let Some(Value::Object(data)) = map.get("data")
                && let Some(Value::Array(added)) = data.get("added")
            {
                found.extend(added.iter().filter(|i| i.is_object()).cloned());
            }

            for key in ["added", "rewards", "reward", "items"] {
                let Some(value) = map.get(key) else {
                    continue;
                };
                let parsed;
                let value = if let Value::String(s) = value {
                    parsed = serde_json::from_str::<Value>(s).ok();
                    parsed.as_ref().unwrap_or(value)
                } else {
                    value
                };

                if let Value::Array(items) = value {
                    for item in items {
                        if let Value::Object(item_map) = item {
                            let is_added =
                                item_map.contains_key("itemId") || item_map.contains_key("item_id");
                            let is_chest = item_map.contains_key("claimableAt")
                                || item_map.contains_key("rewardItemId");
                            if is_added && !is_chest {
                                found.push(item.clone());
                            }
                        }
                    }
                }
            }

            for value in map.values() {
                if value.is_object() || value.is_array() {
                    extract_added_inner(value, found);
                }
            }
        }
        Value::Array(items) => {
            for item in items {
                extract_added_inner(item, found);
            }
        }
        _ => {}
    }
}

fn get_processbox_info(req: &Value) -> Option<Value> {
    let req = req.as_object()?;
    if req.get("functionName")?.as_str()? != "inventory" {
        return None;
    }

    let body = req.get("functionBody")?.get("body")?.as_object()?;
    if body.get("action")?.as_str()? != "processBox" {
        return None;
    }

    let mut created = Vec::new();
    let raw = body.get("createItemList");
    let parsed;
    let raw = if let Some(Value::String(s)) = raw {
        parsed = serde_json::from_str::<Value>(s).ok();
        parsed.as_ref()
    } else {
        raw
    };

    if let Some(Value::Array(items)) = raw {
        for item in items {
            if let Some(item) = item.as_object() {
                let item_id = safe_int(item.get("itemId"));
                let count = safe_int(item.get("count")).unwrap_or(0);
                let drop_key = safe_int(item.get("dropKey"));
                created.push(json!({
                    "itemId": item_id,
                    "count": count,
                    "dropKey": drop_key,
                    "name": box_label(item_id),
                }));
            }
        }
    }

    Some(json!({
        "tn": body.get("tn").cloned().unwrap_or(Value::Null),
        "isReset": body.get("isReset").map(json_value_to_string).unwrap_or_default().to_lowercase() == "true",
        "created": created,
        "at": now_iso(),
    }))
}

fn describe_processbox_request(info: &Value) -> Option<String> {
    let created = info.get("created")?.as_array()?;
    if created.is_empty() {
        return None;
    }

    let parts: Vec<String> = created
        .iter()
        .filter_map(|item| {
            let item = item.as_object()?;
            Some(format!(
                "{} x{} dropKey={}",
                item.get("name")
                    .and_then(Value::as_str)
                    .unwrap_or("Unknown Chest"),
                item.get("count").and_then(Value::as_i64).unwrap_or(0),
                item.get("dropKey")
                    .map(json_value_to_string)
                    .unwrap_or_else(|| "null".to_string())
            ))
        })
        .collect();

    Some(format!(
        "processBox requested tn={} reset={}: {}",
        info.get("tn").map(json_value_to_string).unwrap_or_default(),
        info.get("isReset")
            .and_then(Value::as_bool)
            .unwrap_or(false),
        parts.join(", ")
    ))
}

fn mark_claimed_from_backend_request(req: &Value) -> Vec<String> {
    let Some(req) = req.as_object() else {
        return Vec::new();
    };
    if req.get("functionName").and_then(Value::as_str) != Some("inventory") {
        return Vec::new();
    }

    let Some(body) = req
        .get("functionBody")
        .and_then(|v| v.get("body"))
        .and_then(Value::as_object)
    else {
        return Vec::new();
    };

    let action = body
        .get("action")
        .and_then(Value::as_str)
        .unwrap_or_default();
    let fields: &[&str] = match action {
        "processBox" => &[
            "useItemKeyList",
            "useItemKeys",
            "openItemKeyList",
            "boxKeyList",
        ],
        "exchange" => &["itemKey", "itemKeys", "useItemKeyList"],
        _ => &["useItemKeyList", "itemKey", "itemKeys", "openItemKeyList"],
    };

    fields
        .iter()
        .flat_map(|field| parse_jsonish_list(body.get(*field)))
        .filter(|key| !key.is_empty() && !key.starts_with("manual-"))
        .collect()
}

fn now_iso() -> String {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default();
    format!("{}.{:09}Z", now.as_secs(), now.subsec_nanos())
}

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .with_writer(io::stderr)
        .init();

    let args = Args::parse(std::env::args().skip(1));
    let addr = SocketAddr::from(([127, 0, 0, 1], args.port));
    let (cert_pem, key_pem, ca_cert_path) = load_or_create_ca(args.ca_cert, args.ca_key)?;

    let key_pair = KeyPair::from_pem(&key_pem).context("failed to parse sidecar CA private key")?;
    let issuer = Issuer::from_ca_cert_pem(&cert_pem, key_pair)
        .context("failed to parse sidecar CA certificate")?;
    let provider = aws_lc_rs::default_provider();
    let ca = RcgenAuthority::new(issuer, 1_000, provider.clone());

    eprintln!("[TBH-sidecar] Hudsucker addon loaded, waiting for traffic on {addr}");
    eprintln!("[TBH-sidecar] CA certificate: {}", ca_cert_path.display());

    let proxy = Proxy::builder()
        .with_addr(addr)
        .with_ca(ca)
        .with_rustls_connector(provider)
        .with_http_handler(TbhHandler::default())
        .with_graceful_shutdown(shutdown_signal())
        .build()
        .context("failed to create Hudsucker proxy")?;

    proxy
        .start()
        .await
        .context("failed to start Hudsucker proxy")
}

async fn shutdown_signal() {
    let _ = tokio::signal::ctrl_c().await;
}

#[derive(Debug)]
struct Args {
    port: u16,
    ca_cert: Option<PathBuf>,
    ca_key: Option<PathBuf>,
}

impl Args {
    fn parse<I>(args: I) -> Self
    where
        I: IntoIterator<Item = String>,
    {
        let mut port = DEFAULT_PORT;
        let mut ca_cert = std::env::var_os("TBH_CA_CERT").map(PathBuf::from);
        let mut ca_key = std::env::var_os("TBH_CA_KEY").map(PathBuf::from);
        let mut args = args.into_iter();

        while let Some(arg) = args.next() {
            match arg.as_str() {
                "-p" | "--port" => {
                    if let Some(value) = args.next().and_then(|value| value.parse::<u16>().ok()) {
                        port = value;
                    }
                }
                "--listen" | "--bind" => {
                    if let Some(value) = args.next()
                        && let Some(parsed) =
                            value.rsplit(':').next().and_then(|p| p.parse::<u16>().ok())
                    {
                        port = parsed;
                    }
                }
                "--ca-cert" => ca_cert = args.next().map(PathBuf::from),
                "--ca-key" => ca_key = args.next().map(PathBuf::from),
                "--set" => {
                    let _ = args.next();
                }
                _ => {}
            }
        }

        Self {
            port,
            ca_cert,
            ca_key,
        }
    }
}

fn load_or_create_ca(
    cert_path: Option<PathBuf>,
    key_path: Option<PathBuf>,
) -> Result<(String, String, PathBuf)> {
    let cert_path = cert_path.unwrap_or_else(default_ca_cert_path);
    let key_path = key_path.unwrap_or_else(default_ca_key_path);

    if cert_path.exists() && key_path.exists() {
        let cert_pem = fs::read_to_string(&cert_path)
            .with_context(|| format!("failed to read CA certificate {}", cert_path.display()))?;
        let key_pem = fs::read_to_string(&key_path)
            .with_context(|| format!("failed to read CA private key {}", key_path.display()))?;
        return Ok((cert_pem, key_pem, cert_path));
    }

    let parent = cert_path
        .parent()
        .ok_or_else(|| anyhow!("CA certificate path has no parent: {}", cert_path.display()))?;
    fs::create_dir_all(parent)
        .with_context(|| format!("failed to create CA directory {}", parent.display()))?;
    if let Some(parent) = key_path.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("failed to create CA key directory {}", parent.display()))?;
    }

    let key_pair = KeyPair::generate().context("failed to generate sidecar CA private key")?;
    let mut params = CertificateParams::new(vec!["TaskBarHero Dashboard Local CA".to_string()])?;
    params.distinguished_name = DistinguishedName::new();
    params
        .distinguished_name
        .push(DnType::CommonName, "TaskBarHero Dashboard Local CA");
    params.is_ca = IsCa::Ca(BasicConstraints::Unconstrained);
    params.key_usages = vec![
        KeyUsagePurpose::KeyCertSign,
        KeyUsagePurpose::CrlSign,
        KeyUsagePurpose::DigitalSignature,
    ];

    let cert = params
        .self_signed(&key_pair)
        .context("failed to generate sidecar CA certificate")?;
    let cert_pem = cert.pem();
    let key_pem = key_pair.serialize_pem();

    fs::write(&cert_path, &cert_pem)
        .with_context(|| format!("failed to write CA certificate {}", cert_path.display()))?;
    fs::write(&key_path, &key_pem)
        .with_context(|| format!("failed to write CA private key {}", key_path.display()))?;

    Ok((cert_pem, key_pem, cert_path))
}

fn default_ca_cert_path() -> PathBuf {
    dirs::cache_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("tbhdashboard")
        .join("tbh-hudsucker-ca.pem")
}

fn default_ca_key_path() -> PathBuf {
    dirs::cache_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("tbhdashboard")
        .join("tbh-hudsucker-ca-key.pem")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_claimed_keys_from_process_box() {
        let req = json!({
            "functionName": "inventory",
            "functionBody": {
                "body": {
                    "action": "processBox",
                    "useItemKeyList": "[\"a\", \"manual-b\", \"c\"]"
                }
            }
        });

        assert_eq!(mark_claimed_from_backend_request(&req), vec!["a", "c"]);
    }

    #[test]
    fn extracts_nested_chests() {
        let obj = json!({
            "result": "{\"data\":{\"boxes\":[{\"itemKey\":\"k\",\"claimableAt\":1}]}}"
        });

        let chests = extract_chests_from_any_json(&obj);
        assert_eq!(chests.len(), 1);
        assert_eq!(chests[0]["itemKey"], "k");
    }

    #[test]
    fn extracts_processbox_info() {
        let req = json!({
            "functionName": "inventory",
            "functionBody": {
                "body": {
                    "action": "processBox",
                    "tn": "abc",
                    "isReset": "true",
                    "createItemList": "[{\"itemId\":910651,\"count\":2,\"dropKey\":7}]"
                }
            }
        });

        let info = get_processbox_info(&req).unwrap();
        assert_eq!(info["isReset"], true);
        assert_eq!(info["created"][0]["name"], "Common Treasure Chest");
    }
}
