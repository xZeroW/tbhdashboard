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

/// Resolve the assets root: $TBH_ASSETS or <exe_dir>
pub fn assets_root() -> PathBuf {
    if let Ok(val) = std::env::var("TBH_ASSETS") {
        return PathBuf::from(val);
    }
    std::env::current_exe()
        .unwrap_or_else(|_| PathBuf::from("."))
        .parent()
        .unwrap_or(&PathBuf::from("."))
        .to_path_buf()
}

pub const REFRESH_SECONDS: f64 = 1.0;

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

pub fn rarity_rgb() -> HashMap<&'static str, (u8, u8, u8)> {
    HashMap::from([
        ("COMMON", (190, 190, 190)),
        ("UNCOMMON", (70, 220, 100)),
        ("RARE", (80, 140, 255)),
        ("LEGENDARY", (255, 205, 60)),
        ("IMMORTAL", (210, 110, 255)),
        ("ARCANA", (255, 100, 205)),
        ("BEYOND", (70, 220, 255)),
        ("CELESTIAL", (120, 255, 255)),
        ("DIVINE", (255, 255, 255)),
        ("COSMIC", (255, 90, 90)),
        ("UNKNOWN", (170, 170, 170)),
    ])
}

pub fn rarity_rank() -> HashMap<&'static str, usize> {
    RARITY_ORDER
        .iter()
        .enumerate()
        .map(|(i, &r)| (r, i))
        .collect()
}

pub fn rarity_value() -> HashMap<&'static str, i64> {
    HashMap::from([
        ("COMMON", 1),
        ("UNCOMMON", 2),
        ("RARE", 4),
        ("LEGENDARY", 8),
        ("IMMORTAL", 16),
        ("ARCANA", 32),
        ("BEYOND", 64),
        ("CELESTIAL", 128),
        ("DIVINE", 256),
        ("COSMIC", 512),
        ("UNKNOWN", 0),
    ])
}

pub fn important_rarities() -> &'static [&'static str] {
    &[
        "LEGENDARY",
        "IMMORTAL",
        "ARCANA",
        "BEYOND",
        "CELESTIAL",
        "DIVINE",
        "COSMIC",
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn state_path_default() {
        std::env::remove_var("TBH_STATE");
        let p = state_path();
        assert!(p.to_string_lossy().contains("tbh_dashboard_state.json"));
    }

    #[test]
    fn state_path_env_override() {
        std::env::set_var("TBH_STATE", "/tmp/test_state.json");
        let p = state_path();
        assert_eq!(p, PathBuf::from("/tmp/test_state.json"));
        std::env::remove_var("TBH_STATE");
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

    #[test]
    fn rarity_rank_matches_order() {
        let rank = rarity_rank();
        for (i, &name) in RARITY_ORDER.iter().enumerate() {
            assert_eq!(rank[name], i);
        }
    }

    #[test]
    fn important_rarities_count() {
        assert_eq!(important_rarities().len(), 7);
    }
}
