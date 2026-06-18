mod catalog;
mod capture;
mod chests;
mod commands;
mod config;
mod models;
mod state;
mod utils;

use commands::ManagedState;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_shell::init())
        .manage(ManagedState::new())
        .invoke_handler(tauri::generate_handler![
            commands::get_chest_rows,
            commands::get_box_summary,
            commands::mark_opened,
            commands::mark_all_opened,
            commands::get_last_added,
            commands::get_last_processbox,
            commands::get_farm_ranking,
            commands::get_events,
            commands::get_catalog_status,
            commands::reload_catalog,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
