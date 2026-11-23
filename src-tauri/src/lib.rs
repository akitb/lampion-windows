//! Lampion for Windows: the app entry. Each feature lives in its own module
//! (`servers`, `config`, `logs`, `database`, `app`) with an `init` hook and a `commands` submodule.
//! Register a module's commands only inside its own marked section of `generate_handler!` below.

pub mod app;
pub mod config;
pub mod core;
pub mod database;
pub mod logs;
pub mod servers;

use tauri::Manager;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_clipboard_manager::init())
        .setup(|app| {
            let handle = app.handle().clone();
            core::init(&handle);
            servers::init(&handle);
            config::init(&handle);
            logs::init(&handle);
            database::init(&handle);
            app::init(&handle);
            if let Some(w) = app.get_webview_window("main") {
                let _ = w.set_focus();
            }
            Ok(())
        })
        .on_window_event(|window, event| {
            if window.label() == "main" {
                if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                    // The app decides when to quit (stop servers first, never during a move).
                    if !app::quit::may_close(window.app_handle()) {
                        api.prevent_close();
                    }
                }
            }
        })
        .invoke_handler(tauri::generate_handler![
            // ---- core (shell / windows / settings) ----
            core::commands::app_info,
            core::commands::get_settings,
            core::commands::set_settings,
            core::commands::activity_lines,
            core::commands::open_path,
            core::commands::open_url,
            core::commands::open_terminal,
            core::commands::open_database_window,
            core::commands::pick_path,
            // ---- servers ----
            // ---- config ----
            // ---- logs ----
            // ---- database (data, query, sidebar) ----
            // ---- database (structure, designer) ----
            // ---- app (updater, selftest, quit) ----
        ])
        .run(tauri::generate_context!())
        .expect("error while running Lampion");
}
