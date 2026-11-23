//! The seven log sources and how one line of each reads: time · tag · message and a level
//! (docs/SPEC.md "Naplók", LogSource.swift).

use crate::config::locations::Locations;
use crate::core::activity::Level;
use crate::core::paths;
use once_cell::sync::Lazy;
use regex::{Captures, Regex};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Source {
    Activity,
    ApacheError,
    ApacheAccess,
    Php,
    Mariadb,
    MariadbSlow,
    MariadbGeneral,
}

impl Source {
    pub const ALL: [Source; 7] = [
        Source::Activity,
        Source::ApacheError,
        Source::ApacheAccess,
        Source::Php,
        Source::Mariadb,
        Source::MariadbSlow,
        Source::MariadbGeneral,
    ];

    /// The file behind the log, from the real configuration. Lampion Activity: `Lampion.log` (its lines
    /// come from memory). A log that is not turned on gets the file Lampion's settings would use.
    pub fn path(self, l: &Locations) -> PathBuf {
        let logs = paths::logs();
        match self {
            Source::Activity => paths::local_data().join("Lampion.log"),
            Source::ApacheError => l.apache_error_log.clone(),
            Source::ApacheAccess => l.apache_access_log.clone(),
            Source::Php => l.php_error_log.clone().unwrap_or_else(|| logs.join("php").join("php_errors.log")),
            Source::Mariadb => l.mariadb_error_log.clone().unwrap_or_else(|| logs.join("mariadb").join("mariadb.err")),
            Source::MariadbSlow => l.slow_query_log.clone().unwrap_or_else(|| logs.join("mariadb").join("slow.log")),
            Source::MariadbGeneral => l.general_query_log.clone().unwrap_or_else(|| logs.join("mariadb").join("general.log")),
        }
    }

    /// How serious a line is, from the usual markers of each format.
    pub fn level(self, line: &str) -> Level {
        match self {
            Source::ApacheAccess => {
                // … "GET / HTTP/1.1" 404 196
                match line.rfind("\" ").and_then(|i| line.as_bytes().get(i + 2)) {
                    Some(b'5') => Level::Error,
                    Some(b'4') => Level::Warn,
                    _ => Level::Info,
                }
            }
            Source::MariadbSlow | Source::MariadbGeneral | Source::Activity => Level::Info,
            _ => {
                let l = line.to_lowercase();
                let has = |keys: &[&str]| keys.iter().any(|k| l.contains(k));
                if has(&[":error]", ":emerg]", ":crit]", ":alert]", "[error]", "fatal error", "parse error", "php error"]) {
                    Level::Error
                } else if has(&[":warn]", "[warning]", "php warning", "php deprecated", "php notice"]) {
                    Level::Warn
                } else {
                    Level::Info
                }
            }
        }
    }
}

/// One line as the Logs page shows it (`LogLine` in src/lib/types.ts).
#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Line {
    pub id: u64,
    pub level: Level,
    pub time: Option<String>,
    pub tag: Option<String>,
    /// The line without its time and tag; `None`: show `text`.
    pub message: Option<String>,
    /// The line as written (copied and searched).
    pub text: String,
}

fn re(pattern: &str) -> Regex {
    Regex::new(pattern).expect("valid log pattern")
}

static APACHE_ERROR: Lazy<Regex> =
    Lazy::new(|| re(r"^\[([^\]]+)\] \[([^:\]]*):([a-z0-9]+)\](?: \[pid [^\]]+\])?(?: \[client [^\]]+\])? (?:AH\d+: )?(.*)$"));
static APACHE_ACCESS: Lazy<Regex> = Lazy::new(|| re(r#"^(\S+) \S+ \S+ \[([^\]]+)\] "([^"]*)" (\d{3}) (\S+)(.*)$"#));
static PHP: Lazy<Regex> = Lazy::new(|| re(r"^\[([^\]]+)\] (?:PHP )?([A-Za-z ]+?):\s+(.*)$"));
static MARIADB: Lazy<Regex> = Lazy::new(|| re(r"^\d{4}-\d{2}-\d{2}\s+(\d{1,2}:\d{2}:\d{2})\s+\d+\s+\[([A-Za-z]+)\]\s*(.*)$"));

/// The first `hh:mm:ss` not preceded by a digit: SPEC's `(?<![0-9])[0-9]{2}:[0-9]{2}:[0-9]{2}`, by hand,
/// as Rust's regex has no lookbehind. Without the rule `2026:12:06:52` would give "26:12:06".
pub fn clock(s: &str) -> Option<&str> {
    let b = s.as_bytes();
    (0..b.len().saturating_sub(7))
        .find(|&i| {
            (i == 0 || !b[i - 1].is_ascii_digit())
                && [0, 1, 3, 4, 6, 7].iter().all(|&k| b[i + k].is_ascii_digit())
                && b[i + 2] == b':'
                && b[i + 5] == b':'
        })
        .map(|i| &s[i..i + 8])
}

/// Splits a line into time, tag and message for the known formats.
pub fn parse(source: Source, text: String, id: u64) -> Line {
    let level = source.level(&text);
    let mut line = Line { id, level, time: None, tag: None, message: None, text };
    let fields = |re: &Regex, t: &str| re.captures(t).map(|c: Captures| (1..c.len()).map(|i| c[i].to_string()).collect::<Vec<_>>());
    match source {
        Source::ApacheError => {
            // [Mon Nov 23 12:06:52.891929 2026] [mpm_winnt:notice] [pid 81322] AH00163: message
            if let Some(m) = fields(&APACHE_ERROR, &line.text) {
                line.time = clock(&m[0]).map(String::from);
                line.tag = Some(if m[1].is_empty() { m[2].clone() } else { m[1].clone() });
                line.message = Some(m[3].clone());
            }
        }
        Source::ApacheAccess => {
            // 127.0.0.1 - - [23/Nov/2026:12:00:00 +0100] "GET / HTTP/1.1" 200 45
            if let Some(m) = fields(&APACHE_ACCESS, &line.text) {
                let request = m[2].replace(" HTTP/1.1", "").replace(" HTTP/2.0", "");
                let size = if m[4] == "-" { String::new() } else { format!("  ·  {} B", m[4]) };
                line.level = match m[3].as_bytes()[0] {
                    b'5' => Level::Error,
                    b'4' => Level::Warn,
                    _ => Level::Info,
                };
                line.time = clock(&m[1]).map(String::from);
                line.message = Some(format!("{request}{size}  ·  {}", m[0]));
                line.tag = Some(m[3].clone());
            }
        }
        Source::Php => {
            // [23-Nov-2026 14:00:00 Europe/Budapest] PHP Warning:  message
            if let Some(m) = fields(&PHP, &line.text) {
                line.time = clock(&m[0]).map(String::from);
                line.tag = Some(m[1].clone());
                line.message = Some(m[2].clone());
            }
        }
        Source::Mariadb => {
            // 2026-11-23 14:34:54 0 [Warning] message
            if let Some(m) = fields(&MARIADB, &line.text) {
                line.time = Some(m[0].clone());
                line.tag = Some(m[1].clone());
                line.message = Some(m[2].clone());
            }
        }
        Source::Activity | Source::MariadbSlow | Source::MariadbGeneral => {}
    }
    line
}

#[cfg(test)]
mod tests {
    use super::*;

    fn p(source: Source, text: &str) -> (Level, Option<String>, Option<String>, String) {
        let l = parse(source, text.to_string(), 1);
        let message = l.message.clone().unwrap_or_else(|| l.text.clone());
        (l.level, l.time, l.tag, message)
    }

    fn some(s: &str) -> Option<String> {
        Some(s.to_string())
    }

    #[test]
    fn clock_needs_a_non_digit_before_it() {
        assert_eq!(clock("23/Nov/2026:12:06:52 +0100"), Some("12:06:52"), "not 26:12:06");
        assert_eq!(clock("2026:12:06:52"), Some("12:06:52"));
        assert_eq!(clock("Mon Nov 23 12:06:52.891929 2026"), Some("12:06:52"));
        assert_eq!(clock("12:06:52"), Some("12:06:52"));
        assert_eq!(clock("123:45:67"), None);
        assert_eq!(clock("1:02:03"), None);
        assert_eq!(clock("ár 09:15:00 ő"), Some("09:15:00"));
        assert_eq!(clock(""), None);
    }

    #[test]
    fn apache_error_log() {
        let (level, time, tag, msg) = p(
            Source::ApacheError,
            "[Mon Nov 23 16:16:08.291523 2026] [mpm_winnt:notice] [pid 81322:tid 412] AH00455: Apache/2.4.66 (Win64) PHP/8.4.1 configured -- resuming normal operations",
        );
        assert_eq!((level, time, tag), (Level::Info, some("16:16:08"), some("mpm_winnt")));
        assert_eq!(msg, "Apache/2.4.66 (Win64) PHP/8.4.1 configured -- resuming normal operations");

        let (level, time, tag, msg) = p(
            Source::ApacheError,
            "[Mon Nov 23 19:27:18.123456 2026] [php:error] [pid 1234:tid 5] [client ::1:52044] script 'C:/Lampion/www/index.php' not found or unable to stat",
        );
        assert_eq!((level, time, tag), (Level::Error, some("19:27:18"), some("php")));
        assert_eq!(msg, "script 'C:/Lampion/www/index.php' not found or unable to stat");

        let (level, _, tag, _) = p(Source::ApacheError, "[Mon Nov 23 19:27:18.1 2026] [:warn] [pid 1] something odd");
        assert_eq!((level, tag), (Level::Warn, some("warn")), "an empty module shows the level");
        for marker in ["emerg", "crit", "alert"] {
            let line = format!("[Mon Nov 23 19:27:18.1 2026] [core:{marker}] [pid 1] bad");
            assert_eq!(p(Source::ApacheError, &line).0, Level::Error, "{marker}");
        }
        let raw = p(Source::ApacheError, "Command line: 'C:\\Lampion\\bin\\apache\\bin\\httpd.exe -d C:/Lampion/bin/apache'");
        assert_eq!((raw.1, raw.2), (None, None));
    }

    #[test]
    fn apache_access_log() {
        let (level, time, tag, msg) = p(Source::ApacheAccess, r#"::1 - - [23/Nov/2026:16:16:09 +0100] "GET / HTTP/1.1" 200 191"#);
        assert_eq!((level, time, tag), (Level::Info, some("16:16:09"), some("200")));
        assert_eq!(msg, "GET /  ·  191 B  ·  ::1");

        let (level, time, tag, msg) =
            p(Source::ApacheAccess, r#"127.0.0.1 - - [23/Nov/2026:19:27:18 +0100] "GET /hianyzo-oldal.html HTTP/1.1" 404 236 "-" "Mozilla/5.0""#);
        assert_eq!((level, time, tag), (Level::Warn, some("19:27:18"), some("404")), "combined format, level from the code");
        assert_eq!(msg, "GET /hianyzo-oldal.html  ·  236 B  ·  127.0.0.1");

        let (level, _, _, msg) = p(Source::ApacheAccess, r#"::1 - - [23/Nov/2026:19:27:18 +0100] "POST /x HTTP/2.0" 503 -"#);
        assert_eq!(level, Level::Error);
        assert_eq!(msg, "POST /x  ·  ::1", "no size for -");

        // An escaped quote defeats the pattern; the level still comes from the code after the last quote.
        let odd = p(Source::ApacheAccess, r#"::1 - - [23/Nov/2026:19:27:18 +0100] "GET /a\"b HTTP/1.1" 404 9"#);
        assert_eq!((odd.0, odd.2), (Level::Warn, None));
    }

    #[test]
    fn php_error_log() {
        let (level, time, tag, msg) =
            p(Source::Php, "[23-Nov-2026 19:27:18 Europe/Budapest] PHP Warning:  Undefined variable $x in C:\\Lampion\\www\\a.php on line 3");
        assert_eq!((level, time, tag), (Level::Warn, some("19:27:18"), some("Warning")));
        assert_eq!(msg, "Undefined variable $x in C:\\Lampion\\www\\a.php on line 3");

        let (level, _, tag, msg) = p(
            Source::Php,
            "[23-Nov-2026 19:27:18 Europe/Budapest] PHP Fatal error:  Uncaught ArgumentCountError: strlen() expects exactly 1 argument, 0 given in C:\\Lampion\\www\\a.php:8",
        );
        assert_eq!((level, tag), (Level::Error, some("Fatal error")));
        assert!(msg.starts_with("Uncaught ArgumentCountError"));

        assert_eq!(p(Source::Php, "[23-Nov-2026 16:29:56 UTC] PHP Deprecated:  Function x() is deprecated").0, Level::Warn);
        assert_eq!(p(Source::Php, "[23-Nov-2026 16:29:56 UTC] PHP Notice:  Demo notice").0, Level::Warn);
        assert_eq!(p(Source::Php, "[23-Nov-2026 16:29:56 UTC] PHP Parse error:  syntax error").0, Level::Error);
        let trace = p(Source::Php, "#0 C:\\Lampion\\www\\a.php(8): strlen()");
        assert_eq!((trace.0, trace.1, trace.2), (Level::Info, None, None), "stack trace lines stay plain");
    }

    #[test]
    fn mariadb_error_log() {
        let (level, time, tag, msg) =
            p(Source::Mariadb, "2026-11-23 17:58:37 0 [Warning] Setting lower_case_table_names=2 because file system is case insensitive");
        assert_eq!((level, time, tag), (Level::Warn, some("17:58:37"), some("Warning")));
        assert_eq!(msg, "Setting lower_case_table_names=2 because file system is case insensitive");
        let (level, time, tag, _) = p(Source::Mariadb, "2026-11-23  9:05:01 3 [ERROR] Can't open the mysql.plugin table");
        assert_eq!((level, time, tag), (Level::Error, some("9:05:01"), some("ERROR")));
        let note = p(Source::Mariadb, "2026-11-23 17:58:37 0 [Note] InnoDB: Buffer pool(s) load completed at 261123 17:58:37");
        assert_eq!((note.0, note.2), (Level::Info, some("Note")));
        let raw = p(Source::Mariadb, "Version: '11.8.2-MariaDB'  socket: ''  port: 3306  mariadb.org binary distribution");
        assert_eq!((raw.1, raw.2), (None, None));
    }

    #[test]
    fn query_logs_are_plain_info() {
        for source in [Source::MariadbSlow, Source::MariadbGeneral] {
            let (level, time, tag, msg) = p(source, "# Time: 261123 19:27:27 [ERROR] not a marker here");
            assert_eq!((level, time, tag), (Level::Info, None, None));
            assert_eq!(msg, "# Time: 261123 19:27:27 [ERROR] not a marker here");
        }
    }

    #[test]
    fn paths_come_from_the_configuration() {
        let l = Locations {
            apache_port: 8080,
            document_root: PathBuf::from(r"C:\Lampion\www"),
            apache_error_log: PathBuf::from(r"D:\logs\error.log"),
            apache_access_log: PathBuf::from(r"D:\logs\access.log"),
            mariadb_port: 3306,
            mariadb_datadir: PathBuf::from(r"C:\Lampion\databases"),
            mariadb_error_log: Some(PathBuf::from(r"D:\logs\mariadb.err")),
            php_error_log: None,
            slow_query_log: Some(PathBuf::from(r"D:\logs\slow.log")),
            general_query_log: None,
        };
        assert_eq!(Source::ApacheError.path(&l), PathBuf::from(r"D:\logs\error.log"));
        assert_eq!(Source::ApacheAccess.path(&l), PathBuf::from(r"D:\logs\access.log"));
        assert_eq!(Source::Mariadb.path(&l), PathBuf::from(r"D:\logs\mariadb.err"));
        assert_eq!(Source::MariadbSlow.path(&l), PathBuf::from(r"D:\logs\slow.log"));
        assert_eq!(Source::Php.path(&l), paths::logs().join("php").join("php_errors.log"), "off: the file the setting would use");
        assert_eq!(Source::MariadbGeneral.path(&l), paths::logs().join("mariadb").join("general.log"));
        assert!(Source::Activity.path(&l).ends_with("Lampion.log"));
    }

    #[test]
    fn source_ids_match_the_ui() {
        let ids: Vec<String> = Source::ALL.iter().map(|s| serde_json::to_string(s).unwrap()).collect();
        assert_eq!(ids, ["\"activity\"", "\"apacheError\"", "\"apacheAccess\"", "\"php\"", "\"mariadb\"", "\"mariadbSlow\"", "\"mariadbGeneral\""]);
    }
}
