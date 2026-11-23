//! Splits the Query tab's text into statements the way the mariadb client does: `;` (or the terminator
//! a `DELIMITER` line sets) ends a statement outside quotes and comments. `DELIMITER` lines are client
//! commands and never reach the server. Offsets are byte offsets into the text.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Span {
    pub start: usize,
    pub end: usize,
}

/// The statements in `text`, trimmed, without their terminators. Statements that hold only
/// whitespace and comments are left out (the server would answer "Query was empty").
pub fn split(text: &str) -> Vec<Span> {
    let b = text.as_bytes();
    let n = b.len();
    let mut out = Vec::new();
    let mut delimiter: Vec<u8> = b";".to_vec();
    let (mut i, mut start, mut content) = (0, 0, false);
    while i < n {
        if !content && at_line_start(b, i) && is_delimiter_command(b, i) {
            let end = line_end(b, i);
            if let Some(word) = text[i + 9..end].split_whitespace().next() {
                delimiter = word.as_bytes().to_vec();
            }
            i = end;
            start = i;
            continue;
        }
        let c = b[i];
        if c == b'\'' || c == b'"' || c == b'`' {
            i = skip_quoted(b, i);
            content = true;
        } else if is_line_comment(b, i) {
            i = line_end(b, i);
        } else if c == b'/' && b.get(i + 1) == Some(&b'*') {
            // `/*! … */` is executed by the server (dump files use it), so it counts as content.
            content |= b.get(i + 2) == Some(&b'!');
            i = block_comment_end(b, i);
        } else if b[i..].starts_with(&delimiter) {
            if content {
                out.push(trimmed(b, start, i));
            }
            i += delimiter.len();
            start = i;
            content = false;
        } else {
            content |= !c.is_ascii_whitespace();
            i += 1;
        }
    }
    if content {
        out.push(trimmed(b, start, n));
    }
    out
}

/// Byte offset after a quoted string or name starting at `i` (the end of the text when unclosed).
pub fn skip_quoted(b: &[u8], i: usize) -> usize {
    let q = b[i];
    let mut j = i + 1;
    while j < b.len() {
        let c = b[j];
        if c == b'\\' && q != b'`' {
            j += 2;
            continue;
        }
        if c == q {
            if b.get(j + 1) == Some(&q) {
                j += 2;
                continue;
            }
            return j + 1;
        }
        j += 1;
    }
    b.len()
}

/// `-- ` (dash dash and a space or control character) or `#` starts a comment to the end of the line.
pub fn is_line_comment(b: &[u8], i: usize) -> bool {
    b[i] == b'#' || (b[i] == b'-' && b.get(i + 1) == Some(&b'-') && b.get(i + 2).map_or(true, |&c| c <= b' '))
}

pub fn line_end(b: &[u8], i: usize) -> usize {
    b[i..].iter().position(|&c| c == b'\n').map_or(b.len(), |p| i + p)
}

pub fn block_comment_end(b: &[u8], i: usize) -> usize {
    b[i + 2..].windows(2).position(|w| w == b"*/").map_or(b.len(), |p| i + 2 + p + 2)
}

fn at_line_start(b: &[u8], i: usize) -> bool {
    b[..i].iter().rev().take_while(|&&c| c != b'\n').all(|&c| c == b' ' || c == b'\t' || c == b'\r')
}

fn is_delimiter_command(b: &[u8], i: usize) -> bool {
    b.len() >= i + 9 && b[i..i + 9].eq_ignore_ascii_case(b"delimiter") && b.get(i + 9).map_or(true, |&c| c == b' ' || c == b'\t')
}

fn trimmed(b: &[u8], mut start: usize, mut end: usize) -> Span {
    while start < end && b[start].is_ascii_whitespace() {
        start += 1;
    }
    while end > start && b[end - 1].is_ascii_whitespace() {
        end -= 1;
    }
    Span { start, end }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parts(text: &str) -> Vec<&str> {
        split(text).iter().map(|s| &text[s.start..s.end]).collect()
    }

    #[test]
    fn splits_on_semicolons() {
        assert_eq!(parts("SELECT 1; SELECT 2"), ["SELECT 1", "SELECT 2"]);
        assert_eq!(parts("  SELECT 1 ;\n\n"), ["SELECT 1"]);
        assert_eq!(parts(";;  ;"), Vec::<&str>::new());
    }

    #[test]
    fn ignores_semicolons_in_quotes() {
        assert_eq!(parts("SELECT ';' ; SELECT \"a;b\"; SELECT `x;y`"), ["SELECT ';'", "SELECT \"a;b\"", "SELECT `x;y`"]);
        assert_eq!(parts(r"SELECT 'it\'s;'; SELECT 2"), [r"SELECT 'it\'s;'", "SELECT 2"]);
        assert_eq!(parts("SELECT 'a''b;c'; SELECT 2"), ["SELECT 'a''b;c'", "SELECT 2"]);
        assert_eq!(parts("SELECT `a``;b`; SELECT 2"), ["SELECT `a``;b`", "SELECT 2"]);
    }

    #[test]
    fn ignores_semicolons_in_comments() {
        let text = "-- first; note\nSELECT 1; # x;y\nSELECT 2; /* a;b */ SELECT 3";
        assert_eq!(parts(text), ["-- first; note\nSELECT 1", "# x;y\nSELECT 2", "/* a;b */ SELECT 3"]);
        // `--` without a space is two minus signs, not a comment.
        assert_eq!(parts("SELECT 1--1; SELECT 2"), ["SELECT 1--1", "SELECT 2"]);
    }

    #[test]
    fn skips_comment_only_statements() {
        assert_eq!(parts("SELECT 1;\n-- only a comment\n"), ["SELECT 1"]);
        assert_eq!(parts("/* nothing */ ;"), Vec::<&str>::new());
        // Executable comments are real statements (mariadb-dump writes them).
        assert_eq!(parts("/*!40101 SET NAMES utf8 */;"), ["/*!40101 SET NAMES utf8 */"]);
    }

    #[test]
    fn follows_delimiter_lines() {
        let text = "DELIMITER $$\nCREATE PROCEDURE p() BEGIN SELECT 1; SELECT 2; END$$\nDELIMITER ;\nSELECT 3;";
        assert_eq!(parts(text), ["CREATE PROCEDURE p() BEGIN SELECT 1; SELECT 2; END", "SELECT 3"]);
        assert_eq!(parts("delimiter //\nSELECT 1//SELECT 2//"), ["SELECT 1", "SELECT 2"]);
        // Only a command at the start of a statement: here it is part of the text.
        assert_eq!(parts("SELECT 'DELIMITER x'; SELECT 2"), ["SELECT 'DELIMITER x'", "SELECT 2"]);
    }

    #[test]
    fn unclosed_quote_runs_to_the_end() {
        assert_eq!(parts("SELECT 'open; SELECT 2"), ["SELECT 'open; SELECT 2"]);
    }

    #[test]
    fn keeps_char_boundaries() {
        assert_eq!(parts("SELECT 'árvíztűrő;'; SELECT 'ő'"), ["SELECT 'árvíztűrő;'", "SELECT 'ő'"]);
    }
}
