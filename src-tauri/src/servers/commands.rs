//! Tauri commands of the servers part (registered in lib.rs, in the servers section).
//! Events: `servers://status` (Statuses, on real change), `servers://setup` (SetupState, on change),
//! `servers://moving` (bool).

use super::{control, monitor, moving, php, services, setup, Server};

async fn blocking<T: Send + 'static>(f: impl FnOnce() -> T + Send + 'static) -> Result<T, String> {
    tauri::async_runtime::spawn_blocking(f).await.map_err(|e| e.to_string())
}

/// `{ apache, mariadb }`, as `servers://status` sends it.
#[tauri::command]
pub async fn server_status() -> Result<monitor::Statuses, String> {
    blocking(monitor::status).await
}

/// `server`: "apache" | "mariadb". Resolves when the action has finished; the error says why it failed.
#[tauri::command]
pub async fn server_start(server: Server) -> Result<(), String> {
    control::start(server).await
}

#[tauri::command]
pub async fn server_stop(server: Server) -> Result<(), String> {
    control::stop(server).await
}

#[tauri::command]
pub async fn server_restart(server: Server) -> Result<(), String> {
    control::restart(server).await
}

#[tauri::command]
pub async fn servers_start_all() -> Result<(), String> {
    control::start_all().await
}

#[tauri::command]
pub async fn servers_stop_all() -> Result<(), String> {
    control::stop_all().await
}

#[tauri::command]
pub async fn php_info() -> Result<php::PhpInfo, String> {
    blocking(php::info).await
}

#[tauri::command]
pub fn setup_state() -> setup::SetupState {
    setup::state()
}

/// Set Up (first launch) and Try Again: runs in the background; follow `servers://setup`.
#[tauri::command]
pub fn setup_run() -> Result<(), String> {
    setup::run()
}

#[tauri::command]
pub fn setup_retry() -> Result<(), String> {
    setup::run()
}

#[tauri::command]
pub fn setup_cancel() {
    setup::cancel()
}

/// Another Lampion folder before anything is installed; returns the new state or why the folder is refused.
#[tauri::command]
pub async fn setup_choose_root(path: String) -> Result<setup::SetupState, String> {
    blocking(move || setup::choose_root(&path)).await?
}

/// Downloads and installs the Microsoft Visual C++ runtime (one UAC prompt), then continues the setup.
#[tauri::command]
pub fn setup_install_runtime() -> Result<(), String> {
    setup::install_runtime()
}

/// General › Locations › Move…: moves the Lampion folder; resolves with the new folder.
#[tauri::command]
pub async fn servers_move_root(target: String) -> Result<String, String> {
    moving::move_root(target).await
}

#[tauri::command]
pub fn servers_moving() -> bool {
    moving::active()
}

/// General › Run the servers as Windows services (one UAC prompt); also updates `useServices`.
#[tauri::command]
pub async fn servers_use_services(enabled: bool) -> Result<(), String> {
    services::set_mode(enabled).await
}
