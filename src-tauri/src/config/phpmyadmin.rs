//! phpMyAdmin's config.inc.php: automatic sign-in as root (auth_type 'config') or the login page.

/// shortcut: stub; the config module implements it.
pub fn auto_login_enabled() -> bool {
    false
}

/// Writes config.inc.php (checked with `php -l` first). Creates it from config.sample.inc.php with a
/// random 32-byte blowfish_secret when missing. shortcut: stub; the config module implements it.
pub fn set_auto_login(_enabled: bool) -> Result<(), String> {
    Err("not implemented yet".into())
}
