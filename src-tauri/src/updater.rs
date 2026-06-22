use serde::Serialize;
use tauri::AppHandle;
use tauri_plugin_updater::UpdaterExt;

const UPDATER_TOKEN: Option<&str> = option_env!("TAURI_UPDATER_TOKEN");

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct UpdateInfo {
    pub version: String,
    pub current_version: String,
    pub body: Option<String>,
    pub date: Option<String>,
}

#[tauri::command]
pub async fn check_update(app: AppHandle) -> Result<Option<UpdateInfo>, String> {
    let mut builder = app.updater_builder();
    if let Some(token) = UPDATER_TOKEN {
        if !token.is_empty() {
            builder = builder
                .header("Authorization", &format!("Bearer {token}"))
                .map_err(|e| e.to_string())?;
        }
    }
    let update = builder
        .build()
        .map_err(|e| e.to_string())?
        .check()
        .await
        .map_err(|e| e.to_string())?;
    Ok(update.map(|u| UpdateInfo {
        version: u.version,
        current_version: u.current_version,
        body: u.body,
        date: u.date.map(|d| d.to_string()),
    }))
}

#[tauri::command]
pub async fn install_update(app: AppHandle) -> Result<(), String> {
    let mut builder = app.updater_builder();
    if let Some(token) = UPDATER_TOKEN {
        if !token.is_empty() {
            builder = builder
                .header("Authorization", &format!("Bearer {token}"))
                .map_err(|e| e.to_string())?;
        }
    }
    let update = builder
        .build()
        .map_err(|e| e.to_string())?
        .check()
        .await
        .map_err(|e| e.to_string())?;
    if let Some(update) = update {
        update
            .download_and_install(
                |chunk_length, _content_length| {
                    println!("update download progress: {chunk_length}");
                },
                || {
                    println!("update download finished");
                },
            )
            .await
            .map_err(|e| e.to_string())?;
        println!("update installed, restarting");
        app.restart();
    }
    Ok(())
}
