//! App-level features: the updater, the self-tests (`--selftest`, `--dbtest`), quitting (`quit`, owned
//! by the servers part).

pub mod commands;
pub mod quit;
pub mod trust;
pub mod updater;

pub fn init(_app: &tauri::AppHandle) {
    let args: Vec<String> = std::env::args().collect();
    updater::start(&args);
}
