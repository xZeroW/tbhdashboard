use std::collections::HashMap;
use std::io::{BufRead, BufReader, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::path::PathBuf;
use std::process::Command;
use std::sync::{
    Arc, Mutex,
    atomic::{AtomicBool, Ordering},
};
use std::time::Duration;

use tauri::Manager;
use tauri::State;
use tauri_plugin_dialog::DialogExt;

use crate::assets::{AssetDownloadResult, AssetUpdateStatus};
use crate::capture;
use crate::catalog::StaticCatalog;
use crate::chests;
use crate::config;
use crate::models::*;
use crate::nethelper;
use crate::observations::ObservationUploadResult;
use crate::proxy;
use crate::state::StateRepository;

const STEAM_APP_ID: &str = "3678970";
const SYSTEM_PROXY_EVENT_PORT: u16 = 1421;

/// Shared app state managed by Tauri.
pub struct ManagedState {
    repo: StateRepository,
    pub catalog: Mutex<StaticCatalog>,
    proxy_status: Arc<Mutex<ProxyStatus>>,
    freeze_queue: Arc<AtomicBool>,
    system_proxy: Arc<Mutex<Option<std::process::Child>>>,
    system_proxy_running: Arc<AtomicBool>,
    force_drop_item_id: Arc<Mutex<Option<i64>>>,
}

#[derive(serde::Serialize, Clone)]
pub struct ProxyStatus {
    pub running: bool,
    pub state: String,
    pub message: String,
}

#[derive(serde::Serialize, Clone)]
pub struct LaunchGameResult {
    pub ok: bool,
    pub message: String,
}

#[derive(serde::Serialize, serde::Deserialize, Clone, Default)]
#[serde(rename_all = "camelCase")]
pub struct EntitlementInfo {
    pub tier: String,
    pub source: String,
    pub expires_at: Option<String>,
}

#[derive(serde::Serialize, serde::Deserialize, Clone, Default)]
#[serde(rename_all = "camelCase")]
pub struct AuthUser {
    pub id: String,
    pub username: String,
    pub email: Option<String>,
    #[serde(default)]
    pub is_admin: bool,
    pub entitlement: Option<EntitlementInfo>,
}

#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct ServerLoginResponse {
    token: String,
    user: AuthUser,
}

#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct ServerRegisterResponse {
    user_id: String,
    username: String,
    email: String,
    checkout: CheckoutConfig,
}

#[derive(serde::Deserialize)]
struct ServerErrorResponse {
    error: Option<String>,
    message: Option<String>,
}

#[derive(serde::Deserialize)]
struct ServerActivationStatusResponse {
    active: bool,
}

#[derive(serde::Serialize, serde::Deserialize, Clone, Default)]
#[serde(rename_all = "camelCase")]
pub struct CheckoutConfig {
    pub client_token: Option<String>,
    pub price_id: String,
    pub environment: String,
}

#[derive(serde::Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct LoginResult {
    pub ok: bool,
    pub message: String,
    pub user: Option<AuthUser>,
    pub status: Option<u16>,
}

#[derive(serde::Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct RegisterResult {
    pub ok: bool,
    pub message: String,
    pub user_id: Option<String>,
    pub username: Option<String>,
    pub email: Option<String>,
    pub checkout: Option<CheckoutConfig>,
}

#[derive(serde::Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct ActivationStatusResult {
    pub ok: bool,
    pub message: String,
    pub active: bool,
}

#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct ServerCheckoutResponse {
    user_id: String,
    username: String,
    email: String,
    checkout: CheckoutConfig,
}

#[derive(serde::Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct InactiveCheckoutResult {
    pub ok: bool,
    pub message: String,
    pub user_id: Option<String>,
    pub username: Option<String>,
    pub email: Option<String>,
    pub checkout: Option<CheckoutConfig>,
}

impl ProxyStatus {
    pub fn starting() -> Self {
        Self {
            running: false,
            state: "starting".to_string(),
            message: "Starting".to_string(),
        }
    }
}

impl ManagedState {
    pub fn new() -> Self {
        let repo = StateRepository::new(config::state_path());
        let catalog_root = configured_assets_root(&repo);
        let wiki_root = catalog_root.clone().unwrap_or_else(config::assets_root);
        crate::assets::ensure_wiki_items(&wiki_root);
        Self {
            repo,
            catalog: Mutex::new(StaticCatalog::new(catalog_root)),
            proxy_status: Arc::new(Mutex::new(ProxyStatus::starting())),
            freeze_queue: Arc::new(AtomicBool::new(false)),
            system_proxy: Arc::new(Mutex::new(None)),
            system_proxy_running: Arc::new(AtomicBool::new(false)),
            force_drop_item_id: Arc::new(Mutex::new(None)),
        }
    }

    pub fn repo(&self) -> &StateRepository {
        &self.repo
    }

    pub fn proxy_status(&self) -> Arc<Mutex<ProxyStatus>> {
        self.proxy_status.clone()
    }

    pub fn freeze_queue_state(&self) -> Arc<AtomicBool> {
        self.freeze_queue.clone()
    }

    pub fn force_drop_item_id(&self) -> Arc<Mutex<Option<i64>>> {
        self.force_drop_item_id.clone()
    }
}

fn configured_assets_root(repo: &StateRepository) -> Option<PathBuf> {
    repo.load().assets_path.as_deref().map(PathBuf::from)
}

#[tauri::command]
pub fn get_proxy_status(state: State<'_, ManagedState>) -> ProxyStatus {
    state.proxy_status.lock().unwrap().clone()
}

#[tauri::command]
pub fn get_freeze_queue(state: State<'_, ManagedState>) -> bool {
    state.freeze_queue.load(Ordering::Relaxed)
}

#[tauri::command]
pub fn set_freeze_queue(state: State<'_, ManagedState>, freeze_queue: bool) -> bool {
    state.freeze_queue.store(freeze_queue, Ordering::Relaxed);
    freeze_queue
}

// ---- System Proxy Management ----

#[derive(serde::Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct SystemProxyStatus {
    pub running: bool,
    pub pid: Option<u32>,
    pub message: String,
}

#[tauri::command]
pub fn get_system_proxy_status(state: State<'_, ManagedState>) -> SystemProxyStatus {
    let mut guard = state.system_proxy.lock().unwrap();
    if let Some(ref mut child) = *guard {
        match child.try_wait() {
            Ok(Some(_)) => {
                *guard = None;
                SystemProxyStatus {
                    running: false,
                    pid: None,
                    message: "Proxy exited".to_string(),
                }
            }
            Ok(None) => SystemProxyStatus {
                running: true,
                pid: child.id().into(),
                message: "Running".to_string(),
            },
            Err(e) => SystemProxyStatus {
                running: false,
                pid: None,
                message: format!("Error checking proxy: {e}"),
            },
        }
    } else {
        SystemProxyStatus {
            running: false,
            pid: None,
            message: "Not started".to_string(),
        }
    }
}

fn generate_mitmproxy_addon() -> Option<PathBuf> {
    let script = r#"import json, urllib.request, datetime
from mitmproxy import http

DASHBOARD_URL = "http://127.0.0.1:1421/event"
TARGET_HOST = "thebackend.io"
BLOCKED_PATH = "/data/gameLog"

def request(flow: http.HTTPFlow):
    if TARGET_HOST not in flow.request.pretty_url:
        return
    if flow.request.path and BLOCKED_PATH in flow.request.path:
        print(f"[TBH] Blocked request: {flow.request.method} {flow.request.pretty_host}{flow.request.path}", flush=True)
        flow.response = http.Response.make(200, b"{}", {"Content-Type": "application/json"})
        return
    source = f"{flow.request.method} {flow.request.pretty_host}{flow.request.path}"
    _send(json.dumps({
        "type": "request_log",
        "at": datetime.datetime.utcnow().isoformat(),
        "source": source,
        "method": flow.request.method,
        "host": flow.request.pretty_host,
        "path": flow.request.path,
        "contentType": flow.request.headers.get("Content-Type", ""),
        "bodyBytes": len(flow.request.content or b""),
        "body": (flow.request.text or "")[:4096],
    }))

def response(flow: http.HTTPFlow):
    if TARGET_HOST not in flow.request.pretty_url:
        return
    source = f"{flow.request.method} {flow.request.pretty_host}{flow.request.path}"
    body = flow.response.text or ""
    _send(json.dumps({
        "type": "response_log",
        "source": source,
        "body": body,
        "body_bytes": len(flow.response.content or b""),
    }))

def _send(data: str):
    try:
        req = urllib.request.Request(
            DASHBOARD_URL, data=data.encode(),
            headers={"Content-Type": "application/json"},
            method="POST",
        )
        urllib.request.urlopen(req, timeout=1)
    except Exception:
        pass
"#;
    let mut path = std::env::temp_dir();
    path.push("tbh_mitmproxy_addon.py");
    match std::fs::write(&path, script) {
        Ok(_) => Some(path),
        Err(e) => {
            eprintln!("[TBH] Failed to write mitmproxy addon: {e}");
            None
        }
    }
}

fn start_system_proxy_event_bridge(state: &ManagedState) {
    state.system_proxy_running.store(true, Ordering::Relaxed);

    let (tx, rx) = std::sync::mpsc::channel::<String>();
    let repo_path = state.repo().path.clone();
    let running = state.system_proxy_running.clone();

    std::thread::spawn(move || {
        let repo = StateRepository::new(repo_path);
        while let Ok(line) = rx.recv() {
            let parsed = capture::parse_sidecar_line(&line);
            capture::apply_sidecar_event(parsed, &repo);

            if let Ok(v) = serde_json::from_str::<serde_json::Value>(&line)
                && v.get("type").and_then(|t| t.as_str()) == Some("response_log")
            {
                let source = match v.get("source").and_then(|s| s.as_str()) {
                    Some(s) => s.to_string(),
                    None => continue,
                };
                let body = match v.get("body").and_then(|b| b.as_str()) {
                    Some(b) => b.to_string(),
                    None => continue,
                };
                let Some(obj) = serde_json::from_str::<serde_json::Value>(&body).ok() else {
                    continue;
                };

                let added = proxy::extract_added_from_any_json(&obj);
                if !added.is_empty() {
                    let ev = serde_json::json!({
                        "type": "added_items",
                        "count": added.len(),
                        "source": source,
                        "items": added,
                    });
                    let line = serde_json::to_string(&ev).unwrap();
                    let parsed = capture::parse_sidecar_line(&line);
                    capture::apply_sidecar_event(parsed, &repo);
                }

                let chests = proxy::extract_chests_from_any_json(&obj);
                if !chests.is_empty() {
                    let ev = if chests.len() >= 40 {
                        serde_json::json!({
                            "type": "chests_synced",
                            "count": chests.len(),
                            "old": 0,
                            "source": source,
                            "chests": chests,
                        })
                    } else {
                        serde_json::json!({
                            "type": "chests_upserted",
                            "added": chests.len(),
                            "updated": 0,
                            "source": source,
                            "chests": chests,
                        })
                    };
                    let line = serde_json::to_string(&ev).unwrap();
                    let parsed = capture::parse_sidecar_line(&line);
                    capture::apply_sidecar_event(parsed, &repo);
                }
            }
        }
    });

    let running_clone = running.clone();
    std::thread::spawn(
        move || match TcpListener::bind(("127.0.0.1", SYSTEM_PROXY_EVENT_PORT)) {
            Ok(listener) => {
                listener.set_nonblocking(true).ok();
                for stream in listener.incoming() {
                    if !running_clone.load(Ordering::Relaxed) {
                        break;
                    }
                    match stream {
                        Ok(mut stream) => {
                            let tx = tx.clone();
                            std::thread::spawn(move || {
                                let mut reader = BufReader::new(&stream);
                                let mut request_line = String::new();
                                if reader.read_line(&mut request_line).is_err() {
                                    return;
                                }
                                let mut content_length = 0usize;
                                loop {
                                    let mut line = String::new();
                                    if reader.read_line(&mut line).is_err()
                                        || line == "\r\n"
                                        || line == "\n"
                                    {
                                        break;
                                    }
                                    if let Some(len_str) =
                                        line.trim().strip_prefix("Content-Length:")
                                    {
                                        content_length = len_str.trim().parse().unwrap_or(0);
                                    }
                                }
                                let mut buf = Vec::new();
                                if content_length > 0 {
                                    let mut body_buf = vec![0u8; content_length];
                                    if reader.read_exact(&mut body_buf).is_ok() {
                                        buf = body_buf;
                                    }
                                }
                                let _ = stream
                                    .write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 2\r\n\r\n{}");
                                if let Ok(body_str) = String::from_utf8(buf) {
                                    let _ = tx.send(body_str);
                                }
                            });
                        }
                        Err(_) => {
                            if !running_clone.load(Ordering::Relaxed) {
                                break;
                            }
                        }
                    }
                }
            }
            Err(e) => {
                eprintln!("[TBH] Failed to bind event listener: {e}");
            }
        },
    );
}

fn stop_system_proxy_event_bridge(state: &ManagedState) {
    state.system_proxy_running.store(false, Ordering::Relaxed);
    let _ = TcpStream::connect(("127.0.0.1", SYSTEM_PROXY_EVENT_PORT));
}

#[tauri::command]
pub fn start_system_proxy(state: State<'_, ManagedState>) -> SystemProxyStatus {
    let mut guard = state.system_proxy.lock().unwrap();
    if let Some(ref mut child) = *guard
        && child.try_wait().ok().flatten().is_none()
    {
        return SystemProxyStatus {
            running: true,
            pid: child.id().into(),
            message: "Already running".to_string(),
        };
    }

    stop_system_proxy_event_bridge(&state);
    start_system_proxy_event_bridge(&state);

    let addon_path = generate_mitmproxy_addon();

    let settings = state.repo().load().settings;
    let proxy_url = settings.proxy_url.trim().to_string();
    let port = proxy_url
        .trim_start_matches("http://")
        .trim_start_matches("https://")
        .split(':')
        .nth(1)
        .and_then(|p| p.trim().parse::<u16>().ok())
        .unwrap_or(8080);

    let cmd = detect_mitmproxy_binary();
    let cmd_name = match &cmd {
        Some((name, _)) => name.clone(),
        None => {
            stop_system_proxy_event_bridge(&state);
            return SystemProxyStatus {
                running: false,
                pid: None,
                message: "mitmproxy not found — install mitmproxy or mitmweb".to_string(),
            };
        }
    };

    let mut command = Command::new(&cmd_name);
    command.args([
        "--listen-port",
        &port.to_string(),
        "--set",
        "block_global=false",
        "--web-host",
        "127.0.0.1",
    ]);
    if let Some(path) = &addon_path {
        command.args(["-s", &path.to_string_lossy()]);
    }

    let child = match command.spawn() {
        Ok(c) => c,
        Err(e) => {
            stop_system_proxy_event_bridge(&state);
            return SystemProxyStatus {
                running: false,
                pid: None,
                message: format!("Failed to start {cmd_name}: {e}"),
            };
        }
    };

    let pid = child.id();
    *guard = Some(child);

    *state.proxy_status().lock().unwrap() = ProxyStatus {
        running: true,
        state: "system".to_string(),
        message: format!("Using system {cmd_name} (PID {pid})"),
    };

    SystemProxyStatus {
        running: true,
        pid: Some(pid),
        message: format!("Started {cmd_name} on port {port}"),
    }
}

#[tauri::command]
pub fn stop_system_proxy(state: State<'_, ManagedState>) -> SystemProxyStatus {
    stop_system_proxy_event_bridge(&state);
    let mut guard = state.system_proxy.lock().unwrap();
    if let Some(mut child) = guard.take() {
        let pid = child.id();
        let _ = child.kill();
        let _ = child.wait();
        kill_mitmproxy_processes();
        SystemProxyStatus {
            running: false,
            pid: Some(pid),
            message: format!("Stopped proxy (PID {pid})"),
        }
    } else {
        kill_mitmproxy_processes();
        SystemProxyStatus {
            running: false,
            pid: None,
            message: "Not running".to_string(),
        }
    }
}

fn detect_mitmproxy_binary() -> Option<(String, String)> {
    for name in &["mitmweb", "mitmproxy", "mitmdump"] {
        if Command::new(name).arg("--version").output().is_ok() {
            let display = match *name {
                "mitmweb" => "mitmproxy (web UI)",
                "mitmproxy" => "mitmproxy (terminal)",
                _ => "mitmdump",
            };
            return Some((name.to_string(), display.to_string()));
        }
    }
    None
}

// ---- Force Drop ----

#[tauri::command]
pub fn get_force_drop_item_id(state: State<'_, ManagedState>) -> Option<i64> {
    *state.force_drop_item_id.lock().unwrap()
}

#[tauri::command]
pub fn set_force_drop_item_id(state: State<'_, ManagedState>, id: Option<i64>) -> bool {
    let mut state_data = state.repo().load();
    state_data.settings.force_drop_item_id = id;
    if state.repo().save(&state_data).is_err() {
        return false;
    }
    *state.force_drop_item_id.lock().unwrap() = id;
    true
}

// ---- Settings ----

#[tauri::command]
pub fn get_settings(state: State<'_, ManagedState>) -> AppSettings {
    normalize_settings(state.repo().load().settings)
}

#[tauri::command]
pub fn set_settings(
    app: tauri::AppHandle,
    state: State<'_, ManagedState>,
    settings: AppSettings,
) -> bool {
    let old_settings = state.repo().load().settings;
    let new_settings = normalize_settings(settings);
    let mut state_data = state.repo().load();
    state_data.settings = new_settings.clone();
    if state.repo().save(&state_data).is_err() {
        return false;
    }

    if old_settings.use_system_proxy != new_settings.use_system_proxy {
        if new_settings.use_system_proxy {
            app.state::<Mutex<crate::proxy::ProxyManager>>()
                .lock()
                .unwrap()
                .stop();
            *state.proxy_status().lock().unwrap() = ProxyStatus {
                running: true,
                state: "system".to_string(),
                message: "Using system mitmproxy".to_string(),
            };
        } else {
            stop_system_proxy_event_bridge(&state);
            let mut system_proxy = state.system_proxy.lock().unwrap();
            if let Some(mut child) = system_proxy.take() {
                let _ = child.kill();
                let _ = child.wait();
            }
            drop(system_proxy);
            kill_mitmproxy_processes();
            let repo_path = state.repo().path.clone();
            app.state::<Mutex<crate::proxy::ProxyManager>>()
                .lock()
                .unwrap()
                .start(&app, &crate::state::StateRepository::new(repo_path));
        }
    }

    true
}

#[tauri::command]
pub fn login(
    state: State<'_, ManagedState>,
    server_url: String,
    username: String,
    password: String,
) -> LoginResult {
    let server_url = server_url.trim().trim_end_matches('/').to_string();
    if server_url.is_empty() || username.trim().is_empty() || password.is_empty() {
        return LoginResult {
            ok: false,
            message: "Enter username, and password.".to_string(),
            user: None,
            status: None,
        };
    }

    let client = auth_http_client();
    let response = client
        .post(format!("{server_url}/auth/login"))
        .json(&serde_json::json!({
            "username": username.trim(),
            "password": password,
        }))
        .send();

    let Ok(response) = response else {
        return LoginResult {
            ok: false,
            message: "Could not reach the login server.".to_string(),
            user: None,
            status: None,
        };
    };

    if !response.status().is_success() {
        let status = response.status();
        let server_message = server_error_message(response);
        return LoginResult {
            ok: false,
            message: if status.as_u16() == 401 {
                "Invalid username or password.".to_string()
            } else if status.as_u16() == 403 {
                "Account is not active yet. If you just paid, wait a few seconds and try again."
                    .to_string()
            } else {
                server_message.unwrap_or_else(|| "Login failed.".to_string())
            },
            user: None,
            status: Some(status.as_u16()),
        };
    }

    let login = response.json::<ServerLoginResponse>();
    let Ok(login) = login else {
        return LoginResult {
            ok: false,
            message: "Login server returned an invalid response.".to_string(),
            user: None,
            status: None,
        };
    };

    let mut state_data = state.repo().load();
    state_data.settings.server_url = server_url;
    state_data.settings.auth_token = login.token;
    state_data.settings = normalize_settings(state_data.settings);

    match state.repo().save(&state_data) {
        Ok(()) => LoginResult {
            ok: true,
            message: "Logged in.".to_string(),
            user: Some(login.user),
            status: Some(200),
        },
        Err(err) => LoginResult {
            ok: false,
            message: format!("Logged in, but failed to save token: {err}"),
            user: None,
            status: None,
        },
    }
}

#[tauri::command]
pub fn register(
    state: State<'_, ManagedState>,
    server_url: String,
    username: String,
    email: String,
    password: String,
) -> RegisterResult {
    let server_url = server_url.trim().trim_end_matches('/').to_string();
    if server_url.is_empty()
        || username.trim().is_empty()
        || email.trim().is_empty()
        || password.is_empty()
    {
        return RegisterResult {
            ok: false,
            message: "Enter username, email, and password.".to_string(),
            user_id: None,
            username: None,
            email: None,
            checkout: None,
        };
    }

    let client = auth_http_client();
    let response = client
        .post(format!("{server_url}/auth/register"))
        .json(&serde_json::json!({
            "username": username.trim(),
            "email": email.trim(),
            "password": password,
        }))
        .send();

    let Ok(response) = response else {
        return RegisterResult {
            ok: false,
            message: "Could not reach the registration server.".to_string(),
            user_id: None,
            username: None,
            email: None,
            checkout: None,
        };
    };

    if !response.status().is_success() {
        let server_message = server_error_message(response);
        return RegisterResult {
            ok: false,
            message: server_message.unwrap_or_else(|| "Registration failed.".to_string()),
            user_id: None,
            username: None,
            email: None,
            checkout: None,
        };
    }

    let registration = response.json::<ServerRegisterResponse>();
    let Ok(registration) = registration else {
        return RegisterResult {
            ok: false,
            message: "Registration server returned an invalid response.".to_string(),
            user_id: None,
            username: None,
            email: None,
            checkout: None,
        };
    };

    let mut state_data = state.repo().load();
    state_data.settings.server_url = server_url;
    state_data.settings.auth_token.clear();
    state_data.settings = normalize_settings(state_data.settings);

    let save_message = match state.repo().save(&state_data) {
        Ok(()) => "Account created. Complete checkout to activate it.".to_string(),
        Err(err) => format!("Account created, but failed to save server URL: {err}"),
    };

    RegisterResult {
        ok: true,
        message: save_message,
        user_id: Some(registration.user_id),
        username: Some(registration.username),
        email: Some(registration.email),
        checkout: Some(registration.checkout),
    }
}

#[tauri::command]
pub fn get_activation_status(server_url: String, user_id: String) -> ActivationStatusResult {
    let server_url = server_url.trim().trim_end_matches('/').to_string();
    if server_url.is_empty() || user_id.trim().is_empty() {
        return ActivationStatusResult {
            ok: false,
            message: "Missing server URL or user ID.".to_string(),
            active: false,
        };
    }

    let response = auth_http_client()
        .get(format!("{server_url}/auth/activation-status"))
        .query(&[("userId", user_id.trim())])
        .send();

    let Ok(response) = response else {
        return ActivationStatusResult {
            ok: false,
            message: "Could not reach the activation server.".to_string(),
            active: false,
        };
    };

    if !response.status().is_success() {
        let server_message = server_error_message(response);
        return ActivationStatusResult {
            ok: false,
            message: server_message.unwrap_or_else(|| "Activation check failed.".to_string()),
            active: false,
        };
    }

    match response.json::<ServerActivationStatusResponse>() {
        Ok(status) => ActivationStatusResult {
            ok: true,
            message: String::new(),
            active: status.active,
        },
        Err(_) => ActivationStatusResult {
            ok: false,
            message: "Activation server returned an invalid response.".to_string(),
            active: false,
        },
    }
}

#[tauri::command]
pub fn get_inactive_checkout(
    _state: State<'_, ManagedState>,
    server_url: String,
    username: String,
    password: String,
) -> InactiveCheckoutResult {
    let server_url = server_url.trim().trim_end_matches('/').to_string();
    if server_url.is_empty() || username.trim().is_empty() || password.is_empty() {
        return InactiveCheckoutResult {
            ok: false,
            message: "Enter username, and password.".to_string(),
            user_id: None,
            username: None,
            email: None,
            checkout: None,
        };
    }

    let client = auth_http_client();
    let response = client
        .post(format!("{server_url}/auth/get-checkout"))
        .json(&serde_json::json!({
            "username": username.trim(),
            "password": password,
        }))
        .send();

    let Ok(response) = response else {
        return InactiveCheckoutResult {
            ok: false,
            message: "Could not reach the server.".to_string(),
            user_id: None,
            username: None,
            email: None,
            checkout: None,
        };
    };

    if !response.status().is_success() {
        let server_message = server_error_message(response);
        return InactiveCheckoutResult {
            ok: false,
            message: server_message
                .unwrap_or_else(|| "Could not retrieve checkout details.".to_string()),
            user_id: None,
            username: None,
            email: None,
            checkout: None,
        };
    }

    match response.json::<ServerCheckoutResponse>() {
        Ok(resp) => InactiveCheckoutResult {
            ok: true,
            message: "Checkout details retrieved.".to_string(),
            user_id: Some(resp.user_id),
            username: Some(resp.username),
            email: Some(resp.email),
            checkout: Some(resp.checkout),
        },
        Err(_) => InactiveCheckoutResult {
            ok: false,
            message: "Server returned invalid checkout details.".to_string(),
            user_id: None,
            username: None,
            email: None,
            checkout: None,
        },
    }
}

#[tauri::command]
pub fn get_current_user(state: State<'_, ManagedState>) -> Option<AuthUser> {
    let settings = normalize_settings(state.repo().load().settings);

    if settings.offline_mode {
        return Some(AuthUser {
            id: "local".to_string(),
            username: "Local".to_string(),
            email: None,
            is_admin: true,
            entitlement: None,
        });
    }

    let server_url = settings.server_url.trim().trim_end_matches('/').to_string();
    let auth_token = settings.auth_token.trim().to_string();
    if server_url.is_empty() || auth_token.is_empty() {
        return None;
    }

    auth_http_client()
        .get(format!("{server_url}/me"))
        .bearer_auth(auth_token)
        .send()
        .ok()
        .filter(|response| response.status().is_success())
        .and_then(|response| response.json::<AuthUser>().ok())
}

#[tauri::command]
pub fn logout(state: State<'_, ManagedState>) -> bool {
    let mut state_data = state.repo().load();
    let settings = normalize_settings(state_data.settings.clone());
    let server_url = settings.server_url.trim().trim_end_matches('/').to_string();
    let auth_token = settings.auth_token.trim().to_string();

    if !server_url.is_empty() && !auth_token.is_empty() {
        let _ = auth_http_client()
            .post(format!("{server_url}/auth/logout"))
            .bearer_auth(&auth_token)
            .send();
    }

    state_data.settings.auth_token.clear();
    state_data.settings.offline_mode = false;
    state.repo().save(&state_data).is_ok()
}

#[tauri::command]
pub fn skip_login(state: State<'_, ManagedState>) -> AuthUser {
    let mut state_data = state.repo().load();
    state_data.settings.offline_mode = true;
    state_data.settings.server_url.clear();
    state_data.settings.auth_token.clear();
    let _ = state.repo().save(&state_data);

    AuthUser {
        id: "local".to_string(),
        username: "Local".to_string(),
        email: None,
        is_admin: true,
        entitlement: None,
    }
}

fn auth_http_client() -> reqwest::blocking::Client {
    reqwest::blocking::Client::builder()
        .timeout(Duration::from_secs(10))
        .build()
        .unwrap_or_else(|_| reqwest::blocking::Client::new())
}

fn server_error_message(response: reqwest::blocking::Response) -> Option<String> {
    response
        .json::<ServerErrorResponse>()
        .ok()
        .and_then(|err| err.error.or(err.message))
        .filter(|message| !message.trim().is_empty())
        .map(|message| {
            let trimmed = message.trim();
            if let Some(rest) = trimmed.strip_prefix("bad request:") {
                rest.trim().to_string()
            } else if let Some(rest) = trimmed.strip_prefix("conflict:") {
                rest.trim().to_string()
            } else {
                trimmed.to_string()
            }
        })
}

fn normalize_settings(mut settings: AppSettings) -> AppSettings {
    let linux_default = "HTTP_PROXY=http://127.0.0.1:8080 HTTPS_PROXY=http://127.0.0.1:8080 ALL_PROXY=http://127.0.0.1:8080 %command%";
    let windows_default = "cmd /c \"set HTTP_PROXY=http://127.0.0.1:8080 && set HTTPS_PROXY=http://127.0.0.1:8080 && %command%\"";
    let current_default = default_steam_launch_options();
    let current = settings.steam_launch_options.trim();

    if current.is_empty()
        || (cfg!(target_os = "windows") && current == linux_default)
        || (!cfg!(target_os = "windows") && current == windows_default)
    {
        settings.steam_launch_options = current_default;
    }

    if settings.asset_manifest_url.trim().is_empty() {
        settings.asset_manifest_url = config::default_asset_manifest_url();
    }

    if settings.server_url.trim().is_empty() {
        settings.server_url = config::default_server_url();
    }

    settings
}

#[tauri::command]
pub fn launch_game(app: tauri::AppHandle, state: State<'_, ManagedState>) -> LaunchGameResult {
    let settings = state.repo().load().settings;
    chests::clear_all(state.repo());
    let result = launch_game_from_settings(&settings);
    if result.ok {
        let repo_path = state.repo().path.clone();
        let proxy_url = settings.proxy_url.clone();
        std::thread::spawn(move || {
            monitor_game_process(repo_path, app, proxy_url);
        });
    }
    result
}

pub fn launch_game_from_settings(settings: &AppSettings) -> LaunchGameResult {
    let settings = normalize_settings(settings.clone());
    match launch_steam_app(&settings) {
        Ok(()) => LaunchGameResult {
            ok: true,
            message: launch_success_message(),
        },
        Err(err) => LaunchGameResult {
            ok: false,
            message: err,
        },
    }
}

fn launch_success_message() -> String {
    "Starting game...".to_string()
}

fn launch_steam_app(settings: &AppSettings) -> Result<(), String> {
    open_steam_app(&settings.proxy_url)
}

fn steam_run_url() -> String {
    format!("steam://run/{STEAM_APP_ID}")
}

fn apply_proxy_env(command: &mut Command, proxy_url: &str) {
    let proxy_url = proxy_url.trim();
    if proxy_url.is_empty() {
        return;
    }

    for key in [
        "HTTP_PROXY",
        "HTTPS_PROXY",
        "ALL_PROXY",
        "http_proxy",
        "https_proxy",
        "all_proxy",
    ] {
        command.env(key, proxy_url);
    }
}

#[cfg(target_os = "windows")]
fn open_steam_app(_proxy_url: &str) -> Result<(), String> {
    let steam_url = steam_run_url();
    let direct = Command::new("steam")
        .arg("-applaunch")
        .arg(STEAM_APP_ID)
        .spawn();
    if direct.is_ok() {
        return Ok(());
    }

    Command::new("cmd")
        .args(["/C", "start", "", &steam_url])
        .spawn()
        .map(|_| ())
        .map_err(|err| format!("Failed to ask Steam to launch: {err}"))
}

#[cfg(target_os = "macos")]
fn open_steam_app(proxy_url: &str) -> Result<(), String> {
    let mut command = Command::new("open");
    apply_proxy_env(&mut command, proxy_url);
    command
        .args(["-a", "Steam", "--args", "-applaunch", STEAM_APP_ID])
        .spawn()
        .map(|_| ())
        .map_err(|err| format!("Failed to ask Steam to launch: {err}"))
}

#[cfg(all(unix, not(target_os = "macos")))]
fn open_steam_app(proxy_url: &str) -> Result<(), String> {
    let steam_url = steam_run_url();
    let mut command = Command::new("steam");
    apply_proxy_env(&mut command, proxy_url);
    match command.arg("-applaunch").arg(STEAM_APP_ID).spawn() {
        Ok(_) => Ok(()),
        Err(steam_err) => Command::new("xdg-open")
            .arg(steam_url)
            .spawn()
            .map(|_| ())
            .map_err(|xdg_err| {
                format!("Failed to ask Steam to launch: steam: {steam_err}; xdg-open: {xdg_err}")
            }),
    }
}

// ---- Chest Queue ----

#[tauri::command]
pub fn get_chest_rows(state: State<'_, ManagedState>, include_claimed: bool) -> Vec<ChestRow> {
    let catalog = state.catalog.lock().unwrap();
    chests::get_rows(&catalog, include_claimed, state.repo())
}

#[tauri::command]
pub fn get_box_summary(state: State<'_, ManagedState>) -> HashMap<String, usize> {
    chests::box_summary(state.repo())
}

#[tauri::command]
pub fn mark_opened(state: State<'_, ManagedState>, key: String) -> usize {
    chests::mark_claimed_by_keys(&[key], "manual", state.repo())
}

#[tauri::command]
pub fn mark_all_opened(state: State<'_, ManagedState>) -> usize {
    let state_data = state.repo().load();
    let keys: Vec<String> = state_data
        .chests
        .keys()
        .filter(|k| !state_data.chests[*k].is_get)
        .cloned()
        .collect();
    chests::mark_claimed_by_keys(&keys, "manual all", state.repo())
}

// ---- Boss Drop / Added Items ----

#[tauri::command]
pub fn get_last_added(state: State<'_, ManagedState>) -> Option<AddedItemsSnapshot> {
    state.repo().load().last_added
}

// ---- Reroll Preview / ProcessBox ----

#[tauri::command]
pub fn get_last_processbox(state: State<'_, ManagedState>) -> Option<ProcessBoxInfo> {
    state.repo().load().last_processbox
}

// ---- Farm Ranking ----

#[derive(serde::Serialize)]
pub struct FarmRow {
    pub per_hour: Option<f64>,
    pub expected: f64,
    pub stage_id: i64,
    pub name: String,
    pub difficulty: String,
    pub level: i32,
    pub boxes: Vec<(i64, i32)>,
}

#[tauri::command]
pub fn get_farm_ranking(
    state: State<'_, ManagedState>,
    rarity: Option<String>,
    kind: Option<String>,
    item_id: Option<i64>,
    min_level: Option<i32>,
    max_level: Option<i32>,
    clear_time: Option<f64>,
) -> Vec<FarmRow> {
    let catalog = state.catalog.lock().unwrap();
    let results = catalog.rank_stages(
        rarity.as_deref(),
        kind.as_deref(),
        item_id,
        min_level,
        max_level,
        clear_time,
        50,
    );
    results
        .into_iter()
        .map(|(_, expected, per_hour, stage)| FarmRow {
            per_hour,
            expected,
            stage_id: stage.id,
            name: stage.name,
            difficulty: stage.difficulty,
            level: stage.level,
            boxes: stage.boxes,
        })
        .collect()
}

// ---- Events ----

#[tauri::command]
pub fn get_events(state: State<'_, ManagedState>) -> Vec<StateEvent> {
    state.repo().load().events
}

#[tauri::command]
pub fn get_request_history(state: State<'_, ManagedState>) -> Vec<RequestLogEntry> {
    let mut history = state.repo().load().request_history;
    history.reverse();
    history
}

#[tauri::command]
pub fn clear_request_history(state: State<'_, ManagedState>) -> bool {
    let mut state_data = state.repo().load();
    state_data.request_history.clear();
    state.repo().save(&state_data).is_ok()
}

// ---- Catalog Status ----

#[derive(serde::Serialize)]
pub struct CatalogStatus {
    pub valid: bool,
    pub items_count: usize,
    pub stages_count: usize,
    pub display_names_count: usize,
}

#[tauri::command]
pub fn get_catalog_status(state: State<'_, ManagedState>) -> CatalogStatus {
    let catalog = state.catalog.lock().unwrap();
    CatalogStatus {
        valid: catalog.valid,
        items_count: catalog.items_count(),
        stages_count: catalog.stages_count(),
        display_names_count: catalog.display_names_count(),
    }
}

#[tauri::command]
pub fn get_rarity_order() -> Vec<String> {
    config::RARITY_ORDER
        .iter()
        .copied()
        .filter(|rarity| *rarity != "UNKNOWN")
        .map(String::from)
        .collect()
}

// ---- Reload catalog (after assets update) ----

#[tauri::command]
pub fn reload_catalog(state: State<'_, ManagedState>) -> bool {
    let root = configured_assets_root(state.repo());
    crate::assets::ensure_wiki_items(&root.clone().unwrap_or_else(config::assets_root));
    let mut catalog = state.catalog.lock().unwrap();
    *catalog = StaticCatalog::new(root);
    catalog.valid
}

// ---- Assets path ----

#[tauri::command]
pub fn get_assets_path(state: State<'_, ManagedState>) -> Option<String> {
    state.repo().load().assets_path
}

#[tauri::command]
pub fn set_assets_path(state: State<'_, ManagedState>, path: String) -> bool {
    let mut state_data = state.repo().load();
    state_data.assets_path = Some(path);
    state_data.assets_version = None;
    let _ = state.repo().save(&state_data);

    let root = state_data
        .assets_path
        .as_deref()
        .map(std::path::PathBuf::from);
    crate::assets::ensure_wiki_items(&root.clone().unwrap_or_else(config::assets_root));
    let mut catalog = state.catalog.lock().unwrap();
    *catalog = StaticCatalog::new(root);
    catalog.valid
}

#[tauri::command]
pub fn get_assets_root(state: State<'_, ManagedState>) -> String {
    let _ = state;
    config::assets_root().to_string_lossy().into_owned()
}

#[tauri::command]
pub async fn get_asset_update_status(
    state: State<'_, ManagedState>,
) -> Result<AssetUpdateStatus, String> {
    let repo = StateRepository::new(state.repo().path.clone());
    Ok(crate::assets::check_update(&repo).await)
}

#[tauri::command]
pub async fn download_latest_assets(
    state: State<'_, ManagedState>,
) -> Result<AssetDownloadResult, String> {
    let repo = StateRepository::new(state.repo().path.clone());
    let result = crate::assets::download_latest(&repo).await;

    if result.ok {
        let root = configured_assets_root(state.repo());
        crate::assets::ensure_wiki_items(&root.clone().unwrap_or_else(config::assets_root));
        let mut catalog = state.catalog.lock().unwrap();
        *catalog = StaticCatalog::new(root);
    }

    Ok(result)
}

#[tauri::command]
pub fn upload_claimable_reward_observations(
    state: State<'_, ManagedState>,
) -> Result<ObservationUploadResult, String> {
    let repo = StateRepository::new(state.repo().path.clone());
    let catalog = state.catalog.lock().unwrap();
    Ok(crate::observations::upload_claimable_rewards(
        &repo, &catalog,
    ))
}

#[tauri::command]
pub async fn browse_assets_folder(window: tauri::Window) -> Option<String> {
    let handle = window.app_handle().clone();
    let (tx, rx) = std::sync::mpsc::channel();
    handle
        .dialog()
        .file()
        .set_title("Select Assets Folder")
        .pick_folder(move |path: Option<tauri_plugin_dialog::FilePath>| {
            let _ = tx.send(path);
        });
    let path = rx.recv().ok().flatten()?;
    path.into_path()
        .ok()
        .map(|p| p.to_string_lossy().into_owned())
}

#[tauri::command]
pub fn restart_game(app: tauri::AppHandle, state: State<'_, ManagedState>) -> LaunchGameResult {
    kill_game_process();
    std::thread::sleep(std::time::Duration::from_secs(2));
    chests::clear_all(state.repo());
    let settings = state.repo().load().settings;
    let result = launch_game_from_settings(&settings);
    if result.ok {
        let repo_path = state.repo().path.clone();
        let proxy_url = settings.proxy_url.clone();
        std::thread::spawn(move || {
            monitor_game_process(repo_path, app, proxy_url);
        });
    }
    result
}

fn kill_game_process() {
    let names = ["TaskbarHero", "Task Bar Hero"];
    #[cfg(target_os = "windows")]
    for name in &names {
        let _ = Command::new("taskkill")
            .args(["/F", "/IM", &format!("{}.exe", name)])
            .output();
    }
    #[cfg(not(target_os = "windows"))]
    for name in &names {
        let _ = Command::new("pkill").args(["-f", "-i", name]).output();
    }
}

pub fn kill_mitmproxy_processes() {
    let names = ["mitmweb", "mitmproxy", "mitmdump"];
    #[cfg(target_os = "windows")]
    for name in &names {
        let _ = Command::new("taskkill")
            .args(["/F", "/IM", &format!("{}.exe", name)])
            .output();
    }
    #[cfg(not(target_os = "windows"))]
    for name in &names {
        let _ = Command::new("pkill").args(["-f", "-i", name]).output();
    }
}

pub fn monitor_game_process(repo_path: PathBuf, app: tauri::AppHandle, proxy_url: String) {
    let repo = crate::state::StateRepository::new(repo_path);
    std::thread::sleep(std::time::Duration::from_secs(15));
    let mut was_running = is_game_running();
    #[cfg(not(target_os = "windows"))]
    let _ = (&app, &proxy_url);
    #[cfg(target_os = "windows")]
    let mut attached_pid = None;
    loop {
        std::thread::sleep(std::time::Duration::from_secs(5));

        #[cfg(target_os = "windows")]
        if attached_pid.is_none() {
            if let Some(pid) = find_game_pid() {
                match nethelper::start_for_game(&app, pid, &proxy_url) {
                    Ok(()) => attached_pid = Some(pid),
                    Err(err) => eprintln!("[TBH] Windows network helper error: {err}"),
                }
            }
        }

        let is_running = is_game_running();
        if was_running && !is_running {
            nethelper::stop();
            chests::clear_all(&repo);
            return;
        }
        was_running = is_running;
    }
}

fn is_game_running() -> bool {
    let names = ["TaskbarHero", "TaskBar Hero"];

    #[cfg(target_os = "windows")]
    {
        Command::new("tasklist")
            .arg("/FI")
            .arg("IMAGENAME eq TaskbarHero.exe")
            .arg("/NH")
            .output()
            .map(|o| {
                let s = String::from_utf8_lossy(&o.stdout);
                names.iter().any(|n| s.contains(n))
            })
            .unwrap_or(false)
    }

    #[cfg(not(target_os = "windows"))]
    {
        let pgrep = names.iter().any(|name| {
            Command::new("pgrep")
                .args(["-f", "-i", name])
                .output()
                .map(|o| o.status.success())
                .unwrap_or(false)
        });
        if pgrep {
            return true;
        }
        #[cfg(target_os = "linux")]
        {
            std::fs::read_dir("/proc")
                .map(|entries| {
                    entries.flatten().any(|entry| {
                        let name = entry.file_name();
                        let name_str = name.to_string_lossy();
                        if !name_str.chars().all(|c| c.is_ascii_digit()) {
                            return false;
                        }
                        if let Ok(cmdline) = std::fs::read_to_string(entry.path().join("cmdline")) {
                            names.iter().any(|g| cmdline.contains(g))
                        } else {
                            false
                        }
                    })
                })
                .unwrap_or(false)
        }
        #[cfg(not(target_os = "linux"))]
        {
            false
        }
    }
}

#[cfg(target_os = "windows")]
fn find_game_pid() -> Option<u32> {
    let output = Command::new("tasklist")
        .args(["/FI", "IMAGENAME eq taskbarhero.exe", "/FO", "CSV", "/NH"])
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    stdout.lines().find_map(parse_tasklist_pid)
}

#[cfg(target_os = "windows")]
fn parse_tasklist_pid(line: &str) -> Option<u32> {
    let mut parts = line.trim().trim_matches('"').split("\",\"");
    let image = parts.next()?;
    let pid = parts.next()?;
    if !image.eq_ignore_ascii_case("taskbarhero.exe") {
        return None;
    }
    pid.parse().ok()
}
