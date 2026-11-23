//! Config files as text. `Doc` edits a file line by line and writes back every byte it did not change
//! (CRLF or LF per line, a UTF-8 BOM, the last newline). Saving goes through `check_and_write`: the new
//! text is checked by the server itself on a temp copy, the original is kept once as
//! `<name>.lampion-backup`, and the real file is replaced atomically, or not touched at all.

use crate::core::shell;
use serde::Serialize;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU32, Ordering};
use std::time::Duration;

/// The lines of a text file. Line `i` is followed by `\n` unless it is the last one.
#[derive(Debug, Clone)]
pub struct Doc {
    lines: Vec<String>,
    /// Line `i` ended with `\r\n` (kept per line, so a file with mixed endings stays as it was).
    cr: Vec<bool>,
    /// Ending for added lines: the file's own.
    crlf: bool,
    bom: bool,
}

impl Doc {
    pub fn new(text: &str) -> Doc {
        let (bom, body) = match text.strip_prefix('\u{feff}') {
            Some(rest) => (true, rest),
            None => (false, text),
        };
        let (mut lines, mut cr) = (Vec::new(), Vec::new());
        for seg in body.split('\n') {
            let stripped = seg.strip_suffix('\r');
            lines.push(stripped.unwrap_or(seg).to_string());
            cr.push(stripped.is_some());
        }
        Doc { lines, cr, crlf: body.contains("\r\n"), bom }
    }

    pub fn text(&self) -> String {
        let mut out = String::with_capacity(self.lines.iter().map(|l| l.len() + 2).sum());
        if self.bom {
            out.push('\u{feff}');
        }
        for (i, l) in self.lines.iter().enumerate() {
            if i > 0 {
                out.push('\n');
            }
            out.push_str(l);
            if self.cr[i] {
                out.push('\r');
            }
        }
        out
    }

    pub fn lines(&self) -> &[String] {
        &self.lines
    }

    pub fn line(&self, i: usize) -> &str {
        &self.lines[i]
    }

    pub fn len(&self) -> usize {
        self.lines.len()
    }

    pub fn replace(&mut self, i: usize, line: String) {
        self.lines[i] = line;
    }

    /// Inserts a line before line `i` (at the end when `i` is past the last line).
    pub fn insert(&mut self, i: usize, line: String) {
        if i >= self.lines.len() {
            return self.push(line);
        }
        self.lines.insert(i, line);
        self.cr.insert(i, self.crlf);
    }

    /// Adds a line at the end of the file, before the final newline if there is one.
    pub fn push(&mut self, line: String) {
        let last = self.lines.len() - 1;
        if self.lines[last].is_empty() && !self.cr[last] {
            self.lines.insert(last, line);
            self.cr.insert(last, self.crlf);
        } else {
            self.cr[last] = self.crlf;
            self.lines.push(line);
            self.cr.push(false);
        }
    }

    pub fn remove(&mut self, i: usize) {
        self.lines.remove(i);
        self.cr.remove(i);
    }
}

/// The whitespace a line starts with.
pub fn indent(line: &str) -> &str {
    &line[..line.len() - line.trim_start().len()]
}

/// A value without surrounding double quotes.
pub fn unquote(s: &str) -> &str {
    let t = s.trim();
    if t.len() >= 2 && t.starts_with('"') && t.ends_with('"') {
        &t[1..t.len() - 1]
    } else {
        t
    }
}

// ---- Validation results (shared by the three settings pages) ----

/// A field the user has to fix. `field` is the settings key (camelCase, as the page names it).
#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct Problem {
    pub field: &'static str,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Default)]
pub struct Check {
    /// Saving is refused while there are problems.
    pub problems: Vec<Problem>,
    /// Shown, but saving is allowed.
    pub warnings: Vec<Problem>,
}

impl Check {
    pub fn problem(&mut self, field: &'static str, message: impl Into<String>) {
        self.problems.push(Problem { field, message: message.into() });
    }
    pub fn warning(&mut self, field: &'static str, message: impl Into<String>) {
        self.warnings.push(Problem { field, message: message.into() });
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct SaveOutcome {
    /// Not saved: these fields are invalid.
    pub problems: Vec<Problem>,
    /// The file was written (false: nothing changed, the file was left alone).
    pub changed: bool,
}

// ---- Sizes (`128M`, `2G`, `512K`, `1024`, `-1`) ----

/// `128M`, `2G`, `512K`, `1024` or `-1`.
pub fn is_size(v: &str) -> bool {
    let t = v.trim();
    if t == "-1" {
        return true;
    }
    let digits = t.strip_suffix(&['K', 'M', 'G', 'k', 'm', 'g'][..]).unwrap_or(t);
    !digits.is_empty() && digits.bytes().all(|b| b.is_ascii_digit())
}

/// Bytes for a size, for comparisons. `None` for -1, an invalid value or one too large to count.
pub fn bytes(v: &str) -> Option<i64> {
    let t = v.trim();
    if !is_size(t) || t == "-1" {
        return None;
    }
    let (num, unit) = match t.as_bytes()[t.len() - 1].to_ascii_uppercase() {
        b'K' => (&t[..t.len() - 1], 1i64 << 10),
        b'M' => (&t[..t.len() - 1], 1i64 << 20),
        b'G' => (&t[..t.len() - 1], 1i64 << 30),
        _ => (t, 1),
    };
    num.parse::<i64>().ok()?.checked_mul(unit)
}

/// A size above zero (not 0 or -1) that is not absurdly large.
pub fn is_positive_size(v: &str) -> bool {
    bytes(v).is_some_and(|b| b > 0)
}

// ---- Reading, writing, backups ----

pub fn read(path: &Path) -> Result<String, String> {
    let bytes = std::fs::read(path).map_err(|e| format!("Could not read {}: {e}", path.display()))?;
    String::from_utf8(bytes)
        .map_err(|_| format!("{} is not UTF-8 text, so Lampion does not edit it. Change it by hand.", path.display()))
}

pub fn backup_path(path: &Path) -> PathBuf {
    let mut s = path.as_os_str().to_owned();
    s.push(".lampion-backup");
    PathBuf::from(s)
}

pub fn has_backup(path: &Path) -> bool {
    backup_path(path).is_file()
}

/// Writes the file. The first time Lampion changes it, the original is kept as `<name>.lampion-backup`.
pub fn write(path: &Path, text: &str) -> Result<(), String> {
    let backup = backup_path(path);
    if path.exists() && !backup.exists() {
        std::fs::copy(path, &backup)
            .map_err(|e| format!("Could not keep a copy of the original {}: {e}", path.display()))?;
    }
    replace_file(path, text.as_bytes())
}

/// Puts back the original file saved before Lampion's first change (the backup itself stays).
pub fn restore_backup(path: &Path) -> Result<(), String> {
    let original = std::fs::read(backup_path(path))
        .map_err(|_| "Lampion has not changed this file, so there is no original to restore.".to_string())?;
    replace_file(path, &original)
}

/// Temp file next to the target, then rename: the file is never left half-written.
fn replace_file(path: &Path, bytes: &[u8]) -> Result<(), String> {
    let mut tmp = path.as_os_str().to_owned();
    tmp.push(".lampion-new");
    let tmp = PathBuf::from(tmp);
    std::fs::write(&tmp, bytes).and_then(|_| std::fs::rename(&tmp, path)).map_err(|e| {
        let _ = std::fs::remove_file(&tmp);
        format!("Could not write {}: {e}", path.display())
    })
}

/// A unique temp file name for a check copy, e.g. `%TEMP%\lampion-check-1234-0.ini`.
pub fn temp_file(ext: &str) -> PathBuf {
    static N: AtomicU32 = AtomicU32::new(0);
    std::env::temp_dir().join(format!("lampion-check-{}-{}.{ext}", std::process::id(), N.fetch_add(1, Ordering::Relaxed)))
}

/// Saves `updated` over `path` when it differs from `current`, but only after `check` accepted a temp
/// copy of it. Returns whether the file was written; on any error the real file is unchanged.
pub fn check_and_write(
    path: &Path,
    current: &str,
    updated: &str,
    ext: &str,
    check: impl FnOnce(&Path) -> Result<(), String>,
) -> Result<bool, String> {
    if updated == current {
        return Ok(false);
    }
    let tmp = temp_file(ext);
    std::fs::write(&tmp, updated).map_err(|e| format!("Could not test the change: {e}"))?;
    let verdict = check(&tmp);
    let _ = std::fs::remove_file(&tmp);
    verdict?;
    write(path, updated)?;
    Ok(true)
}

// ---- Running a server program for a check ----

/// Runs `httpd.exe -t`, `php.exe -c`, `mariadbd.exe --help`, `php.exe -l`. Unit tests pass a fake,
/// so they never start a real server program.
pub type Run<'a> = &'a dyn Fn(&Path, &[&str]) -> shell::Output;

pub fn run_real(program: &Path, args: &[&str]) -> shell::Output {
    if !program.is_file() {
        return shell::Output { status: -1, out: format!("{} was not found.", program.display()) };
    }
    shell::run_timeout(program, args, Duration::from_secs(60))
}

/// The last `n` non-empty lines of a program's output.
pub fn tail(out: &str, n: usize) -> String {
    let lines: Vec<&str> = out.lines().map(str::trim).filter(|l| !l.is_empty()).collect();
    lines[lines.len().saturating_sub(n)..].join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn doc_round_trips_every_byte() {
        for text in ["", "a", "a\n", "a\r\nb\r\n", "a\r\nb", "a\nb\r\nc\n", "\u{feff}x = 1\r\n", "\n\n", "a\r"] {
            assert_eq!(Doc::new(text).text(), text, "{text:?}");
        }
    }

    #[test]
    fn doc_keeps_line_endings_when_editing() {
        let mut d = Doc::new("a = 1\r\nb = 2\r\n");
        d.replace(1, "b = 3".into());
        d.push("c = 4".into());
        d.insert(0, "z = 0".into());
        assert_eq!(d.text(), "z = 0\r\na = 1\r\nb = 3\r\nc = 4\r\n");

        let mut lf = Doc::new("a\nb");
        lf.push("c".into());
        assert_eq!(lf.text(), "a\nb\nc");

        let mut mixed = Doc::new("a\r\nb\nc\r\n");
        mixed.replace(1, "B".into());
        assert_eq!(mixed.text(), "a\r\nB\nc\r\n");

        let mut empty = Doc::new("");
        empty.push("x".into());
        assert_eq!(empty.text(), "x\n");
    }

    #[test]
    fn sizes() {
        assert!(is_size("128M") && is_size("2g") && is_size("1024") && is_size("-1") && is_size(" 64K "));
        assert!(!is_size("") && !is_size("M") && !is_size("12MB") && !is_size("-2") && !is_size("1.5G"));
        assert_eq!(bytes("2M"), Some(2 << 20));
        assert_eq!(bytes("1g"), Some(1 << 30));
        assert_eq!(bytes("-1"), None);
        // Pitfall: 9000000000G overflowed and crashed; it is simply not a valid size.
        assert_eq!(bytes("9000000000G"), None);
        assert_eq!(bytes("99999999999999999999"), None);
        assert!(!is_positive_size("9000000000G"));
        assert!(!is_positive_size("0") && is_positive_size("1"));
    }

    #[test]
    fn backup_once_and_restore() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("php.ini");
        std::fs::write(&path, "a = 1\r\n").unwrap();
        write(&path, "a = 2\r\n").unwrap();
        write(&path, "a = 3\r\n").unwrap();
        assert_eq!(std::fs::read_to_string(backup_path(&path)).unwrap(), "a = 1\r\n", "the first original is kept");
        assert_eq!(std::fs::read_to_string(&path).unwrap(), "a = 3\r\n");
        restore_backup(&path).unwrap();
        assert_eq!(std::fs::read_to_string(&path).unwrap(), "a = 1\r\n");
        assert!(has_backup(&path));
        assert!(restore_backup(&dir.path().join("other.ini")).is_err());
    }

    #[test]
    fn rejected_check_leaves_the_file_alone() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("httpd.conf");
        std::fs::write(&path, "Listen 8080\n").unwrap();
        let mut seen = String::new();
        let r = check_and_write(&path, "Listen 8080\n", "Listen x\n", "conf", |tmp| {
            seen = std::fs::read_to_string(tmp).unwrap();
            Err("bad".into())
        });
        assert_eq!(r, Err("bad".into()));
        assert_eq!(seen, "Listen x\n", "the check sees the new text");
        assert_eq!(std::fs::read_to_string(&path).unwrap(), "Listen 8080\n");
        assert!(!has_backup(&path));
        // Unchanged text: no check, no write.
        assert_eq!(check_and_write(&path, "Listen 8080\n", "Listen 8080\n", "conf", |_| panic!("no check")), Ok(false));
    }
}
