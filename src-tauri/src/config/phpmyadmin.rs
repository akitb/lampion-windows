//! phpMyAdmin's config.inc.php: automatic sign-in as root (auth_type 'config') or the login page.
//! phpMyAdmin talks to MariaDB over TCP (127.0.0.1 and the real port). Only this computer can open
//! phpMyAdmin: that is Apache's `Require local` on the /phpmyadmin alias.

use super::file::{self, indent, Doc, Run};
use crate::core::{activity, paths, settings};
use rand::{distributions::Alphanumeric, Rng};
use std::path::Path;

const MARKER: &str = "// Lampion: automatic sign-in";

fn is_comment(t: &str) -> bool {
    t.starts_with("//") || t.starts_with('#') || t.starts_with("/*") || t.starts_with('*')
}

/// The first live line that sets `$cfg['Servers'][$i]['<key>']`.
fn server_line(doc: &Doc, key: &str) -> Option<usize> {
    let needle = format!("['{key}']");
    (0..doc.len()).find(|&i| {
        let t = doc.line(i).trim();
        !is_comment(t) && t.contains("['Servers']") && t.contains(&needle) && t.contains('=')
    })
}

/// Sets `$cfg['Servers'][$i]['<key>'] = <value>;`: replaces the line, else adds it after `after`
/// (else after the last `$i++;`, else at the end). Returns its line.
fn set_server_value(doc: &mut Doc, key: &str, value: &str, after: Option<usize>) -> usize {
    let line = format!("$cfg['Servers'][$i]['{key}'] = {value};");
    if let Some(i) = server_line(doc, key) {
        let new = format!("{}{line}", indent(doc.line(i)));
        doc.replace(i, new);
        return i;
    }
    let at = after
        .or_else(|| (0..doc.len()).rev().find(|&i| doc.line(i).trim().starts_with("$i++")))
        .map(|i| i + 1)
        .or_else(|| (0..doc.len()).rev().find(|&i| doc.line(i).trim() == "?>"))
        .unwrap_or(doc.len());
    doc.insert(at, line);
    server_line(doc, key).unwrap_or(at)
}

/// True when phpMyAdmin opens without a login page.
pub fn is_auto_login(text: &str) -> bool {
    let doc = Doc::new(text);
    server_line(&doc, "auth_type").is_some_and(|i| {
        let v = doc.line(i).split_once('=').map(|(_, v)| v.trim_start()).unwrap_or("");
        v.starts_with("'config'") || v.starts_with("\"config\"")
    })
}

/// The new text: host 127.0.0.1 and the MariaDB port always; with `auto`, the sign-in lines too.
pub fn apply(text: &str, auto: Option<bool>, port: u16) -> String {
    let mut doc = Doc::new(text);
    let mut auth = server_line(&doc, "auth_type");
    if let Some(on) = auto {
        let mut i = 0;
        while i < doc.len() {
            if doc.line(i).contains(MARKER) {
                doc.remove(i);
            } else {
                i += 1;
            }
        }
        let at = set_server_value(&mut doc, "auth_type", if on { "'config'" } else { "'cookie'" }, None);
        set_server_value(&mut doc, "AllowNoPassword", if on { "true" } else { "false" }, Some(at));
        if on {
            doc.insert(at + 1, format!("$cfg['Servers'][$i]['user'] = 'root'; {MARKER}"));
            doc.insert(at + 2, format!("$cfg['Servers'][$i]['password'] = ''; {MARKER}"));
        }
        auth = Some(at);
    }
    set_server_value(&mut doc, "host", "'127.0.0.1'", auth);
    if port != 3306 || server_line(&doc, "port").is_some() {
        let host = server_line(&doc, "host");
        set_server_value(&mut doc, "port", &format!("'{port}'"), host);
    }
    doc.text()
}

/// config.sample.inc.php with a random 32-byte blowfish_secret (phpMyAdmin needs exactly 32 bytes).
fn from_sample(sample: &str) -> String {
    let secret: String = rand::thread_rng().sample_iter(&Alphanumeric).take(32).map(char::from).collect();
    let mut doc = Doc::new(sample);
    let line = format!("$cfg['blowfish_secret'] = '{secret}'; /* Lampion: random, 32 bytes */");
    match (0..doc.len()).find(|&i| !is_comment(doc.line(i).trim()) && doc.line(i).contains("['blowfish_secret']")) {
        Some(i) => {
            let new = format!("{}{line}", indent(doc.line(i)));
            doc.replace(i, new);
        }
        None => doc.insert(1, line),
    }
    doc.text()
}

/// Changes config.inc.php in `dir` (created from the sample when missing) after `php -l` accepted it.
fn write(dir: &Path, auto: Option<bool>, port: u16, run: Run) -> Result<bool, String> {
    let path = dir.join("config.inc.php");
    let (current, start) = if path.exists() {
        let t = file::read(&path)?;
        (t.clone(), t)
    } else {
        let sample = file::read(&dir.join("config.sample.inc.php")).map_err(|_| "phpMyAdmin is not installed.".to_string())?;
        (String::new(), from_sample(&sample))
    };
    let updated = apply(&start, auto, port);
    file::check_and_write(&path, &current, &updated, "php", |tmp| {
        let tmp = tmp.to_string_lossy();
        let r = run(&paths::php_exe(), &["-n", "-l", &tmp]);
        if r.ok() {
            Ok(())
        } else {
            Err(format!("The phpMyAdmin configuration would not be valid, so nothing was changed.\n\n{}", file::tail(&r.out, 3)))
        }
    })
}

/// `None` when phpMyAdmin is not installed.
pub fn state() -> Option<bool> {
    let dir = paths::phpmyadmin_dir();
    (dir.join("config.inc.php").is_file() || dir.join("config.sample.inc.php").is_file()).then(auto_login_enabled)
}

pub fn auto_login_enabled() -> bool {
    file::read(&paths::pma_config()).is_ok_and(|t| is_auto_login(&t))
}

/// Writes config.inc.php (checked with `php -l` first). Creates it from config.sample.inc.php with a
/// random 32-byte blowfish_secret when missing.
pub fn set_auto_login(enabled: bool) -> Result<(), String> {
    write(&paths::phpmyadmin_dir(), Some(enabled), super::locations::current().mariadb_port, &file::run_real)?;
    // The user's choice: first-launch setup never overrides it again.
    settings::update(|s| s.phpmyadmin_auto_login = Some(enabled));
    activity::ok("MySQL", if enabled { "phpMyAdmin now signs in automatically as root" } else { "phpMyAdmin asks for a login again" });
    Ok(())
}

/// Points phpMyAdmin at MariaDB's port after it changed (only when config.inc.php exists).
pub fn sync_port(port: u16) -> Result<bool, String> {
    let dir = paths::phpmyadmin_dir();
    if !dir.join("config.inc.php").is_file() {
        return Ok(false);
    }
    write(&dir, None, port, &file::run_real)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::shell::Output;

    const SAMPLE: &str = "<?php\r\ndeclare(strict_types=1);\r\n\r\n$cfg['blowfish_secret'] = ''; /* YOU MUST FILL IN THIS FOR COOKIE AUTH! */\r\n\
$i = 0;\r\n$i++;\r\n/* Authentication type */\r\n$cfg['Servers'][$i]['auth_type'] = 'cookie';\r\n/* Server parameters */\r\n\
$cfg['Servers'][$i]['host'] = 'localhost';\r\n$cfg['Servers'][$i]['compress'] = false;\r\n$cfg['Servers'][$i]['AllowNoPassword'] = false;\r\n\
// $cfg['Servers'][$i]['controlhost'] = '';\r\n// $cfg['Servers'][$i]['controlport'] = '';\r\n$cfg['UploadDir'] = '';\r\n";

    #[test]
    fn on_and_off() {
        let on = apply(SAMPLE, Some(true), 3306);
        assert!(is_auto_login(&on) && !is_auto_login(SAMPLE));
        assert!(on.contains(
            "$cfg['Servers'][$i]['auth_type'] = 'config';\r\n\
$cfg['Servers'][$i]['user'] = 'root'; // Lampion: automatic sign-in\r\n\
$cfg['Servers'][$i]['password'] = ''; // Lampion: automatic sign-in\r\n\
/* Server parameters */\r\n\
$cfg['Servers'][$i]['host'] = '127.0.0.1';\r\n\
$cfg['Servers'][$i]['compress'] = false;\r\n\
$cfg['Servers'][$i]['AllowNoPassword'] = true;\r\n"
        ), "{on}");
        assert!(!on.contains("['port']"), "the default port needs no line");
        assert!(on.contains("// $cfg['Servers'][$i]['controlhost'] = '';"), "comments untouched");

        let off = apply(&on, Some(false), 3306);
        assert!(!is_auto_login(&off) && !off.contains(MARKER));
        assert!(off.contains("['auth_type'] = 'cookie';") && off.contains("['AllowNoPassword'] = false;"));
        assert_eq!(apply(&off, Some(false), 3306), off, "nothing left to change");
    }

    #[test]
    fn port_follows_mariadb() {
        let on = apply(SAMPLE, Some(true), 3307);
        assert!(on.contains("$cfg['Servers'][$i]['host'] = '127.0.0.1';\r\n$cfg['Servers'][$i]['port'] = '3307';\r\n"));
        let back = apply(&on, None, 3306);
        assert!(back.contains("['port'] = '3306';") && is_auto_login(&back), "only the port changed");
    }

    #[test]
    fn missing_lines_are_added() {
        let text = "<?php\n$i = 0;\n$i++;\n$cfg['Servers'][$i]['host'] = '127.0.0.1';\n";
        let on = apply(text, Some(true), 3306);
        assert_eq!(
            on,
            "<?php\n$i = 0;\n$i++;\n$cfg['Servers'][$i]['auth_type'] = 'config';\n$cfg['Servers'][$i]['user'] = 'root'; // Lampion: automatic sign-in\n\
$cfg['Servers'][$i]['password'] = ''; // Lampion: automatic sign-in\n$cfg['Servers'][$i]['AllowNoPassword'] = true;\n$cfg['Servers'][$i]['host'] = '127.0.0.1';\n"
        );
    }

    #[test]
    fn created_from_the_sample_after_php_lint() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("config.sample.inc.php"), SAMPLE).unwrap();
        let bad = |_: &Path, _: &[&str]| Output { status: 255, out: "PHP Parse error: syntax error".into() };
        assert!(write(dir.path(), Some(true), 3306, &bad).unwrap_err().contains("Parse error"));
        assert!(!dir.path().join("config.inc.php").exists(), "nothing written on a lint error");

        let args = std::cell::RefCell::new(Vec::<String>::new());
        let lint = |_: &Path, a: &[&str]| {
            *args.borrow_mut() = a.iter().map(|s| s.to_string()).collect();
            Output { status: 0, out: "No syntax errors detected".into() }
        };
        assert_eq!(write(dir.path(), Some(true), 3306, &lint), Ok(true));
        assert_eq!(&args.borrow()[..2], ["-n", "-l"]);
        let text = std::fs::read_to_string(dir.path().join("config.inc.php")).unwrap();
        assert!(is_auto_login(&text));
        let secret = text.split("['blowfish_secret'] = '").nth(1).unwrap().split('\'').next().unwrap();
        assert_eq!(secret.len(), 32);
        assert!(secret.bytes().all(|b| b.is_ascii_alphanumeric()));
        assert!(!file::has_backup(&dir.path().join("config.inc.php")), "a new file has no original");
    }
}
