//! Following a log file like `tail -f` (SPEC "Élő követés", LogTailer in LogSource.swift): the last
//! 1 MB on open, then only the new bytes. It starts over when the file got shorter (emptied), was
//! replaced (another file ID: log rotation) or grew by more than 1 MB. An unfinished last line, and with
//! it any half-written UTF-8 character, waits as bytes for the next read; CRLF and LF both end a line.

use std::fs::File;
use std::io::{Read as _, Seek, SeekFrom};
use std::path::Path;

/// Read on open and after a jump.
pub const INITIAL_BYTES: u64 = 1 << 20;
/// Longer lines are cut, with "…".
pub const MAX_CHARS: usize = 2000;
/// An unfinished line keeps at most this many bytes (more than `MAX_CHARS` characters ever need).
const MAX_PARTIAL: usize = 16 << 10;

#[derive(Default)]
pub struct Tail {
    offset: u64,
    /// Volume serial number + file index: a new value means another file under the same name.
    identity: Option<(u32, u64)>,
    partial: Vec<u8>,
    /// Started inside a line: its rest is dropped, up to the next line break.
    skip: bool,
    open: bool,
}

/// What one look at the file found.
#[derive(Debug, PartialEq)]
pub struct Read {
    /// Started over: these lines replace everything shown so far.
    pub reset: bool,
    /// Complete new lines, trimmed, without empty ones.
    pub lines: Vec<String>,
    pub size: u64,
}

impl Tail {
    /// Reads what is new; `None` when the file does not exist (or cannot be opened).
    pub fn poll(&mut self, path: &Path) -> Option<Read> {
        match self.read(path) {
            Ok(r) => Some(r),
            Err(_) => {
                *self = Tail::default();
                None
            }
        }
    }

    fn read(&mut self, path: &Path) -> std::io::Result<Read> {
        // std opens with shared read, write and delete access, so the server can keep writing, rotating
        // or deleting its log; the handle is closed again right after this read.
        let mut f = File::open(path)?;
        let size = f.metadata()?.len();
        let identity = file_identity(&f);
        let reset = !self.open
            || size < self.offset
            || (self.identity.is_some() && identity != self.identity)
            || size - self.offset > INITIAL_BYTES;
        self.open = true;
        self.identity = identity;
        let mut start = self.offset;
        if reset {
            self.offset = size.saturating_sub(INITIAL_BYTES);
            self.partial.clear();
            self.skip = self.offset > 0;
            // One byte early: when it is a line break, the first line is whole and is kept.
            start = self.offset.saturating_sub(1);
        }
        if size <= start {
            return Ok(Read { reset, lines: Vec::new(), size });
        }
        f.seek(SeekFrom::Start(start))?;
        let mut data = Vec::with_capacity((size - start) as usize);
        f.take(size - start).read_to_end(&mut data)?;
        self.offset = start + data.len() as u64;

        let mut chunk = &data[..];
        if self.skip {
            match chunk.iter().position(|&b| b == b'\n') {
                Some(i) => {
                    chunk = &chunk[i + 1..];
                    self.skip = false;
                }
                None => chunk = &[],
            }
        }
        self.partial.extend_from_slice(chunk);
        let lines = match self.partial.iter().rposition(|&b| b == b'\n') {
            Some(last) => {
                let rest = self.partial.split_off(last + 1);
                split(&std::mem::replace(&mut self.partial, rest))
            }
            None => Vec::new(),
        };
        // shortcut: a line still unfinished after 16 KB keeps only its start (all that is ever shown).
        self.partial.truncate(MAX_PARTIAL);
        Ok(Read { reset, lines, size })
    }
}

/// Complete lines → trimmed text, without empty lines, cut after `MAX_CHARS` characters.
fn split(bytes: &[u8]) -> Vec<String> {
    String::from_utf8_lossy(bytes).split('\n').map(str::trim).filter(|l| !l.is_empty()).map(clip).collect()
}

fn clip(line: &str) -> String {
    match line.char_indices().nth(MAX_CHARS) {
        Some((i, _)) => format!("{}…", &line[..i]),
        None => line.to_string(),
    }
}

#[cfg(windows)]
fn file_identity(f: &File) -> Option<(u32, u64)> {
    use std::os::windows::io::AsRawHandle;
    use windows::Win32::Foundation::HANDLE;
    use windows::Win32::Storage::FileSystem::{GetFileInformationByHandle, BY_HANDLE_FILE_INFORMATION};
    let mut info = BY_HANDLE_FILE_INFORMATION::default();
    unsafe { GetFileInformationByHandle(HANDLE(f.as_raw_handle()), &mut info) }.ok()?;
    Some((info.dwVolumeSerialNumber, (u64::from(info.nFileIndexHigh) << 32) | u64::from(info.nFileIndexLow)))
}

#[cfg(not(windows))]
fn file_identity(f: &File) -> Option<(u32, u64)> {
    use std::os::unix::fs::MetadataExt;
    f.metadata().ok().map(|m| (m.dev() as u32, m.ino()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    fn append(path: &Path, bytes: &[u8]) {
        std::fs::OpenOptions::new().create(true).append(true).open(path).unwrap().write_all(bytes).unwrap();
    }

    fn lines(r: &Read) -> Vec<&str> {
        r.lines.iter().map(String::as_str).collect()
    }

    #[test]
    fn reads_the_file_then_only_new_lines() {
        let dir = tempfile::tempdir().unwrap();
        let f = dir.path().join("access.log");
        std::fs::write(&f, "one\r\ntwo\n\n   \nthree").unwrap();
        let mut t = Tail::default();
        let r = t.poll(&f).unwrap();
        assert!(r.reset);
        assert_eq!(lines(&r), ["one", "two"], "CRLF and LF end lines; the unfinished last line waits");
        assert_eq!(r.size, 19);

        let r = t.poll(&f).unwrap();
        assert_eq!((r.reset, r.lines.len(), r.size), (false, 0, 19), "nothing new");

        append(&f, b" and more\r\nfour\n");
        let r = t.poll(&f).unwrap();
        assert!(!r.reset);
        assert_eq!(lines(&r), ["three and more", "four"]);
    }

    #[test]
    fn keeps_a_split_utf8_character_as_bytes() {
        let dir = tempfile::tempdir().unwrap();
        let f = dir.path().join("php_errors.log");
        let text = "ok\nárvíztűrő\n".as_bytes();
        let cut = 4; // "ok\n" and the first byte of the two-byte "á"
        append(&f, &text[..cut]);
        let mut t = Tail::default();
        assert_eq!(lines(&t.poll(&f).unwrap()), ["ok"]);
        append(&f, &text[cut..cut + 4]); // "á" completed, "rv", the first byte of "í"
        assert!(t.poll(&f).unwrap().lines.is_empty());
        append(&f, &text[cut + 4..]);
        assert_eq!(lines(&t.poll(&f).unwrap()), ["árvíztűrő"], "no replacement characters");
    }

    #[test]
    fn starts_over_when_the_file_is_emptied() {
        let dir = tempfile::tempdir().unwrap();
        let f = dir.path().join("error.log");
        std::fs::write(&f, "old line 1\nold line 2\n").unwrap();
        let mut t = Tail::default();
        t.poll(&f).unwrap();
        std::fs::OpenOptions::new().write(true).open(&f).unwrap().set_len(0).unwrap();
        let r = t.poll(&f).unwrap();
        assert_eq!((r.reset, r.lines.len(), r.size), (true, 0, 0));
        append(&f, b"new\n");
        let r = t.poll(&f).unwrap();
        assert_eq!((r.reset, lines(&r)), (false, vec!["new"]), "new lines come again after emptying");
    }

    #[test]
    fn starts_over_when_the_file_is_replaced() {
        let dir = tempfile::tempdir().unwrap();
        let f = dir.path().join("mariadb.err");
        std::fs::write(&f, "first file\n").unwrap();
        let mut t = Tail::default();
        t.poll(&f).unwrap();
        // Rotation: another file takes the name; it is even longer, so only its ID tells.
        let other = dir.path().join("next.err");
        std::fs::write(&other, "second file, line 1\nsecond file, line 2\n").unwrap();
        std::fs::rename(&other, &f).unwrap();
        let r = t.poll(&f).unwrap();
        assert!(r.reset);
        assert_eq!(lines(&r), ["second file, line 1", "second file, line 2"]);
    }

    #[test]
    fn jumps_and_big_files_read_only_the_last_megabyte_from_a_line_start() {
        let dir = tempfile::tempdir().unwrap();
        let f = dir.path().join("general.log");
        std::fs::write(&f, "first\n").unwrap();
        let mut t = Tail::default();
        t.poll(&f).unwrap();
        let mut big = String::new();
        for i in 0..200_000 {
            big.push_str(&format!("line {i}\n"));
        }
        append(&f, big.as_bytes());
        let r = t.poll(&f).unwrap();
        assert!(r.reset, "more than 1 MB at once");
        assert!(r.lines.iter().all(|l| l.starts_with("line ") && l[5..].parse::<u32>().is_ok()), "no cut-off first line");
        assert_eq!(r.lines.last().unwrap(), "line 199999");
        let first: u32 = r.lines[0][5..].parse().unwrap();
        assert_eq!(r.lines.len() as u32, 200_000 - first, "every line after the first one is there");
        assert!((r.lines.iter().map(|l| l.len() + 1).sum::<usize>() as u64) <= INITIAL_BYTES);

        // Opening a big file whose last megabyte starts exactly at a line: that line is kept.
        let g = dir.path().join("exact.log");
        let line = format!("{}\n", "x".repeat(1023)); // 1024 bytes per line
        std::fs::write(&g, line.repeat(1500)).unwrap();
        let r = Tail::default().poll(&g).unwrap();
        assert_eq!(r.lines.len(), 1024, "1 MB = 1024 whole lines");
    }

    #[test]
    fn missing_files_and_files_that_appear_later() {
        let dir = tempfile::tempdir().unwrap();
        let f = dir.path().join("slow.log");
        let mut t = Tail::default();
        assert_eq!(t.poll(&f), None);
        std::fs::write(&f, "# Time: 261123 19:27:27\n").unwrap();
        let r = t.poll(&f).unwrap();
        assert!(r.reset);
        assert_eq!(lines(&r), ["# Time: 261123 19:27:27"]);
        std::fs::remove_file(&f).unwrap();
        assert_eq!(t.poll(&f), None);
    }

    #[test]
    fn long_lines_are_cut() {
        let dir = tempfile::tempdir().unwrap();
        let f = dir.path().join("long.log");
        let long = "é".repeat(2500);
        std::fs::write(&f, format!("{long}\nshort\n")).unwrap();
        let r = Tail::default().poll(&f).unwrap();
        assert_eq!(r.lines[0].chars().count(), MAX_CHARS + 1);
        assert!(r.lines[0].ends_with('…'));
        assert_eq!(r.lines[1], "short");

        // A line written in pieces far beyond the limit still shows its start.
        let g = dir.path().join("growing.log");
        let mut t = Tail::default();
        std::fs::write(&g, "").unwrap();
        t.poll(&g).unwrap();
        for _ in 0..40 {
            append(&g, "abcdefghij".repeat(100).as_bytes());
            assert!(t.poll(&g).unwrap().lines.is_empty());
        }
        append(&g, b"\n");
        let r = t.poll(&g).unwrap();
        assert_eq!(r.lines.len(), 1);
        assert!(r.lines[0].starts_with("abcdefghijabcdefghij") && r.lines[0].ends_with('…'));
    }
}
