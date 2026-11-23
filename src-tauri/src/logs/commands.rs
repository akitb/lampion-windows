//! Tauri commands of the Logs page. Registered in lib.rs under "logs". Copying uses the clipboard
//! plugin and "Show in Explorer" the opener plugin straight from the page.

use super::{Source, Update};
use crate::config::locations;
use serde::Serialize;

async fn blocking<T: Send + 'static>(f: impl FnOnce() -> T + Send + 'static) -> Result<T, String> {
    tauri::async_runtime::spawn_blocking(f).await.map_err(|e| e.to_string())
}

/// A source chip: where its file is and whether it exists (Lampion Activity: `Lampion.log`).
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SourceInfo {
    source: Source,
    path: String,
    exists: bool,
}

#[tauri::command]
pub async fn logs_sources() -> Result<Vec<SourceInfo>, String> {
    blocking(|| {
        let l = locations::current();
        Source::ALL
            .iter()
            .map(|&source| {
                let path = source.path(&l);
                SourceInfo { source, exists: path.is_file(), path: path.display().to_string() }
            })
            .collect()
    })
    .await
}

/// Starts following a server log; returns all it shows now. Changes follow as `logs://update`.
#[tauri::command]
pub async fn logs_follow(source: Source) -> Result<Update, String> {
    blocking(move || super::follow(source)).await?
}

#[tauri::command]
pub fn logs_unfollow(source: Source) {
    super::unfollow(source);
}

/// Reads a server log again from scratch.
#[tauri::command]
pub async fn logs_reload(source: Source) -> Result<Update, String> {
    blocking(move || super::reload(source)).await?
}

/// Lampion Activity: empties the list (the file is kept); a server log: empties the file on disk.
#[tauri::command]
pub async fn logs_clear(source: Source) -> Result<Option<Update>, String> {
    blocking(move || super::clear(source)).await?
}
