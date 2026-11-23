//! The Authenticode check of a downloaded installer (`WinVerifyTrust`, the check Explorer does) and who
//! signed it. wintrust.dll and crypt32.dll are declared here directly, so no further `windows` crate
//! features are needed.

use std::path::Path;

#[derive(Debug, PartialEq, Eq)]
pub enum Signature {
    /// Signed, and the signature and its certificate chain are valid.
    Valid,
    /// No signature at all (Lampion's releases until a code-signing certificate is used).
    Unsigned,
    /// Signed, but changed since, or the certificate is not trusted: never run it. The Windows code.
    Invalid(u32),
}

pub fn check(path: &Path) -> Signature {
    verify(path).0
}

/// Who signed a file whose signature is valid: the signing certificate's name and its chain's root.
pub fn signer(path: &Path) -> Option<(String, String)> {
    verify(path).1
}

/// A program from Microsoft (the Visual C++ runtime installer): validly signed by Microsoft Corporation
/// with a certificate chain that ends in one of Microsoft's own roots.
pub fn signed_by_microsoft(path: &Path) -> bool {
    matches!(signer(path), Some((name, root)) if name == "Microsoft Corporation" && root.starts_with("Microsoft "))
}

/// Opens `path` so that others may read and run it but nobody can change, replace or delete it while the
/// handle lives: what was checked is what runs.
pub fn hold(path: &Path) -> std::io::Result<std::fs::File> {
    let mut o = std::fs::OpenOptions::new();
    o.read(true);
    #[cfg(windows)]
    std::os::windows::fs::OpenOptionsExt::share_mode(&mut o, 0x1); // FILE_SHARE_READ only
    o.open(path)
}

#[cfg(windows)]
fn verify(path: &Path) -> (Signature, Option<(String, String)>) {
    use std::ffi::c_void;
    use std::os::windows::ffi::OsStrExt;
    use std::ptr::{null, null_mut};
    use windows::core::GUID;
    use windows::Win32::Foundation::GetLastError;

    #[repr(C)]
    struct FileInfo {
        cb_struct: u32,
        path: *const u16,
        file: *mut c_void,
        known_subject: *const GUID,
    }

    /// WINTRUST_DATA with the file member of its union.
    #[repr(C)]
    struct TrustData {
        cb_struct: u32,
        policy_callback_data: *mut c_void,
        sip_client_data: *mut c_void,
        ui_choice: u32,
        revocation_checks: u32,
        union_choice: u32,
        file: *mut FileInfo,
        state_action: u32,
        state_data: *mut c_void,
        url_reference: *mut u16,
        prov_flags: u32,
        ui_context: u32,
        signature_settings: *mut c_void,
    }

    /// The first fields of CRYPT_PROVIDER_SGNR and CRYPT_PROVIDER_CERT (wintrust.h), read through pointers.
    #[repr(C)]
    struct ProviderSigner {
        cb_struct: u32,
        verify_as_of: [u32; 2],
        chain_len: u32,
    }
    #[repr(C)]
    struct ProviderCert {
        cb_struct: u32,
        cert: *const c_void,
    }

    #[link(name = "wintrust")]
    extern "system" {
        fn WinVerifyTrust(hwnd: *mut c_void, action: *mut GUID, data: *mut c_void) -> i32;
        fn WTHelperProvDataFromStateData(state: *mut c_void) -> *mut c_void;
        fn WTHelperGetProvSignerFromChain(data: *mut c_void, signer: u32, counter_signer: i32, counter_index: u32) -> *mut ProviderSigner;
        fn WTHelperGetProvCertFromChain(signer: *mut ProviderSigner, index: u32) -> *mut ProviderCert;
    }
    #[link(name = "crypt32")]
    extern "system" {
        fn CertGetNameStringW(cert: *const c_void, kind: u32, flags: u32, para: *const c_void, name: *mut u16, len: u32) -> u32;
    }

    const WTD_UI_NONE: u32 = 2;
    const WTD_REVOKE_NONE: u32 = 0;
    const WTD_CHOICE_FILE: u32 = 1;
    const WTD_STATEACTION_VERIFY: u32 = 1;
    const WTD_STATEACTION_CLOSE: u32 = 2;
    /// No network: a check never hangs on certificate downloads.
    const WTD_CACHE_ONLY_URL_RETRIEVAL: u32 = 0x1000;
    const TRUST_E_NOSIGNATURE: u32 = 0x800B_0100;
    const TRUST_E_SUBJECT_FORM_UNKNOWN: u32 = 0x800B_0003;
    const TRUST_E_PROVIDER_UNKNOWN: u32 = 0x800B_0001;
    const CERT_NAME_SIMPLE_DISPLAY_TYPE: u32 = 4;
    // WINTRUST_ACTION_GENERIC_VERIFY_V2
    let mut action = GUID::from_u128(0x00aac56b_cd44_11d0_8cc2_00c04fc295ee);

    let wide: Vec<u16> = path.as_os_str().encode_wide().chain(Some(0)).collect();
    let mut file = FileInfo { cb_struct: std::mem::size_of::<FileInfo>() as u32, path: wide.as_ptr(), file: null_mut(), known_subject: null() };
    let mut data = TrustData {
        cb_struct: std::mem::size_of::<TrustData>() as u32,
        policy_callback_data: null_mut(),
        sip_client_data: null_mut(),
        ui_choice: WTD_UI_NONE,
        revocation_checks: WTD_REVOKE_NONE,
        union_choice: WTD_CHOICE_FILE,
        file: &mut file,
        state_action: WTD_STATEACTION_VERIFY,
        state_data: null_mut(),
        url_reference: null_mut(),
        prov_flags: WTD_CACHE_ONLY_URL_RETRIEVAL,
        ui_context: 0,
        signature_settings: null_mut(),
    };
    // SAFETY: the structs match wintrust.h (x64) and live until both calls return.
    let status = unsafe { WinVerifyTrust(null_mut(), &mut action, &mut data as *mut TrustData as *mut c_void) } as u32;
    let reason = unsafe { GetLastError() }.0;
    // SAFETY: the signer's chain lives in the state data until it is closed below; leaf first, root last.
    let signer = (status == 0 && !data.state_data.is_null()).then(|| unsafe {
        let sgnr = WTHelperGetProvSignerFromChain(WTHelperProvDataFromStateData(data.state_data), 0, 0, 0);
        if sgnr.is_null() || (*sgnr).chain_len == 0 {
            return None;
        }
        let name = |index: u32| {
            let c = WTHelperGetProvCertFromChain(sgnr, index);
            if c.is_null() || (*c).cert.is_null() {
                return None;
            }
            let mut buf = [0u16; 256];
            let n = CertGetNameStringW((*c).cert, CERT_NAME_SIMPLE_DISPLAY_TYPE, 0, null(), buf.as_mut_ptr(), buf.len() as u32);
            (n > 1).then(|| String::from_utf16_lossy(&buf[..n as usize - 1]))
        };
        Some((name(0)?, name((*sgnr).chain_len - 1)?))
    });
    data.state_action = WTD_STATEACTION_CLOSE;
    unsafe { WinVerifyTrust(null_mut(), &mut action, &mut data as *mut TrustData as *mut c_void) };
    let signature = match status {
        0 => Signature::Valid,
        // Microsoft's sample: "not signed" only for these reasons, anything else is a bad signature.
        TRUST_E_NOSIGNATURE if [TRUST_E_NOSIGNATURE, TRUST_E_SUBJECT_FORM_UNKNOWN, TRUST_E_PROVIDER_UNKNOWN].contains(&reason) => {
            Signature::Unsigned
        }
        code => Signature::Invalid(code),
    };
    (signature, signer.flatten())
}

#[cfg(not(windows))]
fn verify(_: &Path) -> (Signature, Option<(String, String)>) {
    (Signature::Unsigned, None)
}

#[cfg(all(test, windows))]
mod tests {
    use super::*;

    #[test]
    fn an_unsigned_program_is_unsigned() {
        // This test binary itself is an unsigned Windows program.
        assert_eq!(check(&std::env::current_exe().unwrap()), Signature::Unsigned);
        assert!(!signed_by_microsoft(&std::env::current_exe().unwrap()));
    }

    #[test]
    fn a_held_file_can_be_read_but_not_changed_or_replaced() {
        let dir = tempfile::tempdir().unwrap();
        let f = dir.path().join("setup.exe");
        std::fs::write(&f, "MZ").unwrap();
        let held = hold(&f).unwrap();
        assert_eq!(std::fs::read(&f).unwrap(), b"MZ");
        assert!(std::fs::OpenOptions::new().write(true).open(&f).is_err());
        assert!(std::fs::rename(&f, dir.path().join("other.exe")).is_err());
        assert!(std::fs::remove_file(&f).is_err());
        drop(held);
        assert!(std::fs::remove_file(&f).is_ok());
    }

    #[test]
    fn a_changed_signed_program_is_refused() {
        let candidates = [
            r"C:\Program Files (x86)\Microsoft\Edge\Application\msedge.exe",
            r"C:\Program Files\Microsoft\Edge\Application\msedge.exe",
        ];
        let Some(signed) = candidates.iter().map(Path::new).find(|p| p.is_file()) else {
            return; // no Edge here: nothing signed to try
        };
        assert_eq!(check(signed), Signature::Valid);
        let (name, root) = signer(signed).unwrap();
        assert_eq!(name, "Microsoft Corporation");
        assert!(root.starts_with("Microsoft Root"), "{root}");
        assert!(signed_by_microsoft(signed));
        let dir = tempfile::tempdir().unwrap();
        let copy = dir.path().join("changed.exe");
        let mut bytes = std::fs::read(signed).unwrap();
        let middle = bytes.len() / 2;
        bytes[middle] ^= 0xFF;
        std::fs::write(&copy, bytes).unwrap();
        assert!(matches!(check(&copy), Signature::Invalid(_)));
        assert!(!signed_by_microsoft(&copy), "a changed file has no valid signer");
    }
}
