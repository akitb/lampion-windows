//! php.ini: the PHP settings page. Saving writes only the keys whose value changed; PHP itself checks
//! a temp copy first (`php.exe -c <copy>`), and a warning counts as an error.

use super::file::{self, Check, Run};
use super::ini::{truthy, Dialect, Ini};
use super::timezones;
use crate::core::paths;
use serde::{Deserialize, Serialize};
use std::path::Path;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", default)]
pub struct PhpSettings {
    pub memory_limit: String,
    pub upload_max_filesize: String,
    pub post_max_size: String,
    pub max_execution_time: String,
    pub max_input_time: String,
    pub max_input_vars: String,
    pub display_errors: bool,
    pub display_startup_errors: bool,
    pub error_reporting: String,
    pub log_errors: bool,
    /// `error_log` is a file (Lampion's `logs\php\php_errors.log` when it sets it); off: Apache's error log.
    pub separate_error_log: bool,
    /// `date.timezone`; empty = not set.
    pub timezone: String,
    pub short_open_tag: bool,
    pub opcache: bool,
    pub session_lifetime: String,
}

impl Default for PhpSettings {
    fn default() -> Self {
        PhpSettings {
            memory_limit: "128M".into(),
            upload_max_filesize: "2M".into(),
            post_max_size: "8M".into(),
            max_execution_time: "30".into(),
            max_input_time: "60".into(),
            max_input_vars: "1000".into(),
            display_errors: true,
            display_startup_errors: true,
            error_reporting: "E_ALL".into(),
            log_errors: true,
            separate_error_log: false,
            timezone: String::new(),
            short_open_tag: false,
            opcache: true,
            session_lifetime: "1440".into(),
        }
    }
}

pub fn parse(text: &str) -> PhpSettings {
    let ini = Ini::new(text, Dialect::Php);
    let d = PhpSettings::default();
    let s = |k: &str, def: String| ini.value(k).unwrap_or(def);
    let b = |k: &str, def: bool| ini.value(k).map(|v| truthy(&v)).unwrap_or(def);
    PhpSettings {
        memory_limit: s("memory_limit", d.memory_limit),
        upload_max_filesize: s("upload_max_filesize", d.upload_max_filesize),
        post_max_size: s("post_max_size", d.post_max_size),
        max_execution_time: s("max_execution_time", d.max_execution_time),
        max_input_time: s("max_input_time", d.max_input_time),
        max_input_vars: s("max_input_vars", d.max_input_vars),
        display_errors: b("display_errors", d.display_errors),
        display_startup_errors: b("display_startup_errors", d.display_startup_errors),
        error_reporting: s("error_reporting", d.error_reporting),
        log_errors: b("log_errors", d.log_errors),
        // A file path (not `syslog`) means PHP keeps its own error log.
        separate_error_log: ini.value("error_log").is_some_and(|v| Path::new(&v).is_absolute()),
        timezone: s("date.timezone", d.timezone),
        short_open_tag: b("short_open_tag", d.short_open_tag),
        opcache: b("opcache.enable", d.opcache),
        session_lifetime: s("session.gc_maxlifetime", d.session_lifetime),
    }
}

fn size(v: &str) -> String {
    v.trim().to_ascii_uppercase()
}

fn number(v: &str) -> String {
    v.trim().parse::<i64>().map(|n| n.to_string()).unwrap_or_else(|_| v.trim().to_string())
}

fn on_off(b: bool) -> String {
    (if b { "On" } else { "Off" }).into()
}

/// Writes the settings the user changed (`draft` differs from `base`, the values the page loaded)
/// that the file does not already hold. `error_log` is the file used for "Separate PHP error log".
pub fn apply(text: &str, base: &PhpSettings, draft: &PhpSettings, error_log: &str) -> String {
    let cur = parse(text);
    let mut ini = Ini::new(text, Dialect::Php);
    // (key, normalised value of a settings struct): written when the draft's differs from both others.
    let fields: [(&str, fn(&PhpSettings) -> String); 13] = [
        ("memory_limit", |s| size(&s.memory_limit)),
        ("upload_max_filesize", |s| size(&s.upload_max_filesize)),
        ("post_max_size", |s| size(&s.post_max_size)),
        ("max_execution_time", |s| number(&s.max_execution_time)),
        ("max_input_time", |s| number(&s.max_input_time)),
        ("max_input_vars", |s| number(&s.max_input_vars)),
        ("display_errors", |s| on_off(s.display_errors)),
        ("display_startup_errors", |s| on_off(s.display_startup_errors)),
        ("error_reporting", |s| s.error_reporting.trim().to_string()),
        ("log_errors", |s| on_off(s.log_errors)),
        ("short_open_tag", |s| on_off(s.short_open_tag)),
        ("opcache.enable", |s| (if s.opcache { "1" } else { "0" }).into()),
        ("session.gc_maxlifetime", |s| number(&s.session_lifetime)),
    ];
    for (key, value) in fields {
        let v = value(draft);
        if v != value(base) && v != value(&cur) {
            ini.set(key, &v);
        }
    }
    let d = draft.separate_error_log;
    if d != base.separate_error_log && d != cur.separate_error_log {
        if d {
            ini.set("error_log", &format!("\"{error_log}\""));
        } else {
            ini.unset("error_log");
        }
    }
    let tz = |s: &PhpSettings| s.timezone.trim().to_lowercase();
    if tz(draft) != tz(base) && tz(draft) != tz(&cur) {
        match draft.timezone.trim() {
            "" => ini.unset("date.timezone"),
            name => ini.set("date.timezone", timezones::find(name).unwrap_or(name)),
        }
    }
    ini.text()
}

pub fn validate(s: &PhpSettings) -> Check {
    let mut c = Check::default();
    let int = |v: &str| v.trim().parse::<i64>().ok();
    if !(s.memory_limit.trim() == "-1" || file::is_positive_size(&s.memory_limit)) {
        c.problem("memoryLimit", "Memory limit must be a size such as 128M, 1G or -1 (no limit).");
    }
    if !file::is_positive_size(&s.upload_max_filesize) {
        c.problem("uploadMaxFilesize", "Maximum upload size must be a size such as 2M or 64M.");
    }
    if !(matches!(s.post_max_size.trim(), "0" | "-1") || file::is_positive_size(&s.post_max_size)) {
        c.problem("postMaxSize", "Maximum POST size must be a size such as 8M, or 0 for no limit.");
    } else if let (Some(upload), Some(post)) = (file::bytes(&s.upload_max_filesize), file::bytes(&s.post_max_size)) {
        if post != 0 && post < upload {
            c.problem("postMaxSize", "Maximum POST size must be at least the maximum upload size, or uploads of that size fail.");
        }
    }
    if !int(&s.max_execution_time).is_some_and(|n| n >= 0) {
        c.problem("maxExecutionTime", "Maximum run time must be 0 (no limit) or more seconds.");
    }
    if !int(&s.max_input_time).is_some_and(|n| n >= -1) {
        c.problem("maxInputTime", "Maximum input time must be -1 (use the run time) or more seconds.");
    }
    if !int(&s.max_input_vars).is_some_and(|n| n >= 1) {
        c.problem("maxInputVars", "Maximum input variables must be 1 or more.");
    }
    if !int(&s.session_lifetime).is_some_and(|n| n >= 1) {
        c.problem("sessionLifetime", "Session lifetime must be 1 or more seconds.");
    }
    let tz = s.timezone.trim();
    if !tz.is_empty() && timezones::find(tz).is_none() {
        c.problem("timezone", format!("PHP does not know the time zone {tz}. Use a name such as Europe/Budapest."));
    }
    c
}

/// PHP reads the copy; startup warnings are shown (and not logged) so they reach the output.
fn check(run: Run, tmp: &Path) -> Result<(), String> {
    let tmp = tmp.to_string_lossy();
    let args = ["-c", &tmp, "-d", "display_startup_errors=1", "-d", "display_errors=1", "-d", "log_errors=0", "-r", "echo 'ok';"];
    let r = run(&paths::php_exe(), &args);
    let warned = r.out.lines().any(|l| l.contains("Warning") || l.contains("Error"));
    if r.ok() && r.out.trim_end().ends_with("ok") && !warned {
        Ok(())
    } else {
        Err(format!("PHP rejected the new settings, so nothing was changed.\n\n{}", r.out.trim()))
    }
}

/// Writes php.ini after PHP accepted a test copy.
pub fn save(path: &Path, base: &PhpSettings, draft: &PhpSettings, run: Run, error_log: &Path) -> Result<bool, String> {
    let current = file::read(path)?;
    let updated = apply(&current, base, draft, &paths::fwd(error_log));
    let changed = file::check_and_write(path, &current, &updated, "ini", |tmp| check(run, tmp))?;
    if changed && draft.separate_error_log && !error_log.exists() {
        if let Some(dir) = error_log.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        let _ = std::fs::OpenOptions::new().create(true).append(true).open(error_log);
    }
    Ok(changed)
}

/// PHP's version and loaded extensions (`php -m`), as PHP reads Lampion's php.ini.
pub fn modules(run: Run, ini: &Path) -> Result<(String, Vec<String>), String> {
    let ini = ini.to_string_lossy();
    let code = "echo '--lampion--', PHP_EOL, PHP_VERSION, PHP_EOL, implode(PHP_EOL, array_merge(get_loaded_extensions(), get_loaded_extensions(true)));";
    let args = ["-c", &ini, "-d", "display_startup_errors=0", "-d", "display_errors=0", "-d", "log_errors=0", "-r", code];
    let r = run(&paths::php_exe(), &args);
    let body = r.out.split_once("--lampion--").map(|(_, b)| b).filter(|_| r.ok());
    let Some(body) = body else {
        return Err(format!("PHP did not answer: {}", file::tail(&r.out, 3)));
    };
    let mut lines = body.lines().map(str::trim).filter(|l| !l.is_empty());
    let version = lines.next().unwrap_or_default().to_string();
    let mut mods: Vec<String> = lines.map(String::from).collect();
    mods.sort_by_key(|m| m.to_lowercase());
    mods.dedup_by_key(|m| m.to_lowercase());
    Ok((version, mods))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::shell::Output;

    const INI: &str = "[PHP]\r\n\
memory_limit = 128M\r\n\
upload_max_filesize = 2M\r\n\
post_max_size = 8M\r\n\
max_execution_time = 30\r\n\
display_errors = On\r\n\
error_reporting = E_ALL\r\n\
log_errors = On\r\n\
; Example:\r\n\
;error_log = php_errors.log\r\n\
;error_log = syslog\r\n\
[Date]\r\n\
;date.timezone =\r\n\
[opcache]\r\n\
;opcache.enable=1\r\n";

    fn with(f: impl FnOnce(&mut PhpSettings)) -> (PhpSettings, PhpSettings) {
        let base = parse(INI);
        let mut d = base.clone();
        f(&mut d);
        (base, d)
    }

    #[test]
    fn reads_values_and_defaults() {
        let s = parse(INI);
        assert_eq!(s.memory_limit, "128M");
        assert_eq!(s.max_input_vars, "1000", "missing → PHP's default");
        assert!(s.display_errors && s.log_errors && !s.separate_error_log && s.opcache);
        assert_eq!(s.timezone, "");
        assert!(parse("error_log = \"C:/Lampion/logs/php/php_errors.log\"\n").separate_error_log);
        assert!(!parse("error_log = syslog\n").separate_error_log);
    }

    #[test]
    fn unchanged_save_keeps_every_byte() {
        let (b, d) = with(|_| {});
        assert_eq!(apply(INI, &b, &d, "C:/L/php.log"), INI);
    }

    #[test]
    fn only_changed_keys_are_written() {
        let (b, d) = with(|s| {
            s.memory_limit = "256m".into();
            s.timezone = "europe/budapest".into();
            s.separate_error_log = true;
            s.opcache = false;
        });
        let out = apply(INI, &b, &d, "C:/Lampion/logs/php/php_errors.log");
        assert_eq!(
            out,
            INI.replace("memory_limit = 128M", "memory_limit = 256M")
                .replace(";error_log = php_errors.log", "error_log = \"C:/Lampion/logs/php/php_errors.log\"")
                .replace(";date.timezone =", "date.timezone = Europe/Budapest")
                .replace(";opcache.enable=1", "opcache.enable = 0")
        );
        let back = parse(&out);
        assert!(back.separate_error_log && !back.opcache);
        assert_eq!(back.timezone, "Europe/Budapest");
    }

    #[test]
    fn turning_options_off_comments_them_out() {
        let text = "error_log = \"C:/x/php.log\"\ndate.timezone = UTC\n";
        let base = parse(text);
        let mut d = base.clone();
        d.separate_error_log = false;
        d.timezone = String::new();
        assert_eq!(apply(text, &base, &d, "C:/x/php.log"), ";error_log = \"C:/x/php.log\"\n;date.timezone = UTC\n");
    }

    #[test]
    fn quoted_value_with_semicolon() {
        // Pitfall: `"a;b" ; comment` was read wrongly; the value runs to the closing quote.
        let text = "error_reporting = \"E_ALL;x\" ; comment\n";
        assert_eq!(parse(text).error_reporting, "E_ALL;x");
        let base = parse(text);
        assert_eq!(apply(text, &base, &base.clone(), "x"), text);
    }

    #[test]
    fn validation() {
        let ok = PhpSettings { timezone: "Europe/Budapest".into(), ..Default::default() };
        assert!(validate(&ok).problems.is_empty());
        let field = |f: fn(&mut PhpSettings)| {
            let mut s = ok.clone();
            f(&mut s);
            validate(&s).problems.first().map(|p| p.field)
        };
        assert_eq!(field(|s| s.memory_limit = "-1".into()), None);
        assert_eq!(field(|s| s.memory_limit = "0".into()), Some("memoryLimit"));
        // Pitfall: an oversized value overflowed; it is just invalid.
        assert_eq!(field(|s| s.memory_limit = "9000000000G".into()), Some("memoryLimit"));
        assert_eq!(field(|s| s.upload_max_filesize = "-1".into()), Some("uploadMaxFilesize"));
        assert_eq!(field(|s| s.post_max_size = "0".into()), None);
        assert_eq!(field(|s| s.post_max_size = "1M".into()), Some("postMaxSize"), "smaller than the upload size");
        assert_eq!(field(|s| s.max_execution_time = "-1".into()), Some("maxExecutionTime"));
        assert_eq!(field(|s| s.max_input_time = "-1".into()), None);
        assert_eq!(field(|s| s.max_input_vars = "0".into()), Some("maxInputVars"));
        assert_eq!(field(|s| s.session_lifetime = "x".into()), Some("sessionLifetime"));
        assert_eq!(field(|s| s.timezone = "GMT+2".into()), Some("timezone"));
        assert_eq!(field(|s| s.timezone = "CET".into()), None);
        assert_eq!(field(|s| s.timezone = "US/Pacific".into()), None);
        assert_eq!(field(|s| s.timezone = "".into()), None);
    }

    #[test]
    fn php_check_and_rejection() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("php.ini");
        std::fs::write(&path, INI).unwrap();
        let log = dir.path().join("logs").join("php_errors.log");
        let (b, d) = with(|s| {
            s.timezone = "Europe/Budapest".into();
            s.separate_error_log = true;
        });
        let seen = std::cell::RefCell::new(String::new());
        let warn = |_: &Path, a: &[&str]| {
            *seen.borrow_mut() = std::fs::read_to_string(a[1]).unwrap();
            Output { status: 0, out: "PHP Warning:  PHP Startup: Unable to load dynamic library 'x'\nok".into() }
        };
        let err = save(&path, &b, &d, &warn, &log).unwrap_err();
        assert!(err.starts_with("PHP rejected") && err.contains("Unable to load"), "a warning is an error: {err}");
        assert!(seen.borrow().contains("date.timezone = Europe/Budapest"), "PHP checked the new text");
        assert_eq!(std::fs::read_to_string(&path).unwrap(), INI, "the real file is unchanged");
        assert!(!log.exists());

        let fine = |_: &Path, _: &[&str]| Output { status: 0, out: "ok".into() };
        assert_eq!(save(&path, &b, &d, &fine, &log), Ok(true));
        assert!(log.exists(), "the separate log file is created");
        assert_eq!(std::fs::read_to_string(file::backup_path(&path)).unwrap(), INI);
    }

    #[test]
    fn modules_list() {
        let fake = |_: &Path, _: &[&str]| Output { status: 0, out: "--lampion--\r\n8.4.1\r\nCore\r\nmysqli\r\nPDO\r\nZend OPcache\r\nmysqli\r\n".into() };
        let (v, m) = modules(&fake, Path::new("php.ini")).unwrap();
        assert_eq!(v, "8.4.1");
        assert_eq!(m, ["Core", "mysqli", "PDO", "Zend OPcache"]);
        let broken = |_: &Path, _: &[&str]| Output { status: -1, out: "php.exe was not found.".into() };
        assert!(modules(&broken, Path::new("php.ini")).unwrap_err().contains("not found"));
    }
}
