//! Where things are now, read from the real configuration files (never hard-coded),
//! so a hand-edited httpd.conf / my.ini / php.ini is respected.

use super::apache::{split_listen, Conf};
use super::file;
use super::ini::{Dialect, Ini};
use crate::core::paths;
use serde::Serialize;
use std::path::{Path, PathBuf};

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
pub fn current() -> Locations {
    let read = |p: PathBuf| file::read(&p).ok();
    let mut l = from_texts(read(paths::httpd_conf()).as_deref(), read(paths::my_ini()).as_deref(), read(paths::php_ini()).as_deref());
    if l.mariadb_error_log.is_none() {
        l.mariadb_error_log = newest_err(&l.mariadb_datadir);
    }
    l
}

/// A config path as a Windows path.
fn win(p: &str) -> PathBuf {
    PathBuf::from(p.replace('/', "\\"))
}

/// Everything the three files say (missing files: Lampion's defaults).
fn from_texts(httpd: Option<&str>, my_ini: Option<&str>, php_ini: Option<&str>) -> Locations {
    let conf = Conf::new(httpd.unwrap_or(""));
    let apache_logs = paths::logs().join("apache");
    let (error_log, access_log) = match httpd {
        // Apache's built-in ErrorLog is logs/error.log under ServerRoot.
        Some(_) => (conf.log_file("ErrorLog").unwrap_or_else(|| conf.resolve("logs/error.log")), conf.log_file("CustomLog").map(|p| win(&p))),
        None => (paths::fwd(&apache_logs.join("error.log")), None),
    };

    let my = Ini::new(my_ini.unwrap_or(""), Dialect::MyIni);
    let datadir = my
        .value("datadir")
        .filter(|d| !d.trim().is_empty())
        .map(|d| win(d.trim().trim_end_matches(|c| c == '/' || c == '\\')))
        .unwrap_or_else(paths::databases);
    // MariaDB puts relative log names in the data folder.
    let in_data = |v: String| {
        let p = win(&v);
        if p.is_absolute() { p } else { datadir.join(p) }
    };
    let set = |key: &str| my.value(key).filter(|v| !v.trim().is_empty() && v != "1");
    let mariadb_error_log = set("log-error").map(|v| {
        let p = in_data(v);
        if p.extension().is_none() { p.with_extension("err") } else { p }
    });

    // shortcut: a relative php.ini error_log is taken as relative to Apache's ServerRoot (the servers
    // part starts Apache there); upgrade if Apache ever runs from another working folder.
    let php_error_log = php_ini
        .and_then(|t| Ini::new(t, Dialect::Php).value("error_log"))
        .filter(|v| !v.trim().is_empty() && !v.trim().eq_ignore_ascii_case("syslog"))
        .map(|v| win(&conf.resolve(&v)));

    Locations {
        apache_port: conf.value("Listen").and_then(|l| split_listen(&l).1).unwrap_or(8080),
        document_root: conf.value("DocumentRoot").map(|r| win(&conf.resolve(&r))).unwrap_or_else(paths::www),
        apache_error_log: win(&error_log),
        apache_access_log: access_log.unwrap_or_else(|| apache_logs.join("access.log")),
        mariadb_port: my.value("port").and_then(|p| p.trim().parse().ok()).unwrap_or(3306),
        mariadb_error_log,
        php_error_log,
        slow_query_log: set("slow_query_log_file").map(in_data),
        general_query_log: set("general_log_file").map(in_data),
        mariadb_datadir: datadir,
    }
}

/// MariaDB's default log, `<computer name>.err` in the data folder (the newest, as the name can change).
fn newest_err(datadir: &Path) -> Option<PathBuf> {
    std::fs::read_dir(datadir)
        .ok()?
        .filter_map(Result::ok)
        .filter(|e| e.path().extension().is_some_and(|x| x.eq_ignore_ascii_case("err")))
        .max_by_key(|e| e.metadata().and_then(|m| m.modified()).ok())
        .map(|e| e.path())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_the_real_files() {
        let httpd = "Define SRVROOT \"C:/Lampion/bin/apache\"\nServerRoot \"${SRVROOT}\"\nListen [::1]:8081\n\
DocumentRoot \"D:/Sites\"\nErrorLog \"logs/error.log\"\n<VirtualHost *:8081>\n  CustomLog \"logs/vhost.log\" common\n</VirtualHost>\n\
<IfModule log_config_module>\n    CustomLog \"C:/Lampion/logs/apache/access.log\" combined\n</IfModule>\n";
        let my = "[mysqld]\r\ndatadir=D:/Data/mariadb/\r\nport=3307\r\nlog-error=server\r\nslow_query_log_file=\"C:/Lampion/logs/mariadb/slow.log\"\r\ngeneral_log_file=general.log\r\n";
        let php = "error_log = \"C:/Lampion/logs/php/php_errors.log\"\n";
        let l = from_texts(Some(httpd), Some(my), Some(php));
        assert_eq!(l.apache_port, 8081);
        assert_eq!(l.document_root, PathBuf::from(r"D:\Sites"));
        assert_eq!(l.apache_error_log, PathBuf::from(r"C:\Lampion\bin\apache\logs\error.log"), "relative to ServerRoot");
        assert_eq!(l.apache_access_log, PathBuf::from(r"C:\Lampion\logs\apache\access.log"), "not the virtual host's");
        assert_eq!(l.mariadb_port, 3307);
        assert_eq!(l.mariadb_datadir, PathBuf::from(r"D:\Data\mariadb"));
        assert_eq!(l.mariadb_error_log, Some(PathBuf::from(r"D:\Data\mariadb\server.err")));
        assert_eq!(l.slow_query_log, Some(PathBuf::from(r"C:\Lampion\logs\mariadb\slow.log")));
        assert_eq!(l.general_query_log, Some(PathBuf::from(r"D:\Data\mariadb\general.log")));
        assert_eq!(l.php_error_log, Some(PathBuf::from(r"C:\Lampion\logs\php\php_errors.log")));
    }

    #[test]
    fn piped_logs_syslog_and_missing_files() {
        let l = from_texts(Some("ErrorLog \"|bin/rotatelogs.exe x\"\nListen 8080\n"), None, Some("error_log = syslog\n"));
        assert_eq!(l.apache_error_log, win(&paths::fwd(&paths::apache_dir().join("logs").join("error.log"))));
        assert_eq!(l.php_error_log, None);
        assert_eq!(l.mariadb_port, 3306);
        assert_eq!(l.mariadb_datadir, paths::databases());
        assert_eq!(l.mariadb_error_log, None);
        let none = from_texts(None, None, None);
        assert_eq!((none.apache_port, none.document_root), (8080, paths::www()));
    }

    #[test]
    fn newest_err_file() {
        let dir = tempfile::tempdir().unwrap();
        let old = dir.path().join("OLD-PC.err");
        let new = dir.path().join("NEW-PC.err");
        std::fs::write(dir.path().join("ib_logfile0"), "").unwrap();
        std::fs::write(&old, "").unwrap();
        std::fs::write(&new, "").unwrap();
        let t = std::time::SystemTime::now();
        std::fs::File::options().write(true).open(&old).unwrap().set_modified(t - std::time::Duration::from_secs(60)).unwrap();
        std::fs::File::options().write(true).open(&new).unwrap().set_modified(t).unwrap();
        assert_eq!(newest_err(dir.path()), Some(new));
        assert_eq!(newest_err(&dir.path().join("missing")), None);
    }
}
