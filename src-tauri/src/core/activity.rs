//! Lampion's own activity log: kept in memory (last 2000 lines), appended to
//! `%LOCALAPPDATA%\Lampion\Lampion.log`, and sent to the UI as the `activity://line` event.
//! Modules: "Apache", "MySQL", "PHP", "Setup", "Panel", "Move", "Config", "Database", "Update".

use once_cell::sync::Lazy;
use serde::Serialize;
use std::collections::VecDeque;
use std::io::Write;
use std::sync::Mutex;

pub const MAX_LINES: usize = 2000;

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum Level {
    Info,
    Ok,
    Warn,
    Error,
}

#[derive(Debug, Clone, Serialize)]
pub struct Line {
    /// Local time, `HH:MM:SS`.
    pub time: String,
    pub module: String,
    pub level: Level,
    pub text: String,
}

static LINES: Lazy<Mutex<VecDeque<Line>>> = Lazy::new(|| Mutex::new(VecDeque::new()));

pub fn add(module: &str, text: &str, level: Level) {
    let now = chrono::Local::now();
    let line = Line {
        time: now.format("%H:%M:%S").to_string(),
        module: module.to_string(),
        level,
        text: text.to_string(),
    };
    {
        let mut l = LINES.lock().unwrap();
        l.push_back(line.clone());
        while l.len() > MAX_LINES {
            l.pop_front();
        }
    }
    let path = super::paths::local_data().join("Lampion.log");
    if let Ok(mut f) = std::fs::OpenOptions::new().create(true).append(true).open(path) {
        let tag = match level {
            Level::Info => "info",
            Level::Ok => "ok",
            Level::Warn => "warn",
            Level::Error => "error",
        };
        let _ = writeln!(f, "{} [{}] [{}] {}", now.format("%Y-%m-%d %H:%M:%S"), module, tag, text);
    }
    super::emit("activity://line", line);
}

pub fn info(module: &str, text: &str) { add(module, text, Level::Info) }
pub fn ok(module: &str, text: &str) { add(module, text, Level::Ok) }
pub fn warn(module: &str, text: &str) { add(module, text, Level::Warn) }
pub fn error(module: &str, text: &str) { add(module, text, Level::Error) }

pub fn all() -> Vec<Line> {
    LINES.lock().unwrap().iter().cloned().collect()
}

/// Clears the in-memory list (the file is kept).
pub fn clear() {
    LINES.lock().unwrap().clear();
}
