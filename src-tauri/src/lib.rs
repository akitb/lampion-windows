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
            servers::commands::server_status,
            servers::commands::server_start,
            servers::commands::server_stop,
            servers::commands::server_restart,
            servers::commands::servers_start_all,
            servers::commands::servers_stop_all,
            servers::commands::php_info,
            servers::commands::setup_state,
            servers::commands::setup_run,
            servers::commands::setup_retry,
            servers::commands::setup_cancel,
            servers::commands::setup_choose_root,
            servers::commands::setup_install_runtime,
            servers::commands::servers_move_root,
            servers::commands::servers_moving,
            servers::commands::servers_use_services,
            // ---- config ----
            config::commands::config_apache_load,
            config::commands::config_apache_check,
            config::commands::config_apache_save,
            config::commands::config_php_load,
            config::commands::config_php_check,
            config::commands::config_php_save,
            config::commands::config_php_modules,
            config::commands::config_timezones,
            config::commands::config_mariadb_load,
            config::commands::config_mariadb_check,
            config::commands::config_mariadb_save,
            config::commands::config_snippets,
            config::commands::config_root_state,
            config::commands::config_set_root_no_password,
            config::commands::config_phpmyadmin_state,
            config::commands::config_set_phpmyadmin_auto_login,
            config::commands::config_backups,
            config::commands::config_restore,
            config::commands::config_edit_file,
            config::commands::config_locations,
            config::commands::config_folders,
            config::commands::config_open_at_login,
            config::commands::config_set_open_at_login,
            // ---- logs ----
            logs::commands::logs_sources,
            logs::commands::logs_follow,
            logs::commands::logs_unfollow,
            logs::commands::logs_reload,
            logs::commands::logs_clear,
            // ---- database (data, query, sidebar) ----
            database::commands::db_connect,
            database::commands::db_set_password,
            database::commands::db_tables,
            database::commands::db_columns,
            database::commands::db_page,
            database::commands::db_update_row,
            database::commands::db_insert_row,
            database::commands::db_delete_rows,
            database::commands::db_empty_table,
            database::commands::db_drop_table,
            database::commands::db_drop_database,
            database::commands::db_run_query,
            database::commands::db_history,
            database::commands::db_history_clear,
            database::commands::db_complete,
            database::commands::db_syntax,
            database::commands::db_syntax_marks,
            database::commands::db_insert_text,
            database::commands::db_export,
            database::commands::db_import,
            database::commands::db_show_main_window,
            // ---- database (structure, designer) ----
            // ---- app (updater, selftest, quit) ----
        ])
        .run(tauri::generate_context!())
        .expect("error while running Lampion");
}
