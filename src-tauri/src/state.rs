use std::fs;
use std::path::PathBuf;
use anyhow::{Context, Result};
use crate::models::{AppState, StateEvent};
use crate::utils::utc_now_iso;

pub struct StateRepository {
    pub path: PathBuf,
}

impl StateRepository {
    pub fn new(path: PathBuf) -> Self {
        Self { path }
    }

    pub fn load(&self) -> AppState {
        if !self.path.exists() {
            return AppState::default();
        }
        let data = fs::read_to_string(&self.path).unwrap_or_default();
        serde_json::from_str(&data).unwrap_or_default()
    }

    pub fn save(&self, state: &AppState) -> Result<()> {
        if let Some(parent) = self.path.parent() {
            fs::create_dir_all(parent)
                .context("failed to create state directory")?;
        }
        let tmp = self.path.with_extension("tmp");
        let data = serde_json::to_string_pretty(state)
            .context("failed to serialize state")?;
        fs::write(&tmp, data)
            .context("failed to write temp state file")?;
        fs::rename(&tmp, &self.path)
            .context("failed to rename temp state file")?;
        Ok(())
    }

    pub fn add_event(&self, state: &mut AppState, text: &str) {
        state.events.push(StateEvent {
            at: utc_now_iso(),
            text: text.to_string(),
        });
        let keep = state.events.len().saturating_sub(80);
        state.events.drain(..keep);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_repo() -> (StateRepository, tempfile::TempDir) {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("state.json");
        (StateRepository::new(path), dir)
    }

    #[test]
    fn load_nonexistent_returns_default() {
        let (repo, _dir) = temp_repo();
        let state = repo.load();
        assert!(state.chests.is_empty());
        assert!(state.events.is_empty());
    }

    #[test]
    fn save_and_load_roundtrip() {
        let (repo, _dir) = temp_repo();
        let mut state = AppState::default();
        state.chests.insert(
            "key1".to_string(),
            crate::models::ChestRecord {
                item_key: "item_a".to_string(),
                seen_at: "2025-01-01T00:00:00Z".to_string(),
                ..Default::default()
            },
        );
        repo.save(&state).unwrap();
        let loaded = repo.load();
        assert_eq!(loaded.chests.len(), 1);
        assert!(loaded.chests.contains_key("key1"));
    }

    #[test]
    fn add_event_roundtrip() {
        let (repo, _dir) = temp_repo();
        let mut state = AppState::default();
        repo.add_event(&mut state, "test event");
        assert_eq!(state.events.len(), 1);
        assert_eq!(state.events[0].text, "test event");
        repo.save(&state).unwrap();
        let loaded = repo.load();
        assert_eq!(loaded.events.len(), 1);
        assert_eq!(loaded.events[0].text, "test event");
    }

    #[test]
    fn add_event_trims_to_80() {
        let (repo, _dir) = temp_repo();
        let mut state = AppState::default();
        for i in 0..100 {
            repo.add_event(&mut state, &format!("event {i}"));
        }
        assert_eq!(state.events.len(), 80);
        assert_eq!(state.events[0].text, "event 20");
    }
}
