use std::{
    fs,
    io::Cursor,
    path::{Path, PathBuf},
};

use anyhow::{Context, Result, anyhow};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use zip::ZipArchive;

use crate::{config, models::AppState, state::StateRepository};

const ASSET_USER_AGENT: &str = "curl/8.5.0";

#[derive(Serialize, Deserialize, Clone, Default)]
#[serde(rename_all = "camelCase")]
pub struct AssetManifest {
    pub version: String,
    pub url: String,
    pub sha256: Option<String>,
    pub size_bytes: Option<u64>,
    pub notes: Option<String>,
}

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct AssetUpdateStatus {
    pub ok: bool,
    pub message: String,
    pub current_version: Option<String>,
    pub latest_version: Option<String>,
    pub update_available: bool,
    pub manifest: Option<AssetManifest>,
}

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct AssetDownloadResult {
    pub ok: bool,
    pub message: String,
    pub version: Option<String>,
    pub assets_path: Option<String>,
}

pub async fn fetch_manifest(manifest_url: &str) -> Result<AssetManifest> {
    let manifest = asset_http_client()
        .get(manifest_url)
        .send()
        .await
        .with_context(|| format!("failed to fetch asset manifest from {manifest_url}"))?
        .error_for_status()
        .context("asset manifest request failed")?
        .json::<AssetManifest>()
        .await
        .context("failed to parse asset manifest")?;

    if manifest.version.trim().is_empty() || manifest.url.trim().is_empty() {
        return Err(anyhow!("asset manifest is missing version or url"));
    }

    Ok(manifest)
}

pub async fn check_update(repo: &StateRepository) -> AssetUpdateStatus {
    let state = repo.load();
    let manifest_url = state.settings.asset_manifest_url.clone();
    match fetch_manifest(&manifest_url).await {
        Ok(manifest) => {
            let current_version = if assets_root_is_valid(&config::assets_root()) {
                state.assets_version.clone()
            } else {
                None
            };
            let update_available = current_version.as_deref() != Some(manifest.version.as_str());
            AssetUpdateStatus {
                ok: true,
                message: if update_available {
                    "Asset update available".to_string()
                } else {
                    "Assets are up to date".to_string()
                },
                current_version,
                latest_version: Some(manifest.version.clone()),
                update_available,
                manifest: Some(manifest),
            }
        }
        Err(err) => AssetUpdateStatus {
            ok: false,
            message: err.to_string(),
            current_version: state.assets_version,
            latest_version: None,
            update_available: false,
            manifest: None,
        },
    }
}

fn assets_root_is_valid(root: &Path) -> bool {
    let text = root.join("TextAsset");
    text.join("ItemInfoData.txt").exists()
        && text.join("DropInfoData.txt").exists()
        && text.join("StageInfoData.txt").exists()
        && text.join("ItemGroupInfoData.txt").exists()
}

pub async fn download_latest(repo: &StateRepository) -> AssetDownloadResult {
    match download_latest_inner(repo).await {
        Ok((manifest, assets_path)) => AssetDownloadResult {
            ok: true,
            message: format!("Downloaded assets {}", manifest.version),
            version: Some(manifest.version),
            assets_path: Some(assets_path.to_string_lossy().into_owned()),
        },
        Err(err) => AssetDownloadResult {
            ok: false,
            message: err.to_string(),
            version: None,
            assets_path: None,
        },
    }
}

async fn download_latest_inner(repo: &StateRepository) -> Result<(AssetManifest, PathBuf)> {
    let state = repo.load();
    let manifest = fetch_manifest(&state.settings.asset_manifest_url).await?;

    let bytes = asset_http_client()
        .get(&manifest.url)
        .send()
        .await
        .with_context(|| format!("failed to download assets from {}", manifest.url))?
        .error_for_status()
        .context("asset download request failed")?
        .bytes()
        .await
        .context("failed to read asset zip")?;

    verify_zip(&bytes, &manifest)?;

    let staging_dir =
        config::downloaded_assets_base_dir().join(safe_version_dir(&manifest.version));
    if staging_dir.exists() {
        fs::remove_dir_all(&staging_dir).context("failed to replace staged assets")?;
    }
    fs::create_dir_all(&staging_dir).context("failed to create staged assets directory")?;
    extract_zip(&bytes, &staging_dir)?;

    let staged_assets_path = resolve_extracted_assets_root(&staging_dir)?;
    let assets_path = config::assets_root();
    replace_assets_root(&staged_assets_path, &assets_path)?;

    let mut state = repo.load();
    state.assets_path = None;
    state.assets_version = Some(manifest.version.clone());
    repo.add_event(
        &mut state,
        &format!("Assets downloaded: {}", manifest.version),
    );
    repo.save(&state)?;

    Ok((manifest, assets_path))
}

fn asset_http_client() -> reqwest::Client {
    reqwest::Client::builder()
        .user_agent(ASSET_USER_AGENT)
        .build()
        .unwrap_or_else(|_| reqwest::Client::new())
}

fn replace_assets_root(source: &Path, target: &Path) -> Result<()> {
    fs::create_dir_all(target).context("failed to create Assets directory")?;

    for entry in fs::read_dir(target).context("failed to read current Assets directory")? {
        let entry = entry.context("failed to read current Assets entry")?;
        let path = entry.path();
        let file_type = entry
            .file_type()
            .context("failed to read current Assets entry type")?;
        if file_type.is_dir() {
            fs::remove_dir_all(&path).context("failed to remove old Assets directory")?;
        } else {
            fs::remove_file(&path).context("failed to remove old Assets file")?;
        }
    }

    copy_dir_contents(source, target)
}

fn copy_dir_contents(source: &Path, target: &Path) -> Result<()> {
    fs::create_dir_all(target).context("failed to create target Assets directory")?;

    for entry in fs::read_dir(source).context("failed to read staged Assets directory")? {
        let entry = entry.context("failed to read staged Assets entry")?;
        let source_path = entry.path();
        let target_path = target.join(entry.file_name());
        let file_type = entry
            .file_type()
            .context("failed to read staged Assets entry type")?;

        if file_type.is_dir() {
            copy_dir_contents(&source_path, &target_path)?;
        } else if file_type.is_file() {
            fs::copy(&source_path, &target_path).context("failed to copy asset file")?;
        }
    }

    Ok(())
}

fn verify_zip(bytes: &[u8], manifest: &AssetManifest) -> Result<()> {
    if let Some(expected) = manifest.sha256.as_deref().map(str::trim)
        && !expected.is_empty()
    {
        let actual = hex::encode(Sha256::digest(bytes));
        if !actual.eq_ignore_ascii_case(expected) {
            return Err(anyhow!("asset zip checksum mismatch"));
        }
    }

    if let Some(expected_size) = manifest.size_bytes
        && bytes.len() as u64 != expected_size
    {
        return Err(anyhow!("asset zip size mismatch"));
    }

    Ok(())
}

fn extract_zip(bytes: &[u8], target_dir: &Path) -> Result<()> {
    let cursor = Cursor::new(bytes);
    let mut archive = ZipArchive::new(cursor).context("failed to open asset zip")?;

    for index in 0..archive.len() {
        let mut file = archive
            .by_index(index)
            .context("failed to read zip entry")?;
        let Some(enclosed_name) = file.enclosed_name() else {
            continue;
        };
        let out_path = target_dir.join(enclosed_name);

        if file.is_dir() {
            fs::create_dir_all(&out_path).context("failed to create asset directory")?;
            continue;
        }

        if let Some(parent) = out_path.parent() {
            fs::create_dir_all(parent).context("failed to create asset parent directory")?;
        }

        let mut out =
            fs::File::create(&out_path).context("failed to create extracted asset file")?;
        std::io::copy(&mut file, &mut out).context("failed to extract asset file")?;
    }

    Ok(())
}

fn resolve_extracted_assets_root(target_dir: &Path) -> Result<PathBuf> {
    let candidates = [target_dir.join("Assets"), target_dir.to_path_buf()];
    for candidate in candidates {
        let text = candidate.join("TextAsset");
        if text.join("ItemInfoData.txt").exists()
            && text.join("DropInfoData.txt").exists()
            && text.join("StageInfoData.txt").exists()
            && text.join("ItemGroupInfoData.txt").exists()
        {
            return Ok(candidate);
        }
    }

    Err(anyhow!(
        "asset zip must contain Assets/TextAsset with required data files"
    ))
}

fn safe_version_dir(version: &str) -> String {
    version
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() || ch == '.' || ch == '-' || ch == '_' {
                ch
            } else {
                '_'
            }
        })
        .collect()
}

#[allow(dead_code)]
fn _assert_state_default_compat(_: &AppState) {}
