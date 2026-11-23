//! Watches the servers every 2 s and tells the UI (`servers://status`) only when something really changed
//! (an unchanged status sent every tick made the macOS window stutter). Logs "Stopped unexpectedly" with
//! the last error lines of the server's log, and "Started (externally)". Re-reads the configured ports
//! every 10 s, so hand edits of httpd.conf / my.ini show up, and points the database client and
//! phpMyAdmin at the port MariaDB answers on.

use super::probe::{Probe, Snapshot};
use super::{control, Server};
use crate::config::locations;
use crate::core::{activity, emit, shell};
use once_cell::sync::Lazy;
use serde::Serialize;
use std::sync::atomic::{AtomicU16, Ordering::SeqCst};
use std::sync::{Mutex, MutexGuard};
use std::time::Duration;

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum State {
    Running,
    Stopped,
    Busy,
    NotInstalled,
}

/// One server as the sidebar, toolbar and Overview show it (`ServerStatus` in src/lib/types.ts).
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ServerStatus {
    pub state: State,
    pub pids: Vec<u32>,
    pub ports: Vec<u16>,
    /// The configured port (shown even when stopped).
    pub port: u16,
    pub version: Option<String>,
}

/// The `servers://status` payload.
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct Statuses {
    pub apache: ServerStatus,
    pub mariadb: ServerStatus,
}

impl Statuses {
    pub fn get(&self, s: Server) -> &ServerStatus {
        match s {
            Server::Apache => &self.apache,
            Server::Mariadb => &self.mariadb,
        }
    }
}

pub fn server_status(installed: bool, busy: bool, probe: &Probe, port: u16, version: Option<String>) -> ServerStatus {
    let state = if !installed {
        State::NotInstalled
    } else if busy {
        State::Busy
    } else if probe.running {
        State::Running
    } else {
        State::Stopped
    };
    ServerStatus { state, pids: probe.pids.clone(), ports: probe.ports.clone(), port, version }
}

struct Shared {
    busy: [bool; 2],
    /// Running at the last check; `None` before the first one.
    running: [Option<bool>; 2],
    probes: [Probe; 2],
    installed: [bool; 2],
    ports: [u16; 2],
    /// `None` = not asked yet; `Some(None)` = the program did not say.
    versions: [Option<Option<String>>; 2],
    sent: Option<Statuses>,
}

impl Shared {
    fn statuses(&self) -> Statuses {
        let one = |s: Server| {
            let i = s.index();
            server_status(self.installed[i], self.busy[i], &self.probes[i], self.ports[i], self.versions[i].clone().flatten())
        };
        Statuses { apache: one(Server::Apache), mariadb: one(Server::Mariadb) }
    }

    /// The statuses to send, when they differ from what the UI has.
    fn changed(&mut self) -> Option<Statuses> {
        let now = self.statuses();
        (self.sent.as_ref() != Some(&now)).then(|| {
            self.sent = Some(now.clone());
            now
        })
    }
}

static SHARED: Lazy<Mutex<Shared>> = Lazy::new(|| {
    Mutex::new(Shared {
        busy: [false; 2],
        running: [None; 2],
        probes: Default::default(),
        installed: [false; 2],
        ports: [super::configs::APACHE_PORT, super::configs::MARIADB_PORT],
        versions: [None, None],
        sent: None,
    })
});

/// One check at a time, so an older check never overwrites what a finished action recorded.
static CHECK: Mutex<()> = Mutex::new(());

fn shared() -> MutexGuard<'static, Shared> {
    SHARED.lock().unwrap_or_else(|e| e.into_inner())
}

pub fn start() {
    let spawned = std::thread::Builder::new().name("lampion-monitor".into()).spawn(|| {
        for tick in 0u64.. {
            check(&[], tick % 5 == 0);
            std::thread::sleep(Duration::from_secs(2));
        }
    });
    if let Err(e) = spawned {
        activity::error("Panel", &format!("Could not start watching the servers: {e}"));
    }
}

/// The current statuses (checked now when nothing was checked yet). Blocking: call it off the UI thread.
pub fn status() -> Statuses {
    if let Some(s) = shared().sent.clone() {
        return s;
    }
    check(&[], true);
    shared().statuses()
}

/// The statuses last sent to the UI, without checking (never blocks).
pub fn last() -> Option<Statuses> {
    shared().sent.clone()
}

/// Marks servers busy ("Working…") for an action; returns those that were free (only they may proceed).
pub fn set_busy(servers: &[Server]) -> Vec<Server> {
    let (mine, send) = {
        let mut sh = shared();
        let mine: Vec<Server> = servers.iter().copied().filter(|s| !sh.busy[s.index()]).collect();
        for s in &mine {
            sh.busy[s.index()] = true;
        }
        (mine, sh.changed())
    };
    if let Some(st) = send {
        emit("servers://status", st);
    }
    mine
}

/// Ends an action: checks again and records the outcome as expected, so it is never "unexpected".
pub fn finish(servers: &[Server]) {
    check(servers, true);
}

/// A program's version is asked once per installation.
pub fn forget_versions() {
    shared().versions = [None, None];
}

fn check(finished: &[Server], reread_ports: bool) {
    let _one = CHECK.lock().unwrap_or_else(|e| e.into_inner());
    let snap = Snapshot::take();
    let probes = [snap.probe(Server::Apache), snap.probe(Server::Mariadb)];
    let installed = Server::ALL.map(Server::installed);
    let ports = reread_ports.then(|| {
        let l = locations::current();
        [l.apache_port, l.mariadb_port]
    });
    let unknown: Vec<Server> = {
        let sh = shared();
        Server::ALL.into_iter().filter(|s| installed[s.index()] && sh.versions[s.index()].is_none()).collect()
    };
    let versions: Vec<(Server, Option<String>)> = unknown.into_iter().map(|s| (s, version(s))).collect();

    let mut notes: Vec<(Server, &str, String)> = Vec::new();
    let mut stopped = Vec::new();
    let send = {
        let mut sh = shared();
        for s in finished {
            sh.busy[s.index()] = false;
            sh.running[s.index()] = Some(probes[s.index()].running);
        }
        for s in Server::ALL {
            let i = s.index();
            let now = probes[i].running;
            match sh.running[i] {
                None => {
                    let text = if !installed[i] {
                        "Not installed yet".to_string()
                    } else if now {
                        format!("Running – port: {}", control::ports_text(&probes[i].ports))
                    } else {
                        "Stopped".to_string()
                    };
                    notes.push((s, if now { "ok" } else { "info" }, text));
                    sh.running[i] = Some(now);
                }
                Some(was) if was != now && !sh.busy[i] => {
                    if now {
                        notes.push((s, "ok", "Started (externally)".into()));
                    } else {
                        stopped.push(s);
                    }
                    sh.running[i] = Some(now);
                }
                _ => {}
            }
            if !installed[i] {
                sh.versions[i] = None;
            }
        }
        for (s, v) in versions {
            sh.versions[s.index()] = Some(v);
        }
        sh.probes = probes.clone();
        sh.installed = installed;
        if let Some(p) = ports {
            sh.ports = p;
        }
        sh.changed()
    };

    for (s, level, text) in notes {
        if level == "ok" {
            activity::ok(s.module(), &text)
        } else {
            activity::info(s.module(), &text)
        }
    }
    for s in stopped {
        activity::warn(s.module(), "Stopped unexpectedly. The log says:");
        let lines = control::recent_errors(s, None);
        if lines.is_empty() {
            activity::warn(s.module(), "  (no error line found – see Logs)");
        }
        for l in lines {
            activity::warn(s.module(), &format!("  {l}"));
        }
    }
    if let Some(st) = send {
        emit("servers://status", st);
    }
    if let Some([_, maria]) = ports {
        follow_mariadb(control::answering_port(&probes[Server::Mariadb.index()], maria));
    }
    if probes[Server::Mariadb.index()].running {
        super::setup::on_mariadb_running();
    }
    if reread_ports {
        // Files removed by hand (or by an antivirus) bring the setup banner back.
        super::setup::evaluate();
    }
}

/// The database client (keeping a root password the user set) and phpMyAdmin follow the port MariaDB
/// answers on: a port saved in my.ini applies only once MariaDB has restarted.
fn follow_mariadb(port: u16) {
    if crate::database::client::port() != port {
        // `set_port` may close the old pool on the async runtime, so it must run there.
        tauri::async_runtime::spawn(async move { crate::database::client::set_port(port) });
    }
    // A failed change (phpMyAdmin's config rejected by `php -l`) is not retried every 10 s.
    static FAILED_FOR: AtomicU16 = AtomicU16::new(0);
    use crate::config::phpmyadmin;
    if phpmyadmin::configured_port().is_some_and(|p| p != port) && FAILED_FOR.load(SeqCst) != port {
        match phpmyadmin::sync_port(port) {
            Ok(_) => {
                FAILED_FOR.store(0, SeqCst);
                activity::info("MySQL", &format!("phpMyAdmin now connects to port {port}"));
            }
            Err(e) => {
                FAILED_FOR.store(port, SeqCst);
                activity::warn("MySQL", &format!("phpMyAdmin could not be pointed at port {port}: {}", e.lines().next().unwrap_or("")));
            }
        }
    }
}

fn version(s: Server) -> Option<String> {
    let arg = if s == Server::Apache { "-v" } else { "--version" };
    let out = shell::run_timeout(s.exe(), &[arg], Duration::from_secs(10));
    if !out.ok() {
        return None;
    }
    match s {
        Server::Apache => apache_version(&out.out),
        Server::Mariadb => mariadb_version(&out.out),
    }
}

/// "Server version: Apache/2.4.66 (Win64)" → "2.4.66".
pub fn apache_version(out: &str) -> Option<String> {
    let rest = &out[out.find("Apache/")? + 7..];
    Some(rest.split_whitespace().next()?.to_string())
}

/// "mariadbd  Ver 11.8.10-MariaDB for Win64 on AMD64" → "11.8.10".
pub fn mariadb_version(out: &str) -> Option<String> {
    let rest = &out[out.find("Ver ")? + 4..];
    Some(rest.split_whitespace().next()?.replace("-MariaDB", ""))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn versions_from_program_output() {
        assert_eq!(apache_version("Server version: Apache/2.4.66 (Win64)\r\nServer built:   Dec  6 2025").as_deref(), Some("2.4.66"));
        assert_eq!(mariadb_version("mariadbd  Ver 11.8.10-MariaDB for Win64 on AMD64 (mariadb.org binary distribution)").as_deref(), Some("11.8.10"));
        assert_eq!(apache_version("nothing"), None);
    }

    #[test]
    fn status_states() {
        let up = Probe { running: true, pids: vec![10, 11], ports: vec![8080] };
        let s = server_status(true, false, &up, 8080, Some("2.4.66".into()));
        assert_eq!((s.state, s.pids.clone(), s.ports.clone(), s.port), (State::Running, vec![10, 11], vec![8080], 8080));
        assert_eq!(server_status(true, true, &up, 8080, None).state, State::Busy);
        assert_eq!(server_status(false, true, &Probe::default(), 8080, None).state, State::NotInstalled);
        assert_eq!(server_status(true, false, &Probe::default(), 3306, None).state, State::Stopped);
    }

    #[test]
    fn only_real_changes_are_sent() {
        let mut sh = Shared {
            busy: [false; 2],
            running: [None; 2],
            probes: Default::default(),
            installed: [true; 2],
            ports: [8080, 3306],
            versions: [None, None],
            sent: None,
        };
        assert!(sh.changed().is_some(), "the first status is sent");
        assert!(sh.changed().is_none(), "the same status is not sent again");
        sh.busy[0] = true;
        assert_eq!(sh.changed().unwrap().apache.state, State::Busy);
        sh.busy[0] = false;
        sh.probes[0] = Probe { running: true, pids: vec![5], ports: vec![8080] };
        assert_eq!(sh.changed().unwrap().apache.state, State::Running);
        assert!(sh.changed().is_none());
        // JSON shape the UI expects.
        let json = serde_json::to_value(sh.statuses()).unwrap();
        assert_eq!(json["apache"]["state"], "running");
        assert_eq!(json["mariadb"]["state"], "stopped");
        assert_eq!(json["apache"]["ports"][0], 8080);
        assert!(json["mariadb"]["version"].is_null());
    }
}
