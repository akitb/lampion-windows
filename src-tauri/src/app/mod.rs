//! App-level features: the updater, self-tests (`--selftest`, `--dbtest`), quitting.

pub mod commands;
pub mod quit;

pub fn init(_app: &tauri::AppHandle) {}
