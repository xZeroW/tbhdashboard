mod capture;
mod catalog;
mod chests;
mod commands;
mod config;
mod models;
mod sidecar;
mod state;
mod utils;

use commands::ManagedState;
use sidecar::SidecarManager;
use std::sync::Mutex;
use tauri::Manager;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    configure_linux_webkit_env();

    let managed = ManagedState::new();

    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_dialog::init())
        .manage(managed)
        .setup(|app| {
            // Start sidecar on app launch
            let state = app.state::<ManagedState>();
            let mut sidecar = SidecarManager::new(state.proxy_status());
            sidecar.start(app.handle(), state.repo());
            app.manage(Mutex::new(sidecar));

            let settings = state.repo().load().settings;
            if settings.launch_game_on_start {
                let _ = commands::launch_game_from_settings(&settings);
            }

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::get_proxy_status,
            commands::get_settings,
            commands::set_settings,
            commands::launch_game,
            commands::get_chest_rows,
            commands::get_box_summary,
            commands::mark_opened,
            commands::mark_all_opened,
            commands::get_last_added,
            commands::get_last_processbox,
            commands::get_farm_ranking,
            commands::get_events,
            commands::get_catalog_status,
            commands::get_rarity_order,
            commands::reload_catalog,
            commands::get_assets_path,
            commands::set_assets_path,
            commands::get_assets_root,
            commands::browse_assets_folder,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

#[cfg(target_os = "linux")]
fn configure_linux_webkit_env() {
    if std::env::var_os("WEBKIT_DISABLE_DMABUF_RENDERER").is_none() {
        // Avoid WebKitGTK EGL crashes seen on some Linux/Wayland GPU stacks.
        std::env::set_var("WEBKIT_DISABLE_DMABUF_RENDERER", "1");
    }
}

#[cfg(not(target_os = "linux"))]
fn configure_linux_webkit_env() {}
