//! The configuration first setup writes into `config\` (SPEC "Generált konfiguráció lényege") and the few
//! values the servers part reads back from it: `Listen` ports, PID files and php.ini values.
//! Paths use forward slashes, everything listens on 127.0.0.1 only, new files use CRLF line endings.

use crate::core::paths::fwd;
use std::path::{Path, PathBuf};

pub const APACHE_PORT: u16 = 8080;
pub const MARIADB_PORT: u16 = 3306;

/// The PHP extensions setup turns on (mbstring before exif, which depends on it).
pub const PHP_EXTENSIONS: [&str; 11] =
    ["mysqli", "pdo_mysql", "mbstring", "openssl", "curl", "gd", "intl", "zip", "fileinfo", "sodium", "exif"];

/// `C:\Lampion` → `C:/Lampion` (no trailing slash).
fn root_fwd(root: &Path) -> String {
    fwd(root).trim_end_matches('/').to_string()
}

fn crlf(text: &str) -> String {
    text.replace("\r\n", "\n").replace('\n', "\r\n")
}

const HTTPD_CONF: &str = r#"# Apache configuration, written by Lampion. Lampion › Apache edits it; you can edit it by hand too.
# Paths use forward slashes. Apache listens on 127.0.0.1 only, so Windows Firewall does not ask.

Define SRVROOT "@ROOT@/bin/apache"
ServerRoot "${SRVROOT}"

Listen 127.0.0.1:@PORT@

# Modules. Lampion › Apache switches the common ones on and off (mod_deflate needs mod_filter).
LoadModule access_compat_module modules/mod_access_compat.so
LoadModule actions_module modules/mod_actions.so
LoadModule alias_module modules/mod_alias.so
LoadModule allowmethods_module modules/mod_allowmethods.so
LoadModule auth_basic_module modules/mod_auth_basic.so
LoadModule authn_core_module modules/mod_authn_core.so
LoadModule authn_file_module modules/mod_authn_file.so
LoadModule authz_core_module modules/mod_authz_core.so
LoadModule authz_groupfile_module modules/mod_authz_groupfile.so
LoadModule authz_host_module modules/mod_authz_host.so
LoadModule authz_user_module modules/mod_authz_user.so
LoadModule autoindex_module modules/mod_autoindex.so
#LoadModule deflate_module modules/mod_deflate.so
LoadModule dir_module modules/mod_dir.so
LoadModule env_module modules/mod_env.so
#LoadModule expires_module modules/mod_expires.so
#LoadModule filter_module modules/mod_filter.so
#LoadModule headers_module modules/mod_headers.so
#LoadModule http2_module modules/mod_http2.so
#LoadModule info_module modules/mod_info.so
LoadModule log_config_module modules/mod_log_config.so
LoadModule mime_module modules/mod_mime.so
LoadModule negotiation_module modules/mod_negotiation.so
#LoadModule proxy_module modules/mod_proxy.so
#LoadModule proxy_fcgi_module modules/mod_proxy_fcgi.so
#LoadModule proxy_http_module modules/mod_proxy_http.so
#LoadModule proxy_wstunnel_module modules/mod_proxy_wstunnel.so
LoadModule rewrite_module modules/mod_rewrite.so
LoadModule setenvif_module modules/mod_setenvif.so
#LoadModule socache_shmcb_module modules/mod_socache_shmcb.so
#LoadModule ssl_module modules/mod_ssl.so
#LoadModule status_module modules/mod_status.so
#LoadModule vhost_alias_module modules/mod_vhost_alias.so

# Shown on error pages (Lampion › Apache › Administrator email).
#ServerAdmin admin@example.com

ServerName localhost:@PORT@

<Directory />
    AllowOverride none
    Require all denied
</Directory>

# The web folder (Lampion › Apache › Web folder).
DocumentRoot "@ROOT@/www"
<Directory "@ROOT@/www">
    Options Indexes FollowSymLinks
    AllowOverride All
    Require all granted
</Directory>

<IfModule dir_module>
    DirectoryIndex index.php index.html
</IfModule>

<Files ".ht*">
    Require all denied
</Files>

ErrorLog "@ROOT@/logs/apache/error.log"
LogLevel warn

<IfModule log_config_module>
    LogFormat "%h %l %u %t \"%r\" %>s %b \"%{Referer}i\" \"%{User-Agent}i\"" combined
    LogFormat "%h %l %u %t \"%r\" %>s %b" common
    CustomLog "@ROOT@/logs/apache/access.log" combined
</IfModule>

<IfModule mime_module>
    TypesConfig conf/mime.types
    AddType application/x-compress .Z
    AddType application/x-gzip .gz .tgz
</IfModule>

<IfModule headers_module>
    RequestHeader unset Proxy early
</IfModule>

PidFile "@ROOT@/logs/apache/httpd.pid"

# Windows: plain accept() instead of AcceptEx(), which some antivirus and VPN drivers break.
AcceptFilter http none
AcceptFilter https none

# PHP runs inside Apache.
LoadModule php_module "@ROOT@/bin/php/php8apache2_4.dll"
PHPIniDir "@ROOT@/config"
<FilesMatch \.php$>
    SetHandler application/x-httpd-php
</FilesMatch>

# phpMyAdmin at /phpmyadmin, from this computer only. The Host check stops web pages from other sites
# reaching it through a name that points at 127.0.0.1 (DNS rebinding).
Alias /phpmyadmin "@ROOT@/bin/phpmyadmin"
<Directory "@ROOT@/bin/phpmyadmin">
    Options FollowSymLinks
    AllowOverride None
    <RequireAll>
        Require local
        Require expr "%{HTTP_HOST} =~ /^(localhost|127\.0\.0\.1|\[::1\])(:\d+)?$/"
    </RequireAll>
</Directory>
"#;

const MY_INI: &str = r#"# MariaDB configuration, written by Lampion. Lampion › MariaDB edits it; you can edit it by hand too.
# Lampion starts MariaDB with --defaults-file pointing here. Paths use forward slashes.

[mysqld]
datadir=@ROOT@/databases
port=@PORT@
# This computer only, so Windows Firewall does not ask.
bind-address=127.0.0.1
# A fixed log name: the default one is named after the computer and changes with it.
log-error=@ROOT@/logs/mariadb/mariadb.err
pid-file=@ROOT@/logs/mariadb/mariadb.pid
"#;

const README: &str = r#"Lampion keeps everything for your local server in this folder.

bin\         Apache, PHP, MariaDB and phpMyAdmin, downloaded by Lampion.
config\      httpd.conf, php.ini and my.ini. Lampion's settings pages edit these files.
www\         Your sites. http://localhost:8080/ serves this folder.
databases\   MariaDB's data files. Don't copy or edit them while MariaDB runs;
             use Export in Lampion's Database Manager for backups.
logs\        apache\, mariadb\ and php\ logs. Lampion's Logs page shows them live.
backups\     Database exports are saved here by default.
tmp\         PHP sessions and uploads.

Uninstalling Lampion keeps this folder and your data. Delete it yourself when you no longer need it.
"#;

const WELCOME: &str = r#"<!doctype html>
<html lang="en">
<head>
  <meta charset="utf-8">
  <title>It works</title>
  <style>
    body { font: 16px "Segoe UI", system-ui, sans-serif; max-width: 34rem; margin: 15vh auto; padding: 0 1rem; color: #1c1c1f; }
    @media (prefers-color-scheme: dark) { body { background: #1b1b1e; color: #ececef; } }
    code { font-family: "Cascadia Mono", Consolas, monospace; }
  </style>
</head>
<body>
  <h1>It works</h1>
  <p>Apache is serving this page with PHP <?= PHP_VERSION ?>.</p>
  <p>Put your site in <code><?= htmlspecialchars(__DIR__) ?></code>.</p>
</body>
</html>
"#;

/// The complete httpd.conf for a Lampion folder.
pub fn httpd_conf(root: &Path, port: u16) -> String {
    crlf(&HTTPD_CONF.replace("@ROOT@", &root_fwd(root)).replace("@PORT@", &port.to_string()))
}

/// my.ini; the settings page adds Lampion's own block later.
pub fn my_ini(root: &Path, port: u16) -> String {
    crlf(&MY_INI.replace("@ROOT@", &root_fwd(root)).replace("@PORT@", &port.to_string()))
}

pub fn readme() -> String {
    crlf(README)
}

/// `index.php` for an empty web folder.
pub fn welcome_page() -> String {
    crlf(WELCOME)
}

/// php.ini made from the package's `php.ini-development`: extension folder, the usual extensions, the
/// time zone, the error log and PHP's temporary files in `tmp\`. Keeps the template's line endings.
pub fn php_ini(development: &str, root: &Path, timezone: &str) -> String {
    let r = root_fwd(root);
    let mut t = ini_set(development, "extension_dir", &format!("\"{r}/bin/php/ext\""));
    for e in PHP_EXTENSIONS {
        t = ini_enable_extension(&t, e);
    }
    t = ini_set(&t, "date.timezone", timezone);
    t = ini_set(&t, "log_errors", "On");
    t = ini_set(&t, "error_log", &format!("\"{r}/logs/php/php_errors.log\""));
    t = ini_set(&t, "session.save_path", &format!("\"{r}/tmp\""));
    ini_set(&t, "upload_tmp_dir", &format!("\"{r}/tmp\""))
}

/// The line ending a text uses (CRLF if it has any).
fn eol(text: &str) -> &'static str {
    if text.contains("\r\n") {
        "\r\n"
    } else {
        "\n"
    }
}

/// The key of a `key = value` line, `None` for comments, sections and prose.
fn ini_key(line: &str) -> Option<&str> {
    let (k, _) = line.split_once('=')?;
    let k = k.trim();
    let plain = !k.is_empty() && !k.starts_with([';', '#', '[']) && !k.contains(char::is_whitespace);
    plain.then_some(k)
}

/// The rest of a disabled setting line (`;key = …` with no space after the `;`, unlike prose comments).
fn disabled(line: &str) -> Option<&str> {
    line.trim_start().strip_prefix(';').filter(|r| r.starts_with(|c: char| !c.is_whitespace()))
}

/// Sets `key = value`: rewrites the first active `key =` line, else the first disabled `;key =` line,
/// else appends one. Every other line stays byte-for-byte the same.
pub fn ini_set(text: &str, key: &str, value: &str) -> String {
    let nl = eol(text);
    let mut lines: Vec<String> = text.split_inclusive('\n').map(str::to_string).collect();
    let line = format!("{key} = {value}{nl}");
    let at = lines
        .iter()
        .position(|l| ini_key(l.trim()) == Some(key))
        .or_else(|| lines.iter().position(|l| disabled(l).and_then(ini_key) == Some(key)));
    match at {
        Some(i) => lines[i] = line,
        None => {
            if lines.last().is_some_and(|l| !l.ends_with('\n')) {
                lines.last_mut().unwrap().push_str(nl);
            }
            lines.push(line);
        }
    }
    lines.concat()
}

/// `"php_mysqli.dll" ; note` → `mysqli`.
fn extension_name(value: &str) -> String {
    let v = value.split(';').next().unwrap_or("").trim().trim_matches('"');
    let v = v.strip_prefix("php_").unwrap_or(v);
    v.strip_suffix(".dll").unwrap_or(v).to_ascii_lowercase()
}

fn extension_of(line: &str) -> Option<String> {
    let (k, v) = line.split_once('=')?;
    (k.trim() == "extension").then(|| extension_name(v))
}

/// Turns a PHP extension on: uncomments its `;extension=name` line (keeping any note after it), or
/// appends `extension=name`. Unchanged when it is already on.
pub fn ini_enable_extension(text: &str, name: &str) -> String {
    let mut lines: Vec<String> = text.split_inclusive('\n').map(str::to_string).collect();
    if lines.iter().any(|l| extension_of(l.trim()).as_deref() == Some(name)) {
        return text.to_string();
    }
    match lines.iter().position(|l| disabled(l).and_then(extension_of).as_deref() == Some(name)) {
        Some(i) => lines[i] = lines[i].replacen(';', "", 1),
        None => {
            let nl = eol(text);
            if lines.last().is_some_and(|l| !l.ends_with('\n')) {
                lines.last_mut().unwrap().push_str(nl);
            }
            lines.push(format!("extension={name}{nl}"));
        }
    }
    lines.concat()
}

/// The value of the first active `key = value` line: up to the closing quote when quoted
/// (`"a;b" ; note` → `a;b`), else up to a `;` comment.
pub fn ini_value(text: &str, key: &str) -> Option<String> {
    let line = text.lines().map(str::trim).find(|l| ini_key(l) == Some(key))?;
    let v = line.split_once('=')?.1.trim();
    Some(match v.strip_prefix('"') {
        Some(rest) => rest.split('"').next().unwrap_or("").to_string(),
        None => v.split(';').next().unwrap_or("").trim().to_string(),
    })
}

/// Ports of the top-level `Listen` lines: `Listen 8080`, `Listen 127.0.0.1:8080`, `Listen [::1]:8080`,
/// `Listen 8080 http` (the port is after the last `:`). Lines inside `<IfModule>`-style blocks are skipped:
/// Apache binds them only when the condition holds.
pub fn listen_ports(conf: &str) -> Vec<u16> {
    let mut ports = Vec::new();
    let mut depth = 0usize;
    for line in conf.lines().map(str::trim) {
        if line.starts_with("</") {
            depth = depth.saturating_sub(1);
            continue;
        }
        if line.starts_with('<') {
            depth += 1;
            continue;
        }
        let mut words = line.split_whitespace();
        if depth > 0 || !words.next().is_some_and(|w| w.eq_ignore_ascii_case("Listen")) {
            continue;
        }
        let port = words.next().and_then(|a| a.rsplit(':').next()).and_then(|p| p.parse::<u16>().ok());
        if let Some(p) = port.filter(|p| !ports.contains(p)) {
            ports.push(p);
        }
    }
    ports
}

/// First argument of the first active top-level `name` directive, unquoted.
fn directive(conf: &str, name: &str) -> Option<String> {
    conf.lines().map(str::trim).find_map(|l| {
        let (word, rest) = l.split_once(char::is_whitespace)?;
        word.eq_ignore_ascii_case(name).then(|| {
            let rest = rest.trim();
            match rest.strip_prefix('"') {
                Some(q) => q.split('"').next().unwrap_or("").to_string(),
                None => rest.split_whitespace().next().unwrap_or("").to_string(),
            }
        })
    })
}

fn absolute_or(base: &Path, value: &str) -> PathBuf {
    let p = PathBuf::from(value.replace('/', "\\"));
    if p.is_absolute() {
        p
    } else {
        base.join(p)
    }
}

/// Apache's PID file (`PidFile`, relative to ServerRoot; default `logs/httpd.pid`). Apache writes the
/// parent's PID there, in background and in service mode alike.
pub fn apache_pid_file(conf: &str, server_root: &Path) -> PathBuf {
    let value = directive(conf, "PidFile").unwrap_or_else(|| "logs/httpd.pid".into());
    absolute_or(server_root, &value.replace("${SRVROOT}", &fwd(server_root)))
}

/// A server option from my.ini (`[mysqld]`, `[server]`, `[mariadb]`, `[mariadbd]` groups; `-` and `_`
/// are the same). The last one wins, as in MariaDB.
pub fn my_value(ini: &str, key: &str) -> Option<String> {
    let norm = |k: &str| k.trim().replace('_', "-").to_ascii_lowercase();
    let want = norm(key);
    let mut in_server = false;
    let mut found = None;
    for line in ini.lines().map(str::trim) {
        if let Some(group) = line.strip_prefix('[').and_then(|g| g.split(']').next()) {
            in_server = matches!(group.trim().to_ascii_lowercase().as_str(), "mysqld" | "server" | "mariadb" | "mariadbd");
            continue;
        }
        if !in_server || line.starts_with(['#', ';']) {
            continue;
        }
        if let Some((k, v)) = line.split_once('=') {
            if norm(k) == want {
                let v = v.split(" #").next().unwrap_or("").trim().trim_matches(['"', '\'']);
                found = Some(v.to_string());
            }
        }
    }
    found
}

/// MariaDB's PID file (`pid-file`, relative to the data folder; default `<datadir>\<computer>.pid`).
pub fn mariadb_pid_file(ini: &str, root: &Path) -> PathBuf {
    let datadir = my_value(ini, "datadir").map(|d| absolute_or(root, &d)).unwrap_or_else(|| root.join("databases"));
    match my_value(ini, "pid-file") {
        Some(v) if !v.is_empty() => absolute_or(&datadir, &v),
        _ => datadir.join(format!("{}.pid", std::env::var("COMPUTERNAME").unwrap_or_default())),
    }
}

/// Rewrites every path under `old` to the same path under `new`, in `/` and `\` forms, ignoring case
/// like Windows does. Only whole folder names match: `C:/Lampion2` is not under `C:/Lampion`.
pub fn replace_root(text: &str, old: &Path, new: &Path) -> String {
    let trim = |s: String| s.trim_end_matches(['/', '\\']).to_string();
    let forms = [
        (trim(fwd(old)), trim(fwd(new))),
        (trim(old.to_string_lossy().into_owned()), trim(new.to_string_lossy().into_owned())),
    ];
    let mut out = text.to_string();
    for (o, n) in forms {
        out = replace_folder(&out, &o, &n);
    }
    out
}

fn replace_folder(text: &str, old: &str, new: &str) -> String {
    if old.is_empty() {
        return text.to_string();
    }
    // ASCII lowercasing keeps byte offsets, so positions in `lower` are positions in `text`.
    let lower = text.to_ascii_lowercase();
    let needle = old.to_ascii_lowercase();
    let mut out = String::with_capacity(text.len());
    let mut i = 0;
    while let Some(pos) = lower[i..].find(&needle) {
        let at = i + pos;
        let end = at + needle.len();
        let whole = matches!(text[end..].chars().next(), None | Some('/' | '\\' | '"' | '\'' | ' ' | '\t' | '\r' | '\n' | ';'));
        out.push_str(&text[i..at]);
        out.push_str(if whole { new } else { &text[at..end] });
        i = end;
    }
    out.push_str(&text[i..]);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    const DEV_INI: &str = "[PHP]\r\n\
        memory_limit = 128M\r\n\
        ; Example:\r\n\
        ;error_log = php_errors.log\r\n\
        ;error_log = syslog\r\n\
        ;extension_dir = \"./\"\r\n\
        ; On windows:\r\n\
        ;extension_dir = \"ext\"\r\n\
        ;extension=curl\r\n\
        ;extension=mbstring\r\n\
        ;extension=exif      ; Must be after mbstring as it depends on it\r\n\
        ;extension=mysqli\r\n\
        ; The path can be defined as:\r\n\
        ;     session.save_path = \"N;/path\"\r\n\
        ;session.save_path = \"/tmp\"\r\n\
        ;upload_tmp_dir =\r\n\
        [Date]\r\n\
        ;date.timezone =\r\n";

    #[test]
    fn httpd_conf_contents() {
        let c = httpd_conf(Path::new(r"C:\My Lampion\"), 8080);
        assert!(c.contains("Define SRVROOT \"C:/My Lampion/bin/apache\"\r\n"));
        assert!(c.contains("Listen 127.0.0.1:8080\r\n"));
        assert!(c.contains("ServerName localhost:8080\r\n"));
        assert!(c.contains("LoadModule php_module \"C:/My Lampion/bin/php/php8apache2_4.dll\""));
        assert!(c.contains("PHPIniDir \"C:/My Lampion/config\""));
        assert!(c.contains("DocumentRoot \"C:/My Lampion/www\""));
        assert!(c.contains("<Directory \"C:/My Lampion/www\">\r\n    Options Indexes FollowSymLinks\r\n    AllowOverride All"));
        assert!(c.contains("\r\nLoadModule rewrite_module"));
        assert!(c.contains("DirectoryIndex index.php index.html"));
        assert!(c.contains("Alias /phpmyadmin \"C:/My Lampion/bin/phpmyadmin\""));
        assert!(c.contains("    <RequireAll>\r\n        Require local\r\n        Require expr \"%{HTTP_HOST} =~ /"));
        assert!(c.contains("ErrorLog \"C:/My Lampion/logs/apache/error.log\""));
        assert!(c.contains("CustomLog \"C:/My Lampion/logs/apache/access.log\" combined"));
        assert!(c.contains("ServerRoot \"${SRVROOT}\""));
        assert!(!regex::Regex::new(r"[A-Za-z]:\\").unwrap().is_match(&c), "no backslash paths");
        assert!(!c.replace("\r\n", "").contains('\n'), "CRLF only");
        assert_eq!(listen_ports(&c), vec![8080]);
        assert_eq!(apache_pid_file(&c, Path::new(r"C:\My Lampion\bin\apache")), PathBuf::from(r"C:\My Lampion\logs\apache\httpd.pid"));
    }

    #[test]
    fn phpmyadmin_answers_only_to_this_computers_names() {
        // The expression Apache evaluates (ap_expr regexes are PCRE; this one reads the same in Rust).
        let c = httpd_conf(Path::new(r"C:\Lampion"), 8080);
        let line = c.lines().find(|l| l.contains("Require expr")).unwrap();
        let pattern = line.split(" =~ /").nth(1).unwrap().trim_end_matches("/\"");
        let host = regex::Regex::new(pattern).unwrap();
        for ok in ["localhost", "localhost:8080", "127.0.0.1", "127.0.0.1:8080", "[::1]:8080"] {
            assert!(host.is_match(ok), "{ok}");
        }
        for bad in ["evil.example", "localhost.evil.example", "127.0.0.1.nip.io:8080", "127x0x0x1", "localhost:8080@evil", ""] {
            assert!(!host.is_match(bad), "{bad}");
        }
    }

    #[test]
    fn my_ini_contents() {
        let m = my_ini(Path::new(r"D:\Dev\Lampion"), 3307);
        assert!(m.contains("[mysqld]\r\ndatadir=D:/Dev/Lampion/databases\r\nport=3307\r\n"));
        assert!(m.contains("bind-address=127.0.0.1"));
        assert!(m.contains("log-error=D:/Dev/Lampion/logs/mariadb/mariadb.err"));
        assert_eq!(my_value(&m, "port").as_deref(), Some("3307"));
        assert_eq!(my_value(&m, "pid_file").as_deref(), Some("D:/Dev/Lampion/logs/mariadb/mariadb.pid"));
        assert_eq!(mariadb_pid_file(&m, Path::new(r"D:\Dev\Lampion")), PathBuf::from(r"D:\Dev\Lampion\logs\mariadb\mariadb.pid"));
    }

    #[test]
    fn mariadb_pid_file_defaults_to_the_data_folder() {
        let ini = "[client]\nport=1\n[mysqld]\ndatadir=\"E:/data\"\n";
        let host = std::env::var("COMPUTERNAME").unwrap_or_default();
        assert_eq!(mariadb_pid_file(ini, Path::new(r"C:\L")), PathBuf::from(format!(r"E:\data\{host}.pid")));
        let rel = "[mysqld]\npid-file=x.pid\n";
        assert_eq!(mariadb_pid_file(rel, Path::new(r"C:\L")), PathBuf::from(r"C:\L\databases\x.pid"));
        assert_eq!(my_value("[client]\nport=1\n", "port"), None, "only server groups count");
        assert_eq!(my_value("[mysqld]\nport=1\nport=2 # later wins\n", "port").as_deref(), Some("2"));
    }

    #[test]
    fn php_ini_edits() {
        let t = php_ini(DEV_INI, Path::new(r"C:\Lampion"), "Europe/Budapest");
        assert!(t.contains("extension_dir = \"C:/Lampion/bin/php/ext\"\r\n"));
        assert!(t.contains(";extension_dir = \"ext\"\r\n"), "only the first disabled line changes");
        assert!(t.contains("\r\nextension=curl\r\n"));
        assert!(t.contains("\r\nextension=exif      ; Must be after mbstring as it depends on it\r\n"));
        assert!(t.contains("\r\nextension=pdo_mysql\r\n"), "missing ones are appended");
        assert!(t.find("extension=mbstring").unwrap() < t.find("extension=exif").unwrap());
        assert!(t.contains("date.timezone = Europe/Budapest\r\n"));
        assert!(t.contains("error_log = \"C:/Lampion/logs/php/php_errors.log\"\r\n"));
        assert!(t.contains(";error_log = syslog\r\n"));
        assert!(t.contains(";     session.save_path = \"N;/path\"\r\n"), "prose examples stay");
        assert!(t.contains("\r\nsession.save_path = \"C:/Lampion/tmp\"\r\n"));
        assert!(t.contains("upload_tmp_dir = \"C:/Lampion/tmp\"\r\n"));
        assert!(t.contains("log_errors = On\r\n"));
        assert!(!t.replace("\r\n", "").contains('\n'), "keeps CRLF");
        assert_eq!(ini_value(&t, "memory_limit").as_deref(), Some("128M"));
        assert_eq!(ini_enable_extension(&t, "curl"), t, "already on: unchanged");
    }

    #[test]
    fn ini_values_read_to_the_closing_quote() {
        let t = "a = \"x;y\" ; note\nb = 2M ; note\n;c = 1\nd=\n";
        assert_eq!(ini_value(t, "a").as_deref(), Some("x;y"));
        assert_eq!(ini_value(t, "b").as_deref(), Some("2M"));
        assert_eq!(ini_value(t, "c"), None);
        assert_eq!(ini_value(t, "d").as_deref(), Some(""));
        assert_eq!(ini_set("x = 1", "y", "2"), "x = 1\ny = 2\n");
    }

    #[test]
    fn listen_forms() {
        let conf = "Listen 8080\n#Listen 81\nListen [::1]:9090\n  listen 127.0.0.1:7070 http\nListen 8080\n\
                    <IfModule ssl_module>\nListen 443\n</IfModule>\nListenBackLog 5\n";
        assert_eq!(listen_ports(conf), vec![8080, 9090, 7070]);
    }

    #[test]
    fn root_replacement() {
        let text = "Define SRVROOT \"C:/Lampion/bin/apache\"\nDocumentRoot \"c:/lampion/www\"\nX \"C:/Lampion\"\n\
                    Y C:/Lampion2/www\nZ C:\\Lampion\\logs\n";
        let out = replace_root(text, Path::new(r"C:\Lampion"), Path::new(r"D:\Dev\Lampion"));
        assert!(out.contains("\"D:/Dev/Lampion/bin/apache\""));
        assert!(out.contains("\"D:/Dev/Lampion/www\""));
        assert!(out.contains("X \"D:/Dev/Lampion\""));
        assert!(out.contains("Y C:/Lampion2/www"), "another folder with the same prefix stays");
        assert!(out.contains(r"Z D:\Dev\Lampion\logs"));
    }

    #[test]
    fn readme_and_welcome() {
        assert!(readme().contains("www\\ "));
        assert!(welcome_page().contains("<?= PHP_VERSION ?>"));
    }
}
