//! Optional Windows-service mode (`settings.use_services`): Apache and MariaDB run as the services
//! LampionApache and LampionMariaDB and start with Windows. Installing or removing them needs
//! administrator rights, so it is one elevated script (one UAC prompt). The script also lets the
//! signed-in user start and stop the two services, so everyday starts and stops need no prompt.

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

/// The elevated script that installs both services. `log` collects the programs' output.
pub fn install_script(root: &Path, log: &Path) -> String {
    let httpd = root.join(r"bin\apache\bin\httpd.exe");
    let conf = root.join(r"config\httpd.conf");
    let mariadbd = root.join(r"bin\mariadb\bin\mariadbd.exe");
    let my_ini = root.join(r"config\my.ini");
    let php = root.join(r"bin\php");
    let out = format!(">>\"{}\" 2>&1", log.display());
    [
        "@echo off".to_string(),
        "rem Lampion: installs Apache and MariaDB as Windows services (runs once, elevated).".into(),
        "sc.exe stop LampionApache >nul 2>&1".into(),
        "sc.exe delete LampionApache >nul 2>&1".into(),
        "sc.exe stop LampionMariaDB >nul 2>&1".into(),
        "sc.exe delete LampionMariaDB >nul 2>&1".into(),
        format!("\"{}\" -k install -n \"LampionApache\" -f \"{}\" {out} || exit /b 11", httpd.display(), conf.display()),
        format!("sc.exe sdset LampionApache \"{SDDL}\" {out} || exit /b 12"),
        // The service does not inherit Lampion's PATH; Apache needs PHP's folder for PHP's DLLs.
        format!(
            "reg.exe add \"HKLM\\SYSTEM\\CurrentControlSet\\Services\\LampionApache\" /v Environment /t REG_MULTI_SZ /d \"PATH={};%SystemRoot%\\System32;%SystemRoot%\" /f {out} || exit /b 13",
            php.display()
        ),
        format!("\"{}\" --install LampionMariaDB --defaults-file=\"{}\" {out} || exit /b 14", mariadbd.display(), my_ini.display()),
        format!("sc.exe sdset LampionMariaDB \"{SDDL}\" {out} || exit /b 15"),
        "exit /b 0".into(),
        String::new(),
    ]
    .join("\r\n")
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
    let text = if enabled { install_script(&paths::root(), &log) } else { remove_script(&log) };
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
        c => Err(format!("Windows could not change the services (step {c}): {output}")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scripts() {
        let s = install_script(Path::new(r"C:\My Lampion"), Path::new(r"C:\Users\a\AppData\Local\Lampion\services.log"));
        assert!(s.contains("\"C:\\My Lampion\\bin\\apache\\bin\\httpd.exe\" -k install -n \"LampionApache\" -f \"C:\\My Lampion\\config\\httpd.conf\""));
        assert!(s.contains("\"C:\\My Lampion\\bin\\mariadb\\bin\\mariadbd.exe\" --install LampionMariaDB --defaults-file=\"C:\\My Lampion\\config\\my.ini\""));
        assert!(s.contains("PATH=C:\\My Lampion\\bin\\php;"));
        assert!(s.contains(&format!("sc.exe sdset LampionMariaDB \"{SDDL}\"")));
        assert!(s.ends_with("exit /b 0\r\n"));
        let r = remove_script(Path::new(r"C:\x.log"));
        assert!(r.contains("sc.exe delete LampionApache") && r.contains("sc.exe delete LampionMariaDB"));
    }
}
