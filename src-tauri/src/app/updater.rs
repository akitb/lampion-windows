//! Updates from GitHub releases (docs/SPEC.md "Frissítés és terjesztés", Updater.swift). The newest
//! release is offered in the update banner (`update://state`); its installer is downloaded with a
//! cancellable progress bar to `%LOCALAPPDATA%\Lampion\updates`, checked against the SHA-256 GitHub lists
//! for it and against its Authenticode signature when it is signed, then run silently while Lampion
//! quits (`/S /UPDATE /R`: no new shortcuts, and the installer starts the new Lampion). The servers
//! keep running.

use super::{quit, trust};
use crate::core::{activity, emit, paths, settings, shell};
use futures_util::StreamExt;
use once_cell::sync::Lazy;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::io::Write;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering::SeqCst};
use std::sync::{Mutex, MutexGuard};
use std::time::Duration;

/// The running version.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");
const USER_AGENT: &str = "Lampion-Updater";
const CANCELLED: &str = "The download was cancelled.";

/// The repository whose releases update Lampion: `LAMPION_REPO` at build time, so a fork updates from its own.
fn repo() -> &'static str {
    crate::servers::manifest::repo()
}

pub fn releases_page() -> String {
    format!("https://github.com/{}/releases", repo())
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Release {
    pub version: String,
    /// The release on GitHub ("View on GitHub").
    pub page: String,
    /// Only the "What's new" part of the release notes.
    pub notes: String,
    #[serde(skip)]
    pub url: String,
    /// The installer's SHA-256 as GitHub lists it (`digest`), lowercase hex.
    #[serde(skip)]
    pub sha256: Option<String>,
}

/// What the update banner shows (`UpdateState` in src/lib/types.ts).
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(tag = "state", rename_all = "camelCase")]
pub enum State {
    Idle,
    Checking,
    UpToDate,
    Available { release: Release },
    /// `progress`: 0…1, in whole percents.
    Downloading { release: Release, progress: f64 },
    Installing { release: Release },
    /// `page`: the releases page ("Download Manually").
    Failed { message: String, page: String },
}

fn failed(message: String) -> State {
    State::Failed { message, page: releases_page() }
}

static STATE: Lazy<Mutex<State>> = Lazy::new(|| Mutex::new(State::Idle));
static CANCEL: AtomicBool = AtomicBool::new(false);

fn lock() -> MutexGuard<'static, State> {
    STATE.lock().unwrap_or_else(|e| e.into_inner())
}

pub fn state() -> State {
    lock().clone()
}

/// Changes the state; the banner hears about real changes only (sent under the lock, so in order).
fn set_locked(s: &mut State, next: State) {
    if *s != next {
        *s = next.clone();
        emit("update://state", next);
    }
}

fn set(next: State) {
    set_locked(&mut lock(), next);
}

/// True when `a` is a higher version than `b`, number by number ("1.10" is newer than "1.9"; `v` ignored).
pub fn is_newer(a: &str, b: &str) -> bool {
    let parts = |v: &str| -> Vec<u64> {
        v.trim_matches(|c| c == 'v' || c == 'V' || c == ' ').split('.').map(|p| p.parse().unwrap_or(0)).collect()
    };
    let (x, y) = (parts(a), parts(b));
    for i in 0..x.len().max(y.len()) {
        let (l, r) = (x.get(i).copied().unwrap_or(0), y.get(i).copied().unwrap_or(0));
        if l != r {
            return l > r;
        }
    }
    false
}

/// The "What's new" part of release notes: everything before the install instructions.
pub fn highlights(notes: &str) -> String {
    let mut text = notes.replace("\r\n", "\n");
    for heading in ["\n## Install", "\n## Requirements", "\nSHA-256"] {
        if let Some(i) = text.find(heading) {
            text.truncate(i);
        }
    }
    text.trim().to_string()
}

#[derive(Deserialize)]
struct GitHubRelease {
    tag_name: String,
    #[serde(default)]
    html_url: Option<String>,
    #[serde(default)]
    body: Option<String>,
    #[serde(default)]
    assets: Vec<GitHubAsset>,
}

#[derive(Deserialize)]
struct GitHubAsset {
    name: String,
    browser_download_url: String,
    #[serde(default)]
    digest: Option<String>,
}

/// The latest release and its installer (`Lampion-X.Y-setup.exe`) from GitHub's JSON.
pub fn parse_release(json: &str) -> Option<Release> {
    let r: GitHubRelease = serde_json::from_str(json).ok()?;
    let asset = r
        .assets
        .iter()
        .find(|a| a.name.to_ascii_lowercase().ends_with("-setup.exe") && a.browser_download_url.starts_with("https://"))?;
    Some(Release {
        version: r.tag_name.trim_matches(|c| c == 'v' || c == 'V' || c == ' ').to_string(),
        page: r.html_url.filter(|u| u.starts_with("https://")).unwrap_or_else(releases_page),
        notes: highlights(r.body.as_deref().unwrap_or("")),
        url: asset.browser_download_url.clone(),
        sha256: asset.digest.as_deref().and_then(|d| d.strip_prefix("sha256:")).map(str::to_ascii_lowercase),
    })
}

/// Refuses an installer whose SHA-256 differs from the one GitHub lists for it.
pub fn verify_digest(actual: &str, expected: Option<&str>) -> Result<(), String> {
    match expected {
        Some(e) if !actual.trim().eq_ignore_ascii_case(e.trim()) => {
            Err("The download does not match the release (checksum mismatch), so it was not installed.".into())
        }
        Some(_) => Ok(()),
        None => {
            activity::info("Update", "GitHub listed no checksum for the installer, so it could not be compared.");
            Ok(())
        }
    }
}

fn client(timeout: Option<Duration>) -> Result<reqwest::Client, String> {
    let b = reqwest::Client::builder().user_agent(USER_AGENT).connect_timeout(Duration::from_secs(15));
    match timeout {
        Some(t) => b.timeout(t),
        None => b.read_timeout(Duration::from_secs(60)),
    }
    .build()
    .map_err(|e| e.to_string())
}

async fn latest() -> Result<Release, String> {
    let url = format!("https://api.github.com/repos/{}/releases/latest", repo());
    let resp = client(Some(Duration::from_secs(15)))?
        .get(&url)
        .header(reqwest::header::ACCEPT, "application/vnd.github+json")
        .send()
        .await
        .map_err(|e| format!("GitHub could not be reached ({e})."))?;
    match resp.status() {
        s if s.is_success() => {}
        reqwest::StatusCode::NOT_FOUND => return Err("No release has been published yet.".into()),
        s => return Err(format!("GitHub answered {s}.")),
    }
    let text = resp.text().await.map_err(|e| e.to_string())?;
    parse_release(&text).ok_or_else(|| "GitHub did not return a release with an installer.".into())
}

/// Looks for a newer release. `user`: Check Now (General page, or Try Again): the answer always shows,
/// "up to date" and errors included, and a skipped version is offered again.
pub async fn check(user: bool) -> Result<(), String> {
    {
        let mut s = lock();
        if matches!(*s, State::Checking | State::Downloading { .. } | State::Installing { .. }) {
            return Ok(());
        }
        if user {
            set_locked(&mut s, State::Checking);
        }
    }
    let found = latest().await;
    let mut s = lock();
    if !user && *s != State::Idle {
        return Ok(()); // the automatic check never overrides what the user started meanwhile
    }
    match found {
        Err(e) => {
            if !user {
                set_locked(&mut s, State::Idle);
                return Ok(());
            }
            let message = format!("Could not check for updates. {e}");
            activity::warn("Update", &message);
            set_locked(&mut s, failed(message.clone()));
            Err(message)
        }
        Ok(release) => {
            let skipped = settings::get().skipped_version;
            if is_newer(&release.version, VERSION) && (user || skipped.as_deref() != Some(release.version.as_str())) {
                activity::info("Update", &format!("Lampion {} is available", release.version));
                set_locked(&mut s, State::Available { release });
            } else if user {
                set_locked(&mut s, State::UpToDate);
                tauri::async_runtime::spawn(async {
                    tokio::time::sleep(Duration::from_secs(4)).await;
                    let mut s = lock();
                    if *s == State::UpToDate {
                        set_locked(&mut s, State::Idle);
                    }
                });
            } else {
                set_locked(&mut s, State::Idle);
            }
            Ok(())
        }
    }
}

/// Hides the banner until the next launch.
pub fn later() {
    let mut s = lock();
    if matches!(*s, State::Available { .. } | State::UpToDate | State::Failed { .. }) {
        set_locked(&mut s, State::Idle);
    }
}

/// Hides the offer until a newer version than this one comes out.
pub fn skip() {
    let mut s = lock();
    if let State::Available { release } = &*s {
        let version = release.version.clone();
        settings::update(|st| st.skipped_version = Some(version));
        set_locked(&mut s, State::Idle);
    }
}

pub fn cancel() {
    let mut s = lock();
    if let State::Downloading { release, .. } = &*s {
        let release = release.clone();
        CANCEL.store(true, SeqCst);
        set_locked(&mut s, State::Available { release });
    }
}

/// Downloads, checks and runs the installer of the offered release.
pub fn install() -> Result<(), String> {
    let release = {
        let mut s = lock();
        let State::Available { release } = &*s else {
            return Err("There is no update to install.".into());
        };
        let release = release.clone();
        CANCEL.store(false, SeqCst);
        set_locked(&mut s, State::Downloading { release: release.clone(), progress: 0.0 });
        release
    };
    tauri::async_runtime::spawn(async move {
        match download(&release).await {
            Ok(file) => run_installer(release, file).await,
            Err(e) if e == CANCELLED => {}
            Err(e) => fail(e),
        }
    });
    Ok(())
}

fn fail(message: String) {
    activity::error("Update", &message);
    set(failed(message));
}

fn progress(fraction: f64) {
    let p = (fraction.clamp(0.0, 1.0) * 100.0).floor() / 100.0;
    let mut s = lock();
    if let State::Downloading { release, progress } = &*s {
        if *progress != p {
            let release = release.clone();
            set_locked(&mut s, State::Downloading { release, progress: p });
        }
    }
}

/// The installer's file name, from the version (never from the network as is).
fn installer_name(version: &str) -> String {
    let safe: String = version.chars().filter(|c| c.is_ascii_digit() || *c == '.').collect();
    format!("Lampion-{}-setup.exe", if safe.is_empty() { "update" } else { &safe })
}

async fn download(release: &Release) -> Result<PathBuf, String> {
    let dir = paths::local_data().join("updates");
    std::fs::create_dir_all(&dir).map_err(|e| format!("Could not create {}: {e}", dir.display()))?;
    let file = dir.join(installer_name(&release.version));
    let part = file.with_extension("part");
    let resp = client(None)?.get(&release.url).send().await.map_err(|e| format!("The download failed. {e}"))?;
    if !resp.status().is_success() {
        return Err(format!("The download failed (GitHub answered {}).", resp.status()));
    }
    let total = resp.content_length().filter(|t| *t > 0);
    let mut out = std::fs::File::create(&part).map_err(|e| format!("Could not write {}: {e}", part.display()))?;
    let mut hasher = Sha256::new();
    let mut done = 0u64;
    let mut stream = resp.bytes_stream();
    let written: Result<(), String> = async {
        while let Some(chunk) = stream.next().await {
            if CANCEL.load(SeqCst) {
                return Err(CANCELLED.to_string());
            }
            let chunk = chunk.map_err(|e| format!("The download broke off. {e}"))?;
            out.write_all(&chunk).map_err(|e| format!("Could not write {}: {e}", part.display()))?;
            hasher.update(&chunk);
            done += chunk.len() as u64;
            if let Some(total) = total {
                progress(done as f64 / total as f64);
            }
        }
        out.flush().map_err(|e| e.to_string())
    }
    .await;
    drop(out);
    let checked = written.and_then(|()| verify_digest(&hex::encode(hasher.finalize()), release.sha256.as_deref()));
    if let Err(e) = checked {
        let _ = std::fs::remove_file(&part);
        return Err(e);
    }
    let _ = std::fs::remove_file(&file);
    std::fs::rename(&part, &file).map_err(|e| format!("Could not save {}: {e}", file.display()))?;
    Ok(file)
}

async fn run_installer(release: Release, file: PathBuf) {
    if CANCEL.load(SeqCst) {
        let _ = std::fs::remove_file(&file);
        return; // cancelled in the last moment: the offer stays
    }
    set(State::Installing { release: release.clone() });
    let path = file.clone();
    match tauri::async_runtime::spawn_blocking(move || trust::check(&path)).await {
        Ok(trust::Signature::Valid) => activity::info("Update", "The installer's signature is valid"),
        Ok(trust::Signature::Unsigned) => {
            activity::info("Update", "The installer is not signed; Lampion checked it by its SHA-256 checksum")
        }
        Ok(trust::Signature::Invalid(code)) => {
            let _ = std::fs::remove_file(&file);
            return fail(format!("The downloaded installer is damaged: its signature is not valid (0x{code:08X})."));
        }
        Err(e) => return fail(e.to_string()),
    }
    activity::info("Update", &format!("Installing Lampion {}; Lampion opens again in a moment", release.version));
    // The servers keep running: "Stop servers when Lampion quits" does not apply to an update.
    quit::set_relaunching(true);
    if let Err(e) = shell::spawn_detached(&file, &["/S", "/UPDATE", "/R"], None) {
        quit::set_relaunching(false);
        return fail(format!("Could not start the installer: {e}"));
    }
    // A short pause so "Installing" can be read before the window closes.
    tokio::time::sleep(Duration::from_millis(800)).await;
    if let Some(app) = crate::core::app() {
        app.exit(0);
    }
}

/// `--preview-update available|notes|downloading|installing|uptodate|failed`: the banner in one state,
/// without GitHub (for checking its look).
fn preview(name: &str) {
    let release = Release {
        version: "2.0".into(),
        page: releases_page(),
        notes: highlights(
            "## What's new\n\n- **Table structure editing.** A table designer for new tables; change, rename and reorder \
columns; add indexes and foreign keys.\n- **Built-in updates.** Lampion checks GitHub for a newer release and updates \
itself with one click.\n- The Structure and Query tabs now stay selected when you move between tables.\n\n## Install\n\n1. Download the installer.",
        ),
        url: String::new(),
        sha256: None,
    };
    set(match name {
        "available" | "notes" => State::Available { release },
        "downloading" => State::Downloading { release, progress: 0.62 },
        "installing" => State::Installing { release },
        "uptodate" => State::UpToDate,
        "failed" => failed("The download does not match the release (checksum mismatch), so it was not installed.".into()),
        _ => State::Idle,
    });
}

/// At launch: removes installers left from an earlier update, then shows a preview (`--preview-update`)
/// or checks once for a newer release (not during a self-test). `--auto-update` installs what it finds.
pub fn start(args: &[String]) {
    std::thread::spawn(|| {
        let _ = std::fs::remove_dir_all(paths::local_data().join("updates"));
    });
    if let Some(i) = args.iter().position(|a| a == "--preview-update") {
        preview(args.get(i + 1).map(String::as_str).unwrap_or(""));
        return;
    }
    let auto = args.iter().any(|a| a == "--auto-update");
    let testing = args.iter().any(|a| a == "--selftest" || a == "--dbtest");
    if auto || (settings::get().check_for_updates && !testing) {
        tauri::async_runtime::spawn(async move {
            let _ = check(false).await;
            if auto && matches!(state(), State::Available { .. }) {
                let _ = install();
            }
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn compares_version_numbers() {
        assert!(is_newer("1.10", "1.9"));
        assert!(is_newer("v2.0", "1.9.9"));
        assert!(is_newer("1.3.1", "1.3"));
        assert!(is_newer("1.3", "1.2.0"));
        assert!(!is_newer("1.3", "1.3"));
        assert!(!is_newer("1.2", "1.3"));
        assert!(!is_newer("1.2", "1.2.0"), "a missing part counts as 0");
        assert!(!is_newer("V1.0", "1.0.0"));
    }

    const SAMPLE: &str = r###"{"tag_name":"v9.9","html_url":"https://github.com/akitb/lampion-windows/releases/tag/v9.9",
        "body":"## What's new\r\n\r\n- Faster logs\r\n\r\n## Install\r\n\r\n1. Run it.\r\n\r\nSHA-256 of the installer: `abc`",
        "assets":[{"name":"manifest.json","browser_download_url":"https://example.com/manifest.json"},
        {"name":"Lampion-9.9-setup.exe","browser_download_url":"https://example.com/Lampion-9.9-setup.exe",
         "digest":"sha256:ABC123"}]}"###;

    #[test]
    fn reads_the_release_and_its_installer() {
        let r = parse_release(SAMPLE).unwrap();
        assert_eq!(r.version, "9.9");
        assert_eq!(r.page, "https://github.com/akitb/lampion-windows/releases/tag/v9.9");
        assert_eq!(r.url, "https://example.com/Lampion-9.9-setup.exe", "the installer, not the manifest");
        assert_eq!(r.sha256.as_deref(), Some("abc123"));
        assert_eq!(r.notes, "## What's new\n\n- Faster logs", "only the What's new part");
        assert!(parse_release("{}").is_none());
        assert!(parse_release(r#"{"tag_name":"v1.2","assets":[{"name":"notes.txt","browser_download_url":"https://x/n"}]}"#).is_none());
        let http = SAMPLE.replace("https://example.com/Lampion", "http://example.com/Lampion");
        assert!(parse_release(&http).is_none(), "only https downloads");
        let no_digest = parse_release(&SAMPLE.replace(r#""digest":"sha256:ABC123""#, r#""label":"x""#)).unwrap();
        assert_eq!(no_digest.sha256, None);
    }

    #[test]
    fn refuses_a_download_with_another_checksum() {
        let good = hex::encode(Sha256::digest(b"installer"));
        assert!(verify_digest(&good, Some(&good.to_uppercase())).is_ok());
        assert!(verify_digest(&good, Some(&hex::encode(Sha256::digest(b"tampered")))).is_err());
        assert!(verify_digest(&good, None).is_ok(), "no digest listed: nothing to compare");
    }

    #[test]
    fn installer_names_never_come_from_the_network() {
        assert_eq!(installer_name("1.2"), "Lampion-1.2-setup.exe");
        assert_eq!(installer_name("..\\..\\evil"), "Lampion-....-setup.exe");
        assert_eq!(installer_name("x"), "Lampion-update-setup.exe");
    }

    #[test]
    fn the_banner_state_as_the_ui_reads_it() {
        let v = serde_json::to_value(State::Downloading {
            release: Release { version: "2.0".into(), page: "p".into(), notes: "n".into(), url: "u".into(), sha256: None },
            progress: 0.5,
        })
        .unwrap();
        assert_eq!(v, serde_json::json!({ "state": "downloading", "release": { "version": "2.0", "page": "p", "notes": "n" }, "progress": 0.5 }));
        assert_eq!(serde_json::to_value(State::UpToDate).unwrap(), serde_json::json!({ "state": "upToDate" }));
    }
}
