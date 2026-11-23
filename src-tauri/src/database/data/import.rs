//! What an SQL file does with databases before Lampion runs it in a chosen one: a whole-database export
//! carries `CREATE DATABASE shop` and `USE shop`, after which its statements run in `shop` instead.

use std::io::BufRead;

/// The databases that `USE x` and `CREATE DATABASE|SCHEMA [IF NOT EXISTS] x` lines name, in order, each once.
pub fn named_databases(r: impl BufRead) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for line in r.split(b'\n').map_while(Result::ok) {
        if let Some(name) = database_of(&line) {
            if !out.iter().any(|o| same(o, &name)) {
                out.push(name);
            }
        }
    }
    out
}

/// The named databases other than `target` (names compare like Windows' MariaDB does: ignoring case).
pub fn others(named: Vec<String>, target: &str) -> Vec<String> {
    named.into_iter().filter(|n| !same(n, target)).collect()
}

fn same(a: &str, b: &str) -> bool {
    a.to_lowercase() == b.to_lowercase()
}

struct Cursor<'a>(&'a [u8]);

impl Cursor<'_> {
    /// Skips whitespace and `/* … */` comments (mariadb-dump writes `CREATE DATABASE /*!32312 IF NOT EXISTS*/`).
    fn space(&mut self) {
        loop {
            let s = self.0;
            if s.first().is_some_and(|c| c.is_ascii_whitespace()) {
                self.0 = &s[1..];
            } else if s.starts_with(b"/*") {
                self.0 = s[2..].windows(2).position(|w| w == b"*/").map_or(&[][..], |p| &s[p + 4..]);
            } else {
                return;
            }
        }
    }

    /// The keyword `w` (any case) as a whole word.
    fn word(&mut self, w: &str) -> bool {
        self.space();
        let s = self.0;
        let ok = s.len() >= w.len() && s[..w.len()].eq_ignore_ascii_case(w.as_bytes()) && !s.get(w.len()).is_some_and(|&c| is_name_byte(c));
        if ok {
            self.0 = &s[w.len()..];
        }
        ok
    }

    /// A name: in backticks (a doubled backtick is one) or bare.
    fn name(&mut self) -> Option<String> {
        self.space();
        let s = self.0;
        let bytes = if let Some(rest) = s.strip_prefix(b"`") {
            let mut name = Vec::new();
            let mut i = 0;
            while i < rest.len() {
                match (rest[i], rest.get(i + 1)) {
                    (b'`', Some(b'`')) => {
                        name.push(b'`');
                        i += 2;
                    }
                    (b'`', _) => break,
                    (c, _) => {
                        name.push(c);
                        i += 1;
                    }
                }
            }
            name
        } else {
            s.iter().copied().take_while(|&c| is_name_byte(c)).collect()
        };
        (!bytes.is_empty()).then(|| String::from_utf8_lossy(&bytes).into_owned())
    }
}

/// Letters, digits, `_`, `$` and any non-ASCII byte (part of a UTF-8 letter).
fn is_name_byte(c: u8) -> bool {
    c.is_ascii_alphanumeric() || c == b'_' || c == b'$' || c >= 0x80
}

/// The database a `USE x` or `CREATE DATABASE|SCHEMA [IF NOT EXISTS] x` line names.
fn database_of(line: &[u8]) -> Option<String> {
    let mut s = Cursor(line);
    if s.word("use") {
        return s.name();
    }
    if !(s.word("create") && (s.word("database") || s.word("schema"))) {
        return None;
    }
    if s.word("if") && !(s.word("not") && s.word("exists")) {
        return None;
    }
    s.name()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn named(text: &str) -> Vec<String> {
        named_databases(text.as_bytes())
    }

    #[test]
    fn finds_what_a_whole_database_export_switches_to() {
        let dump = "-- MariaDB dump 10.19\n/*!40101 SET NAMES utf8mb4 */;\n\n\
                    CREATE DATABASE /*!32312 IF NOT EXISTS*/ `shop` /*!40100 DEFAULT CHARACTER SET utf8mb4 */;\r\n\
                    \r\nUSE `shop`;\r\nDROP TABLE IF EXISTS `orders`;\nINSERT INTO `orders` VALUES (1,'use x');\n";
        assert_eq!(named(dump), ["shop"]);
        assert_eq!(others(named(dump), "SHOP"), Vec::<String>::new());
        assert_eq!(others(named(dump), "test"), ["shop"]);
    }

    #[test]
    fn reads_every_way_to_name_a_database() {
        assert_eq!(named("use blog;\n  USE `my``db`;\ncreate schema if not exists új_db;\nCREATE DATABASE x2\n"), ["blog", "my`db", "új_db", "x2"]);
        // Not statements about databases.
        assert!(named("CREATE TABLE users (id INT);\nUSER x;\nuseful;\nSELECT 1; USE other;\nCREATE DATABASE;\n").is_empty());
        // Each once, whatever the case.
        assert_eq!(named("USE Shop;\nUSE shop;\nUSE `SHOP`;\n"), ["Shop"]);
    }
}
