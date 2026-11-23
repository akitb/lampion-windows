//! Suggestions in the SQL editor: keywords, functions, types, the current database's tables and
//! columns, other databases, and after `name.` the columns of that table or alias.
//! The schema comes from the sidebar's loads (`set_databases`, `set_tables`).

use super::split::{block_comment_end, is_line_comment, line_end, skip_quoted};
use once_cell::sync::Lazy;
use serde::Serialize;
use std::collections::{HashMap, HashSet};
use std::sync::Mutex;

/// Common words first, then MariaDB's keywords, functions and types alphabetically, then common phrases.
pub const KEYWORDS: &[&str] = &[
    "SELECT", "FROM", "WHERE", "AND", "OR", "NOT", "NULL", "INSERT", "INTO", "VALUES", "UPDATE", "SET", "DELETE",
    "CREATE", "TABLE", "DATABASE", "ALTER", "DROP", "INDEX", "VIEW", "JOIN", "LEFT", "RIGHT", "INNER", "OUTER",
    "CROSS", "ON", "USING", "AS", "ORDER", "BY", "GROUP", "HAVING", "LIMIT", "OFFSET", "ASC", "DESC", "DISTINCT",
    "UNION", "ALL", "IN", "IS", "LIKE", "BETWEEN", "EXISTS", "CASE", "WHEN", "THEN", "ELSE", "END", "IF",
    "PRIMARY", "KEY", "FOREIGN", "REFERENCES", "UNIQUE", "DEFAULT", "AUTO_INCREMENT", "ADD", "COLUMN", "MODIFY",
    "CHANGE", "RENAME", "TO", "TRUNCATE", "REPLACE", "SHOW", "DESCRIBE", "EXPLAIN", "USE", "TABLES", "DATABASES",
    "COLUMNS", "COUNT", "SUM", "AVG", "MIN", "MAX", "CONCAT", "COALESCE", "IFNULL", "NOW", "INT", "VARCHAR",
    "TEXT", "DATE", "DATETIME", "DECIMAL", "BOOLEAN", "ABS", "ACTION", "ADDDATE", "ADDTIME", "AES_DECRYPT",
    "AES_ENCRYPT", "AFTER", "AGAINST", "ALGORITHM", "ALWAYS", "ANALYZE", "ANY", "Aria", "ASCII", "AUTOCOMMIT",
    "AVG_ROW_LENGTH", "BEFORE", "BEGIN", "BIGINT", "BIN", "BINARY", "BIT", "BIT_LENGTH", "BLOB", "BOOL", "BOTH",
    "BTREE", "CALL", "CASCADE", "CASCADED", "CAST", "CEIL", "CEILING", "CHAR", "CHAR_LENGTH", "CHARACTER",
    "CHARSET", "CHECK", "CHECKSUM", "COLLATE", "COLLATION", "COMMENT", "COMMIT", "COMMITTED", "COMPACT",
    "COMPRESSED", "CONCAT_WS", "CONNECTION_ID", "CONSTRAINT", "CONTAINS", "CONVERT", "CURDATE", "CURRENT",
    "CURRENT_DATE", "CURRENT_TIME", "CURRENT_TIMESTAMP", "CURRENT_USER", "CURSOR", "CURTIME", "CYCLE", "DATA",
    "DATE_ADD", "DATE_FORMAT", "DATE_SUB", "DATEDIFF", "DAY", "DAYNAME", "DAYOFMONTH", "DAYOFWEEK", "DAYOFYEAR",
    "DEALLOCATE", "DEC", "DECLARE", "DEFINER", "DELAYED", "DELIMITER", "DENSE_RANK", "DETERMINISTIC", "DISABLE",
    "DIV", "DO", "DOUBLE", "DUPLICATE", "DYNAMIC", "EACH", "ELSEIF", "ELT", "ENABLE", "ENCLOSED", "ENGINE",
    "ENGINES", "ENUM", "ERRORS", "ESCAPE", "ESCAPED", "EVENT", "EVENTS", "EXCEPT", "EXECUTE", "EXIT", "EXP",
    "EXTENDED", "EXTRACT", "FALSE", "FETCH", "FIELD", "FIELDS", "FIND_IN_SET", "FIRST", "FIRST_VALUE", "FIXED",
    "FLOAT", "FLOOR", "FLUSH", "FOLLOWING", "FOR", "FORCE", "FORMAT", "FOUND_ROWS", "FROM_UNIXTIME", "FULL",
    "FULLTEXT", "FUNCTION", "GENERATED", "GEOMETRY", "GLOBAL", "GRANT", "GRANTS", "GREATEST", "GROUP_CONCAT",
    "HANDLER", "HASH", "HEX", "HIGH_PRIORITY", "HOUR", "IDENTIFIED", "IGNORE", "IMPORT", "INCREMENT", "INDEXES",
    "INET_ATON", "INET_NTOA", "INFILE", "InnoDB", "INOUT", "INSERT_METHOD", "INSTR", "INTEGER", "INTERSECT",
    "INTERVAL", "INVISIBLE", "ISNULL", "ISOLATION", "ITERATE", "JSON", "JSON_ARRAY", "JSON_ARRAYAGG",
    "JSON_CONTAINS", "JSON_EXTRACT", "JSON_LENGTH", "JSON_OBJECT", "JSON_OBJECTAGG", "JSON_SET", "JSON_UNQUOTE",
    "JSON_VALUE", "KEYS", "KILL", "LAG", "LANGUAGE", "LAST", "LAST_DAY", "LAST_INSERT_ID", "LAST_VALUE", "latin1",
    "LCASE", "LEAD", "LEADING", "LEAST", "LEAVE", "LENGTH", "LEVEL", "LINES", "LN", "LOAD", "LOCAL", "LOCATE",
    "LOCK", "LOG", "LOGS", "LONGBLOB", "LONGTEXT", "LOOP", "LOW_PRIORITY", "LOWER", "LPAD", "LTRIM", "MAKEDATE",
    "MASTER", "MATCH", "MAXVALUE", "MD5", "MEDIUMBLOB", "MEDIUMINT", "MEDIUMTEXT", "MEMORY", "MERGE", "MINUTE",
    "MINVALUE", "MOD", "MODE", "MONTH", "MONTHNAME", "MyISAM", "NAMES", "NATURAL", "NEXT", "NO", "NOWAIT",
    "NTILE", "NULLIF", "NUMERIC", "NVARCHAR", "OPEN", "OPTIMIZE", "OPTION", "OPTIONALLY", "OUT", "OUTFILE",
    "OVER", "PARTITION", "PARTITIONS", "PASSWORD", "PERSISTENT", "PI", "POSITION", "POW", "POWER", "PRECEDING",
    "PRECISION", "PREPARE", "PRIVILEGES", "PROCEDURE", "PROCESSLIST", "PURGE", "QUARTER", "QUERY", "QUICK",
    "RAND", "RANGE", "RANK", "READ", "REAL", "RECURSIVE", "REDUNDANT", "REGEXP", "RELEASE", "REPAIR", "REPEAT",
    "REPEATABLE", "REQUIRE", "RESET", "RESTRICT", "RETURN", "RETURNING", "RETURNS", "REVERSE", "REVOKE", "RLIKE",
    "ROLE", "ROLLBACK", "ROLLUP", "ROUND", "ROUTINE", "ROW", "ROW_COUNT", "ROW_FORMAT", "ROW_NUMBER", "ROWS",
    "RPAD", "RTRIM", "SAVEPOINT", "SCHEMA", "SCHEMAS", "SECOND", "SECURITY", "SEPARATOR", "SEQUENCE", "SERIAL",
    "SERIALIZABLE", "SESSION", "SHA1", "SHA2", "SIGN", "SIGNAL", "SIGNED", "SLAVE", "SLEEP", "SMALLINT", "SOME",
    "SPATIAL", "SQL", "SQL_CALC_FOUND_ROWS", "SQRT", "START", "STARTING", "STATUS", "STORED", "STR_TO_DATE",
    "STRAIGHT_JOIN", "SUBDATE", "SUBPARTITION", "SUBSTR", "SUBSTRING", "SUBSTRING_INDEX", "SYSDATE", "TEMPORARY",
    "TERMINATED", "TIME", "TIME_FORMAT", "TIMEDIFF", "TIMESTAMP", "TIMESTAMPDIFF", "TINYBLOB", "TINYINT",
    "TINYTEXT", "TO_DAYS", "TRAILING", "TRANSACTION", "TRIGGER", "TRIGGERS", "TRIM", "TRUE", "UCASE", "UNBOUNDED",
    "UNCOMMITTED", "UNDO", "UNHEX", "UNIX_TIMESTAMP", "UNLOCK", "UNSIGNED", "UNTIL", "UPPER", "USAGE", "USER",
    "UTC_TIMESTAMP", "utf8mb4", "utf8mb4_bin", "utf8mb4_general_ci", "utf8mb4_hungarian_ci", "utf8mb4_unicode_ci",
    "UUID", "VALUE", "VARBINARY", "VARIABLES", "VARYING", "VERSION", "VIRTUAL", "WAIT", "WARNINGS", "WEEK",
    "WEEKDAY", "WHILE", "WINDOW", "WITH", "WORK", "WRITE", "XOR", "YEAR", "YEARWEEK", "ZEROFILL", "ORDER BY",
    "GROUP BY", "INSERT INTO", "DELETE FROM", "CREATE TABLE", "CREATE DATABASE", "ALTER TABLE", "DROP TABLE",
    "LEFT JOIN", "INNER JOIN", "PRIMARY KEY", "FOREIGN KEY", "NOT NULL", "IF NOT EXISTS", "IF EXISTS",
    "UNION ALL", "ON DELETE CASCADE", "ON UPDATE CASCADE", "ON DUPLICATE KEY UPDATE", "SHOW TABLES",
    "SHOW DATABASES", "SHOW CREATE TABLE", "IS NULL", "IS NOT NULL", "TRUNCATE TABLE", "ADD COLUMN",
    "DROP COLUMN", "DEFAULT CHARSET",
];

/// Highlighted as functions when followed by `(`.
pub const FUNCTIONS: &[&str] = &[
    "COUNT", "SUM", "AVG", "MIN", "MAX", "CONCAT", "COALESCE", "IFNULL", "NOW", "IF", "ABS", "ADDDATE", "ADDTIME", "AES_DECRYPT",
    "AES_ENCRYPT", "BIN", "BIT_LENGTH", "CAST", "CEIL", "CEILING", "CHAR_LENGTH", "CONCAT_WS", "CONNECTION_ID", "CONVERT", "CURDATE",
    "CURTIME", "DATABASE", "DATEDIFF", "DATE_ADD", "DATE_FORMAT", "DATE_SUB", "DAY", "DAYNAME", "DAYOFMONTH", "DAYOFWEEK", "DAYOFYEAR",
    "DENSE_RANK", "ELT", "EXP", "EXTRACT", "FIELD", "FIND_IN_SET", "FIRST_VALUE", "FLOOR", "FORMAT", "FOUND_ROWS", "FROM_UNIXTIME",
    "GREATEST", "GROUP_CONCAT", "HEX", "HOUR", "INET_ATON", "INET_NTOA", "INSTR", "ISNULL", "JSON_ARRAY", "JSON_ARRAYAGG",
    "JSON_CONTAINS", "JSON_EXTRACT", "JSON_LENGTH", "JSON_OBJECT", "JSON_OBJECTAGG", "JSON_SET", "JSON_UNQUOTE", "JSON_VALUE", "LAG",
    "LAST_DAY", "LAST_INSERT_ID", "LAST_VALUE", "LCASE", "LEAD", "LEAST", "LEFT", "LENGTH", "LN", "LOCATE", "LOG", "LOWER", "LPAD",
    "LTRIM", "MAKEDATE", "MD5", "MINUTE", "MOD", "MONTH", "MONTHNAME", "NULLIF", "NTILE", "PI", "POSITION", "POW", "POWER", "QUARTER",
    "RAND", "RANK", "REPEAT", "REPLACE", "REVERSE", "RIGHT", "ROUND", "ROW_COUNT", "ROW_NUMBER", "RPAD", "RTRIM", "SECOND", "SHA1",
    "SHA2", "SIGN", "SLEEP", "SQRT", "STR_TO_DATE", "SUBDATE", "SUBSTR", "SUBSTRING", "SUBSTRING_INDEX", "SYSDATE", "TIMEDIFF",
    "TIMESTAMPDIFF", "TIME_FORMAT", "TO_DAYS", "TRIM", "TRUNCATE", "UCASE", "UNHEX", "UNIX_TIMESTAMP", "UPPER", "USER", "UTC_TIMESTAMP",
    "UUID", "VERSION", "WEEK", "WEEKDAY", "YEARWEEK",
];

/// Highlighted as types.
pub const TYPES: &[&str] = &[
    "INT", "INTEGER", "BIGINT", "SMALLINT", "MEDIUMINT", "TINYINT", "DECIMAL", "DEC", "NUMERIC", "FLOAT", "DOUBLE", "REAL", "BIT",
    "BOOL", "BOOLEAN", "SERIAL", "CHAR", "VARCHAR", "NVARCHAR", "BINARY", "VARBINARY", "TEXT", "TINYTEXT", "MEDIUMTEXT", "LONGTEXT",
    "BLOB", "TINYBLOB", "MEDIUMBLOB", "LONGBLOB", "DATE", "DATETIME", "TIMESTAMP", "TIME", "YEAR", "ENUM", "JSON", "GEOMETRY",
    "UNSIGNED", "SIGNED", "ZEROFILL",
];

/// Single uppercase words of `KEYWORDS`, for highlighting.
pub fn highlight_keywords() -> Vec<&'static str> {
    KEYWORDS.iter().copied().filter(|k| !k.contains(' ') && k.to_uppercase() == *k).collect()
}

/// What the editor completes from: databases, and per database its tables with their columns.
#[derive(Debug, Default)]
pub struct Schema {
    pub databases: Vec<String>,
    pub tables: HashMap<String, Vec<(String, Vec<String>)>>,
}

static SCHEMA: Lazy<Mutex<Schema>> = Lazy::new(Default::default);

pub fn set_databases(list: &[String]) {
    SCHEMA.lock().unwrap().databases = list.to_vec();
}

pub fn set_tables(db: &str, tables: Vec<(String, Vec<String>)>) {
    SCHEMA.lock().unwrap().tables.insert(db.to_string(), tables);
}

fn same(a: &str, b: &str) -> bool {
    a.to_lowercase() == b.to_lowercase()
}

fn is_word(c: char) -> bool {
    c.is_alphanumeric() || c == '_' || c == '$'
}

/// The name being typed before the caret, and the name before a dot: `users.na` → ("na", Some("users")).
pub fn word_before(before: &str) -> (&str, Option<&str>) {
    let start = before.char_indices().rev().take_while(|&(_, c)| is_word(c)).last().map_or(before.len(), |(i, _)| i);
    let qualifier = before[..start].strip_suffix('.').and_then(|r| {
        let r = r.strip_suffix('`').unwrap_or(r);
        let s = r.char_indices().rev().take_while(|&(_, c)| is_word(c)).last()?.0;
        Some(&r[s..])
    });
    (&before[start..], qualifier)
}

#[derive(Debug, PartialEq)]
enum Tok {
    Word(String),
    Name(String),
    Punct(char),
}

/// Words, backtick names and punctuation, skipping strings and comments.
fn tokens(text: &str) -> Vec<Tok> {
    let b = text.as_bytes();
    let mut out = Vec::new();
    let mut i = 0;
    while i < b.len() {
        let c = b[i];
        if c.is_ascii_whitespace() {
            i += 1;
        } else if c == b'\'' || c == b'"' {
            i = skip_quoted(b, i);
        } else if c == b'`' {
            let end = skip_quoted(b, i);
            out.push(Tok::Name(text[i + 1..end.saturating_sub(1).max(i + 1)].replace("``", "`")));
            i = end;
        } else if is_line_comment(b, i) {
            i = line_end(b, i);
        } else if c == b'/' && b.get(i + 1) == Some(&b'*') {
            i = block_comment_end(b, i);
        } else {
            let ch = text[i..].chars().next().unwrap_or(' ');
            if is_word(ch) {
                let end = text[i..].char_indices().find(|&(_, c)| !is_word(c)).map_or(text.len(), |(p, _)| i + p);
                out.push(Tok::Word(text[i..end].to_string()));
                i = end;
            } else {
                out.push(Tok::Punct(ch));
                i += ch.len_utf8();
            }
        }
    }
    out
}

/// Table aliases in the text: `FROM customers c`, `JOIN shop.orders AS o`, `, items i` → (alias, table).
pub fn aliases(text: &str) -> Vec<(String, String)> {
    let keywords: HashSet<String> = KEYWORDS.iter().filter(|k| !k.contains(' ')).map(|k| k.to_uppercase()).collect();
    let toks = tokens(text);
    let name = |i: usize| match toks.get(i) {
        Some(Tok::Word(w)) if !keywords.contains(&w.to_uppercase()) => Some(w.clone()),
        Some(Tok::Name(n)) => Some(n.clone()),
        _ => None,
    };
    let mut out = Vec::new();
    for i in 0..toks.len() {
        let starts = match &toks[i] {
            Tok::Word(w) => ["FROM", "JOIN", "UPDATE", "INTO"].iter().any(|k| w.eq_ignore_ascii_case(k)),
            Tok::Punct(',') => true,
            _ => false,
        };
        let Some(mut table) = starts.then(|| name(i + 1)).flatten() else { continue };
        let mut j = i + 2;
        if toks.get(j) == Some(&Tok::Punct('.')) {
            let Some(t) = name(j + 1) else { continue };
            table = format!("{table}.{t}");
            j += 2;
        }
        if matches!(toks.get(j), Some(Tok::Word(w)) if w.eq_ignore_ascii_case("AS")) {
            j += 1;
        }
        if let Some(alias) = name(j) {
            out.push((alias, table));
        }
    }
    out
}

/// Suggestions for `prefix`. `qualifier`: the name before a dot (table, alias or database).
pub fn complete(s: &Schema, db: Option<&str>, table: Option<&str>, prefix: &str, qualifier: Option<&str>, aliases: &[(String, String)]) -> Vec<String> {
    let tables_in = |d: &str| s.tables.get(d).or_else(|| s.tables.iter().find(|(k, _)| same(k, d)).map(|(_, v)| v));
    let columns_of = |d: &str, t: &str| tables_in(d)?.iter().find(|(name, _)| same(name, t)).map(|(_, c)| c.clone());
    let all_columns = |d: &str| -> Vec<String> {
        let mut list = tables_in(d).cloned().unwrap_or_default();
        list.sort_by(|a, b| a.0.cmp(&b.0));
        list.into_iter().flat_map(|(_, c)| c).collect()
    };
    let mut candidates: Vec<String> = Vec::new();
    if let Some(q) = qualifier {
        let aliased = || {
            let (_, t) = aliases.iter().find(|(a, _)| same(a, q))?;
            match t.split_once('.') {
                Some((d, t)) => columns_of(d, t),
                None => columns_of(db?, t),
            }
        };
        if let Some(cols) = db.and_then(|d| columns_of(d, q)).or_else(aliased) {
            candidates = cols;
        } else if let Some(d) = s.databases.iter().find(|d| same(d, q)) {
            candidates = tables_in(d).map(|l| l.iter().map(|(t, _)| t.clone()).collect()).unwrap_or_default();
        } else if let Some(d) = db {
            // An alias Lampion couldn't resolve: every column of the database.
            candidates = all_columns(d);
        }
    } else {
        if let Some(d) = db {
            candidates.extend(tables_in(d).into_iter().flatten().map(|(t, _)| t.clone()));
            if let Some(t) = table {
                candidates.extend(columns_of(d, t).unwrap_or_default());
            }
            candidates.extend(all_columns(d));
        }
        candidates.extend(s.databases.iter().cloned());
        // Keywords follow the case being typed: lowercase typing gets lowercase keywords.
        let lower = !prefix.is_empty() && prefix == prefix.to_lowercase() && prefix != prefix.to_uppercase();
        candidates.extend(KEYWORDS.iter().map(|k| if lower { k.to_lowercase() } else { k.to_string() }));
    }
    let p = prefix.to_lowercase();
    let mut seen = HashSet::new();
    candidates
        .into_iter()
        .filter(|c| {
            let l = c.to_lowercase();
            (p.is_empty() || l.starts_with(&p)) && l != p && seen.insert(l)
        })
        .take(60)
        .collect()
}

#[derive(Debug, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Completion {
    /// Length of the typed name in UTF-16 units: the editor replaces that many characters.
    pub prefix_len: usize,
    /// A `name.` comes before the caret (suggest columns even with nothing typed yet).
    pub qualified: bool,
    pub items: Vec<String>,
}

/// `before`: the text up to the caret; `text`: the whole editor (for aliases).
pub fn at_caret(db: Option<&str>, table: Option<&str>, before: &str, text: &str) -> Completion {
    let (prefix, qualifier) = word_before(before);
    let aliases = if qualifier.is_some() && text.len() < 200_000 { aliases(text) } else { Vec::new() };
    let items = complete(&SCHEMA.lock().unwrap(), db, table, prefix, qualifier, &aliases);
    Completion { prefix_len: prefix.encode_utf16().count(), qualified: qualifier.is_some(), items }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn schema() -> Schema {
        let cols = |c: &[&str]| c.iter().map(|s| s.to_string()).collect::<Vec<_>>();
        let mut s = Schema { databases: vec!["_lampion_teszt".into(), "lampion_demo".into(), "mysql".into()], ..Default::default() };
        s.tables.insert("_lampion_teszt".into(), vec![("termekek".into(), cols(&["id", "nev", "ar"]))]);
        s.tables.insert(
            "lampion_demo".into(),
            vec![("customers".into(), cols(&["id", "name", "city"])), ("orders".into(), cols(&["id", "customer_id", "total"]))],
        );
        s
    }

    fn c(prefix: &str, qualifier: Option<&str>) -> Vec<String> {
        complete(&schema(), Some("_lampion_teszt"), None, prefix, qualifier, &[])
    }

    #[test]
    fn tables_columns_and_keywords() {
        assert_eq!(c("terme", None), ["termekek"]);
        assert_eq!(c("", Some("termekek")), ["id", "nev", "ar"]);
        assert_eq!(c("SEL", None), ["SELECT"]);
        assert_eq!(c("CRE", None)[0], "CREATE");
        assert_eq!(c("TAB", None)[..2], ["TABLE", "TABLES"]);
        assert_eq!(c("tab", None)[0], "table");
        for w in ["VARCHAR", "AUTO_INCREMENT", "GROUP_CONCAT", "ENGINE", "CONSTRAINT"] {
            assert!(c(&w[..5], None).iter().any(|x| x == w), "{w}");
        }
        // The word already typed in full is not offered again.
        assert!(!c("select", None).iter().any(|x| x.eq_ignore_ascii_case("select")));
    }

    #[test]
    fn database_qualifier_lists_its_tables() {
        assert_eq!(c("cu", Some("lampion_demo")), ["customers"]);
    }

    #[test]
    fn unknown_alias_falls_back_to_all_columns() {
        assert!(c("n", Some("t")).contains(&"nev".to_string()));
    }

    #[test]
    fn aliases_resolve_to_their_table() {
        let text = "SELECT c.name, o.total FROM customers c JOIN lampion_demo.orders AS o ON o.customer_id = c.id, `orders` x WHERE 1";
        let a = aliases(text);
        assert_eq!(a, [("c".to_string(), "customers".to_string()), ("o".into(), "lampion_demo.orders".into()), ("x".into(), "orders".into())]);
        let s = schema();
        assert_eq!(complete(&s, Some("lampion_demo"), None, "", Some("c"), &a), ["id", "name", "city"]);
        assert_eq!(complete(&s, Some("lampion_demo"), None, "c", Some("o"), &a), ["customer_id"]);
        // Keywords are not aliases.
        assert!(aliases("SELECT * FROM customers WHERE id = 1 ORDER BY id").is_empty());
        assert!(aliases("SELECT 'FROM x y' -- FROM a b\n").is_empty());
    }

    #[test]
    fn caret_word_and_qualifier() {
        assert_eq!(word_before("SELECT users.na"), ("na", Some("users")));
        assert_eq!(word_before("SELECT `users`."), ("", Some("users")));
        assert_eq!(word_before("SELECT név"), ("név", None));
        assert_eq!(word_before("SELECT "), ("", None));
        let done = at_caret(None, None, "SELECT é", "SELECT é");
        assert_eq!(done.prefix_len, 1);
    }

    #[test]
    fn highlight_words_are_single_uppercase() {
        let k = highlight_keywords();
        assert!(k.contains(&"SELECT") && !k.contains(&"ORDER BY") && !k.contains(&"InnoDB"));
    }
}
