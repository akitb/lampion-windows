//! See docs/SPEC.md and docs/AGENTS.md for what this module owns.
//! `locations`, `access` and `phpmyadmin` are called by other modules: keep their signatures.

pub mod access;
pub mod commands;
pub mod locations;
pub mod phpmyadmin;

pub fn init(_app: &tauri::AppHandle) {}
