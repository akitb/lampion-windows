//! Optional Windows-service mode (`settings.use_services`): Apache and MariaDB run as the services
//! LampionApache and LampionMariaDB and start with Windows. Installing or removing them needs
//! administrator rights, so it is one elevated script (one UAC prompt). The script also lets the
//! signed-in user start and stop the two services, so everyday starts and stops need no prompt.
//!
//! The services run as `NT AUTHORITY\LocalService`, never as SYSTEM, and the same script locks the
//! Lampion folder down: it no longer inherits C:\'s "Authenticated Users: Modify", so only
//! Administrators, SYSTEM and the user who turned the mode on can change the programs and configs the
//! services run; LocalService may change the data, logs, sites and temp files and read the rest.

use super::control::{self, op_start, op_stop};
use super::probe::Snapshot;
use super::{elevate, monitor, moving, Server};
use crate::core::{activity, paths, settings, shell};
use std::path::{Path, PathBuf};
use std::time::Duration;

/// The default service permissions, plus start/stop/pause for interactive users (IU).
const SDDL: &str =
    "D:(A;;CCLCSWRPWPDTLOCRRC;;;SY)(A;;CCDCLCSWRPWPDTLOCRSDRCWDWO;;;BA)(A;;CCLCSWRPWPDTLOCRRC;;;IU)(A;;CCLCSWLOCRRC;;;SU)";

fn sc(args: &[&str]) -> shell::Output {
    let windows = std::env::var("SystemRoot").unwrap_or_else(|_| r"C:\Windows".into());
    shell::run_timeout(Path::new(&windows).join(r"System32\sc.exe"), args, Duration::from_secs(30))
}

fn last_line(text: &str) -> String {
    text.lines().map(str::trim).filter(|l| !l.is_empty()).last().unwrap_or("").to_string()
}

/// `sc start` (the exit code is the Windows error number).
pub fn start(s: Server) -> Result<(), String> {
    let out = sc(&["start", s.service()]);
    match out.status {
        0 | 1056 => Ok(()), // 1056: already running
        1060 => Err(format!(
            "The {} service is not installed. Turn “Run the servers as Windows services” off and on again in General.",
            s.service()
        )),
        5 => Err(format!("Windows did not allow Lampion to start the {} service.", s.service())),
        _ => Err(format!("Windows could not start the {} service: {}", s.service(), last_line(&out.out))),
    }
}

/// `sc stop`; a service that is not running counts as stopped.
pub fn stop(s: Server) -> Result<(), String> {
    let out = sc(&["stop", s.service()]);
    match out.status {
        0 | 1062 => Ok(()), // 1062: not started
        _ => Err(format!("Windows could not stop the {} service: {}", s.service(), last_line(&out.out))),
    }
}

/// A path in a cmd script: quoted, with `%` doubled (cmd expands `%name%` even inside quotes).
fn q(p: &Path) -> String {
    format!("\"{}\"", p.display().to_string().replace('%', "%%"))
}

/// The service account: no administrator rights, no SYSTEM.
const SERVICE_ACCOUNT: &str = r"NT AUTHORITY\LocalService";
/// LocalService by SID (like Administrators and SYSTEM below), so icacls works in every Windows language.
const LOCAL_SERVICE: &str = "*S-1-5-19";

/// The script lines that lock the Lampion folder down for the services: it stops inheriting (C:\ gives
/// Authenticated Users Modify on new folders), full control stays with Administrators, SYSTEM and the
/// user `user_sid`; LocalService may read and run `bin` and `config` and change the `writable` folders.
pub fn acl_lines(root: &Path, user_sid: &str, writable: &[PathBuf], out: &str) -> Vec<String> {
    let mut lines = vec![format!(
        "icacls.exe {} /inheritance:r /grant:r \"*S-1-5-32-544:(OI)(CI)F\" \"*S-1-5-18:(OI)(CI)F\" \"*{user_sid}:(OI)(CI)F\" /Q {out} || exit /b 16",
        q(root)
    )];
    for dir in ["bin", "config"] {
        lines.push(format!("icacls.exe {} /grant \"{LOCAL_SERVICE}:(OI)(CI)RX\" /Q {out} || exit /b 17", q(&root.join(dir))));
    }
    for dir in writable {
        lines.push(format!("icacls.exe {} /grant \"{LOCAL_SERVICE}:(OI)(CI)M\" /Q {out} || exit /b 17", q(dir)));
    }
    lines
}

/// The elevated script that installs both services. `log` collects the programs' output; `user_sid`
/// keeps full control of the Lampion folder; LocalService may change the `writable` folders.
pub fn install_script(root: &Path, log: &Path, user_sid: &str, writable: &[PathBuf]) -> String {
    let httpd = root.join(r"bin\apache\bin\httpd.exe");
    let conf = root.join(r"config\httpd.conf");
    let mariadbd = root.join(r"bin\mariadb\bin\mariadbd.exe");
    let my_ini = root.join(r"config\my.ini");
    let php = root.join(r"bin\php");
    let out = format!(">>{} 2>&1", q(log));
    let mut lines = vec![
        "@echo off".to_string(),
        "rem Lampion: installs Apache and MariaDB as Windows services (runs once, elevated).".into(),
        "sc.exe stop LampionApache >nul 2>&1".into(),
        "sc.exe delete LampionApache >nul 2>&1".into(),
        "sc.exe stop LampionMariaDB >nul 2>&1".into(),
        "sc.exe delete LampionMariaDB >nul 2>&1".into(),
    ];
    lines.extend(acl_lines(root, user_sid, writable, &out));
    lines.extend([
        format!("{} -k install -n \"LampionApache\" -f {} {out} || exit /b 11", q(&httpd), q(&conf)),
        format!("sc.exe config LampionApache obj= \"{SERVICE_ACCOUNT}\" password= \"\" {out} || exit /b 18"),
        format!("sc.exe sdset LampionApache \"{SDDL}\" {out} || exit /b 12"),
        // The service does not inherit Lampion's PATH; Apache needs PHP's folder for PHP's DLLs.
        format!(
            "reg.exe add \"HKLM\\SYSTEM\\CurrentControlSet\\Services\\LampionApache\" /v Environment /t REG_MULTI_SZ /d \"PATH={};%SystemRoot%\\System32;%SystemRoot%\" /f {out} || exit /b 13",
            php.display().to_string().replace('%', "%%")
        ),
        format!("{} --install LampionMariaDB --defaults-file={} {out} || exit /b 14", q(&mariadbd), q(&my_ini)),
        format!("sc.exe config LampionMariaDB obj= \"{SERVICE_ACCOUNT}\" password= \"\" {out} || exit /b 18"),
        format!("sc.exe sdset LampionMariaDB \"{SDDL}\" {out} || exit /b 15"),
        "exit /b 0".into(),
        String::new(),
    ]);
    lines.join("\r\n")
}

/// The folders the services write to: the Lampion folder's data, logs, sites and temp files,
/// phpMyAdmin's temp folder (its template cache, in the read-only `bin`; made here when phpMyAdmin is
/// installed), and the data folder, web folder and log folders the configuration names elsewhere.
/// Existing ones only.
fn writable_folders(root: &Path) -> Vec<PathBuf> {
    let l = crate::config::locations::current();
    let logs = [Some(l.apache_error_log), Some(l.apache_access_log), l.mariadb_error_log, l.php_error_log, l.slow_query_log, l.general_query_log];
    // create_dir, not create_dir_all: without phpMyAdmin's folder there is nothing to make.
    let _ = std::fs::create_dir(root.join(r"bin\phpmyadmin\tmp"));
    let mut dirs: Vec<PathBuf> = ["databases", "logs", "tmp", "www", r"bin\phpmyadmin\tmp"].iter().map(|d| root.join(d)).collect();
    dirs.extend([l.mariadb_datadir, l.document_root]);
    dirs.extend(logs.into_iter().flatten().filter_map(|f| f.parent().map(Path::to_path_buf)));
    let mut seen = Vec::new();
    dirs.retain(|d| {
        let key = d.to_string_lossy().trim_end_matches('\\').to_lowercase();
        d.is_dir() && !seen.contains(&key) && {
            seen.push(key);
            true
        }
    });
    dirs
}

/// The signed-in user's SID, from `whoami /user /fo csv /nh` (`"pc\name","S-1-5-21-…"`).
pub fn sid_from_whoami(out: &str) -> Option<String> {
    let sid = out.trim().rsplit(',').next()?.trim().trim_matches('"');
    (sid.starts_with("S-1-") && sid.chars().all(|c| c.is_ascii_digit() || c == '-' || c == 'S')).then(|| sid.to_string())
}

fn user_sid() -> Option<String> {
    let whoami = PathBuf::from(std::env::var("SystemRoot").unwrap_or_else(|_| r"C:\Windows".into())).join(r"System32\whoami.exe");
    sid_from_whoami(&shell::run_timeout(whoami, &["/user", "/fo", "csv", "/nh"], Duration::from_secs(10)).out)
}

/// The elevated script that removes both services.
pub fn remove_script(log: &Path) -> String {
    let out = format!(">>\"{}\" 2>&1", log.display());
    [
        "@echo off".to_string(),
        "rem Lampion: removes the Apache and MariaDB services (runs once, elevated).".into(),
        "sc.exe stop LampionApache >nul 2>&1".into(),
        "sc.exe stop LampionMariaDB >nul 2>&1".into(),
        format!("sc.exe delete LampionApache {out}"),
        format!("sc.exe delete LampionMariaDB {out}"),
        "exit /b 0".into(),
        String::new(),
    ]
    .join("\r\n")
}

/// General › Run the servers as Windows services. Stops the running servers, installs or removes the
/// services with one UAC prompt, and starts again what was running, now in the new mode.
pub async fn set_mode(enabled: bool) -> Result<(), String> {
    if settings::get().use_services == enabled {
        return Ok(());
    }
    if enabled && !Server::ALL.iter().all(|s| s.installed()) {
        return Err("Finish setting up Apache and MariaDB first.".into());
    }
    if moving::active() {
        return Err("The Lampion folder is being moved; try again when that has finished.".into());
    }
    let mine = monitor::set_busy(&Server::ALL);
    if mine.len() < Server::ALL.len() {
        monitor::finish(&mine);
        return Err("A server action is still running; try again when it has finished.".into());
    }
    let job = control::submit(move || {
        let r = switch(enabled);
        monitor::finish(&Server::ALL);
        r
    });
    control::wait(job).await?
}

fn switch(enabled: bool) -> Result<(), String> {
    let snap = Snapshot::take();
    let running: Vec<Server> = Server::ALL.into_iter().filter(|s| snap.probe(*s).running).collect();
    for s in &running {
        op_stop(*s)?;
    }
    let result = run_script(enabled);
    match &result {
        Ok(()) => {
            settings::update(|st| st.use_services = enabled);
            activity::ok(
                "Panel",
                if enabled {
                    "Apache and MariaDB are now Windows services and start with Windows."
                } else {
                    "Apache and MariaDB are no longer Windows services; Lampion runs them in the background."
                },
            );
        }
        Err(e) => activity::error("Panel", e),
    }
    for s in &running {
        let _ = op_start(*s);
    }
    result
}

fn run_script(enabled: bool) -> Result<(), String> {
    let dir = paths::local_data();
    let log: PathBuf = dir.join("services.log");
    let script = dir.join("lampion-services.cmd");
    let _ = std::fs::remove_file(&log);
    let text = if enabled {
        // The user who turns the mode on keeps full control (an elevated script may run as another admin).
        let sid = user_sid().ok_or("Lampion could not find out your Windows account, so nothing was changed.")?;
        let root = paths::root();
        install_script(&root, &log, &sid, &writable_folders(&root))
    } else {
        remove_script(&log)
    };
    std::fs::write(&script, text).map_err(|e| format!("Could not write {}: {e}", script.display()))?;
    activity::info("Panel", "Windows asks for permission to change the services…");
    let cmd = Path::new(&std::env::var("ComSpec").unwrap_or_else(|_| r"C:\Windows\System32\cmd.exe".into())).to_path_buf();
    let code = elevate::run(&cmd, &format!("/d /s /c \"\"{}\"\"", script.display()), Duration::from_secs(180));
    let _ = std::fs::remove_file(&script);
    let output = std::fs::read_to_string(&log).map(|t| last_line(&t)).unwrap_or_default();
    match code? {
        0 => Ok(()),
        elevate::DECLINED => Err("Nothing was changed: the Windows permission prompt was declined.".into()),
        11 => Err(format!("Apache could not be installed as a service: {output}")),
        14 => Err(format!("MariaDB could not be installed as a service: {output}")),
        16 | 17 => Err(format!("Windows could not set the permissions of the Lampion folder: {output}")),
        18 => Err(format!("Windows could not set the services' account (LocalService): {output}")),
        c => Err(format!("Windows could not change the services (step {c}): {output}")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SID: &str = "S-1-5-21-1111111111-2222222222-3333333333-1001";

    #[test]
    fn scripts() {
        let root = Path::new(r"C:\My Lampion");
        let s = install_script(root, Path::new(r"C:\Users\a\AppData\Local\Lampion\services.log"), SID, &[root.join("www")]);
        assert!(s.contains("\"C:\\My Lampion\\bin\\apache\\bin\\httpd.exe\" -k install -n \"LampionApache\" -f \"C:\\My Lampion\\config\\httpd.conf\""));
        assert!(s.contains("\"C:\\My Lampion\\bin\\mariadb\\bin\\mariadbd.exe\" --install LampionMariaDB --defaults-file=\"C:\\My Lampion\\config\\my.ini\""));
        assert!(s.contains("PATH=C:\\My Lampion\\bin\\php;"));
        assert!(s.contains(&format!("sc.exe sdset LampionMariaDB \"{SDDL}\"")));
        assert!(s.ends_with("exit /b 0\r\n"));
        let r = remove_script(Path::new(r"C:\x.log"));
        assert!(r.contains("sc.exe delete LampionApache") && r.contains("sc.exe delete LampionMariaDB"));
    }

    #[test]
    fn services_run_as_local_service_in_a_locked_folder() {
        let root = Path::new(r"C:\My Lampion");
        let writable = [root.join("databases"), root.join(r"bin\phpmyadmin\tmp"), PathBuf::from(r"D:\Sites")];
        let s = install_script(root, Path::new(r"C:\x.log"), SID, &writable);
        let lines: Vec<&str> = s.lines().collect();
        let at = |needle: &str| lines.iter().position(|l| l.contains(needle)).unwrap_or_else(|| panic!("{needle}"));
        for svc in ["LampionApache", "LampionMariaDB"] {
            let obj = at(&format!(r#"sc.exe config {svc} obj= "NT AUTHORITY\LocalService" password= """#));
            assert!(obj > at(if svc == "LampionApache" { "-k install" } else { "--install" }), "after it is installed");
        }
        assert!(!s.contains("LocalSystem"));
        // The folder is locked down before any service exists.
        let lock = at("/inheritance:r");
        assert!(lock < at("-k install"));
        assert!(lines[lock].starts_with(r#"icacls.exe "C:\My Lampion" /inheritance:r /grant:r "*S-1-5-32-544:(OI)(CI)F" "*S-1-5-18:(OI)(CI)F" "*S-1-5-21-1111111111-2222222222-3333333333-1001:(OI)(CI)F""#), "{}", lines[lock]);
        assert!(s.contains(r#"icacls.exe "C:\My Lampion\bin" /grant "*S-1-5-19:(OI)(CI)RX""#));
        assert!(s.contains(r#"icacls.exe "C:\My Lampion\config" /grant "*S-1-5-19:(OI)(CI)RX""#));
        assert!(s.contains(r#"icacls.exe "C:\My Lampion\databases" /grant "*S-1-5-19:(OI)(CI)M""#));
        assert!(s.contains(r#"icacls.exe "D:\Sites" /grant "*S-1-5-19:(OI)(CI)M""#));
        let pma = at(r#"icacls.exe "C:\My Lampion\bin\phpmyadmin\tmp" /grant "*S-1-5-19:(OI)(CI)M""#);
        assert!(pma > at(r#"icacls.exe "C:\My Lampion\bin" /grant"#), "phpMyAdmin's template cache is writable inside the read-only bin");
        assert!(!s.contains(r#""C:\My Lampion\bin" /grant "*S-1-5-19:(OI)(CI)M"#), "the programs stay read-only for the service");
        assert!(s.lines().filter(|l| l.starts_with("icacls.exe")).all(|l| l.ends_with("|| exit /b 16") || l.ends_with("|| exit /b 17")), "a failed step stops the script");
    }

    #[test]
    fn percent_signs_in_paths_are_not_expanded() {
        let root = Path::new(r"C:\100%Lampion");
        let s = install_script(root, Path::new(r"C:\x.log"), SID, &[]);
        assert!(s.contains(r#""C:\100%%Lampion\bin\apache\bin\httpd.exe" -k install"#));
        assert!(!s.contains("100%L"));
    }

    #[test]
    fn the_users_sid_from_whoami() {
        assert_eq!(sid_from_whoami("\"desktop-1\\anna\",\"S-1-5-21-1-2-3-1001\"\r\n").as_deref(), Some("S-1-5-21-1-2-3-1001"));
        assert_eq!(sid_from_whoami("\"pc\\x\",\"S-1-5-21-1&calc\""), None, "only a SID goes into the script");
        assert_eq!(sid_from_whoami("ERROR: no"), None);
        assert_eq!(sid_from_whoami(""), None);
    }

    #[test]
    fn writable_folders_are_existing_and_unique() {
        let dir = tempfile::tempdir().unwrap();
        for d in ["databases", "logs", "www"] {
            std::fs::create_dir_all(dir.path().join(d)).unwrap();
        }
        let w = writable_folders(dir.path());
        assert!(w.contains(&dir.path().join("www")) && w.contains(&dir.path().join("databases")));
        assert!(!w.contains(&dir.path().join("tmp")), "missing folders are left out");
        assert!(!dir.path().join("bin").exists(), "no phpMyAdmin: no folder is made for it");
        std::fs::create_dir_all(dir.path().join(r"bin\phpmyadmin")).unwrap();
        let pma = dir.path().join(r"bin\phpmyadmin\tmp");
        assert!(writable_folders(dir.path()).contains(&pma) && pma.is_dir(), "phpMyAdmin's temp folder is made and included");
        let mut keys: Vec<String> = w.iter().map(|p| p.to_string_lossy().to_lowercase()).collect();
        let n = keys.len();
        keys.dedup();
        assert_eq!(keys.len(), n);
    }
}
