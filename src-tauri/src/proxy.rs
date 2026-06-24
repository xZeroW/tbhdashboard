use crate::capture;
use crate::commands::ProxyStatus;
use crate::state::StateRepository;
use crate::utils::utc_now_iso;
use http_body_util::BodyExt;
use hudsucker::{
    Body, HttpContext, HttpHandler, Proxy, RequestOrResponse,
    certificate_authority::RcgenAuthority,
    decode_response,
    hyper::{
        Request, Response, StatusCode,
        header::{CONTENT_LENGTH, CONTENT_TYPE, HOST},
    },
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
    net::SocketAddr,
    path::PathBuf,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, AtomicUsize, Ordering},
    },
};
use tauri::{AppHandle, Emitter};
use tokio::sync::mpsc;

const DEFAULT_PORT: u16 = 8080;
const TARGET_HOST: &str = "thebackend.io";
const MAX_DIAGNOSTIC_LOGS: usize = 50;

static INTERCEPT_LOGS: AtomicUsize = AtomicUsize::new(0);
static TARGET_REQUEST_LOGS: AtomicUsize = AtomicUsize::new(0);
static PROXY_REQUEST_LOGS: AtomicUsize = AtomicUsize::new(0);

pub struct ProxyManager {
    shutdown_tx: Option<tokio::sync::oneshot::Sender<()>>,
    proxy_handle: Option<tauri::async_runtime::JoinHandle<()>>,
    status: Arc<Mutex<ProxyStatus>>,
    freeze_queue: Arc<AtomicBool>,
    last_queue_response: Arc<Mutex<Option<CachedResponse>>>,
    force_drop_item_id: Arc<Mutex<Option<i64>>>,
}

impl ProxyManager {
    pub fn new(
        status: Arc<Mutex<ProxyStatus>>,
        freeze_queue: Arc<AtomicBool>,
        force_drop_item_id: Arc<Mutex<Option<i64>>>,
    ) -> Self {
        Self {
            shutdown_tx: None,
            proxy_handle: None,
            status,
            freeze_queue,
            last_queue_response: Arc::new(Mutex::new(None)),
            force_drop_item_id,
        }
    }

    fn set_status(&self, running: bool, state: &str, message: impl Into<String>) {
        *self.status.lock().unwrap() = ProxyStatus {
            running,
            state: state.to_string(),
            message: message.into(),
        };
    }

    pub fn start(&mut self, app: &AppHandle, repo: &StateRepository) {
        self.set_status(false, "starting", "Starting");
        let _ = app.emit("proxy-status-changed", ());

        let (event_tx, mut event_rx) = mpsc::unbounded_channel::<Value>();
        let (shutdown_tx, shutdown_rx) = tokio::sync::oneshot::channel::<()>();
        self.shutdown_tx = Some(shutdown_tx);

        let addr = SocketAddr::from(([127, 0, 0, 1], DEFAULT_PORT));
        let app_handle = app.clone();
        let status_handle = self.status.clone();
        let repo_clone = Arc::new(Mutex::new(StateRepository::new(repo.path.clone())));

        let freeze_queue = self.freeze_queue.clone();
        let last_queue_response = self.last_queue_response.clone();
        let force_drop_item_id = self.force_drop_item_id.clone();
        let proxy_handle = tauri::async_runtime::spawn(async move {
            match run_proxy(
                addr,
                event_tx,
                shutdown_rx,
                freeze_queue,
                last_queue_response,
                force_drop_item_id,
            )
            .await
            {
                Ok(()) => {
                    println!("[TBH] Proxy shutdown complete");
                    *status_handle.lock().unwrap() = ProxyStatus {
                        running: false,
                        state: "stopped".to_string(),
                        message: "Stopped".to_string(),
                    };
                }
                Err(e) => {
                    let msg = e.to_string();
                    eprintln!("[TBH] Proxy error: {msg}");
                    let user_msg = if msg.contains("address already in use")
                        || msg.contains("Address already in use")
                    {
                        format!(
                            "Proxy port {DEFAULT_PORT} is already in use. Close the other app using it and restart the dashboard."
                        )
                    } else {
                        format!("Proxy error: {msg}")
                    };
                    *status_handle.lock().unwrap() = ProxyStatus {
                        running: false,
                        state: "error".to_string(),
                        message: user_msg,
                    };
                }
            }
            let _ = app_handle.emit("proxy-status-changed", ());
        });

        let app_handle = app.clone();
        tauri::async_runtime::spawn(async move {
            while let Some(value) = event_rx.recv().await {
                let line = serde_json::to_string(&value).unwrap_or_default();
                let parsed = capture::parse_sidecar_line(&line);
                let repo_guard = repo_clone.lock().unwrap();
                capture::apply_sidecar_event(parsed, &repo_guard);
                let _ = app_handle.emit("state-changed", ());
            }
        });

        self.proxy_handle = Some(proxy_handle);
        self.set_status(true, "running", "Running");
        let _ = app.emit("proxy-status-changed", ());
        println!("[TBH] Proxy started on {addr}");
    }

    pub fn stop(&mut self) {
        if let Some(tx) = self.shutdown_tx.take() {
            let _ = tx.send(());
        }
        if let Some(handle) = self.proxy_handle.take() {
            handle.abort();
        }
        self.set_status(false, "stopped", "Stopped");
        println!("[TBH] Proxy stopped");
    }
}

impl Drop for ProxyManager {
    fn drop(&mut self) {
        self.stop();
    }
}

async fn run_proxy(
    addr: SocketAddr,
    event_tx: mpsc::UnboundedSender<Value>,
    shutdown_rx: tokio::sync::oneshot::Receiver<()>,
    freeze_queue: Arc<AtomicBool>,
    last_queue_response: Arc<Mutex<Option<CachedResponse>>>,
    force_drop_item_id: Arc<Mutex<Option<i64>>>,
) -> anyhow::Result<()> {
    let (cert_pem, key_pem, ca_cert_path) = load_or_create_ca(None, None)?;

    let key_pair = KeyPair::from_pem(&key_pem)
        .map_err(|e| anyhow::anyhow!("failed to parse sidecar CA private key: {e}"))?;
    let issuer = Issuer::from_ca_cert_pem(&cert_pem, key_pair)
        .map_err(|e| anyhow::anyhow!("failed to parse sidecar CA certificate: {e}"))?;
    let provider = aws_lc_rs::default_provider();
    let ca = RcgenAuthority::new(issuer, 1_000, provider.clone());

    eprintln!("[TBH-sidecar] Hudsucker proxy loaded, waiting for traffic on {addr}");
    eprintln!("[TBH-sidecar] CA certificate: {}", ca_cert_path.display());

    let proxy = Proxy::builder()
        .with_addr(addr)
        .with_ca(ca)
        .with_rustls_connector(provider)
        .with_http_handler(TbhHandler::new(
            event_tx,
            freeze_queue,
            last_queue_response,
            force_drop_item_id,
        ))
        .with_graceful_shutdown(async move {
            let _ = shutdown_rx.await;
        })
        .build()
        .map_err(|e| anyhow::anyhow!("failed to create Hudsucker proxy: {e}"))?;

    proxy
        .start()
        .await
        .map_err(|e| anyhow::anyhow!("failed to start Hudsucker proxy: {e}"))
}

#[derive(Clone, Debug)]
struct TbhHandler {
    source: Option<String>,
    method: Option<String>,
    path: Option<String>,
    event_tx: mpsc::UnboundedSender<Value>,
    freeze_queue: Arc<AtomicBool>,
    last_queue_response: Arc<Mutex<Option<CachedResponse>>>,
    force_drop_item_id: Arc<Mutex<Option<i64>>>,
}

#[derive(Clone, Debug)]
struct CachedResponse {
    status: StatusCode,
    content_type: Option<String>,
    body: Vec<u8>,
}

impl TbhHandler {
    fn new(
        event_tx: mpsc::UnboundedSender<Value>,
        freeze_queue: Arc<AtomicBool>,
        last_queue_response: Arc<Mutex<Option<CachedResponse>>>,
        force_drop_item_id: Arc<Mutex<Option<i64>>>,
    ) -> Self {
        Self {
            source: None,
            method: None,
            path: None,
            event_tx,
            freeze_queue,
            last_queue_response,
            force_drop_item_id,
        }
    }

    fn emit(&self, event: Value) {
        let _ = self.event_tx.send(event);
    }

    fn clear_tracked_request(&mut self) {
        self.source = None;
        self.method = None;
        self.path = None;
    }
}

impl HttpHandler for TbhHandler {
    async fn handle_request(
        &mut self,
        _ctx: &HttpContext,
        req: Request<Body>,
    ) -> RequestOrResponse {
        let Some(info) = RequestInfo::from_request(&req) else {
            self.clear_tracked_request();
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

        if !info.host.contains(TARGET_HOST) {
            self.clear_tracked_request();
            return req.into();
        }

        let is_interesting = is_interesting(&info.host, &info.path);

        if info.path.contains("/data/gameLog") {
            eprintln!("[TBH] Blocked request: {}", info.source);
            self.clear_tracked_request();
            return Response::builder()
                .status(StatusCode::OK)
                .header(CONTENT_TYPE, "application/json")
                .body(Body::from("{}"))
                .unwrap()
                .into();
        }

        self.source = Some(info.source.clone());
        self.method = Some(info.method.clone());
        self.path = Some(info.path.clone());

        let content_type = req
            .headers()
            .get(CONTENT_TYPE)
            .and_then(|value| value.to_str().ok())
            .unwrap_or_default()
            .to_string();

        let (parts, body) = req.into_parts();
        let body_bytes = match body.collect().await {
            Ok(collected) => collected.to_bytes(),
            Err(err) => {
                eprintln!("[TBH-sidecar] failed to read request body: {err}");
                return bad_gateway();
            }
        };

        self.emit(json!({
            "type": "request_log",
            "at": utc_now_iso(),
            "method": info.method.clone(),
            "host": info.host.clone(),
            "path": info.full_path.clone(),
            "source": info.source.clone(),
            "contentType": content_type,
            "bodyBytes": body_bytes.len(),
            "body": String::from_utf8_lossy(&body_bytes).into_owned(),
        }));

        if should_replay_frozen_queue(
            &info,
            &body_bytes,
            self.freeze_queue.load(Ordering::Relaxed),
        ) {
            let cached = self.last_queue_response.lock().unwrap().clone();
            if let Some(cached) = cached {
                eprintln!("[TBH-sidecar] freeze queue replayed: {}", info.source);
                self.clear_tracked_request();
                return cached_queue_response(cached);
            }

            eprintln!(
                "[TBH-sidecar] freeze queue has no cached response, forwarding: {}",
                info.source
            );
        }

        if !is_interesting {
            return Request::from_parts(parts, Body::from(body_bytes)).into();
        }

        if info.path.contains("/backend-function/base/v1")
            && let Ok(req_json) = serde_json::from_slice::<Value>(&body_bytes)
        {
            let claimed_keys = mark_claimed_from_backend_request(&req_json);
            if !claimed_keys.is_empty() {
                self.emit(json!({
                    "type": "claimed",
                    "count": claimed_keys.len(),
                    "source": info.source,
                    "keys": claimed_keys,
                }));
            }

            if let Some(pb_info) = get_processbox_info(&req_json)
                && let Some(description) = describe_processbox_request(&pb_info)
            {
                self.emit(json!({
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
        let method = self.method.clone().unwrap_or_default();
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
        let status = parts.status;
        let content_type = parts
            .headers
            .get(CONTENT_TYPE)
            .and_then(|value| value.to_str().ok())
            .map(str::to_string);
        let body_bytes = match body.collect().await {
            Ok(collected) => collected.to_bytes(),
            Err(err) => {
                eprintln!("[TBH-sidecar] failed to read response body: {err}");
                return Response::from_parts(parts, Body::empty());
            }
        };

        self.emit(json!({
            "type": "response_log",
            "at": utc_now_iso(),
            "source": source.clone(),
            "body": String::from_utf8_lossy(&body_bytes).into_owned(),
            "body_bytes": body_bytes.len(),
        }));

        if let Ok(ref mut obj) = serde_json::from_slice::<Value>(&body_bytes) {
            let added_items = extract_added_from_any_json(obj);
            if !added_items.is_empty() {
                self.emit(json!({
                    "type": "added_items",
                    "count": added_items.len(),
                    "source": source,
                    "items": added_items,
                }));
            }

            let chests = extract_chests_from_any_json(obj);
            if !chests.is_empty() {
                if chests.len() >= 40 || path.contains("UserInventory") {
                    self.emit(json!({
                        "type": "chests_synced",
                        "count": chests.len(),
                        "old": 0,
                        "source": source,
                        "chests": chests,
                    }));
                } else {
                    self.emit(json!({
                        "type": "chests_upserted",
                        "added": chests.len(),
                        "updated": 0,
                        "source": source,
                        "chests": chests,
                    }));
                }
            }
        }

        // Force drop: rewrite rewardItemId in claimed boxes
        let body_bytes = {
            let mut guard = self.force_drop_item_id.lock().unwrap();
            if let Some(target_id) = *guard {
                if let Ok(mut obj) = serde_json::from_slice::<Value>(&body_bytes) {
                    if modify_force_drop(&mut obj, target_id) {
                        *guard = None;
                        serde_json::to_vec(&obj).unwrap_or(body_bytes.to_vec())
                    } else {
                        body_bytes.to_vec()
                    }
                } else {
                    body_bytes.to_vec()
                }
            } else {
                body_bytes.to_vec()
            }
        };

        if method.eq_ignore_ascii_case("POST") && path.contains("/backend-function/base/v1") {
            *self.last_queue_response.lock().unwrap() = Some(CachedResponse {
                status,
                content_type,
                body: body_bytes.to_vec(),
            });
        }

        let mut res_parts = parts;
        res_parts
            .headers
            .insert(CONTENT_LENGTH, body_bytes.len().into());

        Response::from_parts(res_parts, Body::from(body_bytes))
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

struct RequestInfo {
    method: String,
    host: String,
    path: String,
    full_path: String,
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
        let full_path = req
            .uri()
            .path_and_query()
            .map(|p| p.as_str().to_string())
            .unwrap_or_else(|| "/".to_string());
        let path_no_query = full_path.split('?').next().unwrap_or("/").to_string();
        let source = format!("{} {}{}", req.method(), host, path_no_query);
        Some(Self {
            method: req.method().to_string(),
            host,
            path: path_no_query,
            full_path,
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

fn cached_queue_response(cached: CachedResponse) -> RequestOrResponse {
    let mut builder = Response::builder().status(cached.status);
    if let Some(content_type) = cached.content_type {
        builder = builder.header(CONTENT_TYPE, content_type);
    }
    builder.body(Body::from(cached.body)).unwrap().into()
}

fn should_replay_frozen_queue(info: &RequestInfo, body_bytes: &[u8], freeze_queue: bool) -> bool {
    if !freeze_queue
        || !info.host.contains(TARGET_HOST)
        || !info.method.eq_ignore_ascii_case("POST")
        || !info.path.contains("/backend-function/base/v1")
    {
        return false;
    }

    let Ok(req_json) = serde_json::from_slice::<Value>(body_bytes) else {
        return false;
    };
    let Some(action) = req_json
        .get("functionBody")
        .and_then(|v| v.get("body"))
        .and_then(|v| v.get("action"))
        .and_then(Value::as_str)
    else {
        return false;
    };
    action.starts_with("processBox")
}

fn is_interesting(host: &str, path: &str) -> bool {
    host.contains(TARGET_HOST)
        && (path.contains("/backend-function/base/v1")
            || path.contains("UserInventory")
            || path.contains("SteamItemInfo"))
}

fn safe_int(value: Option<&Value>) -> Option<i64> {
    match value {
        Some(Value::Number(n)) => n.as_i64().or_else(|| n.as_u64().map(|n| n as i64)),
        Some(Value::String(s)) => s.parse::<i64>().ok(),
        _ => None,
    }
}

fn json_value_to_string(value: &Value) -> String {
    match value {
        Value::String(s) => s.clone(),
        Value::Null => String::new(),
        other => other.to_string(),
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

pub fn extract_chests_from_any_json(obj: &Value) -> Vec<Value> {
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
        Value::String(s) => {
            if let Ok(parsed) = serde_json::from_str(s) {
                found.extend(extract_chests_from_any_json(&parsed));
            }
        }
        _ => {}
    }

    found
}

pub fn extract_added_from_any_json(obj: &Value) -> Vec<Value> {
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
        Value::String(s) => {
            if let Ok(parsed) = serde_json::from_str(s) {
                extract_added_inner(&parsed, found);
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
    if !body.get("action")?.as_str()?.starts_with("processBox") {
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
    let fields: &[&str] = if action.starts_with("processBox") {
        &[
            "useItemKeyList",
            "useItemKeys",
            "openItemKeyList",
            "boxKeyList",
        ]
    } else if action == "exchange" {
        &["itemKey", "itemKeys", "useItemKeyList"]
    } else {
        &["useItemKeyList", "itemKey", "itemKeys", "openItemKeyList"]
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

fn load_or_create_ca(
    cert_path: Option<PathBuf>,
    key_path: Option<PathBuf>,
) -> anyhow::Result<(String, String, PathBuf)> {
    let cert_path = cert_path.unwrap_or_else(default_ca_cert_path);
    let key_path = key_path.unwrap_or_else(default_ca_key_path);

    if cert_path.exists() && key_path.exists() {
        let cert_pem = fs::read_to_string(&cert_path).map_err(|e| {
            anyhow::anyhow!("failed to read CA certificate {}: {e}", cert_path.display())
        })?;
        let key_pem = fs::read_to_string(&key_path).map_err(|e| {
            anyhow::anyhow!("failed to read CA private key {}: {e}", key_path.display())
        })?;
        return Ok((cert_pem, key_pem, cert_path));
    }

    let parent = cert_path.parent().ok_or_else(|| {
        anyhow::anyhow!("CA certificate path has no parent: {}", cert_path.display())
    })?;
    fs::create_dir_all(parent)
        .map_err(|e| anyhow::anyhow!("failed to create CA directory {}: {e}", parent.display()))?;
    if let Some(parent) = key_path.parent() {
        fs::create_dir_all(parent).map_err(|e| {
            anyhow::anyhow!(
                "failed to create CA key directory {}: {e}",
                parent.display()
            )
        })?;
    }

    let key_pair = KeyPair::generate()
        .map_err(|e| anyhow::anyhow!("failed to generate sidecar CA private key: {e}"))?;
    let mut params = CertificateParams::new(vec!["TaskBarHero Dashboard Local CA".to_string()])
        .map_err(|e| anyhow::anyhow!("failed to create CA certificate params: {e}"))?;
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
        .map_err(|e| anyhow::anyhow!("failed to generate sidecar CA certificate: {e}"))?;
    let cert_pem = cert.pem();
    let key_pem = key_pair.serialize_pem();

    fs::write(&cert_path, &cert_pem).map_err(|e| {
        anyhow::anyhow!(
            "failed to write CA certificate {}: {e}",
            cert_path.display()
        )
    })?;
    fs::write(&key_path, &key_pem).map_err(|e| {
        anyhow::anyhow!("failed to write CA private key {}: {e}", key_path.display())
    })?;

    Ok((cert_pem, key_pem, cert_path))
}

fn modify_force_drop(obj: &mut Value, target_id: i64) -> bool {
    let mut modified = false;
    modify_force_drop_inner(obj, target_id, &mut modified);
    modified
}

fn modify_force_drop_inner(obj: &mut Value, target_id: i64, modified: &mut bool) {
    match obj {
        Value::Object(map) => {
            if let Some(Value::String(result)) = map.get("result")
                && let Ok(parsed) = serde_json::from_str::<Value>(result)
            {
                let mut parsed = parsed;
                modify_force_drop_inner(&mut parsed, target_id, modified);
                if *modified {
                    map.insert(
                        "result".to_string(),
                        Value::String(serde_json::to_string(&parsed).unwrap_or_default()),
                    );
                }
            }

            if let Some(Value::Object(data)) = map.get_mut("data")
                && let Some(Value::Array(boxes)) = data.get_mut("boxes")
            {
                for box_val in boxes.iter_mut() {
                    if let Some(box_obj) = box_val.as_object_mut() {
                        let is_get = box_obj.get("isGet").and_then(|v| {
                            if v.as_bool() == Some(true) || v.as_str() == Some("true") {
                                Some(true)
                            } else {
                                None
                            }
                        });
                        if is_get == Some(true) {
                            box_obj.insert(
                                "rewardItemId".to_string(),
                                Value::Number(serde_json::Number::from(target_id)),
                            );
                            *modified = true;
                        }
                    }
                }
            }

            static SKIP: [&str; 2] = ["result", "data"];
            for (key, value) in map.iter_mut() {
                if !SKIP.contains(&key.as_str()) && (value.is_object() || value.is_array()) {
                    modify_force_drop_inner(value, target_id, modified);
                }
            }
        }
        Value::Array(items) => {
            for item in items.iter_mut() {
                modify_force_drop_inner(item, target_id, modified);
            }
        }
        Value::String(s) => {
            if let Ok(parsed) = serde_json::from_str(s) {
                let mut parsed = parsed;
                modify_force_drop_inner(&mut parsed, target_id, modified);
                if *modified {
                    *s = serde_json::to_string(&parsed).unwrap_or_default();
                }
            }
        }
        _ => {}
    }
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
