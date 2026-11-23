//! App settings in `%APPDATA%\Lampion\settings.json`. One struct for the whole app; unknown keys
//! are kept (`extra`) so an older build never drops a newer build's settings.
//! A module that needs a new setting adds a field here with `#[serde(default)]`.

use once_cell::sync::Lazy;
use serde::{Deserialize, Serialize};
use std::sync::Mutex;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct Settings {
    /// The Lampion folder; empty = `C:\Lampion`.
    pub lampion_root: String,
    pub open_at_login: bool,
    pub start_servers_on_open: bool,
    pub stop_servers_on_quit: bool,
    pub check_for_updates: bool,
    /// "Skip This Version" (e.g. "1.2").
    pub skipped_version: Option<String>,
    /// root without a password; `None` until first set up, then the user's choice is kept.
    pub root_no_password: Option<bool>,
    /// phpMyAdmin automatic sign-in; same rule as above.
    pub phpmyadmin_auto_login: Option<bool>,
    /// Run the servers as Windows services instead of background processes.
    pub use_services: bool,
    #[serde(flatten)]
    pub extra: serde_json::Map<String, serde_json::Value>,
}

impl Default for Settings {
    fn default() -> Self {
        Settings {
            lampion_root: String::new(),
            open_at_login: false,
            start_servers_on_open: true,
            stop_servers_on_quit: false,
            check_for_updates: true,
            skipped_version: None,
            root_no_password: None,
            phpmyadmin_auto_login: None,
            use_services: false,
            extra: Default::default(),
        }
    }
}

static SETTINGS: Lazy<Mutex<Settings>> = Lazy::new(|| Mutex::new(Settings::default()));

fn file() -> std::path::PathBuf {
    super::paths::app_data().join("settings.json")
}

pub fn load() {
    let s = std::fs::read_to_string(file())
        .ok()
        .and_then(|t| serde_json::from_str::<Settings>(&t).ok())
        .unwrap_or_default();
    *SETTINGS.lock().unwrap() = s;
}

pub fn get() -> Settings {
    SETTINGS.lock().unwrap().clone()
}

/// Changes settings and writes them to disk at once (atomically: temp file + rename).
pub fn update(f: impl FnOnce(&mut Settings)) -> Settings {
    let mut g = SETTINGS.lock().unwrap();
    f(&mut g);
    let snapshot = g.clone();
    drop(g);
    if let Ok(json) = serde_json::to_string_pretty(&snapshot) {
        let tmp = file().with_extension("json.tmp");
        if std::fs::write(&tmp, json).is_ok() {
            let _ = std::fs::rename(&tmp, file());
        }
    }
    super::emit("settings://changed", snapshot.clone());
    snapshot
}
