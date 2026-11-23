//! What runs and listens: every process (ToolHelp snapshot), every listening TCP socket
//! (`GetExtendedTcpTable`, IPv4 and IPv6) and which of them are Lampion's servers. No program is started,
//! so this is cheap enough for the 2-second status check.

use super::{configs, Server};
use crate::core::paths;
use std::path::Path;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Proc {
    pub pid: u32,
    pub ppid: u32,
    pub name: String,
}

/// One server's processes and the TCP ports they listen on. `pids[0]` is the main process
/// (Apache's parent; its worker child follows).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Probe {
    pub running: bool,
    pub pids: Vec<u32>,
    pub ports: Vec<u16>,
}

/// The machine at one moment: processes and listening sockets (port, owner PID).
pub struct Snapshot {
    pub procs: Vec<Proc>,
    pub listeners: Vec<(u16, u32)>,
}

impl Snapshot {
    pub fn take() -> Snapshot {
        Snapshot { procs: imp::processes(), listeners: imp::listeners() }
    }

    /// Lampion's own processes of `server`: the same program file, or, when Windows does not tell a
    /// normal user the program file (a service runs as SYSTEM), the PID the server wrote to its PID file.
    pub fn probe(&self, server: Server) -> Probe {
        let exe = server.exe();
        let name = exe.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
        let mut file_pid: Option<Option<u32>> = None;
        let pids = ours(&self.procs, &name, |p| match imp::image_path(p.pid) {
            Some(path) => same_path(&path, &exe),
            None => *file_pid.get_or_insert_with(|| pid_file_pid(server)) == Some(p.pid),
        });
        let ports = ports_of(&self.listeners, &pids);
        Probe { running: !pids.is_empty(), pids, ports }
    }

    /// Who listens on `port`: "skype.exe" (or "PID 1234" when the name is unknown).
    pub fn port_owner(&self, port: u16) -> Option<String> {
        owner(&self.listeners, &self.procs, port)
    }
}

/// The PIDs of `name` processes that `is_ours` accepts, plus their children of the same name;
/// main processes first, each group sorted.
pub fn ours(procs: &[Proc], name: &str, mut is_ours: impl FnMut(&Proc) -> bool) -> Vec<u32> {
    let same: Vec<&Proc> = procs.iter().filter(|p| p.name.eq_ignore_ascii_case(name)).collect();
    let direct: Vec<u32> = same.iter().filter(|p| is_ours(p)).map(|p| p.pid).collect();
    let mine: Vec<&Proc> = same.into_iter().filter(|p| direct.contains(&p.pid) || direct.contains(&p.ppid)).collect();
    let pids: Vec<u32> = mine.iter().map(|p| p.pid).collect();
    let (mut main, mut children): (Vec<u32>, Vec<u32>) = (Vec::new(), Vec::new());
    for p in mine {
        if pids.contains(&p.ppid) {
            children.push(p.pid);
        } else {
            main.push(p.pid);
        }
    }
    main.sort_unstable();
    children.sort_unstable();
    main.extend(children);
    main
}

/// The ports these processes listen on, sorted, without duplicates (IPv4 and IPv6 sockets).
pub fn ports_of(listeners: &[(u16, u32)], pids: &[u32]) -> Vec<u16> {
    let mut ports: Vec<u16> = listeners.iter().filter(|(_, pid)| pids.contains(pid)).map(|(port, _)| *port).collect();
    ports.sort_unstable();
    ports.dedup();
    ports
}

/// The program listening on `port`, by name.
pub fn owner(listeners: &[(u16, u32)], procs: &[Proc], port: u16) -> Option<String> {
    let pid = listeners.iter().find(|(p, _)| *p == port)?.1;
    Some(match procs.iter().find(|p| p.pid == pid) {
        Some(p) if !p.name.is_empty() => p.name.clone(),
        _ => format!("PID {pid}"),
    })
}

fn same_path(a: &Path, b: &Path) -> bool {
    let norm = |p: &Path| p.to_string_lossy().trim_start_matches(r"\\?\").replace('/', "\\").to_lowercase();
    norm(a) == norm(b)
}

/// The PID the server wrote to its own PID file, if any.
fn pid_file_pid(server: Server) -> Option<u32> {
    let file = match server {
        Server::Apache => configs::apache_pid_file(&std::fs::read_to_string(paths::httpd_conf()).unwrap_or_default(), &paths::apache_dir()),
        Server::Mariadb => configs::mariadb_pid_file(&std::fs::read_to_string(paths::my_ini()).unwrap_or_default(), &paths::root()),
    };
    read_pid(&file)
}

pub fn read_pid(file: &Path) -> Option<u32> {
    std::fs::read_to_string(file).ok()?.trim().parse().ok()
}

/// Ends a process at once (the last resort when a server does not stop).
pub fn kill(pid: u32) -> bool {
    imp::kill(pid)
}

/// Sets a named event another process waits on; false when it does not exist.
pub fn signal_event(name: &str) -> bool {
    imp::signal_event(name)
}

#[cfg(windows)]
mod imp {
    use super::Proc;
    use std::path::PathBuf;
    use windows::core::{HSTRING, PWSTR};
    use windows::Win32::Foundation::CloseHandle;
    use windows::Win32::NetworkManagement::IpHelper::{
        GetExtendedTcpTable, MIB_TCP6TABLE_OWNER_PID, MIB_TCPTABLE_OWNER_PID, TCP_TABLE_OWNER_PID_LISTENER,
    };
    use windows::Win32::Networking::WinSock::{AF_INET, AF_INET6};
    use windows::Win32::System::Diagnostics::ToolHelp::{
        CreateToolhelp32Snapshot, Process32FirstW, Process32NextW, PROCESSENTRY32W, TH32CS_SNAPPROCESS,
    };
    use windows::Win32::System::Threading::{
        OpenEventW, OpenProcess, QueryFullProcessImageNameW, SetEvent, TerminateProcess, EVENT_MODIFY_STATE,
        PROCESS_NAME_WIN32, PROCESS_QUERY_LIMITED_INFORMATION, PROCESS_TERMINATE,
    };

    pub fn processes() -> Vec<Proc> {
        let mut out = Vec::new();
        unsafe {
            let Ok(snap) = CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0) else { return out };
            let mut e = PROCESSENTRY32W { dwSize: std::mem::size_of::<PROCESSENTRY32W>() as u32, ..Default::default() };
            let mut more = Process32FirstW(snap, &mut e).is_ok();
            while more {
                let len = e.szExeFile.iter().position(|&c| c == 0).unwrap_or(e.szExeFile.len());
                out.push(Proc {
                    pid: e.th32ProcessID,
                    ppid: e.th32ParentProcessID,
                    name: String::from_utf16_lossy(&e.szExeFile[..len]),
                });
                more = Process32NextW(snap, &mut e).is_ok();
            }
            let _ = CloseHandle(snap);
        }
        out
    }

    /// Every listening TCP socket as (port, owner PID).
    pub fn listeners() -> Vec<(u16, u32)> {
        let mut out = Vec::new();
        for family in [AF_INET, AF_INET6] {
            let Some(buf) = table(family.0 as u32) else { continue };
            unsafe {
                if family == AF_INET {
                    let t = &*(buf.as_ptr() as *const MIB_TCPTABLE_OWNER_PID);
                    let rows = std::slice::from_raw_parts(t.table.as_ptr(), t.dwNumEntries as usize);
                    out.extend(rows.iter().map(|r| (u16::from_be(r.dwLocalPort as u16), r.dwOwningPid)));
                } else {
                    let t = &*(buf.as_ptr() as *const MIB_TCP6TABLE_OWNER_PID);
                    let rows = std::slice::from_raw_parts(t.table.as_ptr(), t.dwNumEntries as usize);
                    out.extend(rows.iter().map(|r| (u16::from_be(r.dwLocalPort as u16), r.dwOwningPid)));
                }
            }
        }
        out
    }

    /// The raw listener table (u32 elements keep the rows aligned). Retries when it grew in between.
    fn table(family: u32) -> Option<Vec<u32>> {
        let mut size = 0u32;
        for _ in 0..4 {
            let mut buf = vec![0u32; size as usize / 4 + 64];
            size = (buf.len() * 4) as u32;
            let rc = unsafe {
                GetExtendedTcpTable(Some(buf.as_mut_ptr().cast()), &mut size, false, family, TCP_TABLE_OWNER_PID_LISTENER, 0)
            };
            match rc {
                0 => return Some(buf),
                122 => continue, // ERROR_INSUFFICIENT_BUFFER: `size` now holds what is needed
                _ => return None,
            }
        }
        None
    }

    pub fn image_path(pid: u32) -> Option<PathBuf> {
        unsafe {
            let h = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid).ok()?;
            let mut buf = [0u16; 1024];
            let mut len = buf.len() as u32;
            let r = QueryFullProcessImageNameW(h, PROCESS_NAME_WIN32, PWSTR(buf.as_mut_ptr()), &mut len);
            let _ = CloseHandle(h);
            r.ok()?;
            Some(PathBuf::from(String::from_utf16_lossy(&buf[..len as usize])))
        }
    }

    pub fn kill(pid: u32) -> bool {
        unsafe {
            let Ok(h) = OpenProcess(PROCESS_TERMINATE, false, pid) else { return false };
            let ok = TerminateProcess(h, 1).is_ok();
            let _ = CloseHandle(h);
            ok
        }
    }

    pub fn signal_event(name: &str) -> bool {
        unsafe {
            let Ok(h) = OpenEventW(EVENT_MODIFY_STATE, false, &HSTRING::from(name)) else { return false };
            let ok = SetEvent(h).is_ok();
            let _ = CloseHandle(h);
            ok
        }
    }
}

#[cfg(not(windows))]
mod imp {
    use super::Proc;
    use std::path::PathBuf;
    pub fn processes() -> Vec<Proc> {
        Vec::new()
    }
    pub fn listeners() -> Vec<(u16, u32)> {
        Vec::new()
    }
    pub fn image_path(_: u32) -> Option<PathBuf> {
        None
    }
    pub fn kill(_: u32) -> bool {
        false
    }
    pub fn signal_event(_: &str) -> bool {
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn p(pid: u32, ppid: u32, name: &str) -> Proc {
        Proc { pid, ppid, name: name.into() }
    }

    #[test]
    fn finds_apache_parent_and_child() {
        let procs = vec![p(4, 0, "System"), p(900, 1, "httpd.exe"), p(950, 900, "httpd.exe"), p(700, 1, "httpd.exe"), p(960, 900, "php.exe")];
        // 900 is ours (by program file or PID file); 700 is someone else's Apache (XAMPP).
        assert_eq!(ours(&procs, "httpd.exe", |x| x.pid == 900), vec![900, 950]);
        assert_eq!(ours(&procs, "HTTPD.EXE", |x| x.pid == 950 || x.pid == 900), vec![900, 950]);
        assert!(ours(&procs, "mariadbd.exe", |_| true).is_empty());
    }

    #[test]
    fn listening_ports_and_owner() {
        let procs = vec![p(4, 0, "System"), p(1234, 1, "Skype.exe"), p(900, 1, "httpd.exe")];
        let listeners = vec![(8080, 1234), (80, 4), (8081, 900), (8081, 900), (443, 900), (5000, 77)];
        assert_eq!(ports_of(&listeners, &[900]), vec![443, 8081]);
        assert_eq!(owner(&listeners, &procs, 8080).as_deref(), Some("Skype.exe"));
        assert_eq!(owner(&listeners, &procs, 80).as_deref(), Some("System"));
        assert_eq!(owner(&listeners, &procs, 5000).as_deref(), Some("PID 77"));
        assert_eq!(owner(&listeners, &procs, 3306), None);
    }

    #[test]
    fn paths_compare_like_windows() {
        assert!(same_path(Path::new(r"\\?\C:\Lampion\bin\apache\bin\httpd.exe"), Path::new("c:/lampion/bin/apache/bin/HTTPD.exe")));
        assert!(!same_path(Path::new(r"C:\xampp\apache\bin\httpd.exe"), Path::new(r"C:\Lampion\bin\apache\bin\httpd.exe")));
    }

    #[test]
    fn pid_files() {
        let dir = tempfile::tempdir().unwrap();
        let f = dir.path().join("httpd.pid");
        std::fs::write(&f, "4321\r\n").unwrap();
        assert_eq!(read_pid(&f), Some(4321));
        assert_eq!(read_pid(&dir.path().join("none.pid")), None);
    }

    #[cfg(windows)]
    #[test]
    fn live_tables_are_readable() {
        // Reads this machine's tables without starting anything; this test process is in them.
        let snap = Snapshot::take();
        assert!(snap.procs.iter().any(|x| x.pid == std::process::id()));
        assert!(snap.listeners.iter().all(|(port, _)| *port > 0));
    }
}
