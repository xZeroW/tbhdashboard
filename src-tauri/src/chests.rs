use std::collections::HashMap;

use crate::catalog::StaticCatalog;
use crate::config;
use crate::models::{ChestRecord, ChestRow, DynamicMap};
use crate::state::StateRepository;
use crate::utils::{parse_dt, utc_now_iso};

pub fn box_label(box_id: Option<i64>) -> String {
    let bid = match box_id {
        Some(id) => id,
        None => return "Unknown Chest".to_string(),
    };
    let names = config::box_names();
    if let Some(name) = names.get(&bid) {
        return name.to_string();
    }
    let s = bid.to_string();
    if s.starts_with("910") {
        return format!("Common Treasure Chest ({})", bid);
    }
    if s.starts_with("920") {
        return format!("Stage Treasure Chest ({})", bid);
    }
    format!("Box {}", bid)
}

pub fn normalize_chest(c: &DynamicMap, source: &str) -> Option<ChestRecord> {
    let item_key = c
        .get("itemKey")
        .or_else(|| c.get("item_key"))
        .or_else(|| c.get("inDate"))
        .or_else(|| c.get("uuid"))
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();
    if item_key.is_empty() {
        return None;
    }

    Some(ChestRecord {
        item_key,
        item_id: c
            .get("itemId")
            .or_else(|| c.get("item_id"))
            .and_then(|v| v.as_i64()),
        reward_item_id: c
            .get("rewardItemId")
            .or_else(|| c.get("reward_item_id"))
            .and_then(|v| v.as_i64()),
        reward_item_key: c
            .get("rewardItemKey")
            .or_else(|| c.get("reward_item_key"))
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string(),
        claimable_at: c
            .get("claimableAt")
            .or_else(|| c.get("claimable_at"))
            .or_else(|| c.get("claimTime"))
            .and_then(|v| v.as_str())
            .map(String::from),
        is_get: c.get("isGet").and_then(|v| v.as_bool()).unwrap_or(false),
        source: source.to_string(),
        seen_at: utc_now_iso(),
        raw: c.clone(),
        claimed_at: None,
        claim_source: None,
    })
}

pub fn sync_chests(chests: &[DynamicMap], source: &str, repo: &StateRepository) -> (usize, usize) {
    let mut state = repo.load();
    let old_count = state.chests.len();
    let mut new_db: HashMap<String, ChestRecord> = HashMap::new();
    let mut count = 0;
    for c in chests {
        if let Some(n) = normalize_chest(c, source) {
            new_db.insert(n.item_key.clone(), n);
            count += 1;
        }
    }
    state.chests = new_db;
    repo.add_event(
        &mut state,
        &format!(
            "{}: synced snapshot {}, replaced {}",
            source, count, old_count
        ),
    );
    repo.save(&state).unwrap();
    (count, old_count)
}

pub fn upsert_chests(
    chests: &[DynamicMap],
    source: &str,
    repo: &StateRepository,
) -> (usize, usize) {
    let mut state = repo.load();
    let mut added = 0usize;
    let mut updated = 0usize;
    for c in chests {
        if let Some(n) = normalize_chest(c, source) {
            let key = n.item_key.clone();
            if state.chests.contains_key(&key) {
                updated += 1;
            } else {
                added += 1;
            }
            let existing = state.chests.entry(key).or_default();
            let was_get = existing.is_get;
            let claimed_at = existing.claimed_at.clone();
            let claim_source = existing.claim_source.clone();
            *existing = n;
            if was_get {
                existing.is_get = true;
                existing.claimed_at = claimed_at;
                existing.claim_source = claim_source;
            }
        }
    }
    if added > 0 || updated > 0 {
        repo.add_event(
            &mut state,
            &format!("{}: +{}, updated {}", source, added, updated),
        );
    }
    repo.save(&state).unwrap();
    (added, updated)
}

pub fn mark_claimed_by_keys(keys: &[String], source: &str, repo: &StateRepository) -> usize {
    let mut state = repo.load();
    let mut changed = 0usize;
    let now = utc_now_iso();
    for key in keys {
        if key.is_empty() {
            continue;
        }
        if let Some(chest) = state.chests.get_mut(key)
            && !chest.is_get
        {
            chest.is_get = true;
            chest.claimed_at = Some(now.clone());
            chest.claim_source = Some(source.to_string());
            changed += 1;
        }
    }
    if changed > 0 {
        repo.add_event(
            &mut state,
            &format!("{}: marked {} opened", source, changed),
        );
    }
    repo.save(&state).unwrap();
    changed
}

pub fn get_rows(
    catalog: &StaticCatalog,
    include_claimed: bool,
    repo: &StateRepository,
) -> Vec<ChestRow> {
    let state = repo.load();
    let mut rows: Vec<ChestRow> = state
        .chests
        .values()
        .filter(|c| include_claimed || !c.is_get)
        .map(|c| {
            let claim = c.claimable_at.as_deref().and_then(parse_dt);
            let now = chrono::Utc::now();
            let remaining = claim
                .map(|cl| (cl - now).num_seconds() as f64)
                .unwrap_or(0.0);
            let (rarity, name) = catalog.item_parts(c.reward_item_id);
            let (kind, slot) = catalog.item_kind_slot(c.reward_item_id);
            ChestRow {
                remaining,
                claim: c.claimable_at.as_deref().and_then(parse_dt),
                key: Some(c.item_key.clone()),
                box_label: box_label(c.item_id),
                reward_id: c.reward_item_id,
                rarity,
                name,
                kind,
                slot,
                is_get: c.is_get,
            }
        })
        .collect();
    rows.sort_by(|a, b| {
        a.remaining
            .partial_cmp(&b.remaining)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| a.key.cmp(&b.key))
    });
    rows
}

pub fn clear_all(repo: &StateRepository) {
    let mut state = repo.load();
    let count = state.chests.len();
    state.chests.clear();
    if count > 0 {
        repo.add_event(&mut state, &format!("Queue cleared ({} chests)", count));
    }
    let _ = repo.save(&state);
}

pub fn box_summary(repo: &StateRepository) -> HashMap<String, usize> {
    let state = repo.load();
    let mut summary: HashMap<String, usize> = HashMap::new();
    for chest in state.chests.values() {
        if chest.is_get {
            continue;
        }
        let rarity = chest
            .raw
            .get("rarity")
            .and_then(|v| v.as_str())
            .unwrap_or("UNKNOWN")
            .to_string();
        *summary.entry(rarity).or_insert(0) += 1;
    }
    summary
}
#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn temp_repo() -> (StateRepository, tempfile::TempDir) {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("state.json");
        (StateRepository::new(path), dir)
    }

    fn make_chest_map(item_key: &str, item_id: Option<i64>) -> DynamicMap {
        let mut m = DynamicMap::new();
        m.insert("itemKey".to_string(), json!(item_key));
        if let Some(id) = item_id {
            m.insert("itemId".to_string(), json!(id));
        }
        m
    }

    #[test]
    fn box_label_known_common() {
        assert_eq!(box_label(Some(910651)), "Common Treasure Chest");
    }

    #[test]
    fn box_label_known_stage() {
        assert_eq!(box_label(Some(920651)), "Stage Treasure Chest");
    }

    #[test]
    fn box_label_910_prefix() {
        assert_eq!(box_label(Some(910999)), "Common Treasure Chest (910999)");
    }

    #[test]
    fn box_label_920_prefix() {
        assert_eq!(box_label(Some(920999)), "Stage Treasure Chest (920999)");
    }

    #[test]
    fn box_label_unknown_id() {
        assert_eq!(box_label(Some(12345)), "Box 12345");
    }

    #[test]
    fn box_label_none() {
        assert_eq!(box_label(None), "Unknown Chest");
    }

    #[test]
    fn normalize_chest_valid() {
        let mut m = DynamicMap::new();
        m.insert("itemKey".to_string(), json!("key1"));
        m.insert("itemId".to_string(), json!(910651));
        m.insert("rewardItemId".to_string(), json!(1001));
        m.insert("claimableAt".to_string(), json!("2025-01-15T10:00:00Z"));
        m.insert("isGet".to_string(), json!(false));

        let record = normalize_chest(&m, "test").unwrap();
        assert_eq!(record.item_key, "key1");
        assert_eq!(record.item_id, Some(910651));
        assert_eq!(record.reward_item_id, Some(1001));
        assert_eq!(
            record.claimable_at,
            Some("2025-01-15T10:00:00Z".to_string())
        );
        assert!(!record.is_get);
        assert_eq!(record.source, "test");
    }

    #[test]
    fn normalize_chest_fallback_in_date() {
        let mut m = DynamicMap::new();
        m.insert("inDate".to_string(), json!("fallback_key"));
        let record = normalize_chest(&m, "test").unwrap();
        assert_eq!(record.item_key, "fallback_key");
    }

    #[test]
    fn normalize_chest_fallback_uuid() {
        let mut m = DynamicMap::new();
        m.insert("uuid".to_string(), json!("uuid_key"));
        let record = normalize_chest(&m, "test").unwrap();
        assert_eq!(record.item_key, "uuid_key");
    }

    #[test]
    fn normalize_chest_empty_item_key_returns_none() {
        let m = DynamicMap::new();
        assert!(normalize_chest(&m, "test").is_none());
    }

    #[test]
    fn normalize_chest_empty_string_item_key_returns_none() {
        let mut m = DynamicMap::new();
        m.insert("itemKey".to_string(), json!(""));
        assert!(normalize_chest(&m, "test").is_none());
    }

    #[test]
    fn normalize_chest_snake_case_fields() {
        let mut m = DynamicMap::new();
        m.insert("item_key".to_string(), json!("sk_key"));
        m.insert("item_id".to_string(), json!(5555));
        m.insert("reward_item_id".to_string(), json!(6666));
        m.insert("claimable_at".to_string(), json!("2025-01-15T10:00:00Z"));

        let record = normalize_chest(&m, "test").unwrap();
        assert_eq!(record.item_key, "sk_key");
        assert_eq!(record.item_id, Some(5555));
        assert_eq!(record.reward_item_id, Some(6666));
    }

    #[test]
    fn normalize_chest_claim_time_fallback() {
        let mut m = DynamicMap::new();
        m.insert("itemKey".to_string(), json!("ct_key"));
        m.insert("claimTime".to_string(), json!("2025-01-15T10:00:00Z"));
        let record = normalize_chest(&m, "test").unwrap();
        assert_eq!(
            record.claimable_at,
            Some("2025-01-15T10:00:00Z".to_string())
        );
    }

    #[test]
    fn sync_chests_replaces_all() {
        let (repo, _dir) = temp_repo();
        let chests = vec![
            make_chest_map("k1", Some(910651)),
            make_chest_map("k2", Some(920651)),
        ];
        let (count, old) = sync_chests(&chests, "test", &repo);
        assert_eq!(count, 2);
        assert_eq!(old, 0);
        let state = repo.load();
        assert_eq!(state.chests.len(), 2);
        assert!(state.chests.contains_key("k1"));
        assert!(state.chests.contains_key("k2"));
    }

    #[test]
    fn sync_chests_replaces_existing() {
        let (repo, _dir) = temp_repo();
        let chests1 = vec![make_chest_map("k1", Some(910651))];
        sync_chests(&chests1, "test", &repo);

        let chests2 = vec![make_chest_map("k2", Some(920651))];
        let (count, old) = sync_chests(&chests2, "test", &repo);
        assert_eq!(count, 1);
        assert_eq!(old, 1);

        let state = repo.load();
        assert_eq!(state.chests.len(), 1);
        assert!(state.chests.contains_key("k2"));
        assert!(!state.chests.contains_key("k1"));
    }

    #[test]
    fn sync_chests_counts_invalid() {
        let (repo, _dir) = temp_repo();
        let mut m = DynamicMap::new();
        m.insert("itemKey".to_string(), json!(""));
        let chests = vec![m];
        let (count, _) = sync_chests(&chests, "test", &repo);
        assert_eq!(count, 0);
    }

    #[test]
    fn upsert_chests_adds_new() {
        let (repo, _dir) = temp_repo();
        let chests = vec![make_chest_map("k1", Some(910651))];
        let (added, updated) = upsert_chests(&chests, "test", &repo);
        assert_eq!(added, 1);
        assert_eq!(updated, 0);
        let state = repo.load();
        assert_eq!(state.chests.len(), 1);
    }

    #[test]
    fn upsert_chests_updates_existing() {
        let (repo, _dir) = temp_repo();
        let chests1 = vec![make_chest_map("k1", Some(910651))];
        upsert_chests(&chests1, "test", &repo);

        let mut m = make_chest_map("k1", Some(920651));
        m.insert("rewardItemId".to_string(), json!(7777));
        let chests2 = vec![m];
        let (added, updated) = upsert_chests(&chests2, "test", &repo);
        assert_eq!(added, 0);
        assert_eq!(updated, 1);

        let state = repo.load();
        assert_eq!(state.chests.len(), 1);
        assert_eq!(state.chests["k1"].item_id, Some(920651));
        assert_eq!(state.chests["k1"].reward_item_id, Some(7777));
    }

    #[test]
    fn upsert_chests_preserves_claimed_status() {
        let (repo, _dir) = temp_repo();

        let mut m1 = make_chest_map("k1", Some(910651));
        m1.insert("isGet".to_string(), json!(true));
        upsert_chests(&[m1], "test", &repo);

        let mut state = repo.load();
        state.chests.get_mut("k1").unwrap().claimed_at = Some("2025-01-15T10:00:00Z".to_string());
        state.chests.get_mut("k1").unwrap().claim_source = Some("old_source".to_string());
        repo.save(&state).unwrap();

        let m2 = make_chest_map("k1", Some(920651));
        upsert_chests(&[m2], "test", &repo);

        let state = repo.load();
        let chest = &state.chests["k1"];
        assert!(chest.is_get);
        assert_eq!(chest.claimed_at, Some("2025-01-15T10:00:00Z".to_string()));
        assert_eq!(chest.claim_source, Some("old_source".to_string()));
        assert_eq!(chest.item_id, Some(920651));
    }

    #[test]
    fn mark_claimed_by_keys_marks() {
        let (repo, _dir) = temp_repo();
        let chests = vec![make_chest_map("k1", Some(910651))];
        sync_chests(&chests, "test", &repo);

        let keys = vec!["k1".to_string()];
        let changed = mark_claimed_by_keys(&keys, "manual", &repo);
        assert_eq!(changed, 1);

        let state = repo.load();
        let chest = &state.chests["k1"];
        assert!(chest.is_get);
        assert!(chest.claimed_at.is_some());
        assert_eq!(chest.claim_source, Some("manual".to_string()));
    }

    #[test]
    fn mark_claimed_by_keys_skips_already_claimed() {
        let (repo, _dir) = temp_repo();
        let mut m = make_chest_map("k1", Some(910651));
        m.insert("isGet".to_string(), json!(true));
        sync_chests(&[m], "test", &repo);

        let keys = vec!["k1".to_string()];
        let changed = mark_claimed_by_keys(&keys, "manual", &repo);
        assert_eq!(changed, 0);
    }

    #[test]
    fn mark_claimed_by_keys_skips_empty_keys() {
        let (repo, _dir) = temp_repo();
        let chests = vec![make_chest_map("k1", Some(910651))];
        sync_chests(&chests, "test", &repo);

        let keys = vec!["".to_string(), "k1".to_string()];
        let changed = mark_claimed_by_keys(&keys, "manual", &repo);
        assert_eq!(changed, 1);
    }

    #[test]
    fn mark_claimed_by_keys_skips_nonexistent() {
        let (repo, _dir) = temp_repo();
        let chests = vec![make_chest_map("k1", Some(910651))];
        sync_chests(&chests, "test", &repo);

        let keys = vec!["nonexistent".to_string()];
        let changed = mark_claimed_by_keys(&keys, "manual", &repo);
        assert_eq!(changed, 0);
    }

    #[test]
    fn get_rows_filters_unclaimed() {
        let (repo, _dir) = temp_repo();
        let mut m1 = make_chest_map("k1", Some(910651));
        m1.insert("isGet".to_string(), json!(true));
        let m2 = make_chest_map("k2", Some(920651));
        sync_chests(&[m1, m2], "test", &repo);

        let cat = StaticCatalog::new(None);
        let rows = get_rows(&cat, false, &repo);
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].key.as_deref(), Some("k2"));
    }

    #[test]
    fn get_rows_includes_claimed_when_flagged() {
        let (repo, _dir) = temp_repo();
        let mut m1 = make_chest_map("k1", Some(910651));
        m1.insert("isGet".to_string(), json!(true));
        let m2 = make_chest_map("k2", Some(920651));
        sync_chests(&[m1, m2], "test", &repo);

        let cat = StaticCatalog::new(None);
        let rows = get_rows(&cat, true, &repo);
        assert_eq!(rows.len(), 2);
    }

    #[test]
    fn get_rows_sorts_by_remaining() {
        let (repo, _dir) = temp_repo();
        let m1 = make_chest_map("k1", Some(910651));
        let m2 = make_chest_map("k2", Some(920651));
        sync_chests(&[m1, m2], "test", &repo);

        let cat = StaticCatalog::new(None);
        let rows = get_rows(&cat, false, &repo);
        assert_eq!(rows.len(), 2);
        for i in 1..rows.len() {
            assert!(rows[i - 1].remaining <= rows[i].remaining);
        }
    }

    #[test]
    fn box_summary_counts_by_rarity() {
        let (repo, _dir) = temp_repo();
        let mut m1 = make_chest_map("k1", Some(910651));
        m1.insert("rarity".to_string(), json!("COMMON"));
        let mut m2 = make_chest_map("k2", Some(920651));
        m2.insert("rarity".to_string(), json!("RARE"));
        let mut m3 = make_chest_map("k3", Some(910651));
        m3.insert("rarity".to_string(), json!("COMMON"));
        sync_chests(&[m1, m2, m3], "test", &repo);

        let summary = box_summary(&repo);
        assert_eq!(summary["COMMON"], 2);
        assert_eq!(summary["RARE"], 1);
    }

    #[test]
    fn box_summary_excludes_claimed() {
        let (repo, _dir) = temp_repo();
        let mut m1 = make_chest_map("k1", Some(910651));
        m1.insert("rarity".to_string(), json!("COMMON"));
        m1.insert("isGet".to_string(), json!(true));
        let mut m2 = make_chest_map("k2", Some(920651));
        m2.insert("rarity".to_string(), json!("RARE"));
        sync_chests(&[m1, m2], "test", &repo);

        let summary = box_summary(&repo);
        assert_eq!(summary.get("COMMON"), None);
        assert_eq!(summary["RARE"], 1);
    }
}
