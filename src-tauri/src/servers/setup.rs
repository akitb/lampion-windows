//! First setup (SPEC "Első telepítés", Setup.swift): confirm the Lampion folder, check the Visual C++
//! runtime, download the packages with progress (cancellable), check each SHA-256, unpack them into `bin\`,
//! create MariaDB's data folder, write the configuration, start the servers and, once, turn on root
//! without a password and phpMyAdmin's automatic sign-in. The banner at the top of the main window follows
//! `servers://setup`; every step also goes to the activity log (module `Setup`).

use super::download::{self, CANCELLED};
use super::manifest::{self, Package, VcRuntime};
use super::{configs, control, elevate, monitor, php, Server};
use crate::app::trust;
use crate::core::{activity, emit, paths, settings, shell};
use once_cell::sync::Lazy;
use serde::Serialize;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering::SeqCst};
use std::sync::{Mutex, MutexGuard};
use std::time::Duration;

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum Phase {
    Ready,
    /// First launch: confirm or change the Lampion folder, then Set Up.
    ChooseFolder,
    /// The Visual C++ runtime is missing or too old: Install Runtime.
    NeedsRuntime,
    Running,
    Failed,
    /// Files are missing from a folder set up earlier: Try Again.
    Missing,
}

/// The `servers://setup` payload (`SetupState` in src/lib/types.ts).
#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct SetupState {
    pub phase: Phase,
    /// The Lampion folder (the one being chosen, or the current one).
    pub root: String,
    /// Headline: what runs now, or what is wrong.
    pub title: String,
    pub detail: String,
    /// 0…1 while downloading or unpacking; `None` = no measurable progress.
    pub progress: Option<f64>,
    /// Missing parts, by name ("Apache", "the configuration", …).
    pub missing: Vec<String>,
    pub can_cancel: bool,
    /// Nothing is installed in the folder yet, so another folder may be chosen.
    pub can_change_root: bool,
}

/// The packages and the files that prove each one is complete.
const PACKAGES: [(&str, &str, &[&str]); 4] = [
    ("apache", "Apache", &[r"bin\httpd.exe"]),
    ("php", "PHP", &["php.exe", "php8apache2_4.dll"]),
    ("mariadb", "MariaDB", &[r"bin\mariadbd.exe", r"bin\mariadb-install-db.exe", r"bin\mariadb-admin.exe"]),
    ("phpmyadmin", "phpMyAdmin", &["index.php"]),
];

impl SetupState {
    fn new(phase: Phase, root: &Path, title: &str, detail: &str) -> Self {
        SetupState {
            phase,
            root: root.display().to_string(),
            title: title.into(),
            detail: detail.into(),
            progress: None,
            missing: Vec::new(),
            can_cancel: false,
            can_change_root: false,
        }
    }

    fn ready(root: &Path) -> Self {
        Self::new(Phase::Ready, root, "", "")
    }

    fn choose(root: &Path) -> Self {
        let detail = "Lampion downloads them (about 160 MB) and keeps them, your sites and your databases in one folder:";
        Self { can_change_root: true, ..Self::new(Phase::ChooseFolder, root, "Set up Apache, PHP and MariaDB", detail) }
    }

    fn running(root: &Path, title: &str, detail: &str, progress: Option<f64>, can_cancel: bool) -> Self {
        Self { progress, can_cancel, ..Self::new(Phase::Running, root, title, detail) }
    }

    fn missing(root: &Path, missing: Vec<String>) -> Self {
        let software: Vec<String> = missing.iter().filter(|m| PACKAGES.iter().any(|p| p.1 == m.as_str())).cloned().collect();
        let (title, detail) = if software.is_empty() {
            ("The server setup is not complete".to_string(), format!("Try Again creates what is missing: {}.", list(&missing)))
        } else {
            ("Server software is not installed yet".to_string(), format!("Try Again downloads {} and sets everything up.", list(&software)))
        };
        Self { can_change_root: nothing_installed(root), missing, ..Self::new(Phase::Missing, root, &title, &detail) }
    }

    fn failed(root: &Path, error: &str, missing: Vec<String>) -> Self {
        Self { can_change_root: nothing_installed(root), missing, ..Self::new(Phase::Failed, root, "Setup did not finish", error) }
    }

    fn needs_runtime(root: &Path, min: &str) -> Self {
        let detail = format!(
            "Apache and PHP need Microsoft's Visual C++ runtime ({min} or newer). Lampion downloads it from Microsoft; Windows asks for permission to install it."
        );
        Self::new(Phase::NeedsRuntime, root, "The Microsoft Visual C++ runtime is needed", &detail)
    }
}

static STATE: Lazy<Mutex<SetupState>> = Lazy::new(|| Mutex::new(SetupState::ready(&paths::root())));
static CANCEL: AtomicBool = AtomicBool::new(false);
/// Missing files are downloaded again by themselves once per launch; the banner's button is for retries.
static AUTO_RUN: AtomicBool = AtomicBool::new(false);
static ROOT_DEFAULT_TRIED: AtomicBool = AtomicBool::new(false);
static PMA_DEFAULT_TRIED: AtomicBool = AtomicBool::new(false);

fn lock() -> MutexGuard<'static, SetupState> {
    STATE.lock().unwrap_or_else(|e| e.into_inner())
}

/// Changes the state; the banner hears about real changes only.
fn update(f: impl FnOnce(&mut SetupState)) {
    let changed = {
        let mut s = lock();
        let before = s.clone();
        f(&mut s);
        (*s != before).then(|| s.clone())
    };
    if let Some(s) = changed {
        emit("servers://setup", s);
    }
}

pub fn state() -> SetupState {
    lock().clone()
}

pub fn ready() -> bool {
    lock().phase == Phase::Ready
}

/// A self-test or database test run must not start servers or change logins by itself.
fn test_run() -> bool {
    std::env::args().any(|a| a == "--selftest" || a == "--dbtest")
}

// ---- What the folder has ----

pub fn package_ready(root: &Path, id: &str) -> bool {
    let dir = root.join("bin").join(id);
    match PACKAGES.iter().find(|p| p.0 == id) {
        Some((_, _, files)) => files.iter().all(|f| dir.join(f).is_file()),
        None => dir.is_dir(),
    }
}

fn nothing_installed(root: &Path) -> bool {
    !PACKAGES.iter().any(|p| package_ready(root, p.0))
}

/// MariaDB's data folder as my.ini of the Lampion folder `root` names it (default `databases\`).
pub(super) fn datadir(root: &Path) -> PathBuf {
    configs::mariadb_datadir(&std::fs::read_to_string(root.join(r"config\my.ini")).unwrap_or_default(), root)
}

/// What the Lampion folder still lacks, by name; empty = ready.
pub fn missing_parts(root: &Path) -> Vec<String> {
    let mut missing: Vec<String> = PACKAGES.iter().filter(|p| !package_ready(root, p.0)).map(|p| p.1.to_string()).collect();
    if !["httpd.conf", "php.ini", "my.ini"].iter().all(|f| root.join("config").join(f).is_file()) {
        missing.push("the configuration".into());
    }
    if !datadir(root).join("mysql").is_dir() {
        missing.push("the database files".into());
    }
    missing
}

fn is_empty_dir(p: &Path) -> bool {
    std::fs::read_dir(p).map(|mut d| d.next().is_none()).unwrap_or(false)
}

fn looks_like_lampion(p: &Path) -> bool {
    p.join("bin").is_dir() || p.join("config").is_dir()
}

/// "Apache, PHP and MariaDB".
fn list(names: &[String]) -> String {
    match names {
        [] => String::new(),
        [one] => one.clone(),
        [rest @ .., last] => format!("{} and {last}", rest.join(", ")),
    }
}

/// A folder Lampion may keep its files in: a full local path, not a drive, English letters only (Apache,
/// PHP and MariaDB do not all handle others), never Program Files or OneDrive (SPEC pitfall).
pub fn check_folder(raw: &str) -> Result<PathBuf, String> {
    let s = raw.trim();
    if s.is_empty() {
        return Err("Choose a folder.".into());
    }
    if s.starts_with(r"\\") || s.starts_with("//") {
        return Err("Choose a folder on this computer, not on the network.".into());
    }
    let p = PathBuf::from(s);
    if !p.is_absolute() {
        return Err(r"Use a full path, like C:\Lampion.".into());
    }
    if p.parent().is_none() {
        return Err(r"Choose a folder, not a whole drive (for example D:\Lampion).".into());
    }
    if !s.is_ascii() {
        return Err("Use a folder path with English letters only: Apache, PHP and MariaDB cannot all handle other characters.".into());
    }
    if paths::is_unsafe_location(&p) {
        return Err(r"Lampion cannot keep its files in Program Files (Windows protects it) or in OneDrive (syncing damages database files). Choose another folder, like C:\Lampion.".into());
    }
    if p.exists() && !p.is_dir() {
        return Err("That is a file, not a folder.".into());
    }
    Ok(PathBuf::from(s.trim_end_matches(['\\', '/'])))
}

// ---- The banner's actions ----

/// Works out the state from what is on disk (left alone while a setup runs).
pub fn evaluate() {
    if lock().phase == Phase::Running {
        return;
    }
    let s = settings::get();
    let next = if s.lampion_root.trim().is_empty() {
        let default = PathBuf::from(paths::DEFAULT_ROOT);
        if missing_parts(&default).is_empty() {
            settings::update(|x| x.lampion_root = paths::DEFAULT_ROOT.into());
            SetupState::ready(&default)
        } else {
            let current = state();
            if current.phase == Phase::ChooseFolder {
                current
            } else {
                SetupState::choose(&default)
            }
        }
    } else {
        let root = paths::root();
        let missing = missing_parts(&root);
        let current = state();
        if missing.is_empty() {
            SetupState::ready(&root)
        } else if matches!(current.phase, Phase::Failed | Phase::NeedsRuntime) && current.root == root.display().to_string() {
            current
        } else {
            SetupState::missing(&root, missing)
        }
    };
    update(|st| *st = next);
}

/// At launch: what is missing is set up again (once per launch); a ready folder gets its servers started
/// when General › Start servers when Lampion opens is on.
pub fn init() {
    evaluate();
    let _ = std::thread::Builder::new().name("lampion-setup-check".into()).spawn(|| {
        if test_run() {
            return;
        }
        match state().phase {
            Phase::Missing if !AUTO_RUN.swap(true, SeqCst) => {
                activity::info("Setup", "Some server files are missing; setting them up again.");
                let _ = run();
            }
            Phase::Ready => {
                pma_default();
                php::log_summary();
                let st = monitor::status();
                if settings::get().start_servers_on_open
                    && Server::ALL.iter().any(|s| st.get(*s).state == monitor::State::Stopped)
                {
                    activity::info("Panel", "Starting the servers (General › Start servers when Lampion opens)");
                    let _ = tauri::async_runtime::block_on(control::start_all());
                }
            }
            _ => {}
        }
    });
}

/// "Change…": another folder for a setup that has not installed anything yet.
pub fn choose_root(raw: &str) -> Result<SetupState, String> {
    if !state().can_change_root {
        return Err("Lampion already uses this folder. Move it in General › Locations instead.".into());
    }
    let p = check_folder(raw)?;
    if p.is_dir() && !is_empty_dir(&p) && !looks_like_lampion(&p) {
        return Err(format!(
            "{} already has other files in it. Choose an empty folder, a new one, or a Lampion folder.",
            p.display()
        ));
    }
    update(|s| *s = SetupState::choose(&p));
    Ok(state())
}

/// Set Up / Try Again: runs the setup in the background.
pub fn run() -> Result<(), String> {
    launch(false)
}

/// Install Runtime: downloads and installs the Visual C++ runtime (one UAC prompt), then sets up.
pub fn install_runtime() -> Result<(), String> {
    launch(true)
}

pub fn cancel() {
    CANCEL.store(true, SeqCst);
}

fn launch(runtime_first: bool) -> Result<(), String> {
    let (root, claimed) = {
        let mut s = lock();
        if matches!(s.phase, Phase::Running | Phase::Ready) {
            return Ok(());
        }
        let root = check_folder(&s.root)?;
        *s = SetupState::running(&root, "Preparing the Lampion folder", "", None, false);
        (root, s.clone())
    };
    emit("servers://setup", claimed);
    // The folder becomes Lampion's folder the moment the setup starts.
    let chosen = root.display().to_string();
    if settings::get().lampion_root != chosen {
        settings::update(|s| s.lampion_root = chosen);
    }
    CANCEL.store(false, SeqCst);
    let worker = root.clone();
    std::thread::Builder::new()
        .name("lampion-setup".into())
        .spawn(move || work(&worker, runtime_first))
        .map(|_| ())
        .map_err(|e| {
            update(|s| *s = SetupState::failed(&root, &e.to_string(), missing_parts(&root)));
            e.to_string()
        })
}

enum Outcome {
    Done,
    NeedsRuntime(String),
}

fn work(root: &Path, runtime_first: bool) {
    let result = if runtime_first { install_runtime_now(root).and_then(|()| prepare(root)) } else { prepare(root) };
    match result {
        Ok(Outcome::Done) => finish(root),
        Ok(Outcome::NeedsRuntime(min)) => {
            activity::warn("Setup", &format!("Apache and PHP need the Microsoft Visual C++ runtime {min} or newer."));
            update(|s| *s = SetupState::needs_runtime(root, &min));
        }
        Err(e) => {
            if e == CANCELLED {
                activity::warn("Setup", &e);
            } else {
                activity::error("Setup", &e);
            }
            update(|s| *s = SetupState::failed(root, &e, missing_parts(root)));
        }
    }
}

fn step(root: &Path, title: &str, detail: &str, progress: Option<f64>, can_cancel: bool) {
    update(|s| *s = SetupState::running(root, title, detail, progress, can_cancel));
}

/// A download progress callback that updates the banner only when the shown percentage changes.
fn download_progress() -> impl FnMut(u64, Option<u64>) {
    let mut shown = u64::MAX;
    move |done, total| {
        let mb = |b: u64| b as f64 / 1_048_576.0;
        let (fraction, detail) = match total {
            Some(t) if t > 0 => (Some(done as f64 / t as f64), format!("{:.1} of {:.1} MB", mb(done), mb(t))),
            _ => (None, format!("{:.1} MB", mb(done))),
        };
        let mark = fraction.map_or(done / 1_048_576, |f| (f * 100.0) as u64);
        if mark != shown {
            shown = mark;
            update(|s| {
                s.progress = fraction;
                s.detail = detail;
            });
        }
    }
}

fn prepare(root: &Path) -> Result<Outcome, String> {
    create_layout(root)?;
    let (manifest, source) = tauri::async_runtime::block_on(manifest::load());
    if !runtime_ok(&manifest.vc_runtime) {
        return Ok(Outcome::NeedsRuntime(manifest.vc_runtime.min_version));
    }
    let todo: Vec<&Package> = manifest.packages.iter().filter(|p| !package_ready(root, &p.id)).collect();
    if !todo.is_empty() {
        let names: Vec<String> = todo.iter().map(|p| p.title()).collect();
        activity::info("Setup", &format!("Downloading {} (package list: {source}).", list(&names)));
    }
    let downloads = paths::local_data().join("downloads");
    for (i, p) in todo.iter().enumerate() {
        install_package(root, p, i + 1, todo.len(), &downloads)?;
    }
    write_configs(root)?;
    create_datadir(root)?;
    let www = root.join("www");
    if is_empty_dir(&www) && write_new(&www.join("index.php"), &configs::welcome_page())? {
        activity::info("Setup", "Added a welcome page to the web folder");
    }
    step(root, "Checking the Apache configuration", "", None, false);
    let conf = root.join(r"config\httpd.conf").to_string_lossy().into_owned();
    let test = shell::run_timeout(root.join(r"bin\apache\bin\httpd.exe"), &["-t", "-f", &conf], Duration::from_secs(30));
    if !test.ok() {
        return Err(format!("Apache reports a configuration problem: {}", last_lines(&test.out, 1).join("")));
    }
    Ok(Outcome::Done)
}

fn last_lines(text: &str, n: usize) -> Vec<String> {
    let lines: Vec<String> = text.lines().map(str::trim).filter(|l| !l.is_empty()).map(str::to_string).collect();
    lines[lines.len().saturating_sub(n)..].to_vec()
}

fn create_layout(root: &Path) -> Result<(), String> {
    for dir in ["bin", "config", "www", "databases", r"logs\apache", r"logs\mariadb", r"logs\php", "backups", "tmp"] {
        let d = root.join(dir);
        std::fs::create_dir_all(&d).map_err(|e| format!("Could not create {}: {e}", d.display()))?;
    }
    write_new(&root.join("README.txt"), &configs::readme())?;
    Ok(())
}

/// Writes a new file (through a temp file); an existing one is never overwritten. True when written.
fn write_new(path: &Path, text: &str) -> Result<bool, String> {
    if path.exists() {
        return Ok(false);
    }
    let tmp = PathBuf::from(format!("{}.lampion-tmp", path.display()));
    std::fs::write(&tmp, text)
        .and_then(|()| std::fs::rename(&tmp, path))
        .map(|()| true)
        .map_err(|e| format!("Could not write {}: {e}", path.display()))
}

fn install_package(root: &Path, p: &Package, n: usize, of: usize, downloads: &Path) -> Result<(), String> {
    let title = p.title();
    let count = if of > 1 { format!(" ({n} of {of})") } else { String::new() };
    let archive = downloads.join(format!("{}-{}.zip", p.id, p.version));
    if download::sha256_file(&archive).is_some_and(|d| download::sha256_matches(&d, &p.sha256)) {
        activity::info("Setup", &format!("{title}: using the file downloaded earlier"));
    } else {
        step(root, &format!("Downloading {title}{count}"), "", Some(0.0), true);
        let host = p.url.split('/').nth(2).unwrap_or(&p.url);
        activity::info("Setup", &format!("Downloading {title} from {host}"));
        let mut report = download_progress();
        tauri::async_runtime::block_on(download::fetch(&p.urls(), &archive, Some(&p.sha256), &CANCEL, &mut report))?;
        activity::ok("Setup", &format!("{title}: downloaded, SHA-256 checked"));
    }

    step(root, &format!("Unpacking {title}{count}"), "", Some(0.0), true);
    let bin = root.join("bin");
    let target = bin.join(&p.target);
    let partial = bin.join(format!("{}.partial", p.target));
    let _ = std::fs::remove_dir_all(&partial);
    let mut shown = usize::MAX;
    let unpacked = download::extract_zip(&archive, &partial, &p.strip, &CANCEL, &mut |i, total| {
        let pct = i * 100 / total.max(1);
        if pct != shown {
            shown = pct;
            update(|s| s.progress = Some(pct as f64 / 100.0));
        }
    });
    if let Err(e) = unpacked {
        let _ = std::fs::remove_dir_all(&partial);
        return Err(e);
    }
    if let Err(e) = replace_package(&partial, &target, &kept_paths(root)) {
        let _ = std::fs::remove_dir_all(&partial);
        return Err(e);
    }
    if !package_ready(root, &p.id) {
        return Err(format!("The {title} package does not contain the expected files (the package list may name the wrong folder)."));
    }
    let _ = std::fs::remove_file(&archive);
    activity::ok("Setup", &format!("{title} is in {}", target.display()));
    if p.id == "phpmyadmin" && !target.join("config.inc.php").is_file() {
        // The user's earlier choice (a first setup gets the default from pma_default).
        if let Some(on) = settings::get().phpmyadmin_auto_login {
            if let Err(e) = crate::config::phpmyadmin::set_auto_login(on) {
                activity::warn("Setup", &format!("phpMyAdmin's configuration could not be written again: {e}"));
            }
        }
    }
    Ok(())
}

/// What the configuration keeps in the Lampion folder that may lie inside a package folder: MariaDB's
/// data, the web folder, the log files and phpMyAdmin's config.inc.php.
fn kept_paths(root: &Path) -> Vec<PathBuf> {
    let l = crate::config::locations::current();
    let mut keep = vec![datadir(root), l.document_root, l.apache_error_log, l.apache_access_log, root.join(r"bin\phpmyadmin\config.inc.php")];
    keep.extend([l.mariadb_error_log, l.php_error_log, l.slow_query_log, l.general_query_log].into_iter().flatten());
    keep
}

/// `p` below `dir`, ignoring case like Windows: the rest of the path (empty for `dir` itself).
fn inside(p: &Path, dir: &Path) -> Option<PathBuf> {
    let mut rest = p.components();
    for c in dir.components() {
        let same = rest.next().is_some_and(|r| r.as_os_str().to_string_lossy().eq_ignore_ascii_case(&c.as_os_str().to_string_lossy()));
        if !same {
            return None;
        }
    }
    Some(rest.as_path().to_path_buf())
}

/// Puts a freshly unpacked package (`partial`) in place of `target`. An existing folder is renamed aside,
/// never deleted outright: whatever of `keep` lies inside it moves into the new folder, and the old one
/// is deleted only when nothing of that is left behind (else it stays next to it, and the log says so).
fn replace_package(partial: &Path, target: &Path, keep: &[PathBuf]) -> Result<(), String> {
    if !target.exists() {
        return std::fs::rename(partial, target).map_err(|e| format!("Could not move {} into place: {e}", target.display()));
    }
    if let Some(p) = keep.iter().find(|p| inside(p, target).is_some_and(|rest| rest.as_os_str().is_empty())) {
        return Err(format!("Lampion's configuration uses {} itself, so it cannot be replaced. Point that setting elsewhere, then try again.", p.display()));
    }
    let aside = PathBuf::from(format!("{}.old-{}", target.display(), chrono::Local::now().format("%Y%m%d-%H%M%S")));
    std::fs::rename(target, &aside)
        .map_err(|e| format!("Could not replace {}: {e}. Close the programs that use it, then try again.", target.display()))?;
    if let Err(e) = std::fs::rename(partial, target) {
        let _ = std::fs::rename(&aside, target);
        return Err(format!("Could not move {} into place: {e}", target.display()));
    }
    let mut keep: Vec<&PathBuf> = keep.iter().collect();
    keep.sort_by_key(|p| p.components().count()); // a folder before what is in it (a log in the data folder)
    let mut left = Vec::new();
    for rest in keep.into_iter().filter_map(|p| inside(p, target)) {
        let (from, to) = (aside.join(&rest), target.join(&rest));
        if !from.exists() {
            continue;
        }
        let moved = (|| {
            if to.is_dir() {
                std::fs::remove_dir_all(&to)?; // the package's own copy (htdocs, say) gives way to the user's
            } else if to.exists() {
                std::fs::remove_file(&to)?;
            }
            std::fs::create_dir_all(to.parent().unwrap_or(target))?;
            std::fs::rename(&from, &to)
        })();
        if let Err(e) = moved {
            left.push(format!("{} ({e})", from.display()));
        }
    }
    if !left.is_empty() {
        activity::warn("Setup", &format!("Kept the old folder {}: it still holds {}", aside.display(), left.join(", ")));
    } else if let Err(e) = std::fs::remove_dir_all(&aside) {
        activity::warn("Setup", &format!("The old {} could not be deleted ({e}); delete it yourself.", aside.display()));
    }
    Ok(())
}

/// Windows' time zone as PHP's IANA name ("Europe/Budapest", not "Central Europe Standard Time").
fn timezone() -> String {
    iana_time_zone::get_timezone()
        .ok()
        .and_then(|z| crate::config::timezones::find(&z))
        .unwrap_or("UTC")
        .to_string()
}

fn write_configs(root: &Path) -> Result<(), String> {
    step(root, "Writing the configuration", "", None, false);
    let config = root.join("config");
    if write_new(&config.join("httpd.conf"), &configs::httpd_conf(root, configs::APACHE_PORT))? {
        activity::info("Setup", "Wrote config\\httpd.conf (Apache listens on 127.0.0.1:8080)");
    }
    if write_new(&config.join("my.ini"), &configs::my_ini(root, configs::MARIADB_PORT))? {
        activity::info("Setup", "Wrote config\\my.ini (MariaDB listens on 127.0.0.1:3306)");
    }
    let ini = config.join("php.ini");
    if !ini.exists() {
        let php = root.join(r"bin\php");
        let template = ["php.ini-development", "php.ini-production"]
            .iter()
            .find_map(|f| std::fs::read_to_string(php.join(f)).ok())
            .ok_or("The PHP package has no php.ini-development to start from.")?;
        let tz = timezone();
        write_new(&ini, &configs::php_ini(&template, root, &tz))?;
        activity::info("Setup", &format!("Wrote config\\php.ini (time zone {tz})"));
    }
    Ok(())
}

fn create_datadir(root: &Path) -> Result<(), String> {
    let data = datadir(root);
    if data.join("mysql").is_dir() {
        return Ok(());
    }
    if data.is_dir() && !is_empty_dir(&data) {
        return Err(format!(
            "{} is not empty but has no MariaDB system database. Move its files away, then choose Try Again.",
            data.display()
        ));
    }
    step(root, "Creating MariaDB's data folder", "", None, false);
    let exe = root.join(r"bin\mariadb\bin\mariadb-install-db.exe");
    let out = shell::run_timeout(&exe, &[&format!("--datadir={}", data.display())], Duration::from_secs(300));
    if !out.ok() || !data.join("mysql").is_dir() {
        for line in last_lines(&out.out, 4) {
            activity::error("Setup", &format!("  {line}"));
        }
        return Err(format!("MariaDB could not create its data folder: {}", last_lines(&out.out, 1).join("")));
    }
    activity::ok("Setup", &format!("Created MariaDB's data folder in {}", data.display()));
    Ok(())
}

fn finish(root: &Path) {
    monitor::forget_versions();
    php::forget();
    activity::ok("Setup", "Setup complete. Starting the servers.");
    step(root, "Starting the servers", "", None, false);
    if !test_run() {
        pma_default();
        if tauri::async_runtime::block_on(control::start_all()).is_err() {
            activity::warn("Setup", "Setup finished, but a server did not start; the lines above say why.");
        }
    }
    update(|s| *s = SetupState::ready(root));
    php::log_summary();
    on_mariadb_running();
}

// ---- The Visual C++ runtime ----

/// The installed Visual C++ 2015–2026 x64 runtime, e.g. (14, 44, 35211); `None` when missing.
pub fn vc_runtime_version() -> Option<(u32, u32, u32)> {
    use winreg::enums::{HKEY_LOCAL_MACHINE, KEY_READ, KEY_WOW64_32KEY, KEY_WOW64_64KEY};
    let hklm = winreg::RegKey::predef(HKEY_LOCAL_MACHINE);
    [KEY_WOW64_64KEY, KEY_WOW64_32KEY].into_iter().find_map(|view| {
        let k = hklm.open_subkey_with_flags(r"SOFTWARE\Microsoft\VisualStudio\14.0\VC\Runtimes\x64", KEY_READ | view).ok()?;
        (k.get_value::<u32, _>("Installed").ok()? == 1).then(|| {
            let n = |name: &str| k.get_value::<u32, _>(name).unwrap_or(0);
            (n("Major"), n("Minor"), n("Bld"))
        })
    })
}

fn runtime_enough(installed: Option<(u32, u32, u32)>, min: &str) -> bool {
    match (installed, manifest::parse_version(min)) {
        (Some(v), Some(m)) => v >= m,
        (Some(_), None) => true,
        (None, _) => false,
    }
}

pub fn runtime_ok(vc: &VcRuntime) -> bool {
    runtime_enough(vc_runtime_version(), &vc.min_version)
}

fn install_runtime_now(root: &Path) -> Result<(), String> {
    let (manifest, _) = tauri::async_runtime::block_on(manifest::load());
    let vc = manifest.vc_runtime;
    if runtime_ok(&vc) {
        return Ok(());
    }
    let file = paths::local_data().join("downloads").join("vc_redist.x64.exe");
    step(root, "Downloading the Microsoft Visual C++ runtime", "", Some(0.0), true);
    let mut report = download_progress();
    tauri::async_runtime::block_on(download::fetch(&[vc.url.as_str()], &file, None, &CANCEL, &mut report))?;
    // It runs with administrator rights: only Microsoft's own, unchanged from the check until it has run.
    let held = trust::hold(&file).map_err(|e| format!("Could not open {}: {e}", file.display()))?;
    if !trust::signed_by_microsoft(&file) {
        drop(held);
        let _ = std::fs::remove_file(&file);
        return Err("The downloaded Visual C++ runtime installer is not validly signed by Microsoft, so Lampion did not run it.".into());
    }
    step(root, "Installing the Microsoft Visual C++ runtime", "Windows asks for permission to install it.", None, false);
    activity::info("Setup", "Installing the Microsoft Visual C++ runtime (Windows asks for permission)…");
    let code = elevate::run(&file, "/install /passive /norestart", Duration::from_secs(900));
    drop(held);
    let _ = std::fs::remove_file(&file);
    match code? {
        0 | 1638 => activity::ok("Setup", "The Visual C++ runtime is installed"),
        3010 => activity::warn("Setup", "The Visual C++ runtime is installed; Windows wants a restart to finish it"),
        elevate::DECLINED | 1602 => {
            return Err("The Visual C++ runtime was not installed: the Windows permission prompt was declined.".into())
        }
        c => return Err(format!("The Visual C++ runtime installer failed (exit code {c}).")),
    }
    if !runtime_ok(&vc) {
        return Err("Windows still reports an older Visual C++ runtime. Restart Windows, then choose Try Again.".into());
    }
    Ok(())
}

// ---- Logins, set up once (the user's later choice always stands) ----

/// Like XAMPP: root connects without a password, from this computer only. Done once, the first time
/// MariaDB runs after setup (the monitor calls this while MariaDB runs).
pub fn on_mariadb_running() {
    if ROOT_DEFAULT_TRIED.load(SeqCst) || test_run() || !ready() || settings::get().root_no_password.is_some() {
        return;
    }
    if ROOT_DEFAULT_TRIED.swap(true, SeqCst) {
        return;
    }
    let _ = std::thread::Builder::new().name("lampion-root-login".into()).spawn(|| {
        if let Err(e) = tauri::async_runtime::block_on(crate::config::access::set_root_no_password(true, None)) {
            activity::warn("MySQL", &format!("Could not let root connect without a password: {e}"));
        }
    });
}

/// phpMyAdmin opens without its login page (config.inc.php), once.
fn pma_default() {
    if settings::get().phpmyadmin_auto_login.is_some()
        || !paths::phpmyadmin_dir().join("index.php").is_file()
        || PMA_DEFAULT_TRIED.swap(true, SeqCst)
    {
        return;
    }
    if let Err(e) = crate::config::phpmyadmin::set_auto_login(true) {
        activity::warn("MySQL", &format!("Could not turn on phpMyAdmin's automatic sign-in: {e}"));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn folder_rules() {
        assert_eq!(check_folder(r" C:\Lampion\ ").unwrap(), PathBuf::from(r"C:\Lampion"));
        assert!(check_folder(r"D:\Dev\My Sites\Lampion").is_ok(), "spaces are fine");
        assert!(check_folder("").is_err());
        assert!(check_folder(r"C:\").unwrap_err().contains("whole drive"));
        assert!(check_folder(r"Lampion").unwrap_err().contains("full path"));
        assert!(check_folder(r"\\server\share\Lampion").unwrap_err().contains("network"));
        assert!(check_folder(r"C:\Program Files\Lampion").unwrap_err().contains("Program Files"));
        assert!(check_folder(r"C:\Users\a\OneDrive\Lampion").unwrap_err().contains("OneDrive"));
        assert!(check_folder(r"C:\Fejlesztés\Lampion").unwrap_err().contains("English letters"));
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("a.txt");
        std::fs::write(&file, "x").unwrap();
        assert!(check_folder(&file.to_string_lossy()).unwrap_err().contains("file"));
    }

    #[test]
    fn what_is_missing() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        assert_eq!(missing_parts(root), ["Apache", "PHP", "MariaDB", "phpMyAdmin", "the configuration", "the database files"]);
        assert!(nothing_installed(root));
        for f in [r"bin\apache\bin\httpd.exe", r"bin\php\php.exe", r"bin\php\php8apache2_4.dll", r"bin\phpmyadmin\index.php",
                  r"config\httpd.conf", r"config\php.ini", r"databases\mysql\db.opt"] {
            let p = root.join(f);
            std::fs::create_dir_all(p.parent().unwrap()).unwrap();
            std::fs::write(&p, "").unwrap();
        }
        assert!(!nothing_installed(root));
        assert_eq!(missing_parts(root), ["MariaDB", "the configuration"]);
        // The data folder comes from my.ini.
        std::fs::write(root.join(r"config\my.ini"), "[mysqld]\r\ndatadir=D:/elsewhere/data\r\n").unwrap();
        assert_eq!(datadir(root), PathBuf::from(r"D:\elsewhere\data"));
        assert!(missing_parts(root).contains(&"the database files".to_string()));
        std::fs::write(root.join(r"config\my.ini"), "[mysqld]\r\ndatadir=data\r\n").unwrap();
        assert_eq!(datadir(root), root.join(r"bin\mariadb\data"), "relative to MariaDB's base folder");
        let st = SetupState::missing(root, vec!["the configuration".into()]);
        assert_eq!(st.title, "The server setup is not complete");
        let st = SetupState::missing(root, vec!["PHP".into(), "MariaDB".into(), "the configuration".into()]);
        assert_eq!(st.detail, "Try Again downloads PHP and MariaDB and sets everything up.");
        assert!(!st.can_change_root);
    }

    #[test]
    fn layout_and_new_files_only() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join("Lampion");
        create_layout(&root).unwrap();
        for d in ["bin", "config", "www", "databases", r"logs\apache", r"logs\mariadb", r"logs\php", "backups", "tmp"] {
            assert!(root.join(d).is_dir(), "{d}");
        }
        assert!(std::fs::read_to_string(root.join("README.txt")).unwrap().starts_with("Lampion keeps"));
        let f = root.join(r"config\my.ini");
        assert!(write_new(&f, "first").unwrap());
        assert!(!write_new(&f, "second").unwrap(), "the user's file is never overwritten");
        assert_eq!(std::fs::read_to_string(&f).unwrap(), "first");
    }

    #[test]
    fn a_repair_keeps_what_the_configuration_points_into() {
        let dir = tempfile::tempdir().unwrap();
        let bin = dir.path().join("bin");
        let (target, partial) = (bin.join("mariadb"), bin.join("mariadb.partial"));
        let put = |p: PathBuf, text: &str| {
            std::fs::create_dir_all(p.parent().unwrap()).unwrap();
            std::fs::write(p, text).unwrap();
        };
        let read = |p: PathBuf| std::fs::read_to_string(p).unwrap();
        put(target.join(r"bin\mariadbd.exe"), "old");
        put(target.join(r"data\mysql\user.frm"), "my data");
        put(target.join(r"data\PC.err"), "my log");
        put(target.join(r"htdocs\index.php"), "my site");
        put(partial.join(r"bin\mariadbd.exe"), "new");
        put(partial.join(r"htdocs\index.html"), "the package's page");
        let keep = [target.join(r"data\PC.err"), target.join("data"), target.join("HTDOCS"), dir.path().join(r"logs\x.log")];
        replace_package(&partial, &target, &keep).unwrap();
        assert_eq!(read(target.join(r"bin\mariadbd.exe")), "new");
        assert_eq!(read(target.join(r"data\mysql\user.frm")), "my data");
        assert_eq!(read(target.join(r"data\PC.err")), "my log");
        assert_eq!(read(target.join(r"htdocs\index.php")), "my site");
        assert!(!target.join(r"htdocs\index.html").exists(), "the user's folder, not the package's");
        assert!(!partial.exists());
        assert_eq!(std::fs::read_dir(&bin).unwrap().count(), 1, "the old folder goes once nothing in it is needed");

        // A setting that names the package folder itself: nothing is touched.
        put(partial.join(r"bin\mariadbd.exe"), "newer");
        let itself = PathBuf::from(target.to_string_lossy().to_uppercase());
        assert!(replace_package(&partial, &target, &[itself]).unwrap_err().contains("cannot be replaced"));
        assert_eq!(read(target.join(r"bin\mariadbd.exe")), "new");
        assert_eq!(read(target.join(r"data\mysql\user.frm")), "my data");

        // Nothing there yet: the new folder simply moves in.
        let fresh = bin.join("php");
        replace_package(&partial, &fresh, &[]).unwrap();
        assert_eq!(read(fresh.join(r"bin\mariadbd.exe")), "newer");
    }

    #[test]
    fn paths_inside_folders_like_windows() {
        let data = Path::new(r"C:\L\bin\mariadb\data\mysql");
        assert_eq!(inside(data, Path::new(r"c:\l\BIN\mariadb")), Some(PathBuf::from(r"data\mysql")));
        assert_eq!(inside(Path::new(r"C:\L\bin\mariadb"), Path::new(r"C:\L\bin\mariadb")), Some(PathBuf::new()));
        assert_eq!(inside(Path::new(r"C:\L\bin\mariadb2\x"), Path::new(r"C:\L\bin\mariadb")), None);
        assert_eq!(inside(Path::new(r"C:\L"), Path::new(r"C:\L\bin")), None);
    }

    #[test]
    fn runtime_versions() {
        assert!(runtime_enough(Some((14, 50, 35719)), "14.50"));
        assert!(runtime_enough(Some((14, 52, 1)), "14.50"));
        assert!(!runtime_enough(Some((14, 44, 35211)), "14.50"));
        assert!(!runtime_enough(None, "14.50"));
    }

    #[test]
    fn names_read_well() {
        let s = |v: &[&str]| list(&v.iter().map(|x| x.to_string()).collect::<Vec<_>>());
        assert_eq!(s(&[]), "");
        assert_eq!(s(&["Apache"]), "Apache");
        assert_eq!(s(&["Apache", "PHP", "MariaDB"]), "Apache, PHP and MariaDB");
    }

    #[test]
    fn banner_payload_shape() {
        let json = serde_json::to_value(SetupState::running(Path::new(r"C:\Lampion"), "Downloading PHP 8.4.28 (2 of 4)", "1.0 of 33.9 MB", Some(0.03), true)).unwrap();
        assert_eq!(json["phase"], "running");
        assert_eq!(json["root"], r"C:\Lampion");
        assert_eq!(json["canCancel"], true);
        assert_eq!(json["canChangeRoot"], false);
        assert_eq!(serde_json::to_value(Phase::ChooseFolder).unwrap(), "chooseFolder");
        assert_eq!(serde_json::to_value(Phase::NeedsRuntime).unwrap(), "needsRuntime");
    }
}
