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

/// The settings a page may change directly (General's switches). The others change only through their
/// own commands: the Lampion folder through setup or a move, services, root and phpMyAdmin access,
/// open at login, a skipped update.
const PAGE_SETTINGS: [&str; 3] = ["startServersOnOpen", "stopServersOnQuit", "checkForUpdates"];

/// Merges `patch` (camelCase keys, as `get_settings` returns them) into `s`; only `PAGE_SETTINGS`.
fn apply_patch(s: &mut settings::Settings, patch: &serde_json::Value) -> Result<(), String> {
    let patch = patch.as_object().ok_or("The settings change must be an object.")?;
    if let Some(key) = patch.keys().find(|k| !PAGE_SETTINGS.contains(&k.as_str())) {
        return Err(format!("The setting {key} cannot be changed here."));
    }
    let mut merged = serde_json::to_value(&*s).map_err(|e| e.to_string())?;
    if let Some(obj) = merged.as_object_mut() {
        obj.extend(patch.iter().map(|(k, v)| (k.clone(), v.clone())));
    }
    *s = serde_json::from_value(merged).map_err(|e| e.to_string())?;
    Ok(())
}

/// Changes General's switches; the change is made on the settings as they are at that moment.
#[tauri::command]
pub fn set_settings(patch: serde_json::Value) -> Result<settings::Settings, String> {
    settings::try_update(|s| apply_patch(s, &patch))
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

/// Windows Terminal in `dir` if installed, else PowerShell (both by their full path).
#[tauri::command]
pub fn open_terminal(dir: String) -> Result<(), String> {
    if !std::path::Path::new(&dir).is_dir() {
        return Err(format!("{dir} is not a folder."));
    }
    // The App Execution Alias Windows Terminal installs for every user.
    if let Some(wt) = dirs::data_local_dir().map(|d| d.join(r"Microsoft\WindowsApps\wt.exe")) {
        if std::process::Command::new(wt).args(wt_args(&dir)).spawn().is_ok() {
            return Ok(());
        }
    }
    std::process::Command::new(paths::powershell_exe())
        .args(["-NoExit", "-Command", &format!("Set-Location -LiteralPath '{}'", dir.replace('\'', "''"))])
        .current_dir(&dir)
        .spawn()
        .map(|_| ())
        .map_err(|e| e.to_string())
}

/// `wt.exe -d <dir>`. Windows Terminal reads an unescaped `;` anywhere as "and then run this", so it is escaped.
fn wt_args(dir: &str) -> [String; 2] {
    ["-d".into(), dir.replace(';', r"\;")]
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
/// Async on purpose: building a webview window from a sync command (main thread) deadlocks on Windows.
#[tauri::command]
pub async fn open_database_window(app: AppHandle) -> Result<(), String> {
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_semicolon_in_the_folder_cannot_start_another_command() {
        assert_eq!(wt_args(r"C:\Lampion\www"), ["-d", r"C:\Lampion\www"]);
        assert_eq!(wt_args(r"C:\a;calc.exe"), ["-d", r"C:\a\;calc.exe"]);
        assert!(open_terminal(r"C:\no such folder\x".into()).is_err(), "only an existing folder");
    }

    #[test]
    fn pages_change_only_their_own_settings() {
        let mut s = settings::Settings::default();
        apply_patch(&mut s, &serde_json::json!({ "checkForUpdates": false, "stopServersOnQuit": true })).unwrap();
        assert!(!s.check_for_updates && s.stop_servers_on_quit);
        for refused in [
            serde_json::json!({ "lampionRoot": r"C:\Users\Public" }),
            serde_json::json!({ "useServices": true }),
            serde_json::json!({ "rootNoPassword": true, "checkForUpdates": true }),
            serde_json::json!({ "anything": 1 }),
        ] {
            assert!(apply_patch(&mut s, &refused).unwrap_err().contains("cannot be changed here"), "{refused}");
        }
        assert!(apply_patch(&mut s, &serde_json::json!({ "checkForUpdates": "yes" })).is_err(), "booleans only");
        assert!(apply_patch(&mut s, &serde_json::json!([1])).is_err());
        assert!(!s.check_for_updates && s.lampion_root.is_empty() && !s.use_services, "a refused change changes nothing");
    }
}
