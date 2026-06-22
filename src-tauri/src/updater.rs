use tauri::AppHandle;
use tauri_plugin_updater::UpdaterExt;

const UPDATER_TOKEN: Option<&str> = option_env!("TAURI_UPDATER_TOKEN");

pub fn update_on_startup(app: AppHandle) {
    tauri::async_runtime::spawn(async move {
        if let Err(err) = update(app).await {
            eprintln!("startup update check failed: {err}");
        }
    });
}

async fn update(app: AppHandle) -> tauri_plugin_updater::Result<()> {
    let mut builder = app.updater_builder();
    if let Some(token) = UPDATER_TOKEN
        && !token.is_empty()
    {
        builder = builder.header("Authorization", &format!("Bearer {token}"))?;
    }

    if let Some(update) = builder.build()?.check().await? {
        println!(
            "found update {} for current version {}",
            update.version, update.current_version
        );

        let mut downloaded = 0;
        update
            .download_and_install(
                |chunk_length, content_length| {
                    downloaded += chunk_length;
                    println!("update download progress: {downloaded} of {content_length:?}");
                },
                || {
                    println!("update download finished");
                },
            )
            .await?;
        println!("update installed, restarting");
        app.restart();
    }

    Ok(())
}
