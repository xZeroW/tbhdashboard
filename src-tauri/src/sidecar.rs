use std::sync::{Arc, Mutex};
use tauri::{AppHandle, Emitter};
use tauri_plugin_shell::process::{CommandChild, CommandEvent};
use tauri_plugin_shell::ShellExt;
use crate::capture;
use crate::state::StateRepository;

pub struct SidecarManager {
    child: Option<CommandChild>,
}

impl SidecarManager {
    pub fn new() -> Self {
        Self { child: None }
    }

    pub fn start(&mut self, app: &AppHandle, repo: &StateRepository) {
        let sidecar_command = match app.shell().sidecar("tbhd-sidecar") {
            Ok(cmd) => cmd,
            Err(e) => {
                eprintln!("[TBH] Failed to create sidecar command: {}", e);
                return;
            }
        };

        let (rx, child) = match sidecar_command.spawn() {
            Ok(result) => result,
            Err(e) => {
                eprintln!("[TBH] Failed to spawn sidecar: {}", e);
                return;
            }
        };

        self.child = Some(child);
        println!("[TBH] Sidecar started");

        let app_handle = app.clone();
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
                        eprintln!("[TBH-sidecar] {}", line);
                    }
                    CommandEvent::Error(err) => {
                        eprintln!("[TBH-sidecar] Error: {}", err);
                    }
                    CommandEvent::Terminated(status) => {
                        println!("[TBH-sidecar] Terminated with status: {:?}", status);
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
            println!("[TBH-sidecar] Stopped");
        }
    }
}

impl Drop for SidecarManager {
    fn drop(&mut self) {
        self.stop();
    }
}
