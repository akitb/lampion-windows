//! General › Locations › Move…: moves the whole Lampion folder (HomeFolder.swift's careful move, for
//! Windows). The servers stop, the folder is copied (the original is never touched), the configs in the
//! copy are pointed at the new place and checked by Apache, Lampion switches over, and MariaDB must then
//! run from the new data folder (`SELECT @@datadir`) with every database still there. Each step is
//! recorded the moment it happens; on failure the old folder comes back into use, and the half copy is
//! deleted only when the original is verifiably in place. Quitting is refused while a move runs.

use super::control::{self, op_start, op_stop};
use super::probe::Snapshot;
use super::{configs, monitor, php, setup, Server};
use crate::core::{activity, emit, paths, settings, shell};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering::SeqCst};
use std::time::Duration;

static MOVING: AtomicBool = AtomicBool::new(false);

pub fn active() -> bool {
    MOVING.load(SeqCst)
}

fn set_moving(on: bool) {
    MOVING.store(on, SeqCst);
    emit("servers://moving", on);
}

/// What the move has done so far, recorded immediately, so the rollback undoes exactly that.
#[derive(Debug, Default)]
pub struct Steps {
    /// Servers this move stopped (started again on failure).
    pub stopped: Vec<Server>,
    /// The target folder existed (empty) before the move.
    pub target_existed: bool,
    /// Copying into the target began, so it may hold a half copy.
    pub copying: bool,
    /// Settings point at the new folder.
    pub switched: bool,
    /// Servers started from the new folder.
    pub started: Vec<Server>,
    /// MariaDB's data folder in the original, if it had one.
    pub old_data: Option<PathBuf>,
}

/// The new folder must be a usable Lampion folder (`setup::check_folder`), outside the current one, and
/// empty or new.
pub fn check_target(old: &Path, raw: &str) -> Result<PathBuf, String> {
    let new = setup::check_folder(raw)?;
    let key = |p: &Path| p.to_string_lossy().trim_end_matches(['\\', '/']).to_lowercase() + "\\";
    let (o, n) = (key(old), key(&new));
    if o == n {
        return Err("That is the folder Lampion already uses.".into());
    }
    if n.starts_with(&o) || o.starts_with(&n) {
        return Err("Choose a folder outside the current Lampion folder (and not one that contains it).".into());
    }
    if new.is_dir() && std::fs::read_dir(&new).map(|mut d| d.next().is_some()).unwrap_or(true) {
        return Err(format!("{} is not empty. Choose an empty or a new folder.", new.display()));
    }
    Ok(new)
}

/// Moves the Lampion folder to `target`; returns the new folder.
pub async fn move_root(target: String) -> Result<String, String> {
    let old = paths::root();
    let new = check_target(&old, &target)?;
    if settings::get().use_services {
        return Err("Turn off “Run the servers as Windows services” in General first: the services point at the current folder.".into());
    }
    if setup::state().phase == setup::Phase::Running {
        return Err("Wait until the setup has finished.".into());
    }
    if MOVING.swap(true, SeqCst) {
        return Err("A move is already running.".into());
    }
    let mine = monitor::set_busy(&Server::ALL);
    if mine.len() < Server::ALL.len() {
        monitor::finish(&mine);
        MOVING.store(false, SeqCst);
        return Err("A server action is still running; try again when it has finished.".into());
    }
    set_moving(true);
    activity::info("Panel", &format!("Moving the Lampion folder to {}…", new.display()));
    let job = control::submit(move || {
        let r = run(&old, &new);
        monitor::finish(&Server::ALL);
        r
    });
    let result = control::wait(job).await.and_then(|r| r);
    set_moving(false);
    setup::evaluate();
    match &result {
        Ok(p) => activity::ok("Panel", &format!("Lampion now uses {p}")),
        Err(e) => activity::error("Panel", &format!("The move did not complete: {e}")),
    }
    result
}

fn run(old: &Path, new: &Path) -> Result<String, String> {
    let log = |t: &str| activity::info("Move", t);
    let mut steps = Steps { old_data: setup_data(old), ..Default::default() };
    let snap = Snapshot::take();
    let running: Vec<Server> = Server::ALL.into_iter().filter(|s| snap.probe(*s).running).collect();
    let before = if running.contains(&Server::Mariadb) { databases() } else { None };

    // 1. Stop the servers, so the database files are complete when copied.
    for s in &running {
        log(&format!("Stopping {}…", s.title()));
        if op_stop(*s).is_err() {
            for again in &steps.stopped {
                let _ = op_start(*again);
            }
            return Err(format!("{} did not stop, so nothing was moved.", s.title()));
        }
        steps.stopped.push(*s);
    }

    let result = transfer(old, new, &running, before.as_deref(), &mut steps);
    match result {
        Ok(()) => {
            let note = format!(
                "Lampion now uses {}.\r\nThis folder is the old copy; delete it once everything works there.\r\n",
                new.display()
            );
            let _ = std::fs::write(old.join("MOVED.txt"), note);
            log(&format!("Done. The old folder {} is kept; delete it once you are sure everything works.", old.display()));
            Ok(new.display().to_string())
        }
        Err(e) => {
            activity::warn("Move", &format!("Something went wrong ({e}); putting the old folder back into use."));
            rollback(&steps, old, new);
            Err(e)
        }
    }
}

/// Copy, re-point, check, switch, verify. Every step is recorded in `steps` before it can fail half-way.
fn transfer(old: &Path, new: &Path, running: &[Server], before: Option<&[String]>, steps: &mut Steps) -> Result<(), String> {
    let log = |t: &str| activity::info("Move", t);
    steps.target_existed = new.exists();
    steps.copying = true;
    std::fs::create_dir_all(new).map_err(|e| format!("Could not create {}: {e}", new.display()))?;
    for entry in std::fs::read_dir(old).map_err(|e| format!("Could not read {}: {e}", old.display()))? {
        let entry = entry.map_err(|e| e.to_string())?;
        log(&format!("Copying {}…", entry.file_name().to_string_lossy()));
        copy_entry(&entry.path(), &new.join(entry.file_name()))?;
    }

    log("Pointing httpd.conf, php.ini, my.ini and phpMyAdmin at the new folder…");
    rewrite_configs(new, old, new)?;
    let conf = new.join(r"config\httpd.conf");
    let httpd = new.join(r"bin\apache\bin\httpd.exe");
    if httpd.is_file() && conf.is_file() {
        let t = shell::run_timeout(&httpd, &["-t", "-f", &conf.to_string_lossy()], Duration::from_secs(30));
        if !t.ok() {
            let why = t.out.lines().map(str::trim).filter(|l| !l.is_empty()).last().unwrap_or("httpd -t failed").to_string();
            return Err(format!("Apache rejected the moved configuration: {why}"));
        }
    }

    settings::update(|s| s.lampion_root = new.display().to_string());
    steps.switched = true;
    monitor::forget_versions();
    php::forget();

    // Always run MariaDB once from the new folder: it must really use the new data folder.
    if steps.old_data.is_some() && Server::Mariadb.installed() {
        log("Starting MariaDB from the new folder…");
        op_start(Server::Mariadb)?;
        steps.started.push(Server::Mariadb);
        let expected = crate::config::locations::current().mariadb_datadir;
        let reported = query_retry("SELECT @@datadir").and_then(|r| r.first().cloned().ok_or_else(|| "no answer".to_string()))?;
        if !same_dir(&reported, &expected) {
            return Err(format!("MariaDB still uses {reported} instead of {}.", expected.display()));
        }
        let after = databases().ok_or("MariaDB started, but its databases could not be listed.")?;
        if let Some(missing) = before.map(|b| b.iter().filter(|d| !after.contains(d)).cloned().collect::<Vec<_>>()) {
            if !missing.is_empty() {
                return Err(format!("After the move these databases were missing: {}.", missing.join(", ")));
            }
        }
        if !after.iter().any(|d| d == "mysql") {
            return Err("MariaDB started, but its system database was not found.".into());
        }
        if !running.contains(&Server::Mariadb) {
            op_stop(Server::Mariadb)?;
            steps.started.retain(|s| *s != Server::Mariadb);
        }
    }
    if running.contains(&Server::Apache) {
        log("Starting Apache from the new folder…");
        op_start(Server::Apache)?;
        steps.started.push(Server::Apache);
    }
    Ok(())
}

/// Undoes what `steps` says was done, in reverse.
fn rollback(steps: &Steps, old: &Path, new: &Path) {
    for s in steps.started.iter().rev() {
        let _ = op_stop(*s);
    }
    if steps.switched {
        settings::update(|s| s.lampion_root = old.display().to_string());
        monitor::forget_versions();
        php::forget();
    }
    match discard_copy(steps, old, new) {
        Ok(true) => activity::info("Move", &format!("Removed the unfinished copy in {}", new.display())),
        Ok(false) => {}
        Err(e) => activity::warn("Move", &e),
    }
    for s in &steps.stopped {
        let _ = op_start(*s);
    }
}

/// The data folder of a set-up Lampion folder (None when MariaDB was never set up there).
fn setup_data(root: &Path) -> Option<PathBuf> {
    let ini = std::fs::read_to_string(root.join(r"config\my.ini")).unwrap_or_default();
    let data = configs::my_value(&ini, "datadir")
        .map(|d| PathBuf::from(d.trim().trim_end_matches(['/', '\\']).replace('/', "\\")))
        .unwrap_or_else(|| root.join("databases"));
    data.join("mysql").is_dir().then_some(data)
}

/// Removes the half copy in `new`, but only when this move made it and the original is intact
/// (its configuration and, if it had one, its data folder are in place). `Ok(true)` when removed.
pub fn discard_copy(steps: &Steps, old: &Path, new: &Path) -> Result<bool, String> {
    if !steps.copying || !new.exists() {
        return Ok(false);
    }
    let intact = old.join(r"config\httpd.conf").is_file() && steps.old_data.as_ref().is_none_or(|d| d.join("mysql").is_dir());
    if !intact {
        return Err(format!(
            "Kept the copy in {}: the original folder {} could not be verified, so nothing was deleted.",
            new.display(),
            old.display()
        ));
    }
    let removed = if steps.target_existed {
        // The folder was the user's (empty) choice: empty it, keep it.
        std::fs::read_dir(new).map_err(|e| e.to_string())?.filter_map(Result::ok).try_for_each(|e| {
            let p = e.path();
            if p.is_dir() {
                std::fs::remove_dir_all(&p)
            } else {
                std::fs::remove_file(&p)
            }
        })
    } else {
        std::fs::remove_dir_all(new)
    };
    removed.map(|()| true).map_err(|e| format!("Could not remove the unfinished copy in {}: {e}", new.display()))
}

/// Copies a file or folder. PID files are left behind (the servers are stopped); links are skipped
/// (they point outside, and following them would copy someone else's files).
fn copy_entry(from: &Path, to: &Path) -> Result<(), String> {
    let fail = |e: std::io::Error| format!("Could not copy {}: {e}", from.display());
    let meta = std::fs::symlink_metadata(from).map_err(fail)?;
    if meta.file_type().is_symlink() {
        activity::warn("Move", &format!("Skipped the link {}", from.display()));
        return Ok(());
    }
    if meta.is_dir() {
        std::fs::create_dir_all(to).map_err(fail)?;
        for entry in std::fs::read_dir(from).map_err(fail)? {
            let entry = entry.map_err(fail)?;
            copy_entry(&entry.path(), &to.join(entry.file_name()))?;
        }
        return Ok(());
    }
    if from.extension().is_some_and(|e| e.eq_ignore_ascii_case("pid")) {
        return Ok(());
    }
    std::fs::copy(from, to).map(|_| ()).map_err(fail)
}

/// Points every config file in the copy (and the originals Lampion keeps as `.lampion-backup`) at `new`.
pub fn rewrite_configs(root: &Path, old: &Path, new: &Path) -> Result<(), String> {
    let mut files: Vec<PathBuf> = std::fs::read_dir(root.join("config"))
        .map(|d| d.filter_map(Result::ok).map(|e| e.path()).filter(|p| p.is_file()).collect())
        .unwrap_or_default();
    files.push(root.join(r"bin\phpmyadmin\config.inc.php"));
    for f in files.iter().filter(|f| f.is_file()) {
        let text = std::fs::read_to_string(f).map_err(|e| format!("Could not read {}: {e}", f.display()))?;
        let moved = configs::replace_root(&text, old, new);
        if moved != text {
            let tmp = PathBuf::from(format!("{}.lampion-tmp", f.display()));
            std::fs::write(&tmp, &moved)
                .and_then(|()| std::fs::rename(&tmp, f))
                .map_err(|e| format!("Could not update {}: {e}", f.display()))?;
        }
    }
    Ok(())
}

/// MariaDB reports `C:\Lampion\databases\`; compare like Windows does.
pub fn same_dir(reported: &str, expected: &Path) -> bool {
    let norm = |s: &str| s.trim().replace('/', "\\").trim_end_matches('\\').to_lowercase();
    norm(reported) == norm(&expected.to_string_lossy())
}

/// The database names, through Lampion's own connection (`None` when MariaDB does not answer).
fn databases() -> Option<Vec<String>> {
    query_retry("SHOW DATABASES").ok()
}

/// A query on a fresh start: a pooled connection to the stopped server fails once, the next one is new.
fn query_retry(sql: &str) -> Result<Vec<String>, String> {
    let mut last = String::new();
    for _ in 0..3 {
        match tauri::async_runtime::block_on(crate::database::client::query(sql)) {
            Ok(set) => return Ok(set.rows.into_iter().filter_map(|r| r.into_iter().next().flatten()).collect()),
            Err(e) => last = e,
        }
        std::thread::sleep(Duration::from_millis(500));
    }
    Err(format!("MariaDB did not answer: {last}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn write(p: &Path, text: &str) {
        std::fs::create_dir_all(p.parent().unwrap()).unwrap();
        std::fs::write(p, text).unwrap();
    }

    /// A small Lampion folder: configs with its own paths, a data folder, a site and a stale PID file.
    fn fake_root(root: &Path) {
        let r = paths::fwd(root);
        write(&root.join(r"config\httpd.conf"), &format!("Define SRVROOT \"{r}/bin/apache\"\r\nDocumentRoot \"{r}/www\"\r\n"));
        write(&root.join(r"config\httpd.conf.lampion-backup"), &format!("DocumentRoot \"{r}/www\"\r\n"));
        write(&root.join(r"config\my.ini"), &format!("[mysqld]\r\ndatadir={r}/databases\r\n"));
        write(&root.join(r"config\php.ini"), &format!("error_log = \"{r}/logs/php/php_errors.log\"\r\n"));
        write(&root.join(r"databases\mysql\user.frm"), "frm");
        write(&root.join(r"www\index.php"), "<?php echo 1;");
        write(&root.join(r"logs\apache\httpd.pid"), "1234");
    }

    #[test]
    fn copies_and_repoints() {
        let dir = tempfile::tempdir().unwrap();
        let (old, new) = (dir.path().join("Lampion"), dir.path().join("Moved").join("Lampion"));
        fake_root(&old);
        copy_entry(&old, &new).unwrap();
        rewrite_configs(&new, &old, &new).unwrap();
        let n = paths::fwd(&new);
        assert_eq!(std::fs::read_to_string(new.join(r"www\index.php")).unwrap(), "<?php echo 1;");
        assert!(new.join(r"databases\mysql\user.frm").is_file());
        assert!(!new.join(r"logs\apache\httpd.pid").exists(), "PID files stay behind");
        let conf = std::fs::read_to_string(new.join(r"config\httpd.conf")).unwrap();
        assert_eq!(conf, format!("Define SRVROOT \"{n}/bin/apache\"\r\nDocumentRoot \"{n}/www\"\r\n"), "line endings kept");
        assert!(std::fs::read_to_string(new.join(r"config\httpd.conf.lampion-backup")).unwrap().contains(&n));
        assert!(std::fs::read_to_string(new.join(r"config\my.ini")).unwrap().contains(&format!("datadir={n}/databases")));
        assert!(std::fs::read_to_string(old.join(r"config\my.ini")).unwrap().contains(&paths::fwd(&old)), "the original is untouched");
        assert_eq!(setup_data(&old), Some(PathBuf::from(old.join("databases").to_string_lossy().replace('/', "\\"))));
    }

    #[test]
    fn half_copy_is_removed_only_when_the_original_is_intact() {
        let dir = tempfile::tempdir().unwrap();
        let (old, new) = (dir.path().join("Lampion"), dir.path().join("Target"));
        fake_root(&old);
        copy_entry(&old, &new).unwrap();
        let steps = Steps { copying: true, old_data: setup_data(&old), ..Default::default() };
        assert_eq!(discard_copy(&steps, &old, &new), Ok(true));
        assert!(!new.exists());

        // The original lost its data folder: the copy may be the only one left, so it stays.
        copy_entry(&old, &new).unwrap();
        std::fs::remove_dir_all(old.join(r"databases\mysql")).unwrap();
        assert!(discard_copy(&steps, &old, &new).unwrap_err().contains("could not be verified"));
        assert!(new.join(r"databases\mysql\user.frm").is_file());

        // Nothing copied yet: nothing to remove.
        let none = Steps::default();
        assert_eq!(discard_copy(&none, &old, &new), Ok(false));
    }

    #[test]
    fn a_chosen_empty_folder_is_emptied_not_deleted() {
        let dir = tempfile::tempdir().unwrap();
        let (old, new) = (dir.path().join("Lampion"), dir.path().join("Chosen"));
        fake_root(&old);
        std::fs::create_dir_all(&new).unwrap();
        let steps = Steps { copying: true, target_existed: true, old_data: setup_data(&old), ..Default::default() };
        copy_entry(&old, &new).unwrap();
        assert_eq!(discard_copy(&steps, &old, &new), Ok(true));
        assert!(new.is_dir() && std::fs::read_dir(&new).unwrap().next().is_none());
    }

    #[test]
    fn target_rules() {
        let dir = tempfile::tempdir().unwrap();
        let old = dir.path().join("Lampion");
        std::fs::create_dir_all(&old).unwrap();
        let s = |p: &Path| p.to_string_lossy().into_owned();
        assert!(check_target(&old, &s(&old)).unwrap_err().contains("already uses"));
        assert!(check_target(&old, &s(&old.join("inner"))).unwrap_err().contains("outside"));
        assert!(check_target(&old, &s(dir.path())).unwrap_err().contains("outside"));
        let busy = dir.path().join("Busy");
        write(&busy.join("x.txt"), "x");
        assert!(check_target(&old, &s(&busy)).unwrap_err().contains("not empty"));
        assert_eq!(check_target(&old, &s(&dir.path().join("New"))).unwrap(), dir.path().join("New"));
        assert!(check_target(&old, &format!("{}2", s(&old))).is_ok(), "Lampion2 is not inside Lampion");
    }

    #[test]
    fn datadir_comparison() {
        assert!(same_dir(r"D:\Dev\Lampion\databases\", Path::new(r"D:\Dev\Lampion\databases")));
        assert!(same_dir("d:/dev/lampion/databases/", Path::new(r"D:\Dev\Lampion\databases")));
        assert!(!same_dir(r"C:\Lampion\databases\", Path::new(r"D:\Dev\Lampion\databases")));
    }
}
