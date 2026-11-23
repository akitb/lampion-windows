//! Where to mark the editor: the part a server error points at, and unclosed quotes or unbalanced
//! parentheses while typing. Ranges are byte offsets.

use super::split::{block_comment_end, is_line_comment, line_end, skip_quoted};

fn is_word(c: char) -> bool {
    c.is_alphanumeric() || c == '_' || c == '$'
}

/// The word starting at `at` (up to the next whitespace).
fn word_at(text: &str, at: usize) -> (usize, usize) {
    let end = text[at..].find(char::is_whitespace).map_or(text.len(), |p| at + p);
    (at, end.max(at + text[at..].chars().next().map_or(0, char::len_utf8)))
}

/// Byte offset where line `line` (1-based) starts.
fn line_start(text: &str, line: usize) -> Option<usize> {
    if line <= 1 {
        return Some(0);
    }
    text.match_indices('\n').nth(line - 2).map(|(i, _)| i + 1)
}

fn find_ci(hay: &str, needle: &str, from: usize) -> Option<usize> {
    let (h, n) = (hay.to_lowercase(), needle.to_lowercase());
    // Lowercasing can change byte lengths for a few letters; only trust it when it doesn't.
    if h.len() != hay.len() || n.len() != needle.len() {
        return hay[from..].find(needle).map(|p| p + from);
    }
    h[from..].find(&n).map(|p| p + from)
}

/// The part of `stmt` (the failed statement, as sent) that the MariaDB `message` points at.
pub fn error_range(message: &str, stmt: &str) -> Option<(usize, usize)> {
    if stmt.trim().is_empty() {
        return None;
    }
    let line = message.rfind(" at line ").and_then(|i| message[i + 9..].split(|c: char| !c.is_ascii_digit()).next()?.parse::<usize>().ok());

    // 1) Syntax error: "... near 'FRAGMENT' at line N".
    if let Some(i) = message.find("near '") {
        let rest = &message[i + 6..];
        let fragment = rest.rfind("' at line").map_or(rest, |e| &rest[..e]);
        let head: String = fragment.lines().next().unwrap_or("").chars().take(40).collect();
        if !head.trim().is_empty() {
            let from = line.and_then(|l| line_start(stmt, l)).filter(|&s| stmt[s..].contains(head.as_str())).unwrap_or(0);
            if let Some(p) = stmt[from..].find(head.as_str()) {
                return Some(word_at(stmt, from + p));
            }
        }
        // The error is at the end of the statement: mark its last word.
        let end = stmt.trim_end_matches(|c: char| c.is_whitespace() || c == ';').len();
        let start = stmt[..end].rfind(char::is_whitespace).map_or(0, |p| p + 1);
        return (end > start).then_some((start, end));
    }

    // 2) Unknown column, table or database: the quoted name.
    if let Some(i) = message.find('\'') {
        if let Some(len) = message[i + 1..].find('\'') {
            let quoted = &message[i + 1..i + 1 + len];
            let last = quoted.rsplit('.').next().unwrap_or("");
            for name in [quoted, last] {
                if name.is_empty() {
                    continue;
                }
                let mut from = 0;
                while let Some(p) = find_ci(stmt, name, from) {
                    let before = stmt[..p].chars().next_back();
                    let after = stmt[p + name.len()..].chars().next();
                    if !before.is_some_and(is_word) && !after.is_some_and(is_word) {
                        return Some((p, p + name.len()));
                    }
                    from = p + name.len();
                }
            }
        }
    }

    // 3) Only a line number: the whole line.
    let start = line_start(stmt, line?)?;
    let end = stmt[start..].find('\n').map_or(stmt.len(), |p| start + p);
    (end > start).then_some((start, end))
}

/// Unclosed quotes (to the end of their line) and unbalanced parentheses, marked while typing.
pub fn syntax_marks(text: &str) -> Vec<(usize, usize)> {
    let b = text.as_bytes();
    let (mut out, mut open) = (Vec::new(), Vec::new());
    let mut i = 0;
    while i < b.len() {
        match b[i] {
            b'\'' | b'"' | b'`' if closes(b, i) => i = skip_quoted(b, i),
            b'\'' | b'"' | b'`' => {
                let line = line_end(b, i).max(i + 1);
                out.push((i, line));
                i = line;
            }
            b'/' if b.get(i + 1) == Some(&b'*') => i = block_comment_end(b, i),
            _ if is_line_comment(b, i) => i = line_end(b, i),
            b'(' => {
                open.push(i);
                i += 1;
            }
            b')' => {
                if open.pop().is_none() {
                    out.push((i, i + 1));
                }
                i += 1;
            }
            _ => i += 1,
        }
    }
    out.extend(open.into_iter().map(|p| (p, p + 1)));
    out.sort();
    out
}

/// True when the quote at `i` is closed before the end of the text.
fn closes(b: &[u8], i: usize) -> bool {
    let q = b[i];
    let mut j = i + 1;
    while j < b.len() {
        if b[j] == b'\\' && q != b'`' {
            j += 2;
            continue;
        }
        if b[j] == q {
            if b.get(j + 1) == Some(&q) {
                j += 2;
                continue;
            }
            return true;
        }
        j += 1;
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    fn marked<'a>(message: &str, stmt: &'a str) -> &'a str {
        let (s, e) = error_range(message, stmt).expect("a mark");
        &stmt[s..e]
    }

    #[test]
    fn syntax_error_marks_the_fragment_start() {
        let msg = "ERROR 1064 (42000): You have an error in your SQL syntax; check the manual that corresponds to your MariaDB server \
                   version for the right syntax to use near 'SELEC hiba' at line 1";
        assert_eq!(marked(msg, "SELEC hiba"), "SELEC");
        let stmt = "SELECT name, pricee\nFROM products\nWHER active = 1";
        let msg = "ERROR 1064 (42000): You have an error in your SQL syntax; ... to use near 'active = 1' at line 3";
        assert_eq!(marked(msg, stmt), "active");
    }

    #[test]
    fn syntax_error_uses_the_reported_line() {
        let stmt = "SELECT a FROM t\nWHERE a = = 1 AND a = = 1";
        let msg = "ERROR 1064 (42000): ... near '= 1 AND a = = 1' at line 2";
        let (s, _) = error_range(msg, stmt).unwrap();
        assert_eq!(s, stmt.find("= 1 AND").unwrap());
    }

    #[test]
    fn error_at_the_end_marks_the_last_word() {
        assert_eq!(marked("ERROR 1064 (42000): ... near '' at line 1", "SELECT * FROM ;"), "FROM");
    }

    #[test]
    fn unknown_names_are_found_as_whole_words() {
        let stmt = "SELECT nev,\n  nincs_ilyen\nFROM termekek";
        assert_eq!(marked("ERROR 1054 (42S22): Unknown column 'nincs_ilyen' in 'field list'", stmt), "nincs_ilyen");
        assert_eq!(marked("ERROR 1146 (42S02): Table 'shop.orderz' doesn't exist", "SELECT x_orderz FROM orderz"), "orderz");
        assert_eq!(marked("ERROR 1054 (42S22): Unknown column 'C.Name' in 'field list'", "SELECT c.name FROM t c"), "c.name");
    }

    #[test]
    fn line_number_alone_marks_the_line() {
        assert_eq!(marked("ERROR 9999 (HY000): Something at line 2", "SELECT 1\nBROKEN THING\nx"), "BROKEN THING");
        assert_eq!(error_range("ERROR 2013: Lost connection", "SELECT 1"), None);
    }

    #[test]
    fn unclosed_quotes_and_unbalanced_parentheses() {
        assert_eq!(syntax_marks("SELECT 'open\nFROM t"), [(7, 12)]);
        assert_eq!(syntax_marks("SELECT (1"), [(7, 8)]);
        assert_eq!(syntax_marks("SELECT 1)"), [(8, 9)]);
        assert!(syntax_marks("SELECT ')' , \"(\" , `a``b`, (1) -- (\n").is_empty());
        assert!(syntax_marks("SELECT 'it''s', 'a\\'b'").is_empty());
    }
}
