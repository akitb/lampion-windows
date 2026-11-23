//! The Database Manager backend. `client` is shared by both database parts:
//! `commands` (sidebar, tables, data, query, console, export/import) and
//! `structure_commands` (structure editing, Designer).

pub mod client;
pub mod commands;
pub mod structure_commands;

pub fn init(_app: &tauri::AppHandle) {}
