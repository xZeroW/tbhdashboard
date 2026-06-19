#[cfg(target_os = "windows")]
mod platform {
    use std::{
        env,
        ffi::OsStr,
        net::SocketAddr,
        os::windows::ffi::OsStrExt,
        path::PathBuf,
        process::{Child, Command},
        sync::Mutex,
    };

    use tauri::{AppHandle, Manager};
    use windows_sys::Win32::UI::Shell::ShellExecuteW;

    static NETHELPER_CHILD: Mutex<Option<Child>> = Mutex::new(None);

    pub struct NetHelperCleanup;

    impl Drop for NetHelperCleanup {
        fn drop(&mut self) {
            stop();
        }
    }

    pub fn start_for_game(app: &AppHandle, game_pid: u32, proxy_url: &str) -> Result<(), String> {
        reap_finished_child();

        if NETHELPER_CHILD.lock().unwrap().is_some() {
            return Ok(());
        }

        let proxy_addr = proxy_addr(proxy_url)?;
        let helper = resolve_helper(app)?;
        spawn_elevated(&helper, game_pid, &proxy_addr)?;
        println!("[TBH] Windows network helper attached to TaskbarHero.exe pid {game_pid}");
        Ok(())
    }

    fn spawn_elevated(helper: &PathBuf, game_pid: u32, proxy_addr: &str) -> Result<(), String> {
        let args = format!(
            "start --pid {game_pid} --proxy {proxy_addr} --parent {}",
            std::process::id()
        );
        let operation = wide("runas");
        let file = wide(helper.as_os_str());
        let params = wide(args);
        let result = unsafe {
            ShellExecuteW(
                std::ptr::null_mut(),
                operation.as_ptr(),
                file.as_ptr(),
                params.as_ptr(),
                std::ptr::null(),
                1,
            )
        } as isize;

        if result <= 32 {
            return Err(format!(
                "Failed to elevate Windows network helper at {} (ShellExecuteW code {result}). Approve the UAC prompt, or run the helper manually from an elevated terminal.",
                helper.display()
            ));
        }
        Ok(())
    }

    fn wide(value: impl AsRef<OsStr>) -> Vec<u16> {
        value
            .as_ref()
            .encode_wide()
            .chain(std::iter::once(0))
            .collect()
    }

    pub fn stop() {
        if let Some(mut child) = NETHELPER_CHILD.lock().unwrap().take() {
            let _ = child.kill();
            let _ = child.wait();
            println!("[TBH] Windows network helper stopped");
        }
    }

    fn reap_finished_child() {
        let mut guard = NETHELPER_CHILD.lock().unwrap();
        let finished = match guard.as_mut() {
            Some(child) => child
                .try_wait()
                .map(|status| status.is_some())
                .unwrap_or(true),
            None => false,
        };
        if finished {
            *guard = None;
        }
    }

    fn proxy_addr(proxy_url: &str) -> Result<String, String> {
        let value = proxy_url
            .trim()
            .strip_prefix("http://")
            .or_else(|| proxy_url.trim().strip_prefix("https://"))
            .unwrap_or_else(|| proxy_url.trim())
            .trim_end_matches('/');

        value
            .parse::<SocketAddr>()
            .map(|addr| addr.to_string())
            .map_err(|_| format!("Proxy URL must contain a local host:port, got {proxy_url:?}"))
    }

    fn resolve_helper(app: &AppHandle) -> Result<PathBuf, String> {
        if let Some(path) = env::var_os("TBH_NETHELPER") {
            let path = PathBuf::from(path);
            if path.exists() {
                return Ok(path);
            }
        }

        let mut candidates = Vec::new();
        if let Ok(exe) = env::current_exe() {
            if let Some(dir) = exe.parent() {
                push_helper_candidates(&mut candidates, dir.to_path_buf());
            }
        }
        if let Ok(resource_dir) = app.path().resource_dir() {
            push_helper_candidates(&mut candidates, resource_dir);
        }
        if let Ok(current_dir) = env::current_dir() {
            push_helper_candidates(&mut candidates, current_dir.clone());
            push_helper_candidates(&mut candidates, current_dir.join("src-tauri"));
        }

        candidates.into_iter().find(|path| path.exists()).ok_or_else(|| {
            "Windows network helper was not found. Build it with `cargo build -p tbhdashboard-nethelper --target x86_64-pc-windows-msvc` and copy it into src-tauri/binaries or next to the dashboard executable.".to_string()
        })
    }

    fn push_helper_candidates(candidates: &mut Vec<PathBuf>, dir: PathBuf) {
        for name in [
            "tbhdashboard-nethelper.exe",
            "tbhdashboard-nethelper-x86_64-pc-windows-msvc.exe",
        ] {
            candidates.push(dir.join(name));
            candidates.push(dir.join("binaries").join(name));
        }
    }
}

#[cfg(not(target_os = "windows"))]
mod platform {
    use tauri::AppHandle;

    pub struct NetHelperCleanup;

    #[allow(dead_code)]
    pub fn start_for_game(
        _app: &AppHandle,
        _game_pid: u32,
        _proxy_url: &str,
    ) -> Result<(), String> {
        Ok(())
    }

    pub fn stop() {}
}

pub use platform::*;
