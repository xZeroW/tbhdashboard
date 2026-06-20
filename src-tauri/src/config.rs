use std::collections::HashMap;
use std::path::PathBuf;

/// Resolve the state file path: $TBH_STATE or ~/.cache/tbh_dashboard_state.json
pub fn state_path() -> PathBuf {
    if let Ok(val) = std::env::var("TBH_STATE") {
        return PathBuf::from(val);
    }
    dirs::cache_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("tbh_dashboard_state.json")
}

/// Resolve the assets root: $TBH_ASSETS or <exe_dir>/Assets
pub fn assets_root() -> PathBuf {
    if let Ok(val) = std::env::var("TBH_ASSETS") {
        return PathBuf::from(val);
    }
    std::env::current_exe()
        .unwrap_or_else(|_| PathBuf::from("."))
        .parent()
        .unwrap_or(&PathBuf::from("."))
        .join("Assets")
}

pub fn downloaded_assets_base_dir() -> PathBuf {
    dirs::cache_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("tbhdashboard")
        .join("downloaded-assets")
}

pub fn default_asset_manifest_url() -> String {
    std::env::var("TBH_ASSET_MANIFEST_URL")
        .unwrap_or_else(|_| "http://127.0.0.1:3000/assets/manifest".to_string())
}

pub fn default_server_url() -> String {
    std::env::var("TBH_SERVER_URL").unwrap_or_else(|_| "http://127.0.0.1:3000".to_string())
}

pub const REFRESH_SECONDS: f64 = 0.5;

pub fn box_names() -> HashMap<i64, &'static str> {
    let mut m = HashMap::new();
    m.insert(910651, "Common Treasure Chest");
    m.insert(920651, "Stage Treasure Chest");
    m
}

pub const RARITY_ORDER: &[&str] = &[
    "COMMON",
    "UNCOMMON",
    "RARE",
    "LEGENDARY",
    "IMMORTAL",
    "ARCANA",
    "BEYOND",
    "CELESTIAL",
    "DIVINE",
    "COSMIC",
    "UNKNOWN",
];

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    static ENV_LOCK: Mutex<()> = Mutex::new(());

    #[test]
    fn state_path_default() {
        let _guard = ENV_LOCK.lock().unwrap();
        unsafe { std::env::remove_var("TBH_STATE") };
        let p = state_path();
        assert!(p.to_string_lossy().contains("tbh_dashboard_state.json"));
    }

    #[test]
    fn state_path_env_override() {
        let _guard = ENV_LOCK.lock().unwrap();
        unsafe { std::env::set_var("TBH_STATE", "/tmp/test_state.json") };
        let p = state_path();
        assert_eq!(p, PathBuf::from("/tmp/test_state.json"));
        unsafe { std::env::remove_var("TBH_STATE") };
    }

    #[test]
    fn rarity_order_has_all_variants() {
        assert_eq!(RARITY_ORDER.len(), 11);
        assert!(RARITY_ORDER.contains(&"COMMON"));
        assert!(RARITY_ORDER.contains(&"UNKNOWN"));
    }

    #[test]
    fn box_names_contains_expected() {
        let names = box_names();
        assert_eq!(names[&910651], "Common Treasure Chest");
        assert_eq!(names[&920651], "Stage Treasure Chest");
    }
}
