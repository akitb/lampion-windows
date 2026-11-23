//! The Logs page's backend (docs/SPEC.md "Naplók"). Lampion Activity lives in `core::activity`; each
//! server log the page shows is followed on its own background thread once a second (`tail`), its lines
//! split by `parse`. The page hears about real changes only (`logs://update`).

pub mod commands;
pub mod parse;
pub mod tail;

pub use parse::{Line, Source};

use crate::config::locations;
use crate::core::emit;
use once_cell::sync::Lazy;
use serde::Serialize;
use std::collections::{HashMap, VecDeque};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering::SeqCst};
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::Duration;
use tail::Tail;

/// Lines kept per log.
pub const MAX_LINES: usize = 5000;
/// The configured file names are read again this often (in seconds), for hand-edited configs.
const PATH_EVERY: u32 = 5;

/// One log as the page shows it: the whole state from `logs_follow` / `logs_reload`, then the changes
/// as `logs://update` (`LogUpdate` in src/lib/types.ts).
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Update {
    pub source: Source,
    /// Grows with every update of every log: the page ignores what is not newer than what it has.
    pub seq: u64,
    /// `lines` replace the list (first read, file emptied, replaced, jumped ahead or gone).
    pub reset: bool,
    pub lines: Vec<Line>,
    pub exists: bool,
    pub size: u64,
    pub path: String,
}

static SEQ: AtomicU64 = AtomicU64::new(0);
static LINE_ID: AtomicU64 = AtomicU64::new(0);

struct Follower {
    state: Mutex<State>,
    stop: AtomicBool,
}

struct State {
    source: Source,
    path: PathBuf,
    tail: Tail,
    lines: VecDeque<Line>,
    exists: bool,
    size: u64,
    seq: u64,
    /// The configuration names another file: the next poll starts over.
    moved: bool,
}

impl State {
    fn new(source: Source) -> State {
        State {
            source,
            path: source.path(&locations::current()),
            tail: Tail::default(),
            lines: VecDeque::new(),
            exists: false,
            size: 0,
            seq: 0,
            moved: false,
        }
    }

    /// Follows the file the configuration names now (a hand-edited httpd.conf, my.ini or php.ini).
    fn retarget(&mut self) {
        let path = self.source.path(&locations::current());
        if path != self.path {
            self.path = path;
            self.tail = Tail::default();
            self.moved = true;
        }
    }

    /// One look at the file. `force`: report even when nothing changed (the page wants the whole state).
    fn poll(&mut self, force: bool) -> Option<Update> {
        let force = force || std::mem::take(&mut self.moved);
        let Some(read) = self.tail.poll(&self.path) else {
            if !self.exists && !force {
                return None;
            }
            self.exists = false;
            self.size = 0;
            self.lines.clear();
            return Some(self.update(true, Vec::new()));
        };
        // Only the newest lines are kept, so only they are worth splitting.
        let skip = read.lines.len().saturating_sub(MAX_LINES);
        let new: Vec<Line> = read
            .lines
            .into_iter()
            .skip(skip)
            .map(|text| parse::parse(self.source, text, LINE_ID.fetch_add(1, SeqCst) + 1))
            .collect();
        let changed = force || read.reset || !new.is_empty() || !self.exists || read.size != self.size;
        if !changed {
            return None; // an unchanged size must not redraw the page every second
        }
        let reset = read.reset || force || !self.exists;
        self.exists = true;
        self.size = read.size;
        if reset {
            self.lines.clear();
        }
        self.lines.extend(new.iter().cloned());
        let excess = self.lines.len().saturating_sub(MAX_LINES);
        self.lines.drain(..excess);
        let lines = if reset { self.lines.iter().cloned().collect() } else { new };
        Some(self.update(reset, lines))
    }

    fn update(&mut self, reset: bool, lines: Vec<Line>) -> Update {
        self.seq = SEQ.fetch_add(1, SeqCst) + 1;
        Update {
            source: self.source,
            seq: self.seq,
            reset,
            lines,
            exists: self.exists,
            size: self.size,
            path: self.path.display().to_string(),
        }
    }

    /// Reads the file again from scratch.
    fn reload(&mut self) -> Update {
        self.retarget();
        self.tail = Tail::default();
        self.poll(true).expect("a forced poll always reports")
    }
}

/// Nothing runs at launch: a log is followed only while the Logs page shows it.
pub fn init(_app: &tauri::AppHandle) {}

static FOLLOWERS: Lazy<Mutex<HashMap<Source, Arc<Follower>>>> = Lazy::new(|| Mutex::new(HashMap::new()));

fn followers() -> MutexGuard<'static, HashMap<Source, Arc<Follower>>> {
    FOLLOWERS.lock().unwrap_or_else(|e| e.into_inner())
}

impl Follower {
    fn state(&self) -> MutexGuard<'_, State> {
        self.state.lock().unwrap_or_else(|e| e.into_inner())
    }
}

/// Starts following `source` (if not yet) and returns everything it shows now. Blocking (reads up to
/// 1 MB): call it off the UI thread.
pub fn follow(source: Source) -> Result<Update, String> {
    if source == Source::Activity {
        return Err("Lampion Activity is kept in memory, not followed from a file.".into());
    }
    let (follower, fresh) = {
        let mut map = followers();
        match map.get(&source) {
            Some(f) => (f.clone(), false),
            None => {
                let f = Arc::new(Follower { state: Mutex::new(State::new(source)), stop: AtomicBool::new(false) });
                map.insert(source, f.clone());
                (f, true)
            }
        }
    };
    let snapshot = follower.state().reload();
    if fresh {
        let f = follower.clone();
        std::thread::Builder::new()
            .name(format!("lampion-log-{source:?}"))
            .spawn(move || {
                for tick in 1u32.. {
                    std::thread::sleep(Duration::from_secs(1));
                    if f.stop.load(SeqCst) {
                        break;
                    }
                    let mut st = f.state();
                    if tick % PATH_EVERY == 0 {
                        st.retarget();
                    }
                    if let Some(update) = st.poll(false) {
                        emit("logs://update", update); // under the lock, so `seq` order is send order
                    }
                }
            })
            .map_err(|e| format!("Could not follow the log: {e}"))?;
    }
    Ok(snapshot)
}

/// Stops following `source`; its thread ends within a second.
pub fn unfollow(source: Source) {
    if let Some(f) = followers().remove(&source) {
        f.stop.store(true, SeqCst);
    }
}

/// Reads a followed log again from scratch (starting to follow it when needed).
pub fn reload(source: Source) -> Result<Update, String> {
    let follower = followers().get(&source).cloned();
    match follower {
        Some(f) => Ok(f.state().reload()),
        None => follow(source),
    }
}

/// Empties a log: Lampion Activity only in memory (`None`), a server log on disk (the page asked
/// first). Returns the emptied log's state.
pub fn clear(source: Source) -> Result<Option<Update>, String> {
    if source == Source::Activity {
        crate::core::activity::clear();
        return Ok(None);
    }
    let path = source.path(&locations::current());
    std::fs::OpenOptions::new()
        .write(true)
        .open(&path)
        .and_then(|f| f.set_len(0))
        .map_err(|e| format!("Could not empty {}: {e}", path.display()))?;
    reload(source).map(Some)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn state(path: &std::path::Path) -> State {
        let mut s = State::new(Source::ApacheAccess);
        s.path = path.to_path_buf();
        s
    }

    const GET: &str = "::1 - - [23/Nov/2026:16:16:09 +0100] \"GET / HTTP/1.1\" 200 191\n";

    #[test]
    fn reports_only_real_changes() {
        let dir = tempfile::tempdir().unwrap();
        let f = dir.path().join("access.log");
        let mut s = state(&f);
        let first = s.poll(true).unwrap();
        assert!(first.reset && !first.exists, "missing at first");
        assert!(s.poll(false).is_none(), "still missing: nothing to send");

        std::fs::write(&f, GET).unwrap();
        let u = s.poll(false).unwrap();
        assert!(u.reset && u.exists);
        assert_eq!(u.lines.len(), 1);
        assert_eq!(u.lines[0].tag.as_deref(), Some("200"));
        assert!(u.seq > first.seq);
        assert!(s.poll(false).is_none(), "same size: no update");

        std::fs::write(&f, format!("{GET}{GET}partial")).unwrap();
        let u = s.poll(false).unwrap();
        assert!(!u.reset);
        assert_eq!(u.lines.len(), 1, "only the new complete line");
        assert!(u.lines[0].id > s.lines[0].id);

        std::fs::write(&f, format!("{GET}{GET}partial line")).unwrap();
        let u = s.poll(false).unwrap();
        assert!(u.lines.is_empty() && !u.reset, "the size changed: the status bar shows it");

        std::fs::remove_file(&f).unwrap();
        let u = s.poll(false).unwrap();
        assert!(u.reset && !u.exists && u.lines.is_empty());
        assert!(s.lines.is_empty());
    }

    #[test]
    fn keeps_at_most_5000_lines() {
        let dir = tempfile::tempdir().unwrap();
        let f = dir.path().join("access.log");
        std::fs::write(&f, GET.repeat(4000)).unwrap();
        let mut s = state(&f);
        assert_eq!(s.poll(true).unwrap().lines.len(), 4000);
        std::fs::write(&f, GET.repeat(6500)).unwrap();
        let u = s.poll(false).unwrap();
        assert!(!u.reset);
        assert_eq!(u.lines.len(), 2500);
        assert_eq!(s.lines.len(), MAX_LINES);
        assert!(s.lines.iter().zip(s.lines.iter().skip(1)).all(|(a, b)| a.id < b.id), "oldest dropped first");
    }

    #[test]
    fn clearing_the_activity_list_keeps_the_file() {
        assert!(clear(Source::Activity).unwrap().is_none());
        assert!(follow(Source::Activity).is_err());
    }
}
