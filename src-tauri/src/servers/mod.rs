//! The local servers (docs/SPEC.md "Szerverek Windows-on"): first setup (download, verify, unpack,
//! configure), starting and stopping Apache and MariaDB as background processes or Windows services,
//! watching them every 2 s, and moving the Lampion folder. Server actions run one at a time on a serial
//! queue (`control`), so a start, a stop, a service switch and a move never overlap.

pub mod commands;
pub mod configs;
pub mod control;
mod download;
mod elevate;
pub mod manifest;
pub mod monitor;
pub mod moving;
pub mod php;
pub mod probe;
mod services;
pub mod setup;

use crate::core::paths;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// The two servers Lampion runs (PHP runs inside Apache).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Server {
    Apache,
    Mariadb,
}

impl Server {
    pub const ALL: [Server; 2] = [Server::Apache, Server::Mariadb];

    pub fn title(self) -> &'static str {
        match self {
            Server::Apache => "Apache",
            Server::Mariadb => "MariaDB",
        }
    }

    /// The activity log's module name.
    pub fn module(self) -> &'static str {
        match self {
            Server::Apache => "Apache",
            Server::Mariadb => "MySQL",
        }
    }

    pub fn exe(self) -> PathBuf {
        match self {
            Server::Apache => paths::httpd_exe(),
            Server::Mariadb => paths::mariadbd_exe(),
        }
    }

    pub fn installed(self) -> bool {
        self.exe().is_file()
    }

    /// Name of the Windows service (General › Run the servers as Windows services).
    pub fn service(self) -> &'static str {
        match self {
            Server::Apache => "LampionApache",
            Server::Mariadb => "LampionMariaDB",
        }
    }

    fn index(self) -> usize {
        self as usize
    }
}

pub fn init(_app: &tauri::AppHandle) {
    // Before the monitor starts: it moves the client to MariaDB's port and keeps this password.
    crate::config::access::restore_saved_password();
    quiet_errors();
    setup::init();
    monitor::start();
}

/// A server that cannot load a DLL (no Visual C++ runtime, say) must not pop up a system error box at
/// every check: child processes inherit this error mode and fail quietly; Lampion logs the failure.
#[cfg(windows)]
fn quiet_errors() {
    extern "system" {
        fn GetErrorMode() -> u32;
        fn SetErrorMode(mode: u32) -> u32;
    }
    const SEM_FAILCRITICALERRORS: u32 = 0x0001;
    unsafe {
        SetErrorMode(GetErrorMode() | SEM_FAILCRITICALERRORS);
    }
}

#[cfg(not(windows))]
fn quiet_errors() {}

// The API other parts use (quit, self-test, settings pages).
pub use control::{restart, start, start_all, stop, stop_all, stop_all_then};
pub use monitor::{status, ServerStatus, State, Statuses};

/// True while the Lampion folder is being moved (quitting is refused then).
pub fn moving() -> bool {
    moving::active()
}
