//! Running a program with administrator rights: one UAC prompt, then wait for it to finish. Uses
//! PowerShell's `Start-Process -Verb RunAs` (passed as `-EncodedCommand`, so no quoting problems and no
//! execution policy), which needs no extra Windows API bindings.

use crate::core::shell;
use std::path::Path;
use std::time::Duration;

/// ERROR_CANCELLED: the UAC prompt was declined.
pub const DECLINED: i32 = 1223;

/// Runs `program` with `args` (one Windows command line) elevated and hidden, and waits for it.
/// Returns its exit code (`DECLINED` when the prompt was declined).
pub fn run(program: &Path, args: &str, timeout: Duration) -> Result<i32, String> {
    let script = script(program, args);
    let encoded = base64(&script.encode_utf16().flat_map(u16::to_le_bytes).collect::<Vec<u8>>());
    let out = shell::run_timeout("powershell.exe", &["-NoProfile", "-NonInteractive", "-EncodedCommand", &encoded], timeout);
    match out.status {
        -1 => Err(format!("Could not ask Windows for administrator rights: {}", out.out.trim())),
        9999 => Err(format!("Windows could not run {}: {}", program.display(), out.out.trim())),
        code => Ok(code),
    }
}

/// The PowerShell that starts `program` elevated and exits with its exit code.
fn script(program: &Path, args: &str) -> String {
    let quote = |s: &str| format!("'{}'", s.replace('\'', "''"));
    let arg_list = if args.is_empty() { String::new() } else { format!(" -ArgumentList {}", quote(args)) };
    format!(
        "$ErrorActionPreference = 'Stop'\n\
         try {{\n\
           $p = Start-Process -FilePath {}{arg_list} -Verb RunAs -WindowStyle Hidden -Wait -PassThru\n\
           exit $p.ExitCode\n\
         }} catch {{\n\
           $e = $_.Exception\n\
           while ($e -and -not ($e -is [System.ComponentModel.Win32Exception])) {{ $e = $e.InnerException }}\n\
           if ($e -and $e.NativeErrorCode -eq 1223) {{ exit 1223 }}\n\
           [Console]::Error.WriteLine($_.Exception.Message)\n\
           exit 9999\n\
         }}\n",
        quote(&program.to_string_lossy())
    )
}

/// Standard base64 with padding (what `-EncodedCommand` expects).
fn base64(bytes: &[u8]) -> String {
    const T: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity(bytes.len().div_ceil(3) * 4);
    for c in bytes.chunks(3) {
        let n = (u32::from(c[0]) << 16) | (u32::from(*c.get(1).unwrap_or(&0)) << 8) | u32::from(*c.get(2).unwrap_or(&0));
        for i in 0..4 {
            out.push(if i <= c.len() { T[(n >> (18 - 6 * i)) as usize & 63] as char } else { '=' });
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn base64_vectors() {
        // RFC 4648 test vectors.
        for (input, want) in [("", ""), ("f", "Zg=="), ("fo", "Zm8="), ("foo", "Zm9v"), ("foob", "Zm9vYg=="), ("foobar", "Zm9vYmFy")] {
            assert_eq!(base64(input.as_bytes()), want);
        }
    }

    #[test]
    fn script_quotes_paths() {
        let s = script(Path::new(r"C:\Users\O'Brien\vc_redist.x64.exe"), "/install /passive /norestart");
        assert!(s.contains(r"-FilePath 'C:\Users\O''Brien\vc_redist.x64.exe' -ArgumentList '/install /passive /norestart' -Verb RunAs"));
        assert!(!script(Path::new("a.exe"), "").contains("-ArgumentList"));
    }
}
