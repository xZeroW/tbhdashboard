use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Flexible JSON map (equivalent to Pydantic `extra="allow"`)
pub type DynamicMap = HashMap<String, serde_json::Value>;

#[derive(Serialize, Deserialize, Clone, Default)]
pub struct StaticItem {
    pub id: Option<i64>,
    #[serde(default)]
    pub kind: String,
    #[serde(default)]
    pub rarity: String,
    #[serde(default)]
    pub slot: String,
    #[serde(default)]
    pub subtype: String,
    #[serde(default)]
    pub name: String,
    pub drop_table: Option<i64>,
}

#[derive(Serialize, Deserialize, Clone, Default)]
pub struct StageInfo {
    pub id: i64,
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub difficulty: String,
    #[serde(default = "default_minus_one")]
    pub level: i32,
    #[serde(default)]
    pub boxes: Vec<(i64, i32)>,
}

fn default_minus_one() -> i32 {
    -1
}

#[derive(Serialize, Deserialize, Clone, Default)]
pub struct ChestRecord {
    #[serde(rename = "itemKey")]
    pub item_key: String,
    #[serde(rename = "itemId", skip_serializing_if = "Option::is_none")]
    pub item_id: Option<i64>,
    #[serde(rename = "rewardItemId", skip_serializing_if = "Option::is_none")]
    pub reward_item_id: Option<i64>,
    #[serde(rename = "rewardItemKey", default)]
    pub reward_item_key: String,
    #[serde(rename = "claimableAt", skip_serializing_if = "Option::is_none")]
    pub claimable_at: Option<String>,
    #[serde(rename = "isGet", default)]
    pub is_get: bool,
    #[serde(default)]
    pub source: String,
    #[serde(rename = "seenAt")]
    pub seen_at: String,
    #[serde(default)]
    pub raw: DynamicMap,
    #[serde(rename = "claimedAt", skip_serializing_if = "Option::is_none")]
    pub claimed_at: Option<String>,
    #[serde(rename = "claimSource", skip_serializing_if = "Option::is_none")]
    pub claim_source: Option<String>,
}

#[derive(Serialize, Deserialize, Clone, Default)]
pub struct ChestRow {
    pub remaining: f64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub claim: Option<DateTime<Utc>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub key: Option<String>,
    pub box_label: String,
    #[serde(rename = "rewardId", skip_serializing_if = "Option::is_none")]
    pub reward_id: Option<i64>,
    pub rarity: String,
    pub name: String,
    #[serde(rename = "isGet", default)]
    pub is_get: bool,
}

#[derive(Serialize, Deserialize, Clone, Default)]
pub struct AddedItem {
    pub at: String,
    #[serde(rename = "itemId", skip_serializing_if = "Option::is_none")]
    pub item_id: Option<i64>,
    #[serde(rename = "itemKey", default)]
    pub item_key: String,
    #[serde(default = "default_one")]
    pub count: i32,
    #[serde(default = "default_rarity")]
    pub rarity: String,
    #[serde(default = "default_question_mark")]
    pub name: String,
    #[serde(default)]
    pub raw: DynamicMap,
}

fn default_one() -> i32 {
    1
}

fn default_rarity() -> String {
    "UNKNOWN".to_string()
}

fn default_question_mark() -> String {
    "?".to_string()
}

#[derive(Serialize, Deserialize, Clone, Default)]
pub struct ProcessBoxCreatedItem {
    #[serde(rename = "itemId", skip_serializing_if = "Option::is_none")]
    pub item_id: Option<i64>,
    #[serde(default)]
    pub count: i32,
    #[serde(rename = "dropKey", skip_serializing_if = "Option::is_none")]
    pub drop_key: Option<i64>,
    #[serde(default)]
    pub name: String,
}

#[derive(Serialize, Deserialize, Clone, Default)]
pub struct ProcessBoxInfo {
    pub tn: Option<serde_json::Value>,
    #[serde(rename = "isReset", default)]
    pub is_reset: bool,
    #[serde(default)]
    pub created: Vec<ProcessBoxCreatedItem>,
    pub at: String,
}

#[derive(Serialize, Deserialize, Clone, Default)]
pub struct StateEvent {
    pub at: String,
    pub text: String,
}

/// Top-level application state, persisted as JSON.
#[derive(Serialize, Deserialize, Clone, Default)]
pub struct AppState {
    #[serde(default)]
    pub chests: HashMap<String, ChestRecord>,
    #[serde(default)]
    pub events: Vec<StateEvent>,
    #[serde(rename = "last_processbox", skip_serializing_if = "Option::is_none")]
    pub last_processbox: Option<ProcessBoxInfo>,
    #[serde(rename = "last_added", skip_serializing_if = "Option::is_none")]
    pub last_added: Option<AddedItemsSnapshot>,
    #[serde(rename = "last_snapshot", skip_serializing_if = "Option::is_none")]
    pub last_snapshot: Option<SnapshotInfo>,
}

#[derive(Serialize, Deserialize, Clone, Default)]
pub struct AddedItemsSnapshot {
    pub at: String,
    pub source: String,
    pub items: Vec<AddedItem>,
}

#[derive(Serialize, Deserialize, Clone, Default)]
pub struct SnapshotInfo {
    pub at: String,
    pub source: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub processbox: Option<ProcessBoxInfo>,
    pub count: usize,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn roundtrip<T: Serialize + for<'de> Deserialize<'de> + Clone + Default>(val: &T) -> T {
        let json = serde_json::to_string(val).unwrap();
        serde_json::from_str(&json).unwrap()
    }

    #[test]
    fn static_item_roundtrip() {
        let item = StaticItem {
            id: Some(1),
            kind: "weapon".into(),
            rarity: "RARE".into(),
            ..Default::default()
        };
        let back = roundtrip(&item);
        assert_eq!(back.id, Some(1));
        assert_eq!(back.rarity, "RARE");
    }

    #[test]
    fn stage_info_roundtrip() {
        let stage = StageInfo {
            id: 100,
            name: "Stage 1".into(),
            level: 5,
            ..Default::default()
        };
        let back = roundtrip(&stage);
        assert_eq!(back.id, 100);
        assert_eq!(back.level, 5);
    }

    #[test]
    fn stage_info_default_level() {
        let stage: StageInfo = serde_json::from_str(r#"{"id":1}"#).unwrap();
        assert_eq!(stage.level, -1);
    }

    #[test]
    fn chest_record_roundtrip() {
        let record = ChestRecord {
            item_key: "key".into(),
            item_id: Some(42),
            seen_at: "2025-01-01T00:00:00Z".into(),
            ..Default::default()
        };
        let back = roundtrip(&record);
        assert_eq!(back.item_key, "key");
        assert_eq!(back.item_id, Some(42));
    }

    #[test]
    fn chest_row_roundtrip() {
        let row = ChestRow {
            remaining: 3600.0,
            claim: Some(Utc::now()),
            box_label: "Common".into(),
            rarity: "RARE".into(),
            name: "Sword".into(),
            ..Default::default()
        };
        let back = roundtrip(&row);
        assert_eq!(back.box_label, "Common");
        assert!(back.claim.is_some());
    }

    #[test]
    fn added_item_defaults() {
        let item: AddedItem = serde_json::from_str(r#"{"at":"2025-01-01T00:00:00Z"}"#).unwrap();
        assert_eq!(item.count, 1);
        assert_eq!(item.rarity, "UNKNOWN");
        assert_eq!(item.name, "?");
    }

    #[test]
    fn added_item_roundtrip() {
        let item = AddedItem {
            at: "2025-01-01T00:00:00Z".into(),
            item_key: "item1".into(),
            count: 5,
            rarity: "LEGENDARY".into(),
            name: "Epic".into(),
            ..Default::default()
        };
        let back = roundtrip(&item);
        assert_eq!(back.count, 5);
        assert_eq!(back.rarity, "LEGENDARY");
    }

    #[test]
    fn process_box_info_roundtrip() {
        let info = ProcessBoxInfo {
            is_reset: true,
            at: "2025-01-01T00:00:00Z".into(),
            ..Default::default()
        };
        let back = roundtrip(&info);
        assert!(back.is_reset);
    }

    #[test]
    fn app_state_roundtrip() {
        let mut state = AppState::default();
        state.chests.insert(
            "test".into(),
            ChestRecord {
                item_key: "k".into(),
                seen_at: "2025-01-01T00:00:00Z".into(),
                ..Default::default()
            },
        );
        state.events.push(StateEvent {
            at: "2025-01-01T00:00:00Z".into(),
            text: "hello".into(),
        });
        let back = roundtrip(&state);
        assert_eq!(back.chests.len(), 1);
        assert_eq!(back.events.len(), 1);
    }

    #[test]
    fn app_state_default_is_empty() {
        let state = AppState::default();
        assert!(state.chests.is_empty());
        assert!(state.events.is_empty());
        assert!(state.last_processbox.is_none());
        assert!(state.last_added.is_none());
        assert!(state.last_snapshot.is_none());
    }

    #[test]
    fn snapshot_info_roundtrip() {
        let snap = SnapshotInfo {
            at: "2025-01-01T00:00:00Z".into(),
            source: "test".into(),
            count: 42,
            ..Default::default()
        };
        let back = roundtrip(&snap);
        assert_eq!(back.count, 42);
    }
}
