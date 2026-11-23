//! Where everything lives. The Lampion folder (default `C:\Lampion`) holds the servers, configs,
//! sites, databases, logs and backups, like XAMPP. App data lives in `%APPDATA%\Lampion` (settings)
//! and `%LOCALAPPDATA%\Lampion` (activity log, caches).

use std::path::{Path, PathBuf};

pub const DEFAULT_ROOT: &str = r"C:\Lampion";

/// The Lampion folder, from settings (falls back to `C:\Lampion`).
pub fn root() -> PathBuf {
    let r = super::settings::get().lampion_root;
    if r.trim().is_empty() {
        PathBuf::from(DEFAULT_ROOT)
    } else {
        PathBuf::from(r)
    }
}

pub fn bin() -> PathBuf { root().join("bin") }
pub fn apache_dir() -> PathBuf { bin().join("apache") }
pub fn php_dir() -> PathBuf { bin().join("php") }
pub fn mariadb_dir() -> PathBuf { bin().join("mariadb") }
pub fn phpmyadmin_dir() -> PathBuf { bin().join("phpmyadmin") }
pub fn config_dir() -> PathBuf { root().join("config") }
pub fn www() -> PathBuf { root().join("www") }
pub fn databases() -> PathBuf { root().join("databases") }
pub fn logs() -> PathBuf { root().join("logs") }
pub fn backups() -> PathBuf { root().join("backups") }
pub fn tmp() -> PathBuf { root().join("tmp") }

pub fn httpd_exe() -> PathBuf { apache_dir().join("bin").join("httpd.exe") }
pub fn php_exe() -> PathBuf { php_dir().join("php.exe") }
pub fn mariadbd_exe() -> PathBuf { mariadb_dir().join("bin").join("mariadbd.exe") }
pub fn mariadb_exe() -> PathBuf { mariadb_dir().join("bin").join("mariadb.exe") }
pub fn mariadb_admin_exe() -> PathBuf { mariadb_dir().join("bin").join("mariadb-admin.exe") }
pub fn mariadb_dump_exe() -> PathBuf { mariadb_dir().join("bin").join("mariadb-dump.exe") }
pub fn mariadb_install_db_exe() -> PathBuf { mariadb_dir().join("bin").join("mariadb-install-db.exe") }

pub fn httpd_conf() -> PathBuf { config_dir().join("httpd.conf") }
pub fn php_ini() -> PathBuf { config_dir().join("php.ini") }
pub fn my_ini() -> PathBuf { config_dir().join("my.ini") }
pub fn pma_config() -> PathBuf { phpmyadmin_dir().join("config.inc.php") }

/// `%APPDATA%\Lampion` (settings.json, query history, designer positions).
pub fn app_data() -> PathBuf {
    let d = dirs::config_dir().unwrap_or_else(|| PathBuf::from(".")).join("Lampion");
    let _ = std::fs::create_dir_all(&d);
    d
}

/// `%LOCALAPPDATA%\Lampion` (Lampion.log, downloads, caches).
pub fn local_data() -> PathBuf {
    let d = dirs::data_local_dir().unwrap_or_else(|| PathBuf::from(".")).join("Lampion");
    let _ = std::fs::create_dir_all(&d);
    d
}

/// Windows PowerShell by its full path, never whichever `powershell.exe` a search finds first.
pub fn powershell_exe() -> PathBuf {
    PathBuf::from(std::env::var("SystemRoot").unwrap_or_else(|_| r"C:\Windows".into())).join(r"System32\WindowsPowerShell\v1.0\powershell.exe")
}

/// A Windows path with forward slashes, as Apache, PHP and MariaDB configs want it.
pub fn fwd(p: &Path) -> String {
    p.to_string_lossy().replace('\\', "/")
}

/// True when `p` is inside a OneDrive-synced folder or Program Files, where Lampion must never put data.
pub fn is_unsafe_location(p: &Path) -> bool {
    let s = p.to_string_lossy().to_lowercase();
    if s.contains("\\program files") || s.contains("/program files") {
        return true;
    }
    for var in ["OneDrive", "OneDriveConsumer", "OneDriveCommercial"] {
        if let Ok(od) = std::env::var(var) {
            let od = od.to_lowercase();
            if !od.is_empty() && s.starts_with(&od) {
                return true;
            }
        }
    }
    s.contains("\\onedrive")
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn forward_slashes() {
        assert_eq!(fwd(Path::new(r"C:\Lampion\www")), "C:/Lampion/www");
    }
    #[test]
    fn powershell_by_its_full_path() {
        let p = powershell_exe();
        assert!(p.is_absolute() && p.ends_with(r"System32\WindowsPowerShell\v1.0\powershell.exe"), "{}", p.display());
    }
    #[test]
    fn unsafe_locations() {
        assert!(is_unsafe_location(Path::new(r"C:\Program Files\Lampion")));
        assert!(is_unsafe_location(Path::new(r"C:\Users\a\OneDrive\Lampion")));
        assert!(!is_unsafe_location(Path::new(r"C:\Lampion")));
    }
}
