//! The Database Manager backend. `client` is shared by both database parts:
//! `commands` (sidebar, tables, data, query, console, export/import) and
//! `structure_commands` (structure tab + structure sheets) and `designer_commands` (the Designer).

pub mod client;
pub mod commands;
pub mod designer_commands;
pub mod structure_commands;

pub fn init(_app: &tauri::AppHandle) {}
