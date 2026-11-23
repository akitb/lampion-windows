//! root without a password from this computer only (TCP 127.0.0.1 / ::1 / localhost).

/// Some(true) = root logs in with an empty password over TCP; Some(false) = refused (1045/1698);
/// None = unknown (MariaDB not running).
/// shortcut: stub; the config module implements it.
pub async fn root_no_password_state() -> Option<bool> {
    None
}

/// Turns password-less root on, or off with `password` as the new root password.
/// Never creates root@'%'. shortcut: stub; the config module implements it.
pub async fn set_root_no_password(_enabled: bool, _password: Option<String>) -> Result<(), String> {
    Err("not implemented yet".into())
}
