//! Tauri commands of the update banner and the General page's "Check Now". Registered in lib.rs under
//! "app (updater, selftest, quit)". The banner follows `update://state`.

use super::updater::{self, State};

/// `userInitiated`: Check Now / Try Again. Then the answer always shows and the error comes back here too.
#[tauri::command]
pub async fn update_check(user_initiated: bool) -> Result<(), String> {
    updater::check(user_initiated).await
}

#[tauri::command]
pub fn update_state() -> State {
    updater::state()
}

#[tauri::command]
pub fn update_install() -> Result<(), String> {
    updater::install()
}

#[tauri::command]
pub fn update_cancel() {
    updater::cancel();
}

#[tauri::command]
pub fn update_skip() {
    updater::skip();
}

#[tauri::command]
pub fn update_later() {
    updater::later();
}
