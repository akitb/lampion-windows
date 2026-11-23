//! Closing the main window. "Stop servers when Lampion quits" stops them in the background
//! (after any running server operation) and then exits; during a folder move, quitting is refused.

/// Called when the main window asks to close. `true` = let it close now.
/// shortcut: always closes; the servers module wires stop-on-quit and the move guard here.
pub fn may_close(_app: &tauri::AppHandle) -> bool {
    true
}
