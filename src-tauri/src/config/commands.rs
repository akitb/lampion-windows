//! Tauri commands of the config part: the Apache, PHP, MariaDB and General settings pages.
//! Files and server checks run off the UI thread.

use super::apache::{self, ApacheSettings};
use super::file::{self, Check, SaveOutcome};
use super::mariadb::{self, MariadbSettings};
use super::php::{self, PhpSettings};
use super::{access, locations, phpmyadmin, timezones};
use crate::core::{activity, paths, settings};
use serde::Serialize;
use std::path::{Path, PathBuf};

async fn blocking<T: Send + 'static>(f: impl FnOnce() -> T + Send + 'static) -> Result<T, String> {
    tauri::async_runtime::spawn_blocking(f).await.map_err(|e| e.to_string())
}

/// A config file the page edits; a clear message when it is not there yet.
fn read_config(path: &Path, server: &str) -> Result<String, String> {
    if !path.exists() {
        return Err(format!("{} was not found. Lampion writes it when it sets up {server}.", path.display()));
    }
    file::read(path)
}

/// Logs the result of a save in the Lampion Activity log.
fn logged(module: &str, result: Result<bool, String>) -> Result<SaveOutcome, String> {
    match result {
        Ok(changed) => {
            if changed {
                activity::ok(module, "Settings saved");
            }
            Ok(SaveOutcome { problems: Vec::new(), changed })
        }
        Err(e) => {
            let text: Vec<&str> = e.lines().map(str::trim).filter(|l| !l.is_empty()).collect();
            activity::error(module, &format!("Settings not saved: {}", text.join(" · ")));
            Err(e)
        }
    }
}

fn refused(check: Check) -> Option<SaveOutcome> {
    (!check.problems.is_empty()).then_some(SaveOutcome { problems: check.problems, changed: false })
}

// ---- Apache ----

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ApacheForm {
    settings: ApacheSettings,
    file: String,
    has_backup: bool,
    features: &'static [apache::Feature],
    log_levels: &'static [&'static str],
}

#[tauri::command]
pub async fn config_apache_load() -> Result<ApacheForm, String> {
    blocking(|| {
        let path = paths::httpd_conf();
        let text = read_config(&path, "Apache")?;
        Ok(ApacheForm {
            settings: apache::parse(&text),
            file: path.display().to_string(),
            has_backup: file::has_backup(&path),
            features: apache::FEATURES,
            log_levels: apache::LOG_LEVELS,
        })
    })
    .await?
}

#[tauri::command]
pub async fn config_apache_check(draft: ApacheSettings) -> Check {
    apache::validate(&draft)
}

#[tauri::command]
pub async fn config_apache_save(saved: ApacheSettings, draft: ApacheSettings) -> Result<SaveOutcome, String> {
    blocking(move || {
        if let Some(r) = refused(apache::validate(&draft)) {
            return Ok(r);
        }
        logged("Apache", apache::save(&paths::httpd_conf(), &saved, &draft, &file::run_real))
    })
    .await?
}

// ---- PHP ----

fn php_error_log() -> PathBuf {
    paths::logs().join("php").join("php_errors.log")
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PhpForm {
    settings: PhpSettings,
    file: String,
    has_backup: bool,
    /// The file "Separate PHP error log" uses.
    error_log: String,
    /// This computer's time zone, when PHP knows its name.
    system_timezone: Option<String>,
}

#[tauri::command]
pub async fn config_php_load() -> Result<PhpForm, String> {
    blocking(|| {
        let path = paths::php_ini();
        let text = read_config(&path, "PHP")?;
        Ok(PhpForm {
            settings: php::parse(&text),
            file: path.display().to_string(),
            has_backup: file::has_backup(&path),
            error_log: php_error_log().display().to_string(),
            system_timezone: iana_time_zone::get_timezone().ok().and_then(|tz| timezones::find(&tz)).map(String::from),
        })
    })
    .await?
}

#[tauri::command]
pub async fn config_php_check(draft: PhpSettings) -> Check {
    php::validate(&draft)
}

#[tauri::command]
pub async fn config_php_save(saved: PhpSettings, draft: PhpSettings) -> Result<SaveOutcome, String> {
    blocking(move || {
        if let Some(r) = refused(php::validate(&draft)) {
            return Ok(r);
        }
        logged("PHP", php::save(&paths::php_ini(), &saved, &draft, &file::run_real, &php_error_log()))
    })
    .await?
}

#[derive(Serialize)]
pub struct PhpModules {
    version: String,
    extensions: Vec<String>,
}

/// PHP's version and loaded extensions, as PHP reads Lampion's php.ini.
#[tauri::command]
pub async fn config_php_modules() -> Result<PhpModules, String> {
    blocking(|| php::modules(&file::run_real, &paths::php_ini()).map(|(version, extensions)| PhpModules { version, extensions })).await?
}

/// Every time zone name PHP knows.
#[tauri::command]
pub fn config_timezones() -> Vec<&'static str> {
    timezones::all()
}

// ---- MariaDB ----

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MariadbForm {
    settings: MariadbSettings,
    file: String,
    has_backup: bool,
    /// MariaDB's built-in values, shown greyed in empty fields.
    defaults: MariadbSettings,
    charsets: &'static [mariadb::Charset],
}

#[tauri::command]
pub async fn config_mariadb_load() -> Result<MariadbForm, String> {
    blocking(|| {
        let path = paths::my_ini();
        let text = read_config(&path, "MariaDB")?;
        Ok(MariadbForm {
            settings: mariadb::parse(&text),
            file: path.display().to_string(),
            has_backup: file::has_backup(&path),
            defaults: mariadb::defaults(),
            charsets: mariadb::CHARSETS,
        })
    })
    .await?
}

#[tauri::command]
pub async fn config_mariadb_check(draft: MariadbSettings) -> Check {
    mariadb::validate(&draft)
}

#[tauri::command]
pub async fn config_mariadb_save(saved: MariadbSettings, draft: MariadbSettings) -> Result<SaveOutcome, String> {
    blocking(move || {
        if let Some(r) = refused(mariadb::validate(&draft)) {
            return Ok(r);
        }
        let port_before = locations::current().mariadb_port;
        let logs = paths::logs().join("mariadb");
        let result = mariadb::save(&paths::my_ini(), &saved, &draft, &file::run_real, &logs.join("slow.log"), &logs.join("general.log"));
        let port = locations::current().mariadb_port;
        if result.is_ok() && port != port_before {
            // phpMyAdmin connects over TCP, so it has to follow the port.
            if let Err(e) = phpmyadmin::sync_port(port) {
                activity::warn("MySQL", &format!("phpMyAdmin still uses port {port_before}: {}", e.lines().next().unwrap_or("")));
            }
        }
        logged("MySQL", result)
    })
    .await?
}

/// The "Copy Settings" texts for `database` on `port`.
#[tauri::command]
pub fn config_snippets(database: String, port: u16) -> Vec<access::Snippet> {
    access::snippets(&database, port)
}

/// Some(true) / Some(false); None while MariaDB is not running.
#[tauri::command]
pub async fn config_root_state() -> Option<bool> {
    access::root_no_password_state().await
}

/// Off needs `password` (root's new password). On takes root's current password when the app does not know it.
#[tauri::command]
pub async fn config_set_root_no_password(enabled: bool, password: Option<String>) -> Result<(), String> {
    access::set_root_no_password(enabled, password).await
}

/// None when phpMyAdmin is not installed.
#[tauri::command]
pub async fn config_phpmyadmin_state() -> Option<bool> {
    blocking(phpmyadmin::state).await.ok().flatten()
}

#[tauri::command]
pub async fn config_set_phpmyadmin_auto_login(enabled: bool) -> Result<(), String> {
    blocking(move || phpmyadmin::set_auto_login(enabled)).await?
}

// ---- Files: backups, restore, edit ----

/// `apache` | `vhosts` | `php` | `mariadb` | `phpmyadmin` → (activity module, file name, path).
fn config_file(id: &str) -> Result<(&'static str, &'static str, PathBuf), String> {
    Ok(match id {
        "apache" => ("Apache", "httpd.conf", paths::httpd_conf()),
        "vhosts" => ("Apache", "httpd-vhosts.conf", paths::apache_dir().join("conf").join("extra").join("httpd-vhosts.conf")),
        "php" => ("PHP", "php.ini", paths::php_ini()),
        "mariadb" => ("MySQL", "my.ini", paths::my_ini()),
        "phpmyadmin" => ("MySQL", "config.inc.php", paths::pma_config()),
        _ => return Err(format!("Unknown configuration file: {id}")),
    })
}

#[derive(Serialize)]
pub struct Backup {
    id: &'static str,
    name: &'static str,
    file: String,
    backup: String,
}

/// The configuration files that have an original saved before Lampion's first change.
#[tauri::command]
pub async fn config_backups() -> Result<Vec<Backup>, String> {
    blocking(|| {
        ["apache", "php", "mariadb", "phpmyadmin"]
            .into_iter()
            .filter_map(|id| {
                let (_, name, path) = config_file(id).ok()?;
                file::has_backup(&path).then(|| Backup {
                    id,
                    name,
                    file: path.display().to_string(),
                    backup: file::backup_path(&path).display().to_string(),
                })
            })
            .collect()
    })
    .await
}

/// Puts back the file as it was before Lampion first changed it.
#[tauri::command]
pub async fn config_restore(id: String) -> Result<(), String> {
    blocking(move || {
        let (module, name, path) = config_file(&id)?;
        file::restore_backup(&path)?;
        activity::warn(module, &format!("Restored the original {name}"));
        Ok(())
    })
    .await?
}

/// Opens the file in Notepad (a .conf has no editor of its own on Windows, and a .php must not run).
#[tauri::command]
pub async fn config_edit_file(id: String) -> Result<(), String> {
    let (_, _, path) = config_file(&id)?;
    if !path.is_file() {
        return Err(format!("{} was not found.", path.display()));
    }
    crate::core::shell::command("notepad.exe").arg(&path).spawn().map(|_| ()).map_err(|e| format!("Could not open Notepad: {e}"))
}

// ---- General ----

/// Ports and folders read fresh from the configuration (also sent as `config://locations` on change).
#[tauri::command]
pub async fn config_locations() -> Result<locations::Locations, String> {
    blocking(locations::current).await
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Folders {
    root: PathBuf,
    bin: PathBuf,
    logs: PathBuf,
    /// Lampion's own activity log file.
    app_log: PathBuf,
}

/// The fixed folders of the Locations list (the others come from `config_locations`).
#[tauri::command]
pub fn config_folders() -> Folders {
    Folders { root: paths::root(), bin: paths::bin(), logs: paths::logs(), app_log: paths::local_data().join("Lampion.log") }
}

const RUN_KEY: &str = r"Software\Microsoft\Windows\CurrentVersion\Run";
const RUN_VALUE: &str = "Lampion";

/// The Run value: the exe path in quotes (paths may contain spaces).
fn login_command(exe: &Path) -> String {
    format!("\"{}\"", exe.display())
}

/// Open Lampion at login: `HKCU\…\Run\Lampion` exists.
#[tauri::command]
pub async fn config_open_at_login() -> bool {
    use winreg::{enums::HKEY_CURRENT_USER, RegKey};
    RegKey::predef(HKEY_CURRENT_USER).open_subkey(RUN_KEY).and_then(|k| k.get_value::<String, _>(RUN_VALUE)).is_ok()
}

#[tauri::command]
pub async fn config_set_open_at_login(enabled: bool) -> Result<(), String> {
    use winreg::{enums::HKEY_CURRENT_USER, RegKey};
    let fail = |e: std::io::Error| format!("Could not change this: {e}");
    let (key, _) = RegKey::predef(HKEY_CURRENT_USER).create_subkey(RUN_KEY).map_err(fail)?;
    if enabled {
        let exe = std::env::current_exe().map_err(fail)?;
        key.set_value(RUN_VALUE, &login_command(&exe)).map_err(fail)?;
    } else if let Err(e) = key.delete_value(RUN_VALUE) {
        if e.kind() != std::io::ErrorKind::NotFound {
            return Err(fail(e));
        }
    }
    settings::update(|s| s.open_at_login = enabled);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn login_command_is_quoted() {
        assert_eq!(login_command(Path::new(r"C:\Users\A B\AppData\Local\Programs\Lampion\lampion.exe")), r#""C:\Users\A B\AppData\Local\Programs\Lampion\lampion.exe""#);
    }

    #[test]
    fn config_files() {
        assert_eq!(config_file("apache").unwrap().1, "httpd.conf");
        assert_eq!(config_file("phpmyadmin").unwrap().2, paths::pma_config());
        assert!(config_file("../x").is_err());
    }
}
