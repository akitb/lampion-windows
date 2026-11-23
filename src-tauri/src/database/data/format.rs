//! How values and sizes are shown: grid cells, row estimates (1.2k), sizes (48 KB), type families.

use crate::database::client;
use mysql_async::consts::ColumnType;
use mysql_async::{Column, Value};

/// A cell as text: `None` is SQL NULL, an empty string stays empty, binary data is `0x…` hex.
pub fn cell(v: &Value, binary: bool) -> Option<String> {
    match v {
        Value::Bytes(b) if binary => Some(if b.is_empty() { String::new() } else { format!("0x{}", hex::encode_upper(b)) }),
        other => client::text(other),
    }
}

/// Columns whose bytes are shown as hex: binary strings, blobs, BIT and geometry.
pub fn is_binary_column(c: &Column) -> bool {
    use ColumnType::*;
    c.character_set() == 63
        && matches!(
            c.column_type(),
            MYSQL_TYPE_STRING | MYSQL_TYPE_VAR_STRING | MYSQL_TYPE_VARCHAR | MYSQL_TYPE_TINY_BLOB | MYSQL_TYPE_MEDIUM_BLOB
                | MYSQL_TYPE_LONG_BLOB | MYSQL_TYPE_BLOB | MYSQL_TYPE_BIT | MYSQL_TYPE_GEOMETRY
        )
}

/// A declared type that holds bytes, not text (the inspector shows it read-only).
pub fn is_binary_type(sql_type: &str) -> bool {
    let t = sql_type.to_ascii_lowercase();
    t.contains("blob") || t.contains("binary") || t.starts_with("bit")
}

/// The kind of value a type holds; drives colours, alignment and icons.
pub fn family(sql_type: &str) -> &'static str {
    let t = sql_type.trim().to_ascii_lowercase();
    let any = |list: &[&str]| list.iter().any(|p| t.starts_with(p));
    if t.starts_with("tinyint(1)") || t == "bool" || t == "boolean" {
        "boolean"
    } else if any(&["int", "tinyint", "smallint", "mediumint", "bigint", "integer", "bit", "serial"]) {
        "integer"
    } else if any(&["decimal", "numeric", "float", "double", "real"]) {
        "decimal"
    } else if any(&["date", "time", "year", "timestamp"]) {
        "temporal"
    } else if any(&["enum", "set"]) {
        "enumSet"
    } else if t.starts_with("json") {
        "json"
    } else if any(&["blob", "binary", "varbinary", "tinyblob", "mediumblob", "longblob"]) {
        "binary"
    } else if any(&["geometry", "point", "linestring", "polygon", "multi"]) {
        "spatial"
    } else if any(&["char", "varchar", "text", "tinytext", "mediumtext", "longtext", "uuid", "inet"]) {
        "text"
    } else {
        "other"
    }
}

/// The values of an ENUM or SET type, in order (`''` inside a value is one quote).
pub fn enum_values(sql_type: &str) -> Vec<String> {
    let lower = sql_type.to_ascii_lowercase();
    if !(lower.starts_with("enum(") || lower.starts_with("set(")) {
        return Vec::new();
    }
    let mut out = Vec::new();
    let mut chars = sql_type.chars().peekable();
    while let Some(c) = chars.next() {
        if c != '\'' {
            continue;
        }
        let mut v = String::new();
        while let Some(c) = chars.next() {
            if c == '\'' {
                if chars.peek() == Some(&'\'') {
                    chars.next();
                    v.push('\'');
                } else {
                    break;
                }
            } else {
                v.push(c);
            }
        }
        out.push(v);
    }
    out
}

/// A row estimate for the sidebar: 7, 1.2k, 45k, 3.4M, 120M, 1.5B (rounded down).
pub fn compact_count(n: u64) -> String {
    fn one_decimal(v: f64) -> String {
        let s = format!("{:.1}", (v * 10.0).floor() / 10.0);
        s.strip_suffix(".0").map(str::to_string).unwrap_or(s)
    }
    match n {
        0..=999 => n.to_string(),
        1_000..=9_999 => one_decimal(n as f64 / 1e3) + "k",
        10_000..=999_999 => format!("{}k", n / 1_000),
        1_000_000..=9_999_999 => one_decimal(n as f64 / 1e6) + "M",
        10_000_000..=999_999_999 => format!("{}M", n / 1_000_000),
        _ => one_decimal(n as f64 / 1e9) + "B",
    }
}

/// A size like the macOS byte formatter (binary units): Zero KB, 512 bytes, 48 KB, 1.2 MB, 1.25 GB.
pub fn byte_size(n: u64) -> String {
    const K: f64 = 1024.0;
    let v = n as f64;
    let trim = |s: String| if s.contains('.') { s.trim_end_matches('0').trim_end_matches('.').to_string() } else { s };
    match n {
        0 => "Zero KB".into(),
        1 => "1 byte".into(),
        2..=1023 => format!("{n} bytes"),
        _ if v < K * K => format!("{} KB", (v / K).round()),
        _ if v < K * K * K => trim(format!("{:.1}", v / (K * K))) + " MB",
        _ if v < K * K * K * K => trim(format!("{:.2}", v / (K * K * K))) + " GB",
        _ => trim(format!("{:.2}", v / (K * K * K * K))) + " TB",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cells_keep_null_empty_and_binary_apart() {
        assert_eq!(cell(&Value::NULL, false), None);
        assert_eq!(cell(&Value::NULL, true), None);
        assert_eq!(cell(&Value::Bytes(Vec::new()), false), Some(String::new()));
        assert_eq!(cell(&Value::Bytes(Vec::new()), true), Some(String::new()));
        assert_eq!(cell(&Value::Bytes("Árvíztűrő".as_bytes().to_vec()), false).as_deref(), Some("Árvíztűrő"));
        // Binary columns are always hex, also when the bytes happen to be valid UTF-8.
        assert_eq!(cell(&Value::Bytes(b"0x41".to_vec()), true).as_deref(), Some("0x30783431"));
        assert_eq!(cell(&Value::Bytes(vec![0xff, 0x00]), true).as_deref(), Some("0xFF00"));
        // Invalid UTF-8 in a text column still can't break the grid.
        assert_eq!(cell(&Value::Bytes(vec![0xff]), false).as_deref(), Some("0xFF"));
        assert_eq!(cell(&Value::Int(-5), false).as_deref(), Some("-5"));
    }

    #[test]
    fn type_families() {
        assert_eq!(family("int(10) unsigned"), "integer");
        assert_eq!(family("tinyint(1)"), "boolean");
        assert_eq!(family("decimal(10,2)"), "decimal");
        assert_eq!(family("varchar(120)"), "text");
        assert_eq!(family("datetime"), "temporal");
        assert_eq!(family("enum('a','b')"), "enumSet");
        assert_eq!(family("longblob"), "binary");
        assert_eq!(family("json"), "json");
        assert_eq!(family("point"), "spatial");
        assert_eq!(family("something"), "other");
        assert!(is_binary_type("varbinary(16)") && is_binary_type("bit(1)") && !is_binary_type("varchar(5)"));
    }

    #[test]
    fn enum_values_unquote() {
        assert_eq!(enum_values("enum('new','paid','shipped')"), ["new", "paid", "shipped"]);
        assert_eq!(enum_values("set('a,b','it''s')"), ["a,b", "it's"]);
        assert!(enum_values("varchar(10)").is_empty());
    }

    #[test]
    fn compact_row_counts() {
        let cases = [(0, "0"), (7, "7"), (999, "999"), (1_000, "1k"), (1_234, "1.2k"), (1_299, "1.2k"), (9_999, "9.9k"), (45_678, "45k"),
            (1_250_000, "1.2M"), (120_000_000, "120M"), (1_500_000_000, "1.5B")];
        for (n, s) in cases {
            assert_eq!(compact_count(n), s, "{n}");
        }
    }

    #[test]
    fn byte_sizes() {
        let cases = [(0, "Zero KB"), (1, "1 byte"), (512, "512 bytes"), (16_384, "16 KB"), (49_152, "48 KB"), (344_064, "336 KB"),
            (1_048_576, "1 MB"), (1_258_291, "1.2 MB"), (1_342_177_280, "1.25 GB")];
        for (n, s) in cases {
            assert_eq!(byte_size(n), s, "{n}");
        }
    }
}
