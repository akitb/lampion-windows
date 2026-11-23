//! Where things are now, read from the real configuration files (never hard-coded),
//! so a hand-edited httpd.conf / my.ini / php.ini is respected.

use crate::core::paths;
use serde::Serialize;
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Locations {
    pub apache_port: u16,
    pub document_root: PathBuf,
    pub apache_error_log: PathBuf,
    pub apache_access_log: PathBuf,
    pub mariadb_port: u16,
    pub mariadb_datadir: PathBuf,
    /// `log-error`, else the newest `*.err` in the data folder.
    pub mariadb_error_log: Option<PathBuf>,
    /// php.ini `error_log` when set.
    pub php_error_log: Option<PathBuf>,
    pub slow_query_log: Option<PathBuf>,
    pub general_query_log: Option<PathBuf>,
}

/// Reads the locations fresh from the files (cheap; call it whenever a port or path is needed).
/// shortcut: returns the defaults; the config module replaces this with real parsing.
pub fn current() -> Locations {
    Locations {
        apache_port: 8080,
        document_root: paths::www(),
        apache_error_log: paths::logs().join("apache").join("error.log"),
        apache_access_log: paths::logs().join("apache").join("access.log"),
        mariadb_port: 3306,
        mariadb_datadir: paths::databases(),
        mariadb_error_log: Some(paths::logs().join("mariadb").join("mariadb.err")),
        php_error_log: Some(paths::logs().join("php").join("php_errors.log")),
        slow_query_log: None,
        general_query_log: None,
    }
}
