use serde::{Deserialize, Serialize};
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
extern "C" {
    #[wasm_bindgen(js_namespace = ["window", "__TAURI__", "core"])]
    pub async fn invoke(cmd: &str, args: JsValue) -> JsValue;

    #[wasm_bindgen(js_namespace = ["window", "__TAURI__", "event"])]
    pub async fn listen(event: &str, handler: &JsValue) -> JsValue;

    #[wasm_bindgen(js_namespace = window, js_name = openPaddleCheckout)]
    async fn open_paddle_checkout_js(config: JsValue) -> JsValue;
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
    #[serde(default)]
    pub kind: String,
    #[serde(default)]
    pub slot: String,
    #[serde(rename = "isGet")]
    pub is_get: bool,
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
pub struct CatalogStatus {
    pub valid: bool,
    pub items_count: usize,
    pub stages_count: usize,
    pub display_names_count: usize,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct AppSettings {
    pub refresh_ms: u32,
    pub log_level: String,
    pub proxy_url: String,
    pub include_steam_launch_options: bool,
    pub steam_launch_options: String,
    pub launch_game_on_start: bool,
    pub steam_launch_options_prompted: bool,
    pub asset_manifest_url: String,
    pub server_url: String,
    pub auth_token: String,
    pub steam_id: String,
    pub share_claimable_rewards: bool,
    #[serde(default)]
    pub offline_mode: bool,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct AssetManifest {
    pub version: String,
    pub url: String,
    pub sha256: Option<String>,
    pub size_bytes: Option<u64>,
    pub notes: Option<String>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct AssetUpdateStatus {
    pub ok: bool,
    pub message: String,
    pub current_version: Option<String>,
    pub latest_version: Option<String>,
    pub update_available: bool,
    pub manifest: Option<AssetManifest>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct AssetDownloadResult {
    pub ok: bool,
    pub message: String,
    pub version: Option<String>,
    pub assets_path: Option<String>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct ObservationUploadResult {
    pub ok: bool,
    pub uploaded: usize,
    pub skipped: usize,
    pub message: String,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct LaunchGameResult {
    pub ok: bool,
    pub message: String,
}

#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct EntitlementInfo {
    pub tier: String,
    pub source: String,
    pub expires_at: Option<String>,
}

#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct AuthUser {
    pub id: String,
    pub username: String,
    pub email: Option<String>,
    #[serde(default)]
    pub is_admin: bool,
    pub entitlement: Option<EntitlementInfo>,
}

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
#[serde(rename_all = "camelCase")]
pub struct LoginResult {
    pub ok: bool,
    pub message: String,
    pub user: Option<AuthUser>,
    #[serde(default)]
    pub status: Option<u16>,
}

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
#[serde(rename_all = "camelCase")]
pub struct CheckoutConfig {
    pub client_token: Option<String>,
    pub price_id: String,
    pub environment: String,
}

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
#[serde(rename_all = "camelCase")]
pub struct RegisterResult {
    pub ok: bool,
    pub message: String,
    pub user_id: Option<String>,
    pub username: Option<String>,
    pub email: Option<String>,
    pub checkout: Option<CheckoutConfig>,
}

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
#[serde(rename_all = "camelCase")]
pub struct ActivationStatusResult {
    pub ok: bool,
    pub message: String,
    pub active: bool,
}

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
#[serde(rename_all = "camelCase")]
pub struct InactiveCheckoutResult {
    pub ok: bool,
    pub message: String,
    pub user_id: Option<String>,
    pub username: Option<String>,
    pub email: Option<String>,
    pub checkout: Option<CheckoutConfig>,
}

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
#[serde(rename_all = "camelCase")]
pub struct PaddleCheckoutResult {
    pub opened: bool,
    pub completed: bool,
    pub message: Option<String>,
    pub error: Option<String>,
}

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
#[serde(rename_all = "camelCase")]
pub struct RequestLogEntry {
    pub at: String,
    pub method: String,
    pub host: String,
    pub path: String,
    pub source: String,
    pub content_type: String,
    pub body_bytes: usize,
    pub body: String,
    #[serde(default)]
    pub response_body: String,
    #[serde(default)]
    pub response_body_bytes: usize,
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
            steam_launch_options_prompted: false,
            asset_manifest_url: "http://127.0.0.1:3000/assets/manifest".to_string(),
            server_url: "http://127.0.0.1:3000".to_string(),
            auth_token: String::new(),
            steam_id: String::new(),
            share_claimable_rewards: false,
            offline_mode: false,
        }
    }
}

// ---- Invoke helpers ----

pub async fn invoke_get_chest_rows(include_claimed: bool) -> Vec<ChestRow> {
    let args = serde_wasm_bindgen::to_value(&serde_json::json!({
        "includeClaimed": include_claimed
    }))
    .unwrap();
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
    }))
    .unwrap();
    invoke("mark_opened", args).await;
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
    }))
    .unwrap();
    let result = invoke("get_farm_ranking", args).await;
    serde_wasm_bindgen::from_value(result).unwrap_or_default()
}

pub async fn invoke_get_catalog_status() -> Option<CatalogStatus> {
    let args = serde_wasm_bindgen::to_value(&serde_json::json!({})).unwrap();
    let result = invoke("get_catalog_status", args).await;
    serde_wasm_bindgen::from_value(result).ok()
}

pub async fn invoke_get_rarity_order() -> Vec<String> {
    let args = serde_wasm_bindgen::to_value(&serde_json::json!({})).unwrap();
    let result = invoke("get_rarity_order", args).await;
    serde_wasm_bindgen::from_value(result).unwrap_or_default()
}

pub async fn invoke_get_settings() -> AppSettings {
    let args = serde_wasm_bindgen::to_value(&serde_json::json!({})).unwrap();
    let result = invoke("get_settings", args).await;
    serde_wasm_bindgen::from_value(result).unwrap_or_default()
}

pub async fn invoke_login(server_url: &str, username: &str, password: &str) -> LoginResult {
    let args = serde_wasm_bindgen::to_value(&serde_json::json!({
        "serverUrl": server_url,
        "username": username,
        "password": password,
    }))
    .unwrap();
    let result = invoke("login", args).await;
    serde_wasm_bindgen::from_value(result).unwrap_or(LoginResult {
        ok: false,
        message: "Failed to read login result".to_string(),
        user: None,
        status: None,
    })
}

pub async fn invoke_register(
    server_url: &str,
    username: &str,
    email: &str,
    password: &str,
) -> RegisterResult {
    let args = serde_wasm_bindgen::to_value(&serde_json::json!({
        "serverUrl": server_url,
        "username": username,
        "email": email,
        "password": password,
    }))
    .unwrap();
    let result = invoke("register", args).await;
    serde_wasm_bindgen::from_value(result).unwrap_or(RegisterResult {
        ok: false,
        message: "Failed to read registration result".to_string(),
        user_id: None,
        username: None,
        email: None,
        checkout: None,
    })
}

pub async fn invoke_get_activation_status(
    server_url: &str,
    user_id: &str,
) -> ActivationStatusResult {
    let args = serde_wasm_bindgen::to_value(&serde_json::json!({
        "serverUrl": server_url,
        "userId": user_id,
    }))
    .unwrap();
    let result = invoke("get_activation_status", args).await;
    serde_wasm_bindgen::from_value(result).unwrap_or(ActivationStatusResult {
        ok: false,
        message: "Failed to read activation status".to_string(),
        active: false,
    })
}

pub async fn invoke_get_inactive_checkout(
    server_url: &str,
    username: &str,
    password: &str,
) -> InactiveCheckoutResult {
    let args = serde_wasm_bindgen::to_value(&serde_json::json!({
        "serverUrl": server_url,
        "username": username,
        "password": password,
    }))
    .unwrap();
    let result = invoke("get_inactive_checkout", args).await;
    serde_wasm_bindgen::from_value(result).unwrap_or(InactiveCheckoutResult {
        ok: false,
        message: "Failed to retrieve checkout details".to_string(),
        user_id: None,
        username: None,
        email: None,
        checkout: None,
    })
}

pub async fn invoke_open_paddle_checkout(
    checkout: &CheckoutConfig,
    email: &str,
    user_id: &str,
) -> PaddleCheckoutResult {
    #[derive(Serialize)]
    #[serde(rename_all = "camelCase")]
    struct PaddleJsConfig {
        client_token: Option<String>,
        price_id: String,
        environment: String,
        email: String,
        user_id: String,
    }

    let config = serde_wasm_bindgen::to_value(&PaddleJsConfig {
        client_token: checkout.client_token.clone(),
        price_id: checkout.price_id.clone(),
        environment: checkout.environment.clone(),
        email: email.to_string(),
        user_id: user_id.to_string(),
    })
    .unwrap();

    let result = open_paddle_checkout_js(config).await;
    let mut checkout_result: PaddleCheckoutResult = serde_wasm_bindgen::from_value(result)
        .unwrap_or(PaddleCheckoutResult {
            opened: false,
            completed: false,
            message: Some("Failed to read Paddle checkout result.".to_string()),
            error: None,
        });

    if let Some(error) = checkout_result.error.take() {
        checkout_result.opened = false;
        checkout_result.completed = false;
        checkout_result.message = Some(error);
    }

    checkout_result
}

pub async fn invoke_get_current_user() -> Option<AuthUser> {
    let args = serde_wasm_bindgen::to_value(&serde_json::json!({})).unwrap();
    let result = invoke("get_current_user", args).await;
    serde_wasm_bindgen::from_value(result).ok().flatten()
}

pub async fn invoke_logout() -> bool {
    let args = serde_wasm_bindgen::to_value(&serde_json::json!({})).unwrap();
    let result = invoke("logout", args).await;
    result.as_bool().unwrap_or(false)
}

pub async fn invoke_skip_login() -> AuthUser {
    let args = serde_wasm_bindgen::to_value(&serde_json::json!({})).unwrap();
    let result = invoke("skip_login", args).await;
    serde_wasm_bindgen::from_value(result).unwrap_or_default()
}

pub async fn invoke_set_settings(settings: AppSettings) -> bool {
    let args = serde_wasm_bindgen::to_value(&serde_json::json!({
        "settings": settings
    }))
    .unwrap();
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

pub async fn invoke_get_asset_update_status() -> Option<AssetUpdateStatus> {
    let args = serde_wasm_bindgen::to_value(&serde_json::json!({})).unwrap();
    let result = invoke("get_asset_update_status", args).await;
    serde_wasm_bindgen::from_value(result).ok()
}

pub async fn invoke_download_latest_assets() -> AssetDownloadResult {
    let args = serde_wasm_bindgen::to_value(&serde_json::json!({})).unwrap();
    let result = invoke("download_latest_assets", args).await;
    serde_wasm_bindgen::from_value(result).unwrap_or(AssetDownloadResult {
        ok: false,
        message: "Failed to download assets".to_string(),
        version: None,
        assets_path: None,
    })
}

pub async fn invoke_upload_claimable_reward_observations() -> ObservationUploadResult {
    let args = serde_wasm_bindgen::to_value(&serde_json::json!({})).unwrap();
    let result = invoke("upload_claimable_reward_observations", args).await;
    serde_wasm_bindgen::from_value(result).unwrap_or(ObservationUploadResult {
        ok: false,
        uploaded: 0,
        skipped: 0,
        message: "Failed to upload observations".to_string(),
    })
}

pub async fn invoke_get_request_history() -> Vec<RequestLogEntry> {
    let args = serde_wasm_bindgen::to_value(&serde_json::json!({})).unwrap();
    let result = invoke("get_request_history", args).await;
    serde_wasm_bindgen::from_value(result).unwrap_or_default()
}

pub async fn invoke_clear_request_history() -> bool {
    let args = serde_wasm_bindgen::to_value(&serde_json::json!({})).unwrap();
    let result = invoke("clear_request_history", args).await;
    result.as_bool().unwrap_or(false)
}

// ---- Updater types and invokes ----

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct UpdateInfo {
    pub version: String,
    pub current_version: String,
    pub body: Option<String>,
    pub date: Option<String>,
}

pub async fn invoke_check_update() -> Option<UpdateInfo> {
    let args = serde_wasm_bindgen::to_value(&serde_json::json!({})).unwrap();
    let result = invoke("check_update", args).await;
    serde_wasm_bindgen::from_value(result).ok().flatten()
}

pub async fn invoke_install_update() -> Result<(), String> {
    let args = serde_wasm_bindgen::to_value(&serde_json::json!({})).unwrap();
    let result = invoke("install_update", args).await;
    if result.is_null() {
        Ok(())
    } else {
        Err(serde_wasm_bindgen::from_value::<String>(result)
            .unwrap_or_else(|_| "Unknown error".to_string()))
    }
}
