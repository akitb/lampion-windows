//! PHP for the Overview's PHP card: its version, whether Apache runs it, and two limits from php.ini.

use super::configs;
use crate::core::{activity, paths, shell};
use once_cell::sync::Lazy;
use serde::Serialize;
use std::sync::Mutex;
use std::time::Duration;

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct PhpInfo {
    pub installed: bool,
    pub version: Option<String>,
    /// httpd.conf loads PHP (`LoadModule php_module`) and Apache runs.
    pub linked: bool,
    pub memory_limit: Option<String>,
    pub upload_max_filesize: Option<String>,
}

/// `None` = not asked yet; `Some(None)` = php.exe did not say.
static VERSION: Lazy<Mutex<Option<Option<String>>>> = Lazy::new(|| Mutex::new(None));

/// An active `LoadModule php_module …` line.
pub fn loads_php(conf: &str) -> bool {
    conf.lines().any(|l| {
        let mut w = l.split_whitespace();
        w.next().is_some_and(|d| d.eq_ignore_ascii_case("LoadModule")) && w.next() == Some("php_module")
    })
}

pub fn version() -> Option<String> {
    let mut v = VERSION.lock().unwrap_or_else(|e| e.into_inner());
    if v.is_none() {
        // -n: no php.ini, so a broken extension line cannot add warnings to the answer.
        let out = shell::run_timeout(paths::php_exe(), &["-n", "-r", "echo PHP_VERSION;"], Duration::from_secs(10));
        *v = Some(out.ok().then(|| out.out.trim().to_string()).filter(|s| !s.is_empty()));
    }
    v.clone().flatten()
}

/// After setup or a move the program may be a different one.
pub fn forget() {
    *VERSION.lock().unwrap_or_else(|e| e.into_inner()) = None;
}

/// Blocking (runs php.exe once): call it off the UI thread.
pub fn info() -> PhpInfo {
    let installed = paths::php_exe().is_file();
    let ini = std::fs::read_to_string(paths::php_ini()).unwrap_or_default();
    let conf = std::fs::read_to_string(paths::httpd_conf()).unwrap_or_default();
    PhpInfo {
        installed,
        version: if installed { version() } else { None },
        // Like macOS: "Linked" means httpd.conf loads PHP, whether or not Apache runs right now.
        linked: loads_php(&conf),
        memory_limit: configs::ini_value(&ini, "memory_limit"),
        upload_max_filesize: configs::ini_value(&ini, "upload_max_filesize"),
    }
}

/// One line for the activity log at startup.
pub fn log_summary() {
    if !paths::php_exe().is_file() {
        return;
    }
    let conf = std::fs::read_to_string(paths::httpd_conf()).unwrap_or_default();
    match (version(), loads_php(&conf)) {
        (Some(v), true) => activity::info("PHP", &format!("PHP {v} – linked into Apache")),
        (Some(v), false) => activity::warn("PHP", &format!("PHP {v} – NOT linked into Apache (httpd.conf has no LoadModule php_module)")),
        (None, _) => activity::error("PHP", &format!("PHP does not run ({})", paths::php_exe().display())),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn php_module_line() {
        assert!(loads_php("LoadModule rewrite_module x\r\nLoadModule php_module \"C:/Lampion/bin/php/php8apache2_4.dll\"\r\n"));
        assert!(!loads_php("#LoadModule php_module x\n"));
        assert!(!loads_php("LoadModule php7_module x\n"));
        assert!(loads_php(&configs::httpd_conf(std::path::Path::new(r"C:\Lampion"), 8080)));
    }
}
