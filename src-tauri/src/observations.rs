use std::collections::HashSet;

use anyhow::{Context, Result, anyhow};
use chrono::{SecondsFormat, Utc};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::{catalog::StaticCatalog, models::AppState, state::StateRepository, utils::parse_dt};

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct ObservationUploadResult {
    pub ok: bool,
    pub uploaded: usize,
    pub skipped: usize,
    pub message: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ClaimableRewardObservationRequest {
    steam_id: String,
    catalog_version: Option<String>,
    box_item_id: i64,
    reward_item_id: i64,
    reward_rarity: Option<String>,
    claimable_at: String,
    observed_at: String,
    item_key_hash: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct ClaimableRewardObservationResponse {
    accepted: bool,
    duplicate: bool,
}

pub fn upload_claimable_rewards(
    repo: &StateRepository,
    catalog: &StaticCatalog,
) -> ObservationUploadResult {
    match upload_claimable_rewards_inner(repo, catalog) {
        Ok(result) => result,
        Err(err) => ObservationUploadResult {
            ok: false,
            uploaded: 0,
            skipped: 0,
            message: err.to_string(),
        },
    }
}

fn upload_claimable_rewards_inner(
    repo: &StateRepository,
    catalog: &StaticCatalog,
) -> Result<ObservationUploadResult> {
    let mut state = repo.load();
    let settings = state.settings.clone();
    if !settings.share_claimable_rewards {
        return Ok(ObservationUploadResult {
            ok: true,
            uploaded: 0,
            skipped: 0,
            message: "Claimable reward sharing is disabled".to_string(),
        });
    }

    let server_url = settings.server_url.trim().trim_end_matches('/').to_string();
    let auth_token = settings.auth_token.trim().to_string();
    let steam_id = settings.steam_id.trim().to_string();
    if server_url.is_empty() || auth_token.is_empty() || steam_id.is_empty() {
        return Err(anyhow!(
            "server URL, auth token, and Steam ID are required before uploading observations"
        ));
    }

    let mut uploaded_keys = state
        .uploaded_claimable_observation_keys
        .iter()
        .cloned()
        .collect::<HashSet<_>>();
    let observations = build_observations(&state, catalog, &steam_id, &uploaded_keys);

    if observations.is_empty() {
        return Ok(ObservationUploadResult {
            ok: true,
            uploaded: 0,
            skipped: 0,
            message: "No new claimable reward observations".to_string(),
        });
    }

    let client = reqwest::blocking::Client::new();
    let mut uploaded = 0usize;
    let mut skipped = 0usize;

    for observation in observations {
        let key = observation.item_key_hash.clone();
        let response = client
            .post(format!("{server_url}/observations/claimable-reward"))
            .bearer_auth(&auth_token)
            .json(&observation)
            .send()
            .context("failed to upload claimable reward observation")?
            .error_for_status()
            .context("claimable reward observation upload failed")?
            .json::<ClaimableRewardObservationResponse>()
            .context("failed to parse observation upload response")?;

        if response.accepted || response.duplicate {
            uploaded_keys.insert(key);
            if response.accepted {
                uploaded += 1;
            } else {
                skipped += 1;
            }
        }
    }

    state.uploaded_claimable_observation_keys = uploaded_keys.into_iter().collect();
    state.uploaded_claimable_observation_keys.sort();
    let excess = state
        .uploaded_claimable_observation_keys
        .len()
        .saturating_sub(5000);
    state.uploaded_claimable_observation_keys.drain(..excess);
    if uploaded > 0 {
        repo.add_event(
            &mut state,
            &format!("Uploaded {uploaded} claimable reward observations"),
        );
    }
    repo.save(&state)?;

    Ok(ObservationUploadResult {
        ok: true,
        uploaded,
        skipped,
        message: format!("Uploaded {uploaded}, skipped {skipped} duplicates"),
    })
}

fn build_observations(
    state: &AppState,
    catalog: &StaticCatalog,
    steam_id: &str,
    uploaded_keys: &HashSet<String>,
) -> Vec<ClaimableRewardObservationRequest> {
    let now = Utc::now();
    let observed_at = now.to_rfc3339_opts(SecondsFormat::Secs, true);

    state
        .chests
        .values()
        .filter(|chest| !chest.is_get)
        .filter_map(|chest| {
            let claimable_at = chest.claimable_at.as_deref()?;
            let claim_dt = parse_dt(claimable_at)?;
            if claim_dt > now {
                return None;
            }

            let box_item_id = chest.item_id?;
            let reward_item_id = chest.reward_item_id?;
            let key = observation_key(steam_id, &chest.item_key);
            if uploaded_keys.contains(&key) {
                return None;
            }

            let (rarity, _) = catalog.item_parts(Some(reward_item_id));
            Some(ClaimableRewardObservationRequest {
                steam_id: steam_id.to_string(),
                catalog_version: state.assets_version.clone(),
                box_item_id,
                reward_item_id,
                reward_rarity: Some(rarity),
                claimable_at: claim_dt.to_rfc3339_opts(SecondsFormat::Secs, true),
                observed_at: observed_at.clone(),
                item_key_hash: key,
            })
        })
        .collect()
}

fn observation_key(steam_id: &str, item_key: &str) -> String {
    hex::encode(Sha256::digest(format!("{steam_id}:{item_key}").as_bytes()))
}
