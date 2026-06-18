use std::sync::{Arc, Mutex};
use tauri::{AppHandle, Emitter};
use tauri_plugin_shell::process::{CommandChild, CommandEvent};
use tauri_plugin_shell::ShellExt;
use crate::capture;
use crate::commands::ProxyStatus;
use crate::state::StateRepository;

pub struct SidecarManager {
    child: Option<CommandChild>,
    status: Arc<Mutex<ProxyStatus>>,
}

impl SidecarManager {
    pub fn new(status: Arc<Mutex<ProxyStatus>>) -> Self {
        Self { child: None, status }
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

        let sidecar_command = match app.shell().sidecar("tbhd-sidecar") {
            Ok(cmd) => cmd,
            Err(e) => {
                eprintln!("[TBH] Failed to create sidecar command: {}", e);
                self.set_status(false, "error", format!("Failed to create sidecar: {e}"));
                let _ = app.emit("proxy-status-changed", ());
                return;
            }
        };

        let (rx, child) = match sidecar_command.spawn() {
            Ok(result) => result,
            Err(e) => {
                eprintln!("[TBH] Failed to spawn sidecar: {}", e);
                self.set_status(false, "error", format!("Failed to spawn sidecar: {e}"));
                let _ = app.emit("proxy-status-changed", ());
                return;
            }
        };

        self.child = Some(child);
        self.set_status(true, "running", "Running");
        let _ = app.emit("proxy-status-changed", ());
        println!("[TBH] Sidecar started");

        let app_handle = app.clone();
        let status_handle = self.status.clone();
        let last_error = Arc::new(Mutex::new(None::<String>));
        let last_error_handle = last_error.clone();
        let repo = Arc::new(Mutex::new(
            StateRepository::new(repo.path.clone())
        ));

        // Read stdout/stderr from sidecar in background
        tauri::async_runtime::spawn(async move {
            let mut rx = rx;
            while let Some(event) = rx.recv().await {
                match event {
                    CommandEvent::Stdout(line_bytes) => {
                        let line = String::from_utf8_lossy(&line_bytes);
                        for line in line.lines() {
                            let line = line.trim();
                            if line.is_empty() { continue; }
                            println!("[TBH-sidecar] {}", line);

                            if let Some(message) = proxy_error_message(line) {
                                *last_error_handle.lock().unwrap() = Some(message.clone());
                                *status_handle.lock().unwrap() = ProxyStatus {
                                    running: false,
                                    state: "error".to_string(),
                                    message,
                                };
                                let _ = app_handle.emit("proxy-status-changed", ());
                            }

                            // Parse and apply the event
                            let parsed = capture::parse_sidecar_line(line);
                            let repo_guard = repo.lock().unwrap();
                            capture::apply_sidecar_event(parsed, &repo_guard);

                            // Notify frontend that state changed
                            let _ = app_handle.emit("state-changed", ());
                        }
                    }
                    CommandEvent::Stderr(line_bytes) => {
                        let line = String::from_utf8_lossy(&line_bytes);
                        for line in line.lines() {
                            let line = line.trim();
                            if line.is_empty() { continue; }
                            eprintln!("[TBH-sidecar] {}", line);

                            if let Some(message) = proxy_error_message(line) {
                                *last_error_handle.lock().unwrap() = Some(message.clone());
                                *status_handle.lock().unwrap() = ProxyStatus {
                                    running: false,
                                    state: "error".to_string(),
                                    message,
                                };
                                let _ = app_handle.emit("proxy-status-changed", ());
                            }
                        }
                    }
                    CommandEvent::Error(err) => {
                        eprintln!("[TBH-sidecar] Error: {}", err);
                        *status_handle.lock().unwrap() = ProxyStatus {
                            running: false,
                            state: "error".to_string(),
                            message: err.to_string(),
                        };
                        let _ = app_handle.emit("proxy-status-changed", ());
                    }
                    CommandEvent::Terminated(status) => {
                        println!("[TBH-sidecar] Terminated with status: {:?}", status);
                        let message = last_error.lock().unwrap().clone()
                            .unwrap_or_else(|| format!("Stopped: {:?}", status));
                        *status_handle.lock().unwrap() = ProxyStatus {
                            running: false,
                            state: "stopped".to_string(),
                            message,
                        };
                        let _ = app_handle.emit("proxy-status-changed", ());
                        break;
                    }
                    _ => {}
                }
            }
            println!("[TBH-sidecar] Event loop ended");
        });
    }

    pub fn stop(&mut self) {
        if let Some(child) = self.child.take() {
            let _ = child.kill();
            self.set_status(false, "stopped", "Stopped");
            println!("[TBH-sidecar] Stopped");
        }
    }
}

fn proxy_error_message(line: &str) -> Option<String> {
    if line.contains("address already in use") || line.contains("Address already in use") {
        let port = extract_listen_port(line).unwrap_or_else(|| "8080".to_string());
        return Some(format!(
            "Proxy port {port} is already in use. Close the other app using it and restart the dashboard."
        ));
    }

    if line.contains("failed to listen") {
        return Some(line.to_string());
    }

    None
}

fn extract_listen_port(line: &str) -> Option<String> {
    let marker = "failed to listen on *:";
    let start = line.find(marker)? + marker.len();
    let port: String = line[start..]
        .chars()
        .take_while(|ch| ch.is_ascii_digit())
        .collect();

    if port.is_empty() { None } else { Some(port) }
}

impl Drop for SidecarManager {
    fn drop(&mut self) {
        self.stop();
    }
}
