//! App settings in `%APPDATA%\Lampion\settings.json`. One struct for the whole app; unknown keys
//! are kept (`extra`) so an older build never drops a newer build's settings.
//! A module that needs a new setting adds a field here with `#[serde(default)]`.

use once_cell::sync::Lazy;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::{Mutex, MutexGuard};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct Settings {
    /// The Lampion folder; empty = `C:\Lampion`.
    pub lampion_root: String,
    pub open_at_login: bool,
    pub start_servers_on_open: bool,
    pub stop_servers_on_quit: bool,
    pub check_for_updates: bool,
    /// "Skip This Version" (e.g. "1.2").
    pub skipped_version: Option<String>,
    /// root without a password; `None` until first set up, then the user's choice is kept.
    pub root_no_password: Option<bool>,
    /// phpMyAdmin automatic sign-in; same rule as above.
    pub phpmyadmin_auto_login: Option<bool>,
    /// Run the servers as Windows services instead of background processes.
    pub use_services: bool,
    #[serde(flatten)]
    pub extra: serde_json::Map<String, serde_json::Value>,
}

impl Default for Settings {
    fn default() -> Self {
        Settings {
            lampion_root: String::new(),
            open_at_login: false,
            start_servers_on_open: true,
            stop_servers_on_quit: false,
            check_for_updates: true,
            skipped_version: None,
            root_no_password: None,
            phpmyadmin_auto_login: None,
            use_services: false,
            extra: Default::default(),
        }
    }
}

static SETTINGS: Lazy<Mutex<Settings>> = Lazy::new(|| Mutex::new(Settings::default()));

fn lock() -> MutexGuard<'static, Settings> {
    SETTINGS.lock().unwrap_or_else(|e| e.into_inner())
}

#[cfg(not(test))]
fn file() -> PathBuf {
    super::paths::app_data().join("settings.json")
}

/// Unit tests never touch the user's own settings.
#[cfg(test)]
fn file() -> PathBuf {
    std::env::temp_dir().join(format!("lampion-test-settings-{}.json", std::process::id()))
}

pub fn load() {
    let (s, problem) = load_from(&file());
    *lock() = s;
    if let Some(p) = problem {
        super::activity::warn("Panel", &p);
    }
}

/// The settings in `path` (defaults when there is none). A file that cannot be read as settings is
/// renamed to `settings.json.bad-<time>` and never overwritten; the second value says so for the log.
fn load_from(path: &Path) -> (Settings, Option<String>) {
    let bytes = match std::fs::read(path) {
        Ok(b) => b,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return (Settings::default(), None),
        Err(e) => return (Settings::default(), Some(format!("Could not read {}: {e}. Lampion uses its default settings.", path.display()))),
    };
    match serde_json::from_slice::<Settings>(&bytes) {
        Ok(s) => (s, None),
        Err(e) => {
            let bad = PathBuf::from(format!("{}.bad-{}", path.display(), chrono::Local::now().format("%Y%m%d-%H%M%S")));
            let note = match std::fs::rename(path, &bad) {
                Ok(()) => format!("settings.json could not be read ({e}); it was kept as {} and Lampion uses its default settings.", bad.display()),
                Err(r) => format!("settings.json could not be read ({e}) or set aside ({r}); Lampion uses its default settings."),
            };
            (Settings::default(), Some(note))
        }
    }
}

/// Writes `s` through a temp file of its own (another write, even from another Lampion, never shares
/// it) and a rename, so the file is never half-written.
fn write_to(path: &Path, s: &Settings) -> Result<(), String> {
    static N: AtomicU32 = AtomicU32::new(0);
    let json = serde_json::to_string_pretty(s).map_err(|e| e.to_string())?;
    let tmp = PathBuf::from(format!("{}.{}-{}.tmp", path.display(), std::process::id(), N.fetch_add(1, Ordering::Relaxed)));
    std::fs::write(&tmp, json).and_then(|()| std::fs::rename(&tmp, path)).map_err(|e| {
        let _ = std::fs::remove_file(&tmp);
        format!("Could not save the settings to {}: {e}", path.display())
    })
}

pub fn get() -> Settings {
    lock().clone()
}

/// Changes settings and writes them to disk at once (atomically: temp file + rename).
pub fn update(f: impl FnOnce(&mut Settings)) -> Settings {
    try_update(|s| {
        f(s);
        Ok(())
    })
    .unwrap_or_else(|_| get())
}

/// Like `update`, but `f` may refuse the change (then nothing changes). The change is made and written
/// while the settings are locked, so two changes at once never lose one another or land out of order.
pub fn try_update(f: impl FnOnce(&mut Settings) -> Result<(), String>) -> Result<Settings, String> {
    let mut g = lock();
    let mut next = g.clone();
    f(&mut next)?;
    let written = write_to(&file(), &next);
    *g = next.clone();
    super::emit("settings://changed", next.clone());
    drop(g);
    if let Err(e) = written {
        super::activity::warn("Panel", &e);
    }
    Ok(next)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_unreadable_file_is_kept_aside_not_overwritten() {
        let dir = tempfile::tempdir().unwrap();
        let f = dir.path().join("settings.json");
        assert!(load_from(&f).1.is_none(), "no file yet: defaults, nothing to say");
        std::fs::write(&f, "{ \"lampionRoot\": \"D:\\\\Lampion\", ").unwrap();
        let (s, note) = load_from(&f);
        assert_eq!(s.lampion_root, "", "the defaults");
        assert!(note.unwrap().contains("settings.json.bad-"));
        assert!(!f.exists());
        let kept: Vec<_> = std::fs::read_dir(dir.path()).unwrap().map(|e| e.unwrap().file_name().to_string_lossy().into_owned()).collect();
        assert!(kept.len() == 1 && kept[0].starts_with("settings.json.bad-"), "{kept:?}");
    }

    #[test]
    fn writes_are_whole_and_keep_unknown_keys() {
        let dir = tempfile::tempdir().unwrap();
        let f = dir.path().join("settings.json");
        std::fs::write(&f, r#"{"lampionRoot":"D:\\L","futureSetting":{"a":1}}"#).unwrap();
        let (mut s, _) = load_from(&f);
        s.check_for_updates = false;
        write_to(&f, &s).unwrap();
        let (again, note) = load_from(&f);
        assert!(note.is_none());
        assert_eq!((again.lampion_root.as_str(), again.check_for_updates), (r"D:\L", false));
        assert_eq!(again.extra["futureSetting"]["a"], 1);
        assert_eq!(std::fs::read_dir(dir.path()).unwrap().count(), 1, "no temp file left");
    }

    #[test]
    fn changes_at_the_same_time_are_all_kept() {
        let threads: Vec<_> = (0..8)
            .map(|i| {
                std::thread::spawn(move || {
                    update(|s| {
                        s.extra.insert(format!("test{i}"), serde_json::Value::Bool(true));
                    })
                })
            })
            .collect();
        threads.into_iter().for_each(|t| drop(t.join().unwrap()));
        let (on_disk, _) = load_from(&file());
        for i in 0..8 {
            assert!(get().extra.contains_key(&format!("test{i}")));
            assert!(on_disk.extra.contains_key(&format!("test{i}")), "the file has every change too");
        }
        assert!(try_update(|_| Err("no".into())).is_err());
        let _ = std::fs::remove_file(file());
    }
}
