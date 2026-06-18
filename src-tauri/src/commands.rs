use std::collections::HashMap;
use std::sync::Mutex;

use tauri::State;

use crate::catalog::StaticCatalog;
use crate::chests;
use crate::config;
use crate::models::*;
use crate::state::StateRepository;

/// Shared app state managed by Tauri.
pub struct ManagedState {
    repo: StateRepository,
    pub catalog: Mutex<StaticCatalog>,
}

impl ManagedState {
    pub fn new() -> Self {
        Self {
            repo: StateRepository::new(config::state_path()),
            catalog: Mutex::new(StaticCatalog::new(None)),
        }
    }

    pub fn repo(&self) -> &StateRepository {
        &self.repo
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
    *catalog = StaticCatalog::new(None);
    catalog.valid
}
