//! root's password, when the user gave root one (MariaDB › root without a password: off), kept for the
//! next launch in `%APPDATA%\Lampion\root.secret`, encrypted with DPAPI for this Windows user
//! (`CryptProtectData`). Lampion needs it to stop MariaDB in order and to connect. Never logged.

use std::path::{Path, PathBuf};

fn file() -> PathBuf {
    crate::core::paths::app_data().join("root.secret")
}

/// Keeps the password, or forgets it (`None`).
pub fn save(password: Option<&str>) -> Result<(), String> {
    save_in(&file(), password)
}

/// The kept password; `None` when there is none or it cannot be decrypted (another Windows user).
pub fn load() -> Option<String> {
    load_from(&file())
}

fn save_in(path: &Path, password: Option<&str>) -> Result<(), String> {
    let Some(password) = password else {
        return match std::fs::remove_file(path) {
            Err(e) if e.kind() != std::io::ErrorKind::NotFound => Err(format!("Could not delete {}: {e}", path.display())),
            _ => Ok(()),
        };
    };
    let blob = imp::protect(password.as_bytes())?;
    let tmp = path.with_extension("secret-new");
    std::fs::write(&tmp, blob).and_then(|()| std::fs::rename(&tmp, path)).map_err(|e| {
        let _ = std::fs::remove_file(&tmp);
        format!("Could not save root's password for the next launch: {e}")
    })
}

fn load_from(path: &Path) -> Option<String> {
    String::from_utf8(imp::unprotect(&std::fs::read(path).ok()?)?).ok()
}

#[cfg(windows)]
mod imp {
    use std::ffi::c_void;
    use std::ptr::{null, null_mut};
    use windows::Win32::Foundation::{LocalFree, HLOCAL};

    #[repr(C)]
    struct Blob {
        len: u32,
        data: *mut u8,
    }

    #[link(name = "crypt32")]
    extern "system" {
        fn CryptProtectData(input: *const Blob, descr: *const u16, entropy: *const Blob, reserved: *mut c_void, prompt: *mut c_void, flags: u32, output: *mut Blob) -> i32;
        fn CryptUnprotectData(input: *const Blob, descr: *mut *mut u16, entropy: *const Blob, reserved: *mut c_void, prompt: *mut c_void, flags: u32, output: *mut Blob) -> i32;
    }

    const CRYPTPROTECT_UI_FORBIDDEN: u32 = 0x1;

    /// Runs CryptProtectData / CryptUnprotectData and copies out (and frees) what Windows allocated.
    fn call(bytes: &[u8], encrypt: bool) -> Option<Vec<u8>> {
        let input = Blob { len: u32::try_from(bytes.len()).ok()?, data: bytes.as_ptr() as *mut u8 };
        let mut out = Blob { len: 0, data: null_mut() };
        // SAFETY: `input` points at `bytes` for the call; Windows allocates `out` with LocalAlloc.
        let ok = unsafe {
            if encrypt {
                CryptProtectData(&input, null(), null(), null_mut(), null_mut(), CRYPTPROTECT_UI_FORBIDDEN, &mut out)
            } else {
                CryptUnprotectData(&input, null_mut(), null(), null_mut(), null_mut(), CRYPTPROTECT_UI_FORBIDDEN, &mut out)
            }
        };
        if ok == 0 || out.data.is_null() {
            return None;
        }
        // SAFETY: `out` is the buffer Windows just returned; it is freed once, right after the copy.
        let copy = unsafe { std::slice::from_raw_parts(out.data, out.len as usize).to_vec() };
        unsafe { LocalFree(HLOCAL(out.data.cast())) };
        Some(copy)
    }

    pub fn protect(bytes: &[u8]) -> Result<Vec<u8>, String> {
        call(bytes, true).ok_or_else(|| format!("Windows could not encrypt root's password: {}", std::io::Error::last_os_error()))
    }

    pub fn unprotect(blob: &[u8]) -> Option<Vec<u8>> {
        call(blob, false)
    }
}

#[cfg(not(windows))]
mod imp {
    pub fn protect(_: &[u8]) -> Result<Vec<u8>, String> {
        Err("Saving root's password needs Windows.".into())
    }
    pub fn unprotect(_: &[u8]) -> Option<Vec<u8>> {
        None
    }
}

#[cfg(all(test, windows))]
mod tests {
    use super::*;

    #[test]
    fn kept_encrypted_and_forgotten() {
        let dir = tempfile::tempdir().unwrap();
        let f = dir.path().join("root.secret");
        save_in(&f, Some("pä$$ wörd")).unwrap();
        let raw = std::fs::read(&f).unwrap();
        assert!(!raw.windows(3).any(|w| w == b"$$ "), "never stored as plain text");
        assert_eq!(load_from(&f).as_deref(), Some("pä$$ wörd"));
        save_in(&f, None).unwrap();
        assert!(!f.exists());
        save_in(&f, None).unwrap();
        std::fs::write(&f, b"not a DPAPI blob").unwrap();
        assert_eq!(load_from(&f), None);
    }
}
