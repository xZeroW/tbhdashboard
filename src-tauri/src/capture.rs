use crate::chests;
use crate::models::*;
use crate::state::StateRepository;
use crate::utils::{parse_jsonish_list, utc_now_iso};

/// A parsed event from the sidecar.
pub enum SidecarEvent {
    ChestsSynced {
        count: usize,
        old: usize,
        source: String,
        chests: Vec<DynamicMap>,
    },
    ChestsUpserted {
        added: usize,
        updated: usize,
        source: String,
        chests: Vec<DynamicMap>,
    },
    AddedItems {
        count: usize,
        source: String,
        items: Vec<AddedItem>,
    },
    ProcessBox {
        info: ProcessBoxInfo,
        description: String,
    },
    Claimed {
        count: usize,
        source: String,
        keys: Vec<String>,
    },
    RequestLogged(RequestLogEntry),
    ResponseLogged { source: String, body: String, body_bytes: usize },
    Unknown,
}

/// Parse a JSON line from the sidecar into a SidecarEvent.
pub fn parse_sidecar_line(line: &str) -> SidecarEvent {
    let v: serde_json::Value = match serde_json::from_str(line) {
        Ok(v) => v,
        Err(_) => return SidecarEvent::Unknown,
    };
    let typ = v.get("type").and_then(|t| t.as_str()).unwrap_or("");

    match typ {
        "chests_synced" => SidecarEvent::ChestsSynced {
            count: v.get("count").and_then(|c| c.as_u64()).unwrap_or(0) as usize,
            old: v.get("old").and_then(|c| c.as_u64()).unwrap_or(0) as usize,
            source: v
                .get("source")
                .and_then(|s| s.as_str())
                .unwrap_or("")
                .to_string(),
            chests: v
                .get("chests")
                .and_then(|c| c.as_array())
                .map(|arr| {
                    arr.iter()
                        .filter_map(|item| {
                            item.as_object().map(|m| m.clone().into_iter().collect())
                        })
                        .collect()
                })
                .unwrap_or_default(),
        },
        "chests_upserted" => SidecarEvent::ChestsUpserted {
            added: v.get("added").and_then(|c| c.as_u64()).unwrap_or(0) as usize,
            updated: v.get("updated").and_then(|c| c.as_u64()).unwrap_or(0) as usize,
            source: v
                .get("source")
                .and_then(|s| s.as_str())
                .unwrap_or("")
                .to_string(),
            chests: v
                .get("chests")
                .and_then(|c| c.as_array())
                .map(|arr| {
                    arr.iter()
                        .filter_map(|item| {
                            item.as_object().map(|m| m.clone().into_iter().collect())
                        })
                        .collect()
                })
                .unwrap_or_default(),
        },
        "added_items" => SidecarEvent::AddedItems {
            count: v.get("count").and_then(|c| c.as_u64()).unwrap_or(0) as usize,
            source: v
                .get("source")
                .and_then(|s| s.as_str())
                .unwrap_or("")
                .to_string(),
            items: v
                .get("items")
                .and_then(|c| c.as_array())
                .map(|arr| {
                    arr.iter()
                        .filter_map(|item| serde_json::from_value(item.clone()).ok())
                        .collect()
                })
                .unwrap_or_default(),
        },
        "process_box" => SidecarEvent::ProcessBox {
            info: v
                .get("info")
                .and_then(|i| serde_json::from_value(i.clone()).ok())
                .unwrap_or_default(),
            description: v
                .get("description")
                .and_then(|s| s.as_str())
                .unwrap_or("")
                .to_string(),
        },
        "claimed" => SidecarEvent::Claimed {
            count: v.get("count").and_then(|c| c.as_u64()).unwrap_or(0) as usize,
            source: v
                .get("source")
                .and_then(|s| s.as_str())
                .unwrap_or("sidecar")
                .to_string(),
            keys: v.get("keys").map(parse_jsonish_list).unwrap_or_default(),
        },
        "request_log" => serde_json::from_value(v)
            .map(SidecarEvent::RequestLogged)
            .unwrap_or(SidecarEvent::Unknown),
        "response_log" => SidecarEvent::ResponseLogged {
            source: v.get("source").and_then(|s| s.as_str()).unwrap_or("").to_string(),
            body: v.get("body").and_then(|s| s.as_str()).unwrap_or("").to_string(),
            body_bytes: v.get("body_bytes").and_then(|c| c.as_u64()).unwrap_or(0) as usize,
        },
        _ => SidecarEvent::Unknown,
    }
}

/// Apply a sidecar event to the persistent state.
pub fn apply_sidecar_event(event: SidecarEvent, repo: &StateRepository) {
    match event {
        SidecarEvent::ChestsSynced {
            count,
            old,
            source,
            chests,
        } => {
            chests::sync_chests(&chests, &source, repo);
            println!(
                "[TBH-Rust] synced {}, replaced {} -> {:?}",
                count, old, repo.path
            );
        }
        SidecarEvent::ChestsUpserted {
            added,
            updated,
            source,
            chests,
        } => {
            chests::upsert_chests(&chests, &source, repo);
            println!(
                "[TBH-Rust] chests +{}, updated {} -> {:?}",
                added, updated, repo.path
            );
        }
        SidecarEvent::AddedItems {
            count,
            source,
            items,
        } => {
            let mut state = repo.load();
            state.last_added = Some(AddedItemsSnapshot {
                at: utc_now_iso(),
                source: source.clone(),
                items,
            });
            repo.add_event(
                &mut state,
                &format!("{}: immediate added items ({})", source, count),
            );
            repo.save(&state).unwrap();
        }
        SidecarEvent::ProcessBox { info, description } => {
            let mut state = repo.load();
            state.last_processbox = Some(info.clone());
            repo.add_event(&mut state, &description);
            repo.save(&state).unwrap();
        }
        SidecarEvent::Claimed {
            count,
            source,
            keys,
        } => {
            if keys.is_empty() {
                if count > 0 {
                    let mut state = repo.load();
                    repo.add_event(&mut state, &format!("{}: claimed {} opened", source, count));
                    repo.save(&state).unwrap();
                }
            } else {
                let changed = chests::mark_claimed_by_keys(&keys, &source, repo);
                println!("[TBH-Rust] marked {} claimed from sidecar", changed);
            }
        }
        SidecarEvent::RequestLogged(entry) => {
            let mut state = repo.load();
            repo.add_request_log(&mut state, entry);
            repo.save(&state).unwrap();
        }
        SidecarEvent::ResponseLogged { source, body, body_bytes } => {
            let mut state = repo.load();
            if let Some(entry) = state.request_history.iter_mut().rev().find(|e| e.source == source) {
                entry.response_body = body;
                entry.response_body_bytes = body_bytes;
            }
            repo.save(&state).unwrap();
        }
        SidecarEvent::Unknown => {}
    }
}
