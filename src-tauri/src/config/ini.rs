//! `key = value` files: php.ini and MariaDB's my.ini. Only the lines of the keys being set change;
//! a changed value is replaced in place (indentation, spacing and an inline comment stay).

use super::file::{indent, Doc};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Dialect {
    /// php.ini: `;` comments, keys are case-sensitive, every section is global
    /// except `[PATH=…]` / `[HOST=…]`.
    Php,
    /// my.ini: `#` or `;` comments, `-` and `_` are the same in a key, a key may stand alone (`= 1`),
    /// only the server's sections count; new keys go at the end of `[mysqld]`.
    MyIni,
}

/// The option groups mariadbd reads.
const SERVER_GROUPS: &[&str] = &["mysqld", "server", "mariadb", "mariadbd", "client-server"];

/// The comment above the keys Lampion adds to my.ini: its own block of settings.
pub const MY_INI_BLOCK: &str = "# Lampion › MariaDB settings (an empty field there means MariaDB's default)";

#[derive(Debug)]
struct Entry {
    line: usize,
    /// Normalised key (my.ini: lowercase, `_`).
    key: String,
    /// The key as written, for rewriting a whole line.
    raw_key: String,
    commented: bool,
    value: String,
    /// Byte range of the value in the line (active lines with a non-empty value).
    span: Option<(usize, usize)>,
}

pub struct Ini {
    doc: Doc,
    dialect: Dialect,
}

impl Ini {
    pub fn new(text: &str, dialect: Dialect) -> Ini {
        Ini { doc: Doc::new(text), dialect }
    }

    pub fn text(&self) -> String {
        self.doc.text()
    }

    fn norm(&self, key: &str) -> String {
        match self.dialect {
            Dialect::Php => key.to_string(),
            Dialect::MyIni => key.to_ascii_lowercase().replace('-', "_"),
        }
    }

    fn comment_char(&self) -> char {
        match self.dialect {
            Dialect::Php => ';',
            Dialect::MyIni => '#',
        }
    }

    fn section_counts(&self, section: &str) -> bool {
        match self.dialect {
            Dialect::Php => !section.starts_with("path=") && !section.starts_with("host="),
            Dialect::MyIni => SERVER_GROUPS.contains(&section),
        }
    }

    /// Every key line (active or commented out) in a section that counts, in file order.
    fn entries(&self) -> Vec<Entry> {
        let mut section = String::new();
        let mut out = Vec::new();
        for (i, line) in self.doc.lines().iter().enumerate() {
            let t = line.trim();
            if t.starts_with('[') {
                section = t.trim_start_matches('[').split(']').next().unwrap_or("").trim().to_ascii_lowercase();
                continue;
            }
            if self.section_counts(&section) {
                if let Some(e) = self.entry(i, line) {
                    out.push(e);
                }
            }
        }
        out
    }

    fn entry(&self, i: usize, line: &str) -> Option<Entry> {
        let start = line.len() - line.trim_start().len();
        let mut rest = &line[start..];
        let mut offset = start;
        let comments: &[char] = match self.dialect {
            Dialect::Php => &[';'],
            Dialect::MyIni => &['#', ';'],
        };
        let commented = rest.starts_with(comments);
        if commented {
            let stripped = rest.trim_start_matches(comments).trim_start();
            offset += rest.len() - stripped.len();
            rest = stripped;
        }
        if rest.is_empty() || rest.starts_with('[') || rest.starts_with('!') {
            return None;
        }
        let inline = self.comment_char();
        let Some(eq) = rest.find('=') else {
            // my.ini: a lone key (`skip-name-resolve`) means "on".
            let key = rest.split(inline).next().unwrap_or("").trim();
            if self.dialect == Dialect::Php || key.is_empty() || key.contains(char::is_whitespace) {
                return None;
            }
            return Some(Entry { line: i, key: self.norm(key), raw_key: key.into(), commented, value: "1".into(), span: None });
        };
        let key = rest[..eq].trim();
        if key.is_empty() || key.contains(char::is_whitespace) {
            return None;
        }
        let after = &rest[eq + 1..];
        let vstart = offset + eq + 1 + (after.len() - after.trim_start().len());
        let raw = &line[vstart..];
        let quote = raw.chars().next().filter(|c| *c == '"' || (*c == '\'' && self.dialect == Dialect::MyIni));
        let (value, vend) = match quote {
            // "quoted value" ; comment: the value runs to the closing quote (`"a;b"` stays `a;b`).
            Some(q) => match raw[1..].find(q) {
                Some(close) => (raw[1..1 + close].to_string(), vstart + close + 2),
                None => (raw[1..].trim_end().to_string(), line.len()),
            },
            None => {
                let v = raw.split(inline).next().unwrap_or("").trim_end();
                (v.to_string(), vstart + v.len())
            }
        };
        let span = (!commented && vend > vstart).then_some((vstart, vend));
        Some(Entry { line: i, key: self.norm(key), raw_key: key.into(), commented, value, span })
    }

    /// The value the program uses: the last active line, `None` when missing or commented out.
    pub fn value(&self, key: &str) -> Option<String> {
        let k = self.norm(key);
        self.entries().into_iter().filter(|e| !e.commented && e.key == k).last().map(|e| e.value)
    }

    /// Sets a key: replaces the value of the last active line, else rewrites the first commented-out
    /// `;key =` line, else adds the key (php.ini: at the end; my.ini: at the end of `[mysqld]`).
    pub fn set(&mut self, key: &str, value: &str) {
        let k = self.norm(key);
        let entries = self.entries();
        if let Some(e) = entries.iter().filter(|e| !e.commented && e.key == k).last() {
            let line = self.doc.line(e.line);
            let new = match e.span {
                Some((a, b)) => format!("{}{}{}", &line[..a], value, &line[b..]),
                None => format!("{}{} = {}", indent(line), e.raw_key, value),
            };
            self.doc.replace(e.line, new);
        } else if let Some(e) = entries.iter().find(|e| e.commented && e.key == k) {
            let new = format!("{}{} = {}", indent(self.doc.line(e.line)), e.raw_key, value);
            self.doc.replace(e.line, new);
        } else {
            self.append(format!("{key} = {value}"));
        }
    }

    /// Comments the key out, so the built-in default applies.
    pub fn unset(&mut self, key: &str) {
        let k = self.norm(key);
        for e in self.entries().into_iter().filter(|e| !e.commented && e.key == k) {
            let line = self.doc.line(e.line);
            let ind = indent(line);
            let new = format!("{ind}{}{}", self.comment_char(), &line[ind.len()..]);
            self.doc.replace(e.line, new);
        }
    }

    fn append(&mut self, line: String) {
        if self.dialect == Dialect::Php {
            return self.doc.push(line);
        }
        // After the last non-blank line of the last [mysqld] section.
        let mut section = String::new();
        let mut last = None;
        for (i, l) in self.doc.lines().iter().enumerate() {
            let t = l.trim();
            if t.starts_with('[') {
                section = t.trim_start_matches('[').split(']').next().unwrap_or("").trim().to_ascii_lowercase();
                if section == "mysqld" {
                    last = Some(i);
                }
            } else if section == "mysqld" && !t.is_empty() {
                last = Some(i);
            }
        }
        let mut block: Vec<String> = Vec::new();
        if !self.doc.lines().iter().any(|l| l.trim() == MY_INI_BLOCK) {
            if last.is_some_and(|i| !self.doc.line(i).trim().starts_with('[')) {
                block.push(String::new());
            }
            block.push(MY_INI_BLOCK.into());
        }
        block.push(line);
        match last {
            Some(i) => {
                for (k, l) in block.into_iter().enumerate() {
                    self.doc.insert(i + 1 + k, l);
                }
            }
            None => {
                self.doc.push("[mysqld]".into());
                block.into_iter().for_each(|l| self.doc.push(l));
            }
        }
    }
}

/// php.ini / my.ini booleans: `On`, `1`, `true`, `yes` (and PHP's `stdout` / `stderr` for display_errors).
pub fn truthy(v: &str) -> bool {
    matches!(v.trim().to_ascii_lowercase().as_str(), "1" | "on" | "true" | "yes" | "stdout" | "stderr")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn php_values_and_comments() {
        let ini = Ini::new(
            "[PHP]\r\nmemory_limit = 128M ; note\r\n;date.timezone =\r\nerror_log = \"C:/x;y/php.log\" ; quoted\r\nmemory_limit=256M\r\n[PATH=C:/site]\r\nshort_open_tag = On\r\n",
            Dialect::Php,
        );
        assert_eq!(ini.value("memory_limit").as_deref(), Some("256M"), "the last active line wins");
        assert_eq!(ini.value("date.timezone"), None, "commented out");
        // Pitfall: a quoted value with `;` was read wrongly; it runs to the closing quote.
        assert_eq!(ini.value("error_log").as_deref(), Some("C:/x;y/php.log"));
        assert_eq!(ini.value("short_open_tag"), None, "[PATH=] sections are not global");
    }

    #[test]
    fn php_set_rewrites_in_place_and_keeps_crlf() {
        let text = "a = 1 ; keep\r\n;date.timezone =\r\n;date.timezone = UTC\r\nb = \"x\"\r\n";
        let mut ini = Ini::new(text, Dialect::Php);
        ini.set("a", "2");
        ini.set("date.timezone", "Europe/Budapest");
        ini.set("b", "\"y\"");
        ini.set("new_key", "On");
        assert_eq!(
            ini.text(),
            "a = 2 ; keep\r\ndate.timezone = Europe/Budapest\r\n;date.timezone = UTC\r\nb = \"y\"\r\nnew_key = On\r\n"
        );
    }

    #[test]
    fn php_unset_then_set_reuses_the_line() {
        let mut ini = Ini::new("x = 1\n  error_log = \"a.log\"\n", Dialect::Php);
        ini.unset("error_log");
        assert_eq!(ini.text(), "x = 1\n  ;error_log = \"a.log\"\n");
        ini.set("error_log", "\"b.log\"");
        assert_eq!(ini.text(), "x = 1\n  error_log = \"b.log\"\n");
    }

    #[test]
    fn empty_value_gets_a_clean_line() {
        let mut ini = Ini::new("date.timezone =\n", Dialect::Php);
        ini.set("date.timezone", "UTC");
        assert_eq!(ini.text(), "date.timezone = UTC\n");
    }

    #[test]
    fn my_ini_keys_and_sections() {
        let text = "[client]\r\nport=3307\r\n[mysqld]\r\nbind_address=127.0.0.1\r\nport=3306 # main\r\nskip-name-resolve\r\nsql_mode = \"STRICT_TRANS_TABLES\"\r\n\r\n[mysqldump]\r\nquick\r\n";
        let mut ini = Ini::new(text, Dialect::MyIni);
        assert_eq!(ini.value("port").as_deref(), Some("3306"), "[client] does not count");
        assert_eq!(ini.value("bind-address").as_deref(), Some("127.0.0.1"), "- and _ are the same");
        assert_eq!(ini.value("skip_name_resolve").as_deref(), Some("1"));
        assert_eq!(ini.value("sql_mode").as_deref(), Some("STRICT_TRANS_TABLES"));
        ini.set("port", "3310");
        ini.set("max_connections", "200");
        ini.unset("bind-address");
        assert_eq!(
            ini.text(),
            format!("[client]\r\nport=3307\r\n[mysqld]\r\n#bind_address=127.0.0.1\r\nport=3310 # main\r\nskip-name-resolve\r\nsql_mode = \"STRICT_TRANS_TABLES\"\r\n\r\n{MY_INI_BLOCK}\r\nmax_connections = 200\r\n\r\n[mysqldump]\r\nquick\r\n")
        );
        ini.set("max_allowed_packet", "64M");
        assert!(ini.text().contains("max_connections = 200\r\nmax_allowed_packet = 64M\r\n\r\n[mysqldump]"), "into the same block");
        // Turned back on: the commented line is reused, not duplicated.
        ini.set("bind-address", "0.0.0.0");
        assert!(ini.text().contains("\r\nbind_address = 0.0.0.0\r\n"));
    }

    #[test]
    fn my_ini_without_mysqld_section() {
        let mut ini = Ini::new("[client]\nport=3306\n", Dialect::MyIni);
        ini.set("port", "3307");
        assert_eq!(ini.text(), format!("[client]\nport=3306\n[mysqld]\n{MY_INI_BLOCK}\nport = 3307\n"));
    }
}
