use wasm_bindgen::prelude::*;
use serde::{Deserialize, Serialize};

#[wasm_bindgen]
extern "C" {
    #[wasm_bindgen(js_namespace = ["window", "__TAURI__", "core"])]
    pub async fn invoke(cmd: &str, args: JsValue) -> JsValue;

    #[wasm_bindgen(js_namespace = ["window", "__TAURI__", "event"])]
    pub async fn listen(event: &str, handler: &JsValue) -> JsValue;
}

// ---- Types matching Rust Tauri command return types ----

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct ChestRow {
    pub remaining: f64,
    pub claim: Option<String>,
    pub key: Option<String>,
    pub box_label: String,
    #[serde(rename = "rewardId")]
    pub reward_id: Option<i64>,
    pub rarity: String,
    pub name: String,
    #[serde(rename = "isGet")]
    pub is_get: bool,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct AddedItem {
    pub at: String,
    #[serde(rename = "itemId")]
    pub item_id: Option<i64>,
    #[serde(rename = "itemKey")]
    pub item_key: String,
    pub count: i32,
    pub rarity: String,
    pub name: String,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct AddedItemsSnapshot {
    pub at: String,
    pub source: String,
    pub items: Vec<AddedItem>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct ProcessBoxCreatedItem {
    #[serde(rename = "itemId")]
    pub item_id: Option<i64>,
    pub count: i32,
    #[serde(rename = "dropKey")]
    pub drop_key: Option<i64>,
    pub name: String,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct ProcessBoxInfo {
    pub tn: Option<serde_json::Value>,
    #[serde(rename = "isReset")]
    pub is_reset: bool,
    pub created: Vec<ProcessBoxCreatedItem>,
    pub at: String,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct FarmRow {
    pub per_hour: Option<f64>,
    pub expected: f64,
    pub stage_id: i64,
    pub name: String,
    pub difficulty: String,
    pub level: i32,
    pub boxes: Vec<(i64, i32)>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct StateEvent {
    pub at: String,
    pub text: String,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct CatalogStatus {
    pub valid: bool,
    pub items_count: usize,
    pub stages_count: usize,
    pub display_names_count: usize,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct ProxyStatus {
    pub running: bool,
    pub state: String,
    pub message: String,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct AppSettings {
    pub refresh_ms: u32,
    pub log_level: String,
    pub proxy_url: String,
    pub include_steam_launch_options: bool,
    pub steam_launch_options: String,
    pub launch_game_on_start: bool,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct LaunchGameResult {
    pub ok: bool,
    pub message: String,
}

impl Default for AppSettings {
    fn default() -> Self {
        Self {
            refresh_ms: 500,
            log_level: "info".to_string(),
            proxy_url: "http://127.0.0.1:8080".to_string(),
            include_steam_launch_options: false,
            steam_launch_options: String::new(),
            launch_game_on_start: false,
        }
    }
}

// ---- Invoke helpers ----

pub async fn invoke_get_chest_rows(include_claimed: bool) -> Vec<ChestRow> {
    let args = serde_wasm_bindgen::to_value(&serde_json::json!({
        "includeClaimed": include_claimed
    })).unwrap();
    let result = invoke("get_chest_rows", args).await;
    serde_wasm_bindgen::from_value(result).unwrap_or_default()
}

pub async fn invoke_get_box_summary() -> std::collections::HashMap<String, usize> {
    let args = serde_wasm_bindgen::to_value(&serde_json::json!({})).unwrap();
    let result = invoke("get_box_summary", args).await;
    serde_wasm_bindgen::from_value(result).unwrap_or_default()
}

pub async fn invoke_mark_opened(key: &str) {
    let args = serde_wasm_bindgen::to_value(&serde_json::json!({
        "key": key
    })).unwrap();
    invoke("mark_opened", args).await;
}

pub async fn invoke_mark_all_opened() {
    let args = serde_wasm_bindgen::to_value(&serde_json::json!({})).unwrap();
    invoke("mark_all_opened", args).await;
}

pub async fn invoke_get_last_added() -> Option<AddedItemsSnapshot> {
    let args = serde_wasm_bindgen::to_value(&serde_json::json!({})).unwrap();
    let result = invoke("get_last_added", args).await;
    serde_wasm_bindgen::from_value(result).ok()
}

pub async fn invoke_get_last_processbox() -> Option<ProcessBoxInfo> {
    let args = serde_wasm_bindgen::to_value(&serde_json::json!({})).unwrap();
    let result = invoke("get_last_processbox", args).await;
    serde_wasm_bindgen::from_value(result).ok()
}

pub async fn invoke_get_farm_ranking(
    rarity: Option<String>,
    kind: Option<String>,
    item_id: Option<i64>,
    min_level: Option<i32>,
    max_level: Option<i32>,
    clear_time: Option<f64>,
) -> Vec<FarmRow> {
    let args = serde_wasm_bindgen::to_value(&serde_json::json!({
        "rarity": rarity,
        "kind": kind,
        "itemId": item_id,
        "minLevel": min_level,
        "maxLevel": max_level,
        "clearTime": clear_time,
    })).unwrap();
    let result = invoke("get_farm_ranking", args).await;
    serde_wasm_bindgen::from_value(result).unwrap_or_default()
}

pub async fn invoke_get_events() -> Vec<StateEvent> {
    let args = serde_wasm_bindgen::to_value(&serde_json::json!({})).unwrap();
    let result = invoke("get_events", args).await;
    serde_wasm_bindgen::from_value(result).unwrap_or_default()
}

pub async fn invoke_get_catalog_status() -> Option<CatalogStatus> {
    let args = serde_wasm_bindgen::to_value(&serde_json::json!({})).unwrap();
    let result = invoke("get_catalog_status", args).await;
    serde_wasm_bindgen::from_value(result).ok()
}

pub async fn invoke_get_proxy_status() -> Option<ProxyStatus> {
    let args = serde_wasm_bindgen::to_value(&serde_json::json!({})).unwrap();
    let result = invoke("get_proxy_status", args).await;
    serde_wasm_bindgen::from_value(result).ok()
}

pub async fn invoke_get_settings() -> AppSettings {
    let args = serde_wasm_bindgen::to_value(&serde_json::json!({})).unwrap();
    let result = invoke("get_settings", args).await;
    serde_wasm_bindgen::from_value(result).unwrap_or_default()
}

pub async fn invoke_set_settings(settings: AppSettings) -> bool {
    let args = serde_wasm_bindgen::to_value(&serde_json::json!({
        "settings": settings
    })).unwrap();
    let result = invoke("set_settings", args).await;
    result.as_bool().unwrap_or(false)
}

pub async fn invoke_launch_game() -> LaunchGameResult {
    let args = serde_wasm_bindgen::to_value(&serde_json::json!({})).unwrap();
    let result = invoke("launch_game", args).await;
    serde_wasm_bindgen::from_value(result).unwrap_or(LaunchGameResult {
        ok: false,
        message: "Failed to read launch result".to_string(),
    })
}

pub async fn invoke_reload_catalog() -> bool {
    let args = serde_wasm_bindgen::to_value(&serde_json::json!({})).unwrap();
    let result = invoke("reload_catalog", args).await;
    result.as_bool().unwrap_or(false)
}

pub async fn invoke_get_assets_path() -> Option<String> {
    let args = serde_wasm_bindgen::to_value(&serde_json::json!({})).unwrap();
    let result = invoke("get_assets_path", args).await;
    result.as_string()
}

pub async fn invoke_set_assets_path(path: &str) -> bool {
    let args = serde_wasm_bindgen::to_value(&serde_json::json!({
        "path": path
    })).unwrap();
    let result = invoke("set_assets_path", args).await;
    result.as_bool().unwrap_or(false)
}

pub async fn invoke_get_assets_root() -> String {
    let args = serde_wasm_bindgen::to_value(&serde_json::json!({})).unwrap();
    let result = invoke("get_assets_root", args).await;
    result.as_string().unwrap_or_default()
}

pub async fn invoke_browse_assets_folder() -> Option<String> {
    let args = serde_wasm_bindgen::to_value(&serde_json::json!({})).unwrap();
    let result = invoke("browse_assets_folder", args).await;
    result.as_string()
}
