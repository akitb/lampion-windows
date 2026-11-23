//! Downloading a package with progress (cancellable), checking its SHA-256 while it streams in, and
//! unpacking a zip into a folder with its top folder stripped (`Apache24\bin\httpd.exe` → `bin\httpd.exe`).

use futures_util::StreamExt;
use sha2::{Digest, Sha256};
use std::io::{Read, Write};
use std::path::{Component, Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

pub const CANCELLED: &str = "Setup was cancelled.";

pub fn client() -> Result<reqwest::Client, String> {
    reqwest::Client::builder()
        .user_agent(concat!("Lampion/", env!("CARGO_PKG_VERSION")))
        .connect_timeout(Duration::from_secs(20))
        .read_timeout(Duration::from_secs(60))
        .build()
        .map_err(|e| e.to_string())
}

/// The file's SHA-256 in lowercase hex; `None` when it cannot be read.
pub fn sha256_file(path: &Path) -> Option<String> {
    let mut f = std::fs::File::open(path).ok()?;
    let mut hasher = Sha256::new();
    let mut buf = vec![0u8; 1 << 16];
    loop {
        let n = f.read(&mut buf).ok()?;
        if n == 0 {
            break;
        }
        hasher.update(&buf[..n]);
    }
    Some(hex::encode(hasher.finalize()))
}

pub fn sha256_matches(actual: &str, expected: &str) -> bool {
    actual.trim().eq_ignore_ascii_case(expected.trim())
}

/// Downloads the first URL that works into `dest` (through `dest.part`). With `sha256`, a file that does
/// not match is deleted and refused. `progress(bytes, total)` follows the download.
pub async fn fetch(
    urls: &[&str],
    dest: &Path,
    sha256: Option<&str>,
    cancel: &AtomicBool,
    progress: &mut dyn FnMut(u64, Option<u64>),
) -> Result<(), String> {
    let mut last = String::from("No download address.");
    for url in urls {
        match fetch_one(url, dest, sha256, cancel, progress).await {
            Ok(()) => return Ok(()),
            Err(e) if e == CANCELLED => return Err(e),
            Err(e) => {
                crate::core::activity::warn("Setup", &e);
                last = e;
            }
        }
    }
    Err(last)
}

async fn fetch_one(
    url: &str,
    dest: &Path,
    sha256: Option<&str>,
    cancel: &AtomicBool,
    progress: &mut dyn FnMut(u64, Option<u64>),
) -> Result<(), String> {
    let name = dest.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
    let resp = client()?.get(url).send().await.map_err(|e| format!("Could not download {url}: {e}"))?;
    if !resp.status().is_success() {
        return Err(format!("{url} answered {}.", resp.status()));
    }
    // A removed file is often answered with a web page and status 200.
    let html = resp
        .headers()
        .get(reqwest::header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .is_some_and(|t| t.trim_start().to_ascii_lowercase().starts_with("text/html"));
    if html {
        return Err(format!("{url} is no longer available (the server sent a web page instead of the file)."));
    }
    let total = resp.content_length();
    if let Some(dir) = dest.parent() {
        std::fs::create_dir_all(dir).map_err(|e| format!("Could not create {}: {e}", dir.display()))?;
    }
    let part = PathBuf::from(format!("{}.part", dest.display()));
    let mut file = std::fs::File::create(&part).map_err(|e| format!("Could not write {}: {e}", part.display()))?;
    let mut hasher = Sha256::new();
    let mut done = 0u64;
    let mut stream = resp.bytes_stream();
    let result: Result<(), String> = async {
        while let Some(chunk) = stream.next().await {
            if cancel.load(Ordering::SeqCst) {
                return Err(CANCELLED.to_string());
            }
            let chunk = chunk.map_err(|e| format!("The download of {name} broke off: {e}"))?;
            file.write_all(&chunk).map_err(|e| format!("Could not write {}: {e}", part.display()))?;
            hasher.update(&chunk);
            done += chunk.len() as u64;
            progress(done, total);
        }
        file.flush().map_err(|e| e.to_string())
    }
    .await;
    drop(file);
    let checked = result.and_then(|()| match sha256 {
        Some(expected) if !sha256_matches(&hex::encode(hasher.finalize()), expected) => {
            Err(format!("{name} from {url} did not match its SHA-256 checksum, so Lampion did not use it."))
        }
        _ => Ok(()),
    });
    if let Err(e) = checked {
        let _ = std::fs::remove_file(&part);
        return Err(e);
    }
    let _ = std::fs::remove_file(dest);
    std::fs::rename(&part, dest).map_err(|e| format!("Could not save {}: {e}", dest.display()))
}

/// The entry path below `strip` (`""` keeps every path); `None` for entries outside it. Any `..` is
/// refused: `Apache24/../x` passes zip's own check but would escape once `Apache24` is stripped.
fn below(rel: &Path, strip: &str) -> Option<PathBuf> {
    if !rel.components().all(|c| matches!(c, Component::Normal(_) | Component::CurDir)) {
        return None;
    }
    if strip.is_empty() {
        return Some(rel.to_path_buf());
    }
    let mut parts = rel.components();
    match parts.next() {
        Some(Component::Normal(first)) if first.to_string_lossy().eq_ignore_ascii_case(strip) => Some(parts.as_path().to_path_buf()),
        _ => None,
    }
}

/// Unpacks `archive` into `dest`, keeping only what is inside the `strip` folder. Entries that would
/// land outside `dest` (`..\`, absolute paths) are skipped. Returns the number of files written.
pub fn extract_zip(
    archive: &Path,
    dest: &Path,
    strip: &str,
    cancel: &AtomicBool,
    progress: &mut dyn FnMut(usize, usize),
) -> Result<usize, String> {
    let file = std::fs::File::open(archive).map_err(|e| format!("Could not open {}: {e}", archive.display()))?;
    let mut zip = zip::ZipArchive::new(std::io::BufReader::new(file))
        .map_err(|e| format!("{} is not a valid zip file: {e}", archive.display()))?;
    let total = zip.len();
    let mut written = 0;
    for i in 0..total {
        if cancel.load(Ordering::SeqCst) {
            return Err(CANCELLED.to_string());
        }
        progress(i + 1, total);
        let mut entry = zip.by_index(i).map_err(|e| format!("{} is damaged: {e}", archive.display()))?;
        let Some(rel) = entry.enclosed_name().and_then(|p| below(&p, strip)) else { continue };
        if rel.as_os_str().is_empty() {
            continue;
        }
        let out = dest.join(&rel);
        let fail = |e: std::io::Error| format!("Could not unpack {}: {e}", out.display());
        if entry.is_dir() {
            std::fs::create_dir_all(&out).map_err(fail)?;
        } else {
            if let Some(dir) = out.parent() {
                std::fs::create_dir_all(dir).map_err(fail)?;
            }
            let mut f = std::fs::File::create(&out).map_err(fail)?;
            std::io::copy(&mut entry, &mut f).map_err(fail)?;
            written += 1;
        }
    }
    Ok(written)
}

#[cfg(test)]
mod tests {
    use super::*;
    use zip::write::SimpleFileOptions;

    fn sha256_hex(data: &[u8]) -> String {
        hex::encode(Sha256::digest(data))
    }

    #[test]
    fn sha256_known_values() {
        assert_eq!(sha256_hex(b"abc"), "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad");
        let dir = tempfile::tempdir().unwrap();
        let f = dir.path().join("a.bin");
        std::fs::write(&f, b"abc").unwrap();
        let digest = sha256_file(&f).unwrap();
        assert!(sha256_matches(&digest, "BA7816BF8F01CFEA414140DE5DAE2223B00361A396177A9CB410FF61F20015AD "));
        assert!(!sha256_matches(&digest, &sha256_hex(b"abd")));
        assert_eq!(sha256_file(&dir.path().join("missing")), None);
    }

    fn make_zip(path: &Path, entries: &[(&str, &str)]) {
        let mut w = zip::ZipWriter::new(std::fs::File::create(path).unwrap());
        for (name, body) in entries {
            if name.ends_with('/') {
                w.add_directory(*name, SimpleFileOptions::default()).unwrap();
            } else {
                w.start_file(*name, SimpleFileOptions::default()).unwrap();
                w.write_all(body.as_bytes()).unwrap();
            }
        }
        w.finish().unwrap();
    }

    #[test]
    fn unpacks_with_the_top_folder_stripped() {
        let dir = tempfile::tempdir().unwrap();
        let archive = dir.path().join("httpd.zip");
        make_zip(
            &archive,
            &[
                ("ReadMe.txt", "outside the folder"),
                ("Apache24/", ""),
                ("Apache24/bin/httpd.exe", "MZ"),
                ("apache24/conf/httpd.conf", "# conf"),
                ("Apache24/../escape.txt", "no"),
                ("Other/x.txt", "no"),
            ],
        );
        let dest = dir.path().join("out").join("apache");
        let mut seen = Vec::new();
        let n = extract_zip(&archive, &dest, "Apache24", &AtomicBool::new(false), &mut |i, t| seen.push((i, t))).unwrap();
        assert_eq!(n, 2);
        assert_eq!(std::fs::read_to_string(dest.join(r"bin\httpd.exe")).unwrap(), "MZ");
        assert!(dest.join(r"conf\httpd.conf").is_file(), "the folder name matches regardless of case");
        assert!(!dest.join("ReadMe.txt").exists());
        assert!(!dir.path().join("out").join("escape.txt").exists());
        assert!(!dest.join("x.txt").exists() && !dest.join("Other").exists());
        assert_eq!(seen.last(), Some(&(6, 6)), "progress reaches the end even when the last entries are skipped");
    }

    #[test]
    fn unpacks_everything_without_strip_and_stops_when_cancelled() {
        let dir = tempfile::tempdir().unwrap();
        let archive = dir.path().join("php.zip");
        make_zip(&archive, &[("php.exe", "MZ"), ("ext/php_curl.dll", "MZ"), ("../../evil.txt", "no")]);
        let dest = dir.path().join("php");
        assert_eq!(extract_zip(&archive, &dest, "", &AtomicBool::new(false), &mut |_, _| {}).unwrap(), 2);
        assert!(dest.join(r"ext\php_curl.dll").is_file());
        assert!(!dir.path().join("evil.txt").exists());
        let again = dir.path().join("again");
        let err = extract_zip(&archive, &again, "", &AtomicBool::new(true), &mut |_, _| {}).unwrap_err();
        assert_eq!(err, CANCELLED);
        assert!(extract_zip(&dir.path().join("none.zip"), &again, "", &AtomicBool::new(false), &mut |_, _| {}).is_err());
    }

    #[test]
    fn strip_matching() {
        assert_eq!(below(Path::new("a/b/c"), "A"), Some(PathBuf::from("b/c")));
        assert_eq!(below(Path::new("x/b"), "a"), None);
        assert_eq!(below(Path::new("a"), "a"), Some(PathBuf::new()));
        assert_eq!(below(Path::new("a/b"), ""), Some(PathBuf::from("a/b")));
        assert_eq!(below(Path::new("a/../b"), "a"), None);
        assert_eq!(below(Path::new("a/../b"), ""), None);
    }
}
