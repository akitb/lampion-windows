//! my.ini: the MariaDB settings page. Lampion's settings live in the server's `[mysqld]` section;
//! an empty field means the server's default (the key is commented out). MariaDB checks a temp copy
//! first (`mariadbd.exe --defaults-file=<copy> --help --verbose`: only that file, as the server starts).

use super::file::{self, Check, Run};
use super::ini::{truthy, Dialect, Ini};
use crate::core::paths;
use serde::{Deserialize, Serialize};
use std::path::Path;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct MariadbSettings {
    pub port: String,
    /// `bind-address = 127.0.0.1`; off writes `0.0.0.0`.
    pub local_only: bool,
    pub max_connections: String,
    pub max_allowed_packet: String,
    pub buffer_pool_size: String,
    pub character_set: String,
    pub collation: String,
    /// `default` | `strict` | `relaxed` | `custom`.
    pub sql_mode: String,
    /// The value kept when the file has a mode that matches no preset.
    pub custom_sql_mode: String,
    pub slow_query_log: bool,
    pub long_query_time: String,
    pub general_log: bool,
}

pub const STRICT: &str = "STRICT_TRANS_TABLES,ERROR_FOR_DIVISION_BY_ZERO,NO_AUTO_CREATE_USER,NO_ENGINE_SUBSTITUTION";
pub const RELAXED: &str = "NO_ENGINE_SUBSTITUTION";

#[derive(Debug, Clone, Copy, Serialize)]
pub struct Charset {
    pub charset: &'static str,
    pub collations: &'static [&'static str],
}

pub const CHARSETS: &[Charset] = &[
    Charset { charset: "utf8mb4", collations: &["utf8mb4_uca1400_ai_ci", "utf8mb4_unicode_ci", "utf8mb4_general_ci", "utf8mb4_hungarian_ci", "utf8mb4_bin"] },
    Charset { charset: "utf8mb3", collations: &["utf8mb3_unicode_ci", "utf8mb3_general_ci", "utf8mb3_hungarian_ci"] },
    Charset { charset: "latin1", collations: &["latin1_swedish_ci", "latin1_general_ci", "latin1_bin"] },
    Charset { charset: "latin2", collations: &["latin2_hungarian_ci", "latin2_general_ci"] },
];

/// MariaDB's built-in values, shown greyed in empty fields.
pub fn defaults() -> MariadbSettings {
    MariadbSettings {
        port: "3306".into(),
        max_connections: "151".into(),
        max_allowed_packet: "16M".into(),
        buffer_pool_size: "128M".into(),
        character_set: "utf8mb4".into(),
        collation: "utf8mb4_uca1400_ai_ci".into(),
        sql_mode: "default".into(),
        long_query_time: "10".into(),
        ..Default::default()
    }
}

pub fn parse(text: &str) -> MariadbSettings {
    let ini = Ini::new(text, Dialect::MyIni);
    let v = |k: &str| ini.value(k).unwrap_or_default();
    let (sql_mode, custom_sql_mode) = match ini.value("sql_mode") {
        None => ("default", String::new()),
        Some(m) if m == STRICT => ("strict", String::new()),
        Some(m) if m == RELAXED => ("relaxed", String::new()),
        Some(m) => ("custom", m),
    };
    MariadbSettings {
        port: v("port"),
        local_only: matches!(ini.value("bind-address").as_deref().map(str::trim), Some("127.0.0.1" | "localhost" | "::1")),
        max_connections: v("max_connections"),
        max_allowed_packet: v("max_allowed_packet"),
        buffer_pool_size: v("innodb_buffer_pool_size"),
        character_set: v("character-set-server"),
        collation: v("collation-server"),
        sql_mode: sql_mode.into(),
        custom_sql_mode,
        slow_query_log: ini.value("slow_query_log").is_some_and(|x| truthy(&x)),
        long_query_time: v("long_query_time"),
        general_log: ini.value("general_log").is_some_and(|x| truthy(&x)),
    }
}

fn sql_mode_value(s: &MariadbSettings) -> Option<String> {
    match s.sql_mode.as_str() {
        "strict" => Some(STRICT.into()),
        "relaxed" => Some(RELAXED.into()),
        "custom" => Some(s.custom_sql_mode.trim().into()),
        _ => None,
    }
}

/// Writes the settings the user changed (`draft` differs from `base`, the values the page loaded) that
/// the file does not already hold. `slow_log` / `general_log` are the files used when a query log is
/// turned on and my.ini names none.
pub fn apply(text: &str, base: &MariadbSettings, draft: &MariadbSettings, slow_log: &str, general_log: &str) -> String {
    let cur = parse(text);
    let mut ini = Ini::new(text, Dialect::MyIni);
    let fields: [(&str, fn(&MariadbSettings) -> String); 7] = [
        ("port", |s| s.port.trim().into()),
        ("max_connections", |s| s.max_connections.trim().into()),
        ("max_allowed_packet", |s| s.max_allowed_packet.trim().to_ascii_uppercase()),
        ("innodb_buffer_pool_size", |s| s.buffer_pool_size.trim().to_ascii_uppercase()),
        ("character-set-server", |s| s.character_set.trim().into()),
        ("collation-server", |s| s.collation.trim().into()),
        ("long_query_time", |s| s.long_query_time.trim().into()),
    ];
    for (key, value) in fields {
        let v = value(draft);
        if v != value(base) && v != value(&cur) {
            if v.is_empty() {
                ini.unset(key);
            } else {
                ini.set(key, &v);
            }
        }
    }
    let d = draft.local_only;
    if d != base.local_only && d != cur.local_only {
        ini.set("bind-address", if d { "127.0.0.1" } else { "0.0.0.0" });
    }
    let mode = sql_mode_value(draft);
    if mode != sql_mode_value(base) && mode != sql_mode_value(&cur) {
        match mode {
            None => ini.unset("sql_mode"),
            Some(m) => ini.set("sql_mode", &format!("\"{m}\"")),
        }
    }
    let logs = [
        (draft.slow_query_log, base.slow_query_log, cur.slow_query_log, "slow_query_log", "slow_query_log_file", slow_log),
        (draft.general_log, base.general_log, cur.general_log, "general_log", "general_log_file", general_log),
    ];
    for (d, b, c, key, file_key, path) in logs {
        if d != b && d != c {
            if d {
                ini.set(key, "1");
                if ini.value(file_key).is_none() {
                    ini.set(file_key, &format!("\"{path}\""));
                }
            } else {
                ini.unset(key);
            }
        }
    }
    ini.text()
}

pub fn validate(s: &MariadbSettings) -> Check {
    let mut c = Check::default();
    let port = s.port.trim();
    if !port.is_empty() && !port.parse::<u16>().is_ok_and(|p| p >= 1024) {
        c.problem("port", "The port must be a number between 1024 and 65535.");
    }
    let conns = s.max_connections.trim();
    if !conns.is_empty() && !conns.parse::<u32>().is_ok_and(|n| n >= 1) {
        c.problem("maxConnections", "Maximum connections must be a positive number.");
    }
    for (field, name, v) in [("maxAllowedPacket", "Maximum packet size", &s.max_allowed_packet), ("bufferPoolSize", "InnoDB buffer pool", &s.buffer_pool_size)] {
        if !v.trim().is_empty() && !file::is_positive_size(v) {
            c.problem(field, format!("{name} must be a size such as 64M or 1G."));
        }
    }
    let threshold = s.long_query_time.trim();
    if !threshold.is_empty() && !threshold.parse::<f64>().is_ok_and(|x| x.is_finite() && x >= 0.0) {
        c.problem("longQueryTime", "The slow query threshold must be a number of seconds, such as 2 or 0.5.");
    }
    if !s.local_only {
        c.warning("localOnly", "Other devices on your network can reach the database. Windows Firewall will ask whether to allow MariaDB.");
    }
    c
}

/// MariaDB reads every option and stops at an unknown or invalid one, without starting a server. Like
/// the real start, it reads only this file (`--defaults-extra-file` would also read C:\my.ini and the
/// other global option files, so the check could pass or fail on settings the server never sees).
fn check(run: Run, tmp: &Path) -> Result<(), String> {
    let arg = format!("--defaults-file={}", tmp.display());
    let r = run(&paths::mariadbd_exe(), &[&arg, "--help", "--verbose"]);
    if r.ok() {
        return Ok(());
    }
    let errors: Vec<&str> = r.out.lines().filter(|l| l.contains("[ERROR]")).map(|l| l.rsplit("[ERROR] ").next().unwrap_or(l).trim()).take(3).collect();
    let detail = if errors.is_empty() { file::tail(&r.out, 4) } else { errors.join("\n") };
    Err(format!("MariaDB rejected the new settings, so nothing was changed.\n\n{detail}"))
}

/// Writes my.ini after MariaDB accepted a test copy.
pub fn save(path: &Path, base: &MariadbSettings, draft: &MariadbSettings, run: Run, slow_log: &Path, general_log: &Path) -> Result<bool, String> {
    let current = file::read(path)?;
    let updated = apply(&current, base, draft, &paths::fwd(slow_log), &paths::fwd(general_log));
    let changed = file::check_and_write(path, &current, &updated, "ini", |tmp| check(run, tmp))?;
    if changed && (draft.slow_query_log || draft.general_log) {
        // MariaDB does not create the folder of a log file.
        for f in [slow_log, general_log] {
            if let Some(dir) = f.parent() {
                let _ = std::fs::create_dir_all(dir);
            }
        }
    }
    Ok(changed)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::shell::Output;

    const MY: &str = "[mysqld]\r\ndatadir=C:/Lampion/databases\r\nport=3306\r\nbind-address=127.0.0.1\r\nlog-error=C:/Lampion/logs/mariadb/mariadb.err\r\n\r\n[client]\r\nport=3306\r\n";

    fn with(f: impl FnOnce(&mut MariadbSettings)) -> (MariadbSettings, MariadbSettings) {
        let base = parse(MY);
        let mut d = base.clone();
        f(&mut d);
        (base, d)
    }

    #[test]
    fn reads_values_empty_means_default() {
        let s = parse(MY);
        assert_eq!(s.port, "3306");
        assert!(s.local_only);
        assert_eq!(s.max_connections, "", "not set: the server default applies");
        assert_eq!(s.sql_mode, "default");
        let custom = parse("[mysqld]\nsql_mode = \"ANSI_QUOTES\"\n");
        assert_eq!((custom.sql_mode.as_str(), custom.custom_sql_mode.as_str()), ("custom", "ANSI_QUOTES"));
        assert_eq!(parse(&format!("[mysqld]\nsql-mode='{STRICT}'\n")).sql_mode, "strict");
    }

    #[test]
    fn unchanged_save_keeps_every_byte() {
        let (b, d) = with(|_| {});
        assert_eq!(apply(MY, &b, &d, "s", "g"), MY);
    }

    #[test]
    fn changes_go_into_mysqld() {
        let (b, d) = with(|s| {
            s.port = "3307".into();
            s.max_connections = "300".into();
            s.max_allowed_packet = "64m".into();
            s.sql_mode = "relaxed".into();
            s.slow_query_log = true;
            s.long_query_time = "2".into();
            s.local_only = false;
        });
        let out = apply(MY, &b, &d, "C:/Lampion/logs/mariadb/slow.log", "g");
        assert_eq!(
            out,
            format!(
                "[mysqld]\r\ndatadir=C:/Lampion/databases\r\nport=3307\r\nbind-address=0.0.0.0\r\nlog-error=C:/Lampion/logs/mariadb/mariadb.err\r\n\
\r\n{}\r\nmax_connections = 300\r\nmax_allowed_packet = 64M\r\nlong_query_time = 2\r\nsql_mode = \"NO_ENGINE_SUBSTITUTION\"\r\n\
slow_query_log = 1\r\nslow_query_log_file = \"C:/Lampion/logs/mariadb/slow.log\"\r\n\r\n[client]\r\nport=3306\r\n",
                super::super::ini::MY_INI_BLOCK
            )
        );
        let mut want = d.clone();
        want.max_allowed_packet = "64M".into();
        assert_eq!(parse(&out), want, "reads back what was saved");
    }

    #[test]
    fn emptied_fields_and_logs_off_are_commented_out() {
        let text = "[mysqld]\nport=3306\nmax_connections=500\nslow_query_log=1\nslow_query_log_file=C:/x/slow.log\n";
        let base = parse(text);
        let mut d = base.clone();
        d.max_connections = String::new();
        d.slow_query_log = false;
        assert_eq!(apply(text, &base, &d, "s", "g"), "[mysqld]\nport=3306\n#max_connections=500\n#slow_query_log=1\nslow_query_log_file=C:/x/slow.log\n");
    }

    #[test]
    fn custom_sql_mode_is_kept() {
        let text = "[mysqld]\nsql_mode=\"ANSI_QUOTES\"\nport=3306\n";
        let (base, mut d) = (parse(text), parse(text));
        d.port = "3308".into();
        assert_eq!(apply(text, &base, &d, "s", "g"), "[mysqld]\nsql_mode=\"ANSI_QUOTES\"\nport=3308\n");
    }

    #[test]
    fn validation() {
        let ok = MariadbSettings { local_only: true, ..Default::default() };
        assert!(validate(&ok).problems.is_empty() && validate(&ok).warnings.is_empty(), "empty fields are fine");
        let field = |f: fn(&mut MariadbSettings)| {
            let mut s = ok.clone();
            f(&mut s);
            validate(&s).problems.first().map(|p| p.field)
        };
        assert_eq!(field(|s| s.port = "80".into()), Some("port"));
        assert_eq!(field(|s| s.port = "3307".into()), None);
        assert_eq!(field(|s| s.max_connections = "0".into()), Some("maxConnections"));
        assert_eq!(field(|s| s.max_allowed_packet = "9000000000G".into()), Some("maxAllowedPacket"));
        assert_eq!(field(|s| s.buffer_pool_size = "-1".into()), Some("bufferPoolSize"));
        assert_eq!(field(|s| s.long_query_time = "0.5".into()), None);
        assert_eq!(field(|s| s.long_query_time = "-1".into()), Some("longQueryTime"));
        assert_eq!(field(|s| s.long_query_time = "NaN".into()), Some("longQueryTime"));
        assert_eq!(validate(&MariadbSettings::default()).warnings[0].field, "localOnly");
    }

    #[test]
    fn rejection_shows_mariadb_errors_and_keeps_the_file() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("my.ini");
        std::fs::write(&path, MY).unwrap();
        let (b, d) = with(|s| s.max_connections = "300".into());
        let args = std::cell::RefCell::new(Vec::<String>::new());
        let reject = |_: &Path, a: &[&str]| {
            *args.borrow_mut() = a.iter().map(|s| s.to_string()).collect();
            Output { status: 7, out: "2026-11-23 10:00:00 0 [ERROR] mariadbd: unknown variable 'max_connectionz=300'\n2026-11-23 10:00:00 0 [ERROR] Aborting\n".into() }
        };
        let logs = dir.path().join("logs");
        let err = save(&path, &b, &d, &reject, &logs.join("slow.log"), &logs.join("general.log")).unwrap_err();
        assert_eq!(err, "MariaDB rejected the new settings, so nothing was changed.\n\nmariadbd: unknown variable 'max_connectionz=300'\nAborting");
        assert!(args.borrow()[0].starts_with("--defaults-file="), "the same option file only, as the server starts: {:?}", args.borrow());
        assert_eq!(&args.borrow()[1..], ["--help", "--verbose"]);
        assert_eq!(std::fs::read_to_string(&path).unwrap(), MY);

        let accept = |_: &Path, _: &[&str]| Output { status: 0, out: String::new() };
        assert_eq!(save(&path, &b, &d, &accept, &logs.join("slow.log"), &logs.join("general.log")), Ok(true));
        assert!(std::fs::read_to_string(&path).unwrap().contains("max_connections = 300\r\n"));
        assert!(file::has_backup(&path));
    }
}
