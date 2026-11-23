//! Commands shared by every page: app info, settings, the activity log, opening things.

use super::{activity, paths, settings};
use serde::Serialize;
use tauri::{AppHandle, Manager, WebviewUrl, WebviewWindowBuilder};

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AppInfo {
    pub version: String,
    pub lampion_root: String,
    pub args: Vec<String>,
}

#[tauri::command]
pub fn app_info() -> AppInfo {
    AppInfo {
        version: env!("CARGO_PKG_VERSION").to_string(),
        lampion_root: paths::root().to_string_lossy().into_owned(),
        args: std::env::args().skip(1).collect(),
    }
}

#[tauri::command]
pub fn get_settings() -> settings::Settings {
    settings::get()
}

/// Merges the given fields into the settings (camelCase keys, as `get_settings` returns them).
#[tauri::command]
pub fn set_settings(patch: serde_json::Value) -> Result<settings::Settings, String> {
    let mut current = serde_json::to_value(settings::get()).map_err(|e| e.to_string())?;
    if let (Some(obj), Some(p)) = (current.as_object_mut(), patch.as_object()) {
        for (k, v) in p {
            obj.insert(k.clone(), v.clone());
        }
    }
    let next: settings::Settings = serde_json::from_value(current).map_err(|e| e.to_string())?;
    Ok(settings::update(|s| *s = next))
}

#[tauri::command]
pub fn activity_lines() -> Vec<activity::Line> {
    activity::all()
}

/// Opens a file or folder in Explorer (or its default app).
#[tauri::command]
pub fn open_path(path: String) -> Result<(), String> {
    tauri_plugin_opener::open_path(path, None::<&str>).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn open_url(url: String) -> Result<(), String> {
    tauri_plugin_opener::open_url(url, None::<&str>).map_err(|e| e.to_string())
}

/// Windows Terminal in `dir` if installed, else PowerShell.
#[tauri::command]
pub fn open_terminal(dir: String) -> Result<(), String> {
    let wt = std::process::Command::new("wt.exe").args(["-d", &dir]).spawn();
    if wt.is_ok() {
        return Ok(());
    }
    std::process::Command::new("powershell.exe")
        .args(["-NoExit", "-Command", &format!("Set-Location -LiteralPath '{}'", dir.replace('\'', "''"))])
        .current_dir(&dir)
        .spawn()
        .map(|_| ())
        .map_err(|e| e.to_string())
}

/// Native dialog. `kind`: "folder" | "open" | "save". `filter`: [label, "*.sql;*.txt"]. `None` = cancelled.
#[tauri::command]
pub async fn pick_path(
    kind: String,
    title: String,
    start_dir: Option<String>,
    filter: Option<(String, String)>,
    default_name: Option<String>,
) -> Option<String> {
    use super::dialog::{pick, Kind};
    let kind = match kind.as_str() {
        "folder" => Kind::Folder,
        "save" => Kind::SaveFile,
        _ => Kind::OpenFile,
    };
    tauri::async_runtime::spawn_blocking(move || {
        pick(kind, &title, start_dir.as_deref(), filter.as_ref().map(|(a, b)| (a.as_str(), b.as_str())), default_name.as_deref())
    })
    .await
    .ok()
    .flatten()
}

/// Opens (or focuses) the Database Manager window (`/database` route).
#[tauri::command]
pub fn open_database_window(app: AppHandle) -> Result<(), String> {
    if let Some(w) = app.get_webview_window("database") {
        let _ = w.unminimize();
        return w.set_focus().map_err(|e| e.to_string());
    }
    WebviewWindowBuilder::new(&app, "database", WebviewUrl::App("database".into()))
        .title("Database Manager")
        .inner_size(1120.0, 720.0)
        .min_inner_size(820.0, 520.0)
        .build()
        .map(|_| ())
        .map_err(|e| e.to_string())
}
