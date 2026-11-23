//! Where the cards were dragged, per database: `%APPDATA%\Lampion\designer\<db>.json`,
//! `{ "table": [x, y], … }` in diagram points. A missing or broken file just means "not arranged yet".

use std::collections::BTreeMap;
use std::path::Path;

pub type Positions = BTreeMap<String, [f64; 2]>;

const DEVICES: [&str; 22] = [
    "CON", "PRN", "AUX", "NUL", "COM1", "COM2", "COM3", "COM4", "COM5", "COM6", "COM7", "COM8", "COM9", "LPT1", "LPT2",
    "LPT3", "LPT4", "LPT5", "LPT6", "LPT7", "LPT8", "LPT9",
];

/// A file name for any database name: letters, digits, `_` and `-` stay, every other byte becomes `%XX`,
/// and a Windows device name (CON, NUL, COM1…) gets its first letter escaped.
// shortcut: a 64-character name of non-Latin letters can exceed the 255-character file name limit; saving then fails.
pub fn file_name(db: &str) -> String {
    let mut out = String::new();
    for b in db.bytes() {
        if b.is_ascii_alphanumeric() || b == b'_' || b == b'-' {
            out.push(b as char);
        } else {
            out.push_str(&format!("%{b:02X}"));
        }
    }
    if DEVICES.contains(&out.to_ascii_uppercase().as_str()) {
        out = format!("%{:02X}{}", out.as_bytes()[0], &out[1..]);
    }
    format!("{out}.json")
}

pub fn read(path: &Path) -> Positions {
    std::fs::read_to_string(path).ok().and_then(|t| serde_json::from_str(&t).ok()).unwrap_or_default()
}

/// Saves the positions; an empty map forgets them (Auto Arrange).
pub fn write(path: &Path, positions: &Positions) -> Result<(), String> {
    if positions.is_empty() {
        return match std::fs::remove_file(path) {
            Err(e) if e.kind() != std::io::ErrorKind::NotFound => Err(format!("Could not forget the card positions: {e}")),
            _ => Ok(()),
        };
    }
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir).map_err(|e| format!("Could not save the card positions: {e}"))?;
    }
    let json = serde_json::to_string(positions).map_err(|e| e.to_string())?;
    std::fs::write(path, json).map_err(|e| format!("Could not save the card positions: {e}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn file_names_are_safe_for_windows() {
        assert_eq!(file_name("lampion_demo"), "lampion_demo.json");
        assert_eq!(file_name("my-db"), "my-db.json");
        assert_eq!(file_name("a/b:c*?\"<>|. "), "a%2Fb%3Ac%2A%3F%22%3C%3E%7C%2E%20.json");
        assert_eq!(file_name("100%"), "100%25.json", "% itself is escaped, so names never collide");
        assert_eq!(file_name("con"), "%63on.json");
        assert_eq!(file_name("COM1"), "%43OM1.json");
        assert_eq!(file_name("console"), "console.json");
        assert_eq!(file_name("ár"), "%C3%A1r.json");
    }

    #[test]
    fn saves_reads_and_forgets() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("designer").join("shop.json");
        assert!(read(&path).is_empty(), "nothing saved yet");
        let mut p = Positions::new();
        p.insert("orders".into(), [40.0, 120.0]);
        p.insert("customers".into(), [-3.0, 0.5]);
        write(&path, &p).unwrap();
        assert_eq!(read(&path), p);
        write(&path, &Positions::new()).unwrap();
        assert!(!path.exists());
        write(&path, &Positions::new()).unwrap();
    }

    #[test]
    fn a_broken_file_reads_as_empty() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("x.json");
        std::fs::write(&path, "{\"orders\": [1]").unwrap();
        assert!(read(&path).is_empty());
        std::fs::write(&path, "{\"orders\": \"left\"}").unwrap();
        assert!(read(&path).is_empty());
    }
}
