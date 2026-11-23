//! Start, stop and restart (SPEC "Indítás", "Leállítás", Panel.swift), one action at a time on a serial
//! queue. Start: config test (Apache), a free port (naming the program that holds it), launch, wait up to
//! 12 s for the port, then 2.5 s later the same PID must still run; otherwise it is stopped and the last
//! error lines of its log are shown. A MariaDB that is recovering its data gets up to 5 minutes and is
//! never stopped in the middle of it. Stop: MariaDB through its shutdown event (else `mariadb-admin
//! shutdown`; InnoDB stays consistent) with 60 s before any force, Apache through its shutdown event
//! (parent and child), force only as the last resort.

use super::probe::{self, Probe, Snapshot};
use super::{configs, monitor, services, Server};
use crate::config::locations::{self, Locations};
use crate::core::{activity, paths, settings, shell};
use once_cell::sync::Lazy;
use std::path::{Path, PathBuf};
use std::sync::{mpsc, Mutex};
use std::time::Duration;
use tokio::sync::oneshot;

pub const POLL: Duration = Duration::from_millis(500);
/// 24 × 0.5 s = 12 s for a server to listen.
pub const START_TRIES: usize = 24;
/// A started server must still run with the same PID after this long.
pub const SETTLE: Duration = Duration::from_millis(2500);
/// 30 s for Apache to stop in order.
const STOP_TRIES: usize = 60;
/// 60 s for MariaDB: InnoDB writes its buffers out before it exits, which can take a while.
const MARIADB_STOP_TRIES: usize = 120;

type Job = Box<dyn FnOnce() + Send>;

static QUEUE: Lazy<Mutex<mpsc::Sender<Job>>> = Lazy::new(|| {
    let (tx, rx) = mpsc::channel::<Job>();
    let _ = std::thread::Builder::new().name("lampion-servers".into()).spawn(move || {
        for job in rx {
            job();
        }
    });
    Mutex::new(tx)
});

/// Runs `f` on the server queue after every action queued before it.
pub fn submit<T: Send + 'static>(f: impl FnOnce() -> T + Send + 'static) -> oneshot::Receiver<T> {
    let (tx, rx) = oneshot::channel();
    let job: Job = Box::new(move || {
        let _ = tx.send(f());
    });
    let _ = QUEUE.lock().unwrap_or_else(|e| e.into_inner()).send(job);
    rx
}

pub async fn wait<T>(rx: oneshot::Receiver<T>) -> Result<T, String> {
    rx.await.map_err(|_| "Lampion's server queue stopped; restart Lampion.".to_string())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Op {
    Start,
    Stop,
    Restart,
}

pub async fn start(s: Server) -> Result<(), String> {
    run(&[s], Op::Start).await
}

pub async fn stop(s: Server) -> Result<(), String> {
    run(&[s], Op::Stop).await
}

pub async fn restart(s: Server) -> Result<(), String> {
    run(&[s], Op::Restart).await
}

/// Starts every installed server that is not running.
pub async fn start_all() -> Result<(), String> {
    let statuses = tauri::async_runtime::spawn_blocking(monitor::status).await.map_err(|e| e.to_string())?;
    let todo: Vec<Server> =
        Server::ALL.into_iter().filter(|s| statuses.get(*s).state == monitor::State::Stopped).collect();
    run(&todo, Op::Start).await
}

pub async fn stop_all() -> Result<(), String> {
    let statuses = tauri::async_runtime::spawn_blocking(monitor::status).await.map_err(|e| e.to_string())?;
    let todo: Vec<Server> =
        Server::ALL.into_iter().filter(|s| statuses.get(*s).state == monitor::State::Running).collect();
    run(&todo, Op::Stop).await
}

/// Queues the action for each server (all show "Working…" at once) and waits for every one.
async fn run(servers: &[Server], op: Op) -> Result<(), String> {
    let mine = monitor::set_busy(servers);
    let mut errors: Vec<String> = servers
        .iter()
        .filter(|s| !mine.contains(s))
        .map(|s| format!("{} is busy with another action.", s.title()))
        .collect();
    let jobs: Vec<_> = mine
        .into_iter()
        .map(|s| {
            submit(move || {
                let r = match op {
                    Op::Start => op_start(s),
                    Op::Stop => op_stop(s),
                    Op::Restart => op_restart(s),
                };
                monitor::finish(&[s]);
                r
            })
        })
        .collect();
    for rx in jobs {
        if let Err(e) = wait(rx).await.and_then(|r| r) {
            errors.push(e);
        }
    }
    if errors.is_empty() {
        Ok(())
    } else {
        Err(errors.join("\n"))
    }
}

/// For quitting: after the running action, stops every server, then calls `done`.
pub fn stop_all_then(done: impl FnOnce() + Send + 'static) {
    monitor::set_busy(&Server::ALL);
    let _ = submit(move || {
        for s in Server::ALL {
            if Snapshot::take().probe(s).running {
                let _ = op_stop(s);
            }
        }
        done();
    });
}

/// Logs an error and returns it, for `Err(fail(...))`.
fn fail(module: &str, text: String) -> String {
    activity::error(module, &text);
    text
}

pub fn ports_text(ports: &[u16]) -> String {
    if ports.is_empty() {
        "–".into()
    } else {
        ports.iter().map(u16::to_string).collect::<Vec<_>>().join(", ")
    }
}

// ---- The actions (they run on the queue thread and may block) ----

pub(super) fn op_start(s: Server) -> Result<(), String> {
    let m = s.module();
    if !s.installed() {
        return Err(fail(m, format!("{} is not installed yet. Finish the setup at the top of the window.", s.title())));
    }
    let snap = Snapshot::take();
    if snap.probe(s).running {
        activity::info(m, "Already running");
        return Ok(());
    }
    activity::info(m, "Starting…");
    let l = locations::current();
    if s == Server::Apache {
        check_apache_config("not starting")?;
    }
    for port in configured_ports(s, &l) {
        if let Some(owner) = snap.port_owner(port) {
            return Err(fail(
                m,
                format!("Port {port} is already in use by {owner}. Quit that program, or pick another port in {} settings.", s.title()),
            ));
        }
    }
    prepare_folders(s, &l);
    let log = error_log(s, &l);
    let offset = std::fs::metadata(&log).map(|m| m.len()).unwrap_or(0);
    launch(s)?;
    let mut started = await_start(|| Snapshot::take().probe(s), std::thread::sleep);
    if matches!(&started, Err(last) if s == Server::Mariadb && last.running && recovering(&read_since(&log, offset))) {
        activity::info(m, "MariaDB is recovering its data after an unclean stop; waiting for it (up to 5 minutes)…");
        let report = |secs: u64, line: Option<String>| match line {
            Some(line) => activity::info(m, &format!("Still recovering ({secs} s): {line}")),
            None => activity::info(m, &format!("Still recovering… ({secs} s)")),
        };
        started = await_recovery(|| Snapshot::take().probe(s), || read_since(&log, offset), std::thread::sleep, report);
        if matches!(&started, Err(last) if last.running) {
            // Never ended by force in the middle of a recovery: it would have to start all over again.
            let text = "MariaDB is still recovering its data and keeps starting in the background. Logs › MariaDB shows its progress.";
            activity::warn(m, text);
            return Err(text.into());
        }
    }
    match started {
        Ok(p) => {
            let pid = p.pids.first().map(u32::to_string).unwrap_or_else(|| "?".into());
            activity::ok(m, &format!("Started – PID: {pid}, port: {}", ports_text(&p.ports)));
            Ok(())
        }
        Err(_) => {
            // Never leave a half-started server behind (and a service must not keep retrying).
            let left = Snapshot::take().probe(s);
            if left.running {
                halt(s, &left);
            }
            activity::error(m, "Could not start. The log says:");
            let lines = recent_errors(s, Some(offset));
            if lines.is_empty() {
                activity::error(m, "  (no error line found – see Logs)");
            }
            for line in &lines {
                activity::error(m, &format!("  {line}"));
            }
            Err(match lines.last() {
                Some(line) => format!("{} could not start: {line}", s.title()),
                None => format!("{} could not start. See Logs for details.", s.title()),
            })
        }
    }
}

pub(super) fn op_stop(s: Server) -> Result<(), String> {
    let m = s.module();
    let p = Snapshot::take().probe(s);
    if !p.running {
        activity::info(m, "Already stopped");
        return Ok(());
    }
    activity::info(m, "Stopping…");
    if halt(s, &p) {
        activity::ok(m, "Stopped");
        Ok(())
    } else {
        Err(fail(m, format!("{} could not be stopped.", s.title())))
    }
}

pub(super) fn op_restart(s: Server) -> Result<(), String> {
    let m = s.module();
    let snap = Snapshot::take();
    let p = snap.probe(s);
    if !p.running {
        return op_start(s);
    }
    activity::info(m, "Restarting to apply the new settings…");
    if s == Server::Apache {
        check_apache_config("not restarting")?;
    }
    // A newly configured port must be free, or the server would not come back up.
    for port in configured_ports(s, &locations::current()) {
        if !p.ports.contains(&port) {
            if let Some(owner) = snap.port_owner(port) {
                return Err(fail(m, format!("Not restarting: the new port {port} is already in use by {owner}. Pick another port.")));
            }
        }
    }
    if !halt(s, &p) {
        return Err(fail(m, format!("{} could not be stopped, so it was not restarted.", s.title())));
    }
    op_start(s)
}

/// Stops a running server in order, by force as the last resort. True when it is gone.
fn halt(s: Server, p: &Probe) -> bool {
    let m = s.module();
    let probe_now = || Snapshot::take().probe(s);
    let asked = if settings::get().use_services {
        services::stop(s).map_err(|e| activity::warn(m, &e)).is_ok()
    } else {
        match s {
            // mpm_winnt's parent waits on the event "ap<PID>_shutdown" (the PID in httpd.pid) and stops its child.
            Server::Apache => p.pids.first().is_some_and(|pid| probe::signal_event(&format!("ap{pid}_shutdown"))),
            Server::Mariadb => mariadb_shutdown(p),
        }
    };
    // MariaDB always gets its time before any force (InnoDB may still be writing); Apache when asked.
    let tries = match s {
        Server::Mariadb => MARIADB_STOP_TRIES,
        Server::Apache if asked => STOP_TRIES,
        Server::Apache => 0,
    };
    if !asked && s == Server::Mariadb {
        activity::warn(m, "MariaDB could not be asked to stop; waiting up to 60 s before ending it by force.");
    }
    let waited = |secs: u64| activity::info(m, &format!("Still stopping… ({secs} s)"));
    if await_stop(probe_now, std::thread::sleep, tries, waited) {
        return true;
    }
    let left = probe_now();
    if !left.running {
        return true;
    }
    activity::warn(m, if s == Server::Mariadb {
        "Did not stop in time; ending it by force (InnoDB recovers at the next start)."
    } else {
        "Did not stop in time; ending it by force."
    });
    // Children first: an orphaned Apache child would keep the port.
    for pid in left.pids.iter().rev() {
        probe::kill(*pid);
    }
    await_stop(probe_now, std::thread::sleep, 10, |_| {})
}

/// Asks MariaDB to stop in order: its own shutdown event `MySQLShutdown<PID>` (MariaDB's
/// sql/handle_connections_win.cc; no password needed), else `mariadb-admin shutdown` on the port it
/// listens on with root's password when Lampion knows it, else `SHUTDOWN` over Lampion's connection.
fn mariadb_shutdown(p: &Probe) -> bool {
    if p.pids.first().is_some_and(|pid| probe::signal_event(&format!("MySQLShutdown{pid}"))) {
        return true;
    }
    let port = answering_port(p, locations::current().mariadb_port);
    let mut admin = shell::command(paths::mariadb_admin_exe());
    // MYSQL_PWD rather than --password: every program can read a command line.
    admin.args(admin_args(port)).env("MYSQL_PWD", crate::database::client::password().unwrap_or_default());
    let out = shell::output(admin, Duration::from_secs(60));
    if out.ok() {
        return true;
    }
    let sql = tauri::async_runtime::block_on(crate::database::client::query("SHUTDOWN"));
    if sql.is_ok() {
        return true;
    }
    let why = out.out.lines().map(str::trim).filter(|l| !l.is_empty()).last().unwrap_or("no answer").to_string();
    activity::warn("MySQL", &format!("mariadb-admin could not shut MariaDB down: {why}"));
    false
}

/// `mariadb-admin shutdown` on `port` (the password comes from MYSQL_PWD, never the command line).
pub fn admin_args(port: u16) -> Vec<String> {
    let port = format!("--port={port}");
    ["--no-defaults", "--protocol=TCP", "--host=127.0.0.1", &port, "--user=root", "--connect-timeout=5", "shutdown"].map(String::from).to_vec()
}

/// The port MariaDB answers on: the configured one when it listens there, else the one it listens on
/// (my.ini changed, restart pending), else the configured one (stopped).
pub fn answering_port(p: &Probe, configured: u16) -> u16 {
    if p.ports.contains(&configured) {
        configured
    } else {
        p.ports.first().copied().unwrap_or(configured)
    }
}

fn launch(s: Server) -> Result<(), String> {
    if settings::get().use_services {
        return services::start(s).map_err(|e| fail(s.module(), e));
    }
    let (exe, dir) = match s {
        Server::Apache => {
            php_on_path();
            (paths::httpd_exe(), paths::apache_dir())
        }
        Server::Mariadb => (paths::mariadbd_exe(), paths::mariadb_dir()),
    };
    let args = launch_args(s, &paths::httpd_conf(), &paths::my_ini());
    let args: Vec<&str> = args.iter().map(String::as_str).collect();
    shell::spawn_detached(&exe, &args, Some(dir.as_path()))
        .map(|_| ())
        .map_err(|e| fail(s.module(), format!("Could not run {}: {e}", exe.display())))
}

/// The command line of a background server. Only the config file: the data folder, ports and logs come
/// from httpd.conf / my.ini and never from the command line (SPEC pitfall: Homebrew's launcher passed an
/// old `--datadir` that silently beat the setting).
pub fn launch_args(s: Server, httpd_conf: &Path, my_ini: &Path) -> Vec<String> {
    match s {
        Server::Apache => vec!["-f".into(), httpd_conf.to_string_lossy().into_owned()],
        Server::Mariadb => vec![format!("--defaults-file={}", my_ini.display())],
    }
}

/// php8apache2_4.dll and PHP's extensions need DLLs from PHP's folder (libssh2, libcrypto, ICU…).
/// Windows finds them through PATH, as PHP's manual advises, so PHP's folder goes first in Lampion's
/// own PATH, which the Apache it starts inherits.
fn php_on_path() {
    let php = paths::php_dir();
    let current = std::env::var_os("PATH").unwrap_or_default();
    let mut dirs: Vec<PathBuf> = std::env::split_paths(&current).collect();
    let same = |d: &PathBuf| d.to_string_lossy().eq_ignore_ascii_case(&php.to_string_lossy());
    if dirs.first().is_some_and(same) {
        return;
    }
    dirs.retain(|d| !same(d));
    dirs.insert(0, php);
    if let Ok(joined) = std::env::join_paths(dirs) {
        std::env::set_var("PATH", joined);
    }
}

/// `httpd.exe -t`: a configuration Apache rejects is never started.
fn check_apache_config(when: &str) -> Result<(), String> {
    let conf = paths::httpd_conf().to_string_lossy().into_owned();
    let t = shell::run_timeout(paths::httpd_exe(), &["-t", "-f", &conf], Duration::from_secs(30));
    if t.ok() {
        return Ok(());
    }
    activity::error("Apache", &format!("Invalid configuration (httpd.conf), {when}:"));
    let lines: Vec<&str> = t.out.lines().map(str::trim).filter(|l| !l.is_empty()).collect();
    for line in &lines[lines.len().saturating_sub(6)..] {
        activity::error("Apache", &format!("  {line}"));
    }
    Err(format!("Apache's configuration has an error: {}", lines.first().copied().unwrap_or("httpd -t failed")))
}

/// Every port the server will bind: Apache's top-level `Listen` lines, MariaDB's `port`.
fn configured_ports(s: Server, l: &Locations) -> Vec<u16> {
    match s {
        Server::Apache => {
            let ports = configs::listen_ports(&std::fs::read_to_string(paths::httpd_conf()).unwrap_or_default());
            if ports.is_empty() {
                vec![l.apache_port]
            } else {
                ports
            }
        }
        Server::Mariadb => vec![l.mariadb_port],
    }
}

/// Apache and MariaDB do not create the folders of their logs and PID files.
fn prepare_folders(s: Server, l: &Locations) {
    let mut files: Vec<PathBuf> = match s {
        Server::Apache => vec![
            l.apache_error_log.clone(),
            l.apache_access_log.clone(),
            configs::apache_pid_file(&std::fs::read_to_string(paths::httpd_conf()).unwrap_or_default(), &paths::apache_dir()),
        ],
        Server::Mariadb => vec![configs::mariadb_pid_file(&std::fs::read_to_string(paths::my_ini()).unwrap_or_default(), &paths::root())],
    };
    files.extend(l.mariadb_error_log.clone().filter(|_| s == Server::Mariadb));
    files.extend(l.php_error_log.clone().filter(|_| s == Server::Apache));
    for dir in files.iter().filter_map(|f| f.parent()) {
        let _ = std::fs::create_dir_all(dir);
    }
    let _ = std::fs::create_dir_all(paths::tmp()); // PHP sessions and uploads
}

fn error_log(s: Server, l: &Locations) -> PathBuf {
    match s {
        Server::Apache => l.apache_error_log.clone(),
        Server::Mariadb => l.mariadb_error_log.clone().unwrap_or_default(),
    }
}

/// The last 3 error lines of the server's log, written after `since` (byte offset) when given.
pub fn recent_errors(s: Server, since: Option<u64>) -> Vec<String> {
    let text = read_since(&error_log(s, &locations::current()), since.unwrap_or(0));
    error_lines(&text, 3)
}

/// What a log gained after `offset` (at most its last 32 KB; the whole tail when the file shrank).
pub fn read_since(path: &Path, offset: u64) -> String {
    use std::io::{Read, Seek, SeekFrom};
    let Ok(mut f) = std::fs::File::open(path) else { return String::new() };
    let len = f.metadata().map(|m| m.len()).unwrap_or(0);
    let from = if offset <= len { offset } else { 0 }.max(len.saturating_sub(32 * 1024));
    let mut buf = Vec::new();
    if f.seek(SeekFrom::Start(from)).is_err() || f.read_to_end(&mut buf).is_err() {
        return String::new();
    }
    String::from_utf8_lossy(&buf).into_owned()
}

/// The last `count` error lines (Apache `:error]` `:emerg]` `:crit]` `:alert]`, MariaDB `[ERROR]`),
/// each at most 140 characters.
pub fn error_lines(text: &str, count: usize) -> Vec<String> {
    let lines: Vec<&str> = text
        .lines()
        .filter(|l| ["[ERROR]", ":error]", ":emerg]", ":crit]", ":alert]"].iter().any(|k| l.contains(k)) && !l.contains("Aborting"))
        .collect();
    lines[lines.len().saturating_sub(count)..]
        .iter()
        .map(|l| {
            let l = l.trim();
            if l.chars().count() > 140 {
                format!("{}…", l.chars().take(140).collect::<String>())
            } else {
                l.to_string()
            }
        })
        .collect()
}

// ---- The start and stop decisions, without real processes (unit-tested) ----

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StartStep {
    Wait,
    Listening,
    /// No process any more: the program exited (a bad setting, a missing DLL…).
    Died,
}

/// One look at a starting server. The process exists right after the launch, so if it is gone a
/// second later it has died and waiting the full 12 s is pointless.
pub fn start_step(p: &Probe, attempt: usize) -> StartStep {
    if p.running && !p.ports.is_empty() {
        StartStep::Listening
    } else if !p.running && attempt >= 2 {
        StartStep::Died
    } else {
        StartStep::Wait
    }
}

/// Still the same server a moment later (not crashed and restarted, which would loop forever).
pub fn stable(first: &Probe, again: &Probe) -> bool {
    again.running && !again.ports.is_empty() && again.pids.first() == first.pids.first()
}

/// Polls every 0.5 s (up to 12 s) until the server listens, then checks it is stable 2.5 s later.
/// `Ok` with the final look, or `Err` with the last one.
pub fn await_start(mut probe: impl FnMut() -> Probe, mut sleep: impl FnMut(Duration)) -> Result<Probe, Probe> {
    let mut last = Probe::default();
    for attempt in 0..START_TRIES {
        last = probe();
        match start_step(&last, attempt) {
            StartStep::Listening => {
                sleep(SETTLE);
                let again = probe();
                return if stable(&last, &again) { Ok(again) } else { Err(again) };
            }
            StartStep::Died => return Err(last),
            StartStep::Wait => sleep(POLL),
        }
    }
    Err(last)
}

/// How long a MariaDB that recovers its data (after a crash or a forced stop) may take to start.
pub const RECOVERY_LIMIT: Duration = Duration::from_secs(300);

/// MariaDB's log says it is recovering (InnoDB's "Starting crash recovery", "To recover: N pages",
/// "Parsed redo log up to LSN"; Aria's "starting recovery"), so starting takes longer than usual.
pub fn recovering(log: &str) -> bool {
    log.lines().any(|l| progress(l).is_some())
}

/// A recovery progress line without its time and level: "InnoDB: To recover: 1234 pages".
fn progress(line: &str) -> Option<&str> {
    const MARKS: [&str; 6] = ["crash recovery", "to recover:", "redo log up to", "batch recovery", "starting recovery", "recovered pages"];
    let lower = line.to_ascii_lowercase();
    MARKS.iter().any(|m| lower.contains(m)).then(|| line.split_once("] ").map_or(line, |(_, rest)| rest).trim())
}

/// After the normal 12 s, while MariaDB recovers: polls every 0.5 s (at most `RECOVERY_LIMIT`) until it
/// listens and is stable, or dies. `report(seconds, newest progress line)` is called every 15 s.
/// `Ok` with the final look; `Err` with the last one (still running = still recovering).
pub fn await_recovery(
    mut probe: impl FnMut() -> Probe,
    mut log: impl FnMut() -> String,
    mut sleep: impl FnMut(Duration),
    mut report: impl FnMut(u64, Option<String>),
) -> Result<Probe, Probe> {
    let tries = (RECOVERY_LIMIT.as_millis() / POLL.as_millis()) as usize;
    let mut last = Probe::default();
    for i in 0..tries {
        last = probe();
        match start_step(&last, START_TRIES + i) {
            StartStep::Listening => {
                sleep(SETTLE);
                let again = probe();
                return if stable(&last, &again) { Ok(again) } else { Err(again) };
            }
            StartStep::Died => return Err(last),
            StartStep::Wait if i > 0 && i % 30 == 0 => {
                let text = log();
                report((START_TRIES + i) as u64 / 2, text.lines().rev().find_map(progress).map(String::from));
            }
            StartStep::Wait => {}
        }
        sleep(POLL);
    }
    Err(last)
}

/// Polls every 0.5 s until the server is gone; false after `tries` looks. `waited(seconds)` hears
/// every 10 s that it is still there.
pub fn await_stop(mut probe: impl FnMut() -> Probe, mut sleep: impl FnMut(Duration), tries: usize, mut waited: impl FnMut(u64)) -> bool {
    for i in 0..tries {
        if !probe().running {
            return true;
        }
        if i > 0 && i % 20 == 0 {
            waited(i as u64 / 2);
        }
        if i + 1 < tries {
            sleep(POLL);
        }
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::RefCell;

    fn up(pid: u32) -> Probe {
        Probe { running: true, pids: vec![pid], ports: vec![8080] }
    }
    fn not_listening(pid: u32) -> Probe {
        Probe { running: true, pids: vec![pid], ports: vec![] }
    }

    /// Plays back looks in order (the last one repeats) and records the waits.
    fn script(looks: Vec<Probe>) -> (impl FnMut() -> Probe, RefCell<Vec<Duration>>) {
        let mut i = 0;
        let probe = move || {
            let p = looks[i.min(looks.len() - 1)].clone();
            i += 1;
            p
        };
        (probe, RefCell::new(Vec::new()))
    }

    #[test]
    fn starts_when_listening_and_stable() {
        let (probe, waits) = script(vec![Probe::default(), not_listening(7), up(7), up(7)]);
        let r = await_start(probe, |d| waits.borrow_mut().push(d));
        assert_eq!(r, Ok(up(7)));
        assert_eq!(*waits.borrow(), vec![POLL, POLL, SETTLE]);
    }

    #[test]
    fn a_restarted_pid_is_not_stable() {
        let (probe, waits) = script(vec![up(7), up(9)]);
        assert_eq!(await_start(probe, |d| waits.borrow_mut().push(d)), Err(up(9)));
    }

    #[test]
    fn crashed_after_listening_fails() {
        let (probe, waits) = script(vec![up(7), Probe::default()]);
        assert!(await_start(probe, |d| waits.borrow_mut().push(d)).is_err());
    }

    #[test]
    fn a_dead_process_fails_fast() {
        let (probe, waits) = script(vec![Probe::default()]);
        assert_eq!(await_start(probe, |d| waits.borrow_mut().push(d)), Err(Probe::default()));
        assert_eq!(waits.borrow().len(), 2, "gives up after about a second, not 12");
    }

    #[test]
    fn never_listening_gives_up_after_12_seconds() {
        let (probe, waits) = script(vec![not_listening(7)]);
        assert_eq!(await_start(probe, |d| waits.borrow_mut().push(d)), Err(not_listening(7)));
        let total: Duration = waits.borrow().iter().sum();
        assert_eq!(total, POLL * START_TRIES as u32);
    }

    #[test]
    fn steps() {
        assert_eq!(start_step(&up(1), 0), StartStep::Listening);
        assert_eq!(start_step(&Probe::default(), 0), StartStep::Wait);
        assert_eq!(start_step(&Probe::default(), 2), StartStep::Died);
        assert_eq!(start_step(&not_listening(1), 20), StartStep::Wait);
        assert!(stable(&up(1), &up(1)));
        assert!(!stable(&up(1), &not_listening(1)));
    }

    #[test]
    fn stop_waits() {
        let (probe, waits) = script(vec![up(7), up(7), Probe::default()]);
        assert!(await_stop(probe, |d| waits.borrow_mut().push(d), 10, |_| {}));
        assert_eq!(waits.borrow().len(), 2);
        let (probe, waits) = script(vec![up(7)]);
        assert!(!await_stop(probe, |d| waits.borrow_mut().push(d), 4, |_| {}));
        assert_eq!(waits.borrow().len(), 3);
        let (probe, _) = script(vec![up(7)]);
        assert!(!await_stop(probe, |_| {}, 0, |_| {}), "no time at all: not stopped");
    }

    const RECOVERY_LOG: &str = "2026-11-23 10:00:00 0 [Note] InnoDB: Completed initialization of buffer pool\n\
        2026-11-23 10:00:00 0 [Note] InnoDB: Starting crash recovery from checkpoint LSN=46846\n\
        2026-11-23 10:00:15 0 [Note] InnoDB: To recover: 1234 pages\n";

    #[test]
    fn recovery_is_recognised_from_the_log() {
        assert!(recovering(RECOVERY_LOG));
        assert!(recovering("2026-11-23 10:00:15 0 [Note] InnoDB: Parsed redo log up to LSN=99; to recover: 7 pages"));
        let normal = "2026-11-23 10:00:00 0 [Note] InnoDB: 128 rollback segments in 3 undo tablespaces are active.\n\
                      2026-11-23 10:00:00 0 [Note] InnoDB: log sequence number 46846; transaction id 14\n";
        assert!(!recovering(normal), "a normal start is not a recovery");
        assert_eq!(RECOVERY_LOG.lines().rev().find_map(progress), Some("InnoDB: To recover: 1234 pages"));
    }

    #[test]
    fn a_recovering_mariadb_is_waited_for() {
        let mut looks = vec![not_listening(7); 200];
        looks.extend([up(7), up(7)]);
        let (probe, waits) = script(looks);
        let reports = RefCell::new(Vec::new());
        let r = await_recovery(probe, || RECOVERY_LOG.to_string(), |d| waits.borrow_mut().push(d), |s, l| reports.borrow_mut().push((s, l)));
        assert_eq!(r, Ok(up(7)), "listening after 100 s more");
        assert_eq!(reports.borrow().len(), 6, "every 15 s");
        assert_eq!(reports.borrow()[0], (27, Some("InnoDB: To recover: 1234 pages".into())));

        let (probe, _) = script(vec![not_listening(7), Probe::default()]);
        assert_eq!(await_recovery(probe, String::new, |_| {}, |_, _| {}), Err(Probe::default()), "it died");

        let (probe, waits) = script(vec![not_listening(7)]);
        assert_eq!(await_recovery(probe, String::new, |d| waits.borrow_mut().push(d), |_, _| {}), Err(not_listening(7)));
        assert_eq!(waits.borrow().iter().sum::<Duration>(), RECOVERY_LIMIT, "bounded; still running = still recovering");
    }

    #[test]
    fn mariadb_gets_a_minute_and_progress_every_ten_seconds() {
        let (probe, waits) = script(vec![up(7)]);
        let reports = RefCell::new(Vec::new());
        assert!(!await_stop(probe, |d| waits.borrow_mut().push(d), MARIADB_STOP_TRIES, |s| reports.borrow_mut().push(s)));
        let total: Duration = waits.borrow().iter().sum();
        assert!(total >= Duration::from_secs(59), "{total:?}");
        assert_eq!(*reports.borrow(), [10, 20, 30, 40, 50]);
    }

    #[test]
    fn mariadb_is_asked_on_the_port_it_answers_without_a_password_on_the_command_line() {
        let maria = |ports: Vec<u16>| Probe { running: !ports.is_empty(), pids: vec![9], ports };
        assert_eq!(answering_port(&maria(vec![3306]), 3307), 3306, "my.ini says 3307, MariaDB still listens on 3306");
        assert_eq!(answering_port(&maria(vec![3306, 3307]), 3307), 3307);
        assert_eq!(answering_port(&maria(vec![]), 3307), 3307, "stopped: the configured port");
        let args = admin_args(3306);
        assert_eq!(args[0], "--no-defaults", "must come first");
        assert!(args.contains(&"--port=3306".to_string()) && args.last().unwrap() == "shutdown");
        assert!(!args.iter().any(|a| a.starts_with("--password") || a.starts_with("-p")), "{args:?}");
    }

    #[test]
    fn servers_start_only_from_their_config_files() {
        let conf = Path::new(r"C:\My Lampion\config\httpd.conf");
        let ini = Path::new(r"C:\My Lampion\config\my.ini");
        assert_eq!(launch_args(Server::Apache, conf, ini), ["-f", r"C:\My Lampion\config\httpd.conf"]);
        let maria = launch_args(Server::Mariadb, conf, ini);
        assert_eq!(maria, [r"--defaults-file=C:\My Lampion\config\my.ini"], "nothing may override my.ini");
    }

    #[test]
    fn the_queue_runs_actions_in_order() {
        // Quitting queues its stop behind the running action; nothing ever overlaps or jumps the queue.
        let order = std::sync::Arc::new(Mutex::new(Vec::new()));
        let jobs: Vec<_> = (0..5)
            .map(|i| {
                let order = order.clone();
                submit(move || {
                    std::thread::sleep(Duration::from_millis(if i == 0 { 30 } else { 1 }));
                    order.lock().unwrap().push(i);
                })
            })
            .collect();
        for rx in jobs {
            tauri::async_runtime::block_on(wait(rx)).unwrap();
        }
        assert_eq!(*order.lock().unwrap(), [0, 1, 2, 3, 4]);
    }

    #[test]
    fn error_lines_from_logs() {
        let log = "[Mon Nov 23 10:00:00.1 2026] [mpm_winnt:notice] [pid 1] AH00455: Apache started\n\
                   [Mon Nov 23 10:00:01.1 2026] [core:error] [pid 1] AH00015: Unable to open logs\n\
                   2026-11-23 10:00:02 0 [ERROR] Can't start server: Bind on TCP/IP port. Got error: 10048\n\
                   2026-11-23 10:00:02 0 [ERROR] Aborting\n\
                   [x] [ssl:emerg] [pid 2] AH02572: Failed to configure at least one certificate\n\
                   [x] [core:warn] [pid 2] just a warning\n";
        let lines = error_lines(log, 3);
        assert_eq!(lines.len(), 3);
        assert!(lines[0].contains("AH00015"));
        assert!(lines[1].contains("Bind on TCP/IP port"));
        assert!(lines[2].contains("AH02572"));
        let long = format!("[ERROR] {}", "x".repeat(300));
        assert_eq!(error_lines(&long, 3)[0].chars().count(), 141);
    }

    #[test]
    fn reads_only_what_the_log_gained() {
        let dir = tempfile::tempdir().unwrap();
        let f = dir.path().join("error.log");
        std::fs::write(&f, "old [ERROR] line\n").unwrap();
        let offset = std::fs::metadata(&f).unwrap().len();
        std::fs::write(&f, "old [ERROR] line\nnew [ERROR] line\n").unwrap();
        assert_eq!(read_since(&f, offset), "new [ERROR] line\n");
        assert_eq!(read_since(&f, 10_000), "old [ERROR] line\nnew [ERROR] line\n", "a rotated log is read from the start");
        assert_eq!(read_since(&dir.path().join("none.log"), 0), "");
        assert_eq!(ports_text(&[8080, 8443]), "8080, 8443");
        assert_eq!(ports_text(&[]), "–");
    }
}
