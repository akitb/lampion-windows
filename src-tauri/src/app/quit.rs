//! Closing the main window. "Stop servers when Lampion quits" stops them in the background, after any
//! server action still running, and then exits; during a folder move quitting is refused; while the
//! updater relaunches Lampion the servers keep running.

use crate::core::{activity, settings};
use crate::servers::{self, State};
use std::sync::atomic::{AtomicBool, Ordering::SeqCst};

static RELAUNCHING: AtomicBool = AtomicBool::new(false);
static STOPPING: AtomicBool = AtomicBool::new(false);
static STOPPED: AtomicBool = AtomicBool::new(false);

/// The updater calls this before it closes Lampion to install a new version.
pub fn set_relaunching(on: bool) {
    RELAUNCHING.store(on, SeqCst);
}

/// Called when the main window asks to close. `true` = let it close now.
pub fn may_close(app: &tauri::AppHandle) -> bool {
    if servers::moving() {
        activity::warn("Panel", "Lampion quits after the folder move has finished.");
        return false;
    }
    if STOPPED.load(SeqCst) || RELAUNCHING.load(SeqCst) || !settings::get().stop_servers_on_quit {
        return true;
    }
    // The last status the monitor sent (never blocks the UI thread here).
    let st = servers::monitor::last();
    let busy = st.as_ref().is_some_and(|s| [&s.apache, &s.mariadb].iter().any(|x| matches!(x.state, State::Running | State::Busy)));
    if st.is_some() && !busy {
        return true;
    }
    if STOPPING.swap(true, SeqCst) {
        return false; // already stopping; Lampion exits when that is done
    }
    activity::info("Panel", "Stopping the servers before quitting…");
    let app = app.clone();
    servers::stop_all_then(move || {
        STOPPED.store(true, SeqCst);
        app.exit(0);
    });
    false
}
