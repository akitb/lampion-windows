//! Shared building blocks every module uses: paths, settings, the activity log, running programs.

pub mod activity;
pub mod commands;
pub mod dialog;
pub mod paths;
pub mod settings;
pub mod shell;

use once_cell::sync::OnceCell;
use tauri::AppHandle;

static APP: OnceCell<AppHandle> = OnceCell::new();

/// The running app, for emitting events from background threads. `None` in unit tests.
pub fn app() -> Option<&'static AppHandle> {
    APP.get()
}

pub fn init(app: &AppHandle) {
    let _ = APP.set(app.clone());
    settings::load();
    activity::info("Panel", &format!("Lampion {} started", env!("CARGO_PKG_VERSION")));
}

/// Emits an event to every window. Silently does nothing without a running app (tests).
pub fn emit<S: serde::Serialize + Clone>(event: &str, payload: S) {
    use tauri::Emitter;
    if let Some(app) = app() {
        let _ = app.emit(event, payload);
    }
}
