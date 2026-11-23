//! The Query tab's history: the last 30 queries run, newest first, kept in
//! `%APPDATA%\Lampion\query-history.json` so they survive a restart.

use once_cell::sync::Lazy;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

pub const LIMIT: usize = 30;

static LIST: Lazy<Mutex<Option<Vec<String>>>> = Lazy::new(|| Mutex::new(None));

fn file() -> PathBuf {
    crate::core::paths::app_data().join("query-history.json")
}

/// The query as it is run and remembered: trimmed, without trailing semicolons.
pub fn normalize(text: &str) -> String {
    let mut t = text.trim();
    while let Some(rest) = t.strip_suffix(';') {
        t = rest.trim_end();
    }
    t.to_string()
}

/// Puts `text` first (removing an older copy) and keeps at most `LIMIT` entries.
pub fn record(list: &mut Vec<String>, text: &str) {
    let text = normalize(text);
    if text.is_empty() {
        return;
    }
    list.retain(|q| *q != text);
    list.insert(0, text);
    list.truncate(LIMIT);
}

pub fn read(path: &Path) -> Vec<String> {
    let mut list: Vec<String> = std::fs::read_to_string(path).ok().and_then(|t| serde_json::from_str(&t).ok()).unwrap_or_default();
    list.truncate(LIMIT);
    list
}

pub fn write(path: &Path, list: &[String]) {
    if let Ok(json) = serde_json::to_string_pretty(list) {
        let _ = std::fs::write(path, json);
    }
}

fn with<T>(f: impl FnOnce(&mut Vec<String>) -> T) -> T {
    let mut g = LIST.lock().unwrap();
    let list = g.get_or_insert_with(|| read(&file()));
    f(list)
}

pub fn all() -> Vec<String> {
    with(|l| l.clone())
}

/// Records a run query and saves the list; returns it.
pub fn add(text: &str) -> Vec<String> {
    with(|l| {
        record(l, text);
        write(&file(), l);
        l.clone()
    })
}

pub fn clear() {
    with(|l| {
        l.clear();
        write(&file(), l);
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn newest_first_without_duplicates() {
        let mut list = vec!["SELECT 2".to_string(), "SELECT 1".to_string()];
        record(&mut list, "  SELECT 1;; \n");
        assert_eq!(list, ["SELECT 1", "SELECT 2"]);
        record(&mut list, "  ;  ");
        assert_eq!(list.len(), 2);
    }

    #[test]
    fn keeps_thirty() {
        let mut list = Vec::new();
        for i in 0..40 {
            record(&mut list, &format!("SELECT {i}"));
        }
        assert_eq!(list.len(), LIMIT);
        assert_eq!(list[0], "SELECT 39");
        assert_eq!(list[29], "SELECT 10");
    }

    #[test]
    fn survives_a_restart() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("query-history.json");
        assert!(read(&path).is_empty());
        write(&path, &["SELECT 'é'".to_string(), "SHOW TABLES".to_string()]);
        assert_eq!(read(&path), ["SELECT 'é'", "SHOW TABLES"]);
        std::fs::write(&path, "not json").unwrap();
        assert!(read(&path).is_empty());
    }
}
