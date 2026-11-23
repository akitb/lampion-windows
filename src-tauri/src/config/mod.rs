//! See docs/SPEC.md and docs/AGENTS.md for what this module owns.
//! `locations`, `access` and `phpmyadmin` are called by other modules: keep their signatures.
//! The editors (`apache`, `php`, `mariadb` on top of `file` and `ini`) only touch the lines of the
//! values that changed, and a server checks every change on a temp copy before the real file is written.

pub mod access;
pub mod apache;
pub mod commands;
pub mod file;
pub mod ini;
pub mod locations;
pub mod mariadb;
pub mod php;
pub mod phpmyadmin;
pub mod timezones;

/// Hand edits of httpd.conf / my.ini / php.ini: the ports and folders are read again every 10 s and
/// the UI hears about it (`config://locations`) only when something changed.
pub fn init(_app: &tauri::AppHandle) {
    std::thread::spawn(|| {
        let mut last = None;
        loop {
            let now = locations::current();
            if last.as_ref() != Some(&now) {
                crate::core::emit("config://locations", now.clone());
                last = Some(now);
            }
            std::thread::sleep(std::time::Duration::from_secs(10));
        }
    });
}
