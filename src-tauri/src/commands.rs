use std::collections::HashMap;
use std::process::Command;
use std::sync::{Arc, Mutex};

use tauri::Manager;
use tauri::State;
use tauri_plugin_dialog::DialogExt;

use crate::catalog::StaticCatalog;
use crate::chests;
use crate::config;
use crate::models::*;
use crate::state::StateRepository;

const STEAM_APP_ID: &str = "3678970";

/// Shared app state managed by Tauri.
pub struct ManagedState {
    repo: StateRepository,
    pub catalog: Mutex<StaticCatalog>,
    proxy_status: Arc<Mutex<ProxyStatus>>,
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
        let saved = repo.load();
        let root = saved.assets_path.as_deref().map(std::path::PathBuf::from);
        Self {
            repo,
            catalog: Mutex::new(StaticCatalog::new(root)),
            proxy_status: Arc::new(Mutex::new(ProxyStatus::starting())),
        }
    }

    pub fn repo(&self) -> &StateRepository {
        &self.repo
    }

    pub fn proxy_status(&self) -> Arc<Mutex<ProxyStatus>> {
        self.proxy_status.clone()
    }
}

#[tauri::command]
pub fn get_proxy_status(state: State<'_, ManagedState>) -> ProxyStatus {
    state.proxy_status.lock().unwrap().clone()
}

// ---- Settings ----

#[tauri::command]
pub fn get_settings(state: State<'_, ManagedState>) -> AppSettings {
    normalize_settings(state.repo().load().settings)
}

#[tauri::command]
pub fn set_settings(state: State<'_, ManagedState>, settings: AppSettings) -> bool {
    let mut state_data = state.repo().load();
    state_data.settings = normalize_settings(settings);
    state.repo().save(&state_data).is_ok()
}

fn normalize_settings(mut settings: AppSettings) -> AppSettings {
    let linux_default = "HTTP_PROXY=http://127.0.0.1:8080 HTTPS_PROXY=http://127.0.0.1:8080 ALL_PROXY=http://127.0.0.1:8080 %command%";
    let windows_default = "cmd /c \"set HTTP_PROXY=http://127.0.0.1:8080 && set HTTPS_PROXY=http://127.0.0.1:8080 && %command%\"";
    let current_default = default_steam_launch_options();
    let current = settings.steam_launch_options.trim();

    if current.is_empty() || (cfg!(target_os = "windows") && current == linux_default) || (!cfg!(target_os = "windows") && current == windows_default) {
        settings.steam_launch_options = current_default;
    }

    settings
}

#[tauri::command]
pub fn launch_game(state: State<'_, ManagedState>) -> LaunchGameResult {
    let settings = state.repo().load().settings;
    launch_game_from_settings(&settings)
}

pub fn launch_game_from_settings(settings: &AppSettings) -> LaunchGameResult {
    let settings = normalize_settings(settings.clone());
    match launch_steam_app(&settings) {
        Ok(()) => LaunchGameResult {
            ok: true,
            message: "Launch request sent to Steam".to_string(),
        },
        Err(err) => LaunchGameResult {
            ok: false,
            message: err,
        },
    }
}

fn launch_steam_app(settings: &AppSettings) -> Result<(), String> {
    let options = settings.steam_launch_options.trim();
    let args = if settings.include_steam_launch_options && !options.is_empty() {
        split_launch_options(options)?
    } else {
        Vec::new()
    };
    open_steam_app(&args)
}

fn split_launch_options(value: &str) -> Result<Vec<String>, String> {
    let mut args = Vec::new();
    let mut current = String::new();
    let mut quote = None;
    let mut escaped = false;

    for ch in value.chars() {
        if escaped {
            current.push(ch);
            escaped = false;
            continue;
        }

        if ch == '\\' {
            escaped = true;
            continue;
        }

        if let Some(quote_ch) = quote {
            if ch == quote_ch {
                quote = None;
            } else {
                current.push(ch);
            }
            continue;
        }

        match ch {
            '\'' | '"' => quote = Some(ch),
            ch if ch.is_whitespace() => {
                if !current.is_empty() {
                    args.push(std::mem::take(&mut current));
                }
            }
            _ => current.push(ch),
        }
    }

    if escaped {
        current.push('\\');
    }

    if quote.is_some() {
        return Err("Steam launch options have an unclosed quote".to_string());
    }

    if !current.is_empty() {
        args.push(current);
    }

    Ok(args)
}

#[cfg(target_os = "windows")]
fn open_steam_app(args: &[String]) -> Result<(), String> {
    let direct = Command::new("steam")
        .arg("-applaunch")
        .arg(STEAM_APP_ID)
        .args(args)
        .spawn();
    if direct.is_ok() {
        return Ok(());
    }

    if args.is_empty() {
        Command::new("cmd")
            .args(["/C", "start", "", &format!("steam://run/{STEAM_APP_ID}")])
            .spawn()
            .map(|_| ())
            .map_err(|err| format!("Failed to ask Steam to launch: {err}"))
    } else {
        Err("Custom launch options require steam.exe to be available on PATH".to_string())
    }
}

#[cfg(target_os = "macos")]
fn open_steam_app(args: &[String]) -> Result<(), String> {
    Command::new("open")
        .args(["-a", "Steam", "--args", "-applaunch", STEAM_APP_ID])
        .args(args)
        .spawn()
        .map(|_| ())
        .map_err(|err| format!("Failed to ask Steam to launch: {err}"))
}

#[cfg(all(unix, not(target_os = "macos")))]
fn open_steam_app(args: &[String]) -> Result<(), String> {
    match Command::new("steam")
        .arg("-applaunch")
        .arg(STEAM_APP_ID)
        .args(args)
        .spawn()
    {
        Ok(_) => Ok(()),
        Err(steam_err) if args.is_empty() => {
            Command::new("xdg-open")
                .arg(format!("steam://run/{STEAM_APP_ID}"))
                .spawn()
                .map(|_| ())
                .map_err(|xdg_err| {
                    format!("Failed to ask Steam to launch: steam: {steam_err}; xdg-open: {xdg_err}")
                })
        }
        Err(steam_err) => Err(format!("Failed to run Steam directly: {steam_err}")),
    }
}

// ---- Chest Queue ----

#[tauri::command]
pub fn get_chest_rows(
    state: State<'_, ManagedState>,
    include_claimed: bool,
) -> Vec<ChestRow> {
    let catalog = state.catalog.lock().unwrap();
    chests::get_rows(&catalog, include_claimed, state.repo())
}

#[tauri::command]
pub fn get_box_summary(
    state: State<'_, ManagedState>,
) -> HashMap<String, usize> {
    chests::box_summary(state.repo())
}

#[tauri::command]
pub fn mark_opened(
    state: State<'_, ManagedState>,
    key: String,
) -> usize {
    chests::mark_claimed_by_keys(&[key], "manual", state.repo())
}

#[tauri::command]
pub fn mark_all_opened(
    state: State<'_, ManagedState>,
) -> usize {
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
pub fn get_last_added(
    state: State<'_, ManagedState>,
) -> Option<AddedItemsSnapshot> {
    state.repo().load().last_added
}

// ---- Reroll Preview / ProcessBox ----

#[tauri::command]
pub fn get_last_processbox(
    state: State<'_, ManagedState>,
) -> Option<ProcessBoxInfo> {
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
pub fn get_events(
    state: State<'_, ManagedState>,
) -> Vec<StateEvent> {
    state.repo().load().events
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
pub fn get_catalog_status(
    state: State<'_, ManagedState>,
) -> CatalogStatus {
    let catalog = state.catalog.lock().unwrap();
    CatalogStatus {
        valid: catalog.valid,
        items_count: catalog.items_count(),
        stages_count: catalog.stages_count(),
        display_names_count: catalog.display_names_count(),
    }
}

// ---- Reload catalog (after assets update) ----

#[tauri::command]
pub fn reload_catalog(
    state: State<'_, ManagedState>,
) -> bool {
    let mut catalog = state.catalog.lock().unwrap();
    let saved = state.repo().load();
    let root = saved.assets_path.as_deref().map(std::path::PathBuf::from);
    *catalog = StaticCatalog::new(root);
    catalog.valid
}

// ---- Assets path ----

#[tauri::command]
pub fn get_assets_path(
    state: State<'_, ManagedState>,
) -> Option<String> {
    state.repo().load().assets_path
}

#[tauri::command]
pub fn set_assets_path(
    state: State<'_, ManagedState>,
    path: String,
) -> bool {
    let mut state_data = state.repo().load();
    state_data.assets_path = Some(path);
    let _ = state.repo().save(&state_data);

    let root = state_data.assets_path.as_deref().map(std::path::PathBuf::from);
    let mut catalog = state.catalog.lock().unwrap();
    *catalog = StaticCatalog::new(root);
    catalog.valid
}

#[tauri::command]
pub fn get_assets_root(
    state: State<'_, ManagedState>,
) -> String {
    let saved = state.repo().load();
    match saved.assets_path {
        Some(ref p) => p.clone(),
        None => config::assets_root().to_string_lossy().into_owned(),
    }
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
    path.into_path().ok().map(|p| p.to_string_lossy().into_owned())
}
