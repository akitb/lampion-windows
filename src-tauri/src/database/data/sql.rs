//! SQL for the Data tab: the page query, and finding, changing, adding and deleting one row.
//! Values travel as `?` parameters; `display` is the same statement with the values written out,
//! for the SQL console. Binary values (`0x…`, as the grid shows them) are written as hex literals.

use super::format;
use super::split::split;
use crate::database::client::{ident, qualified};
use serde::Serialize;

/// Rows per page in the Data tab.
pub const PAGE_SIZE: u64 = 500;

/// A column of a table, from `SHOW FULL COLUMNS`, with what the grid, inspector and insert sheet need.
#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ColumnInfo {
    pub name: String,
    /// The declared type, e.g. `varchar(100)` or `int(10) unsigned`.
    #[serde(rename = "type")]
    pub sql_type: String,
    pub nullable: bool,
    /// `PRI`, `UNI`, `MUL` or empty.
    pub key: String,
    pub default: Option<String>,
    pub extra: String,
    pub comment: String,
    pub collation: String,
    /// integer, decimal, text, temporal, binary, enumSet, json, spatial, boolean or other.
    pub family: &'static str,
    pub numeric: bool,
    pub binary: bool,
    pub auto_increment: bool,
    /// NOT NULL, no default and not filled in automatically: an insert must give a value.
    pub required: bool,
    pub enum_values: Vec<String>,
}

impl ColumnInfo {
    pub fn new(name: &str, sql_type: &str, null: &str, key: &str, default: Option<&str>, extra: &str, comment: &str, collation: &str) -> Self {
        let nullable = null.eq_ignore_ascii_case("YES");
        let auto_increment = extra.to_ascii_lowercase().contains("auto_increment");
        let family = format::family(sql_type);
        ColumnInfo {
            name: name.to_string(),
            sql_type: sql_type.to_string(),
            nullable,
            key: key.to_string(),
            default: default.map(str::to_string),
            extra: extra.to_string(),
            comment: comment.to_string(),
            collation: collation.to_string(),
            family,
            numeric: family == "integer" || family == "decimal",
            binary: format::is_binary_type(sql_type),
            auto_increment,
            required: !nullable && default.is_none() && !auto_increment,
            enum_values: format::enum_values(sql_type),
        }
    }
}

/// One statement with `?` parameters, and how the SQL console shows it.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Built {
    pub sql: String,
    pub params: Vec<String>,
    pub display: String,
}

impl Built {
    fn raw(&mut self, s: &str) -> &mut Self {
        self.sql.push_str(s);
        self.display.push_str(s);
        self
    }

    fn value(&mut self, v: Option<&str>, binary: bool) -> &mut Self {
        match v {
            None => self.raw("NULL"),
            Some(s) if binary && is_hex(s) => self.raw(s),
            Some(s) => {
                self.sql.push('?');
                self.params.push(s.to_string());
                self.display.push_str(&string_literal(s));
                self
            }
        }
    }
}

/// A string literal as the console and "Copy Row as INSERT" write it: `'` doubled, `\` escaped.
pub fn string_literal(s: &str) -> String {
    format!("'{}'", s.replace('\\', "\\\\").replace('\'', "''"))
}

/// `0x` and an even number of hex digits: a binary value as the grid shows it.
pub fn is_hex(s: &str) -> bool {
    s.len() > 2 && s.len() % 2 == 0 && (s.starts_with("0x") || s.starts_with("0X")) && s[2..].bytes().all(|c| c.is_ascii_hexdigit())
}

/// The page query and the row count, sent together (the console shows them as one line).
/// `filter` is the user's WHERE condition; it must be a single condition.
pub fn page(db: &str, table: &str, filter: &str, order: Option<(&str, bool)>, offset: u64) -> Result<String, String> {
    let filter = filter.trim().trim_end_matches(|c: char| c == ';' || c.is_whitespace());
    if split(filter).len() > 1 {
        return Err("The filter must be a single condition. Remove the “;” from it.".into());
    }
    let fq = qualified(db, table);
    let cond = match filter {
        "" => String::new(),
        // A comment in the filter must not swallow the rest of the query.
        f if f.contains("--") || f.contains('#') || f.contains("/*") => format!(" WHERE {f}\n"),
        f => format!(" WHERE {f}"),
    };
    let order = order.map(|(c, asc)| format!(" ORDER BY {} {}", ident(c), if asc { "ASC" } else { "DESC" })).unwrap_or_default();
    Ok(format!("SELECT * FROM {fq}{cond}{order} LIMIT {PAGE_SIZE} OFFSET {offset}; SELECT COUNT(*) FROM {fq}{cond};"))
}

/// The columns that find a row again: the primary key, else a UNIQUE NOT NULL column, else every column.
/// Indexes into `names`, the grid's columns (whose order the row values follow).
pub fn key_columns(info: &[ColumnInfo], names: &[String]) -> Vec<usize> {
    let pos = |name: &str| names.iter().position(|c| c == name);
    let pk: Vec<Option<usize>> = info.iter().filter(|c| c.key == "PRI").map(|c| pos(&c.name)).collect();
    if !pk.is_empty() && pk.iter().all(Option::is_some) {
        return pk.into_iter().flatten().collect();
    }
    if let Some(i) = info.iter().filter(|c| c.key == "UNI" && !c.nullable).find_map(|c| pos(&c.name)) {
        return vec![i];
    }
    (0..names.len()).collect()
}

fn is_binary(info: &[ColumnInfo], name: &str) -> bool {
    info.iter().any(|c| c.name == name && (c.binary || c.family == "spatial"))
}

/// `WHERE` for one row, NULL-safe (`<=>`).
fn match_row(q: &mut Built, info: &[ColumnInfo], names: &[String], row: &[Option<String>]) -> Result<(), String> {
    if row.len() != names.len() || names.is_empty() {
        return Err("The row doesn't match the table's columns. Reload the rows and try again.".into());
    }
    for (n, i) in key_columns(info, names).into_iter().enumerate() {
        if n > 0 {
            q.raw(" AND ");
        }
        q.raw(&ident(&names[i])).raw(" <=> ").value(row[i].as_deref(), is_binary(info, &names[i]));
    }
    Ok(())
}

/// `SELECT COUNT(*)` of the rows matching `row`: run before a change, so 0 is reported instead of a
/// silent no-op (a table without a key and with FLOAT values may not match exactly).
pub fn count_row(db: &str, table: &str, info: &[ColumnInfo], names: &[String], row: &[Option<String>]) -> Result<Built, String> {
    let mut q = Built::default();
    q.raw(&format!("SELECT COUNT(*) FROM {} WHERE ", qualified(db, table)));
    match_row(&mut q, info, names, row)?;
    Ok(q)
}

/// `UPDATE … SET … WHERE <row> LIMIT 1`. `changes`: (index into `names`, new value).
pub fn update_row(db: &str, table: &str, info: &[ColumnInfo], names: &[String], row: &[Option<String>], changes: &[(usize, Option<String>)]) -> Result<Built, String> {
    let mut q = Built::default();
    q.raw(&format!("UPDATE {} SET ", qualified(db, table)));
    for (n, (i, v)) in changes.iter().enumerate() {
        let name = names.get(*i).ok_or("Unknown column.")?;
        if n > 0 {
            q.raw(", ");
        }
        q.raw(&ident(name)).raw(" = ").value(v.as_deref(), is_binary(info, name));
    }
    q.raw(" WHERE ");
    match_row(&mut q, info, names, row)?;
    q.raw(" LIMIT 1");
    Ok(q)
}

pub fn delete_row(db: &str, table: &str, info: &[ColumnInfo], names: &[String], row: &[Option<String>]) -> Result<Built, String> {
    let mut q = Built::default();
    q.raw(&format!("DELETE FROM {} WHERE ", qualified(db, table)));
    match_row(&mut q, info, names, row)?;
    q.raw(" LIMIT 1");
    Ok(q)
}

/// `INSERT` of the filled-in fields (in table order); the others get their default.
pub fn insert_row(db: &str, table: &str, info: &[ColumnInfo], values: &[(String, String)]) -> Built {
    let given: Vec<(&ColumnInfo, &str)> =
        info.iter().filter_map(|c| values.iter().find(|(n, _)| *n == c.name).map(|(_, v)| (c, v.as_str()))).collect();
    let mut q = Built::default();
    let names: Vec<String> = given.iter().map(|(c, _)| ident(&c.name)).collect();
    q.raw(&format!("INSERT INTO {} ({}) VALUES (", qualified(db, table), names.join(", ")));
    for (n, (c, v)) in given.iter().enumerate() {
        if n > 0 {
            q.raw(", ");
        }
        q.value(Some(v), c.binary || c.family == "spatial");
    }
    q.raw(")");
    q
}

/// "Copy Row as INSERT": one statement per row. Numbers and `0x…` binary values stay unquoted.
pub fn rows_as_insert(table: &str, names: &[String], numeric: &[bool], binary: &[bool], rows: &[Vec<Option<String>>]) -> String {
    let cols = names.iter().map(|n| ident(n)).collect::<Vec<_>>().join(", ");
    rows.iter()
        .map(|row| {
            let values: Vec<String> = row
                .iter()
                .enumerate()
                .map(|(i, v)| match v {
                    None => "NULL".to_string(),
                    Some(s) if numeric.get(i) == Some(&true) && s.trim().parse::<f64>().is_ok() => s.clone(),
                    Some(s) if binary.get(i) == Some(&true) && is_hex(s) => s.clone(),
                    Some(s) => string_literal(s),
                })
                .collect();
            format!("INSERT INTO {} ({cols}) VALUES ({});", ident(table), values.join(", "))
        })
        .collect::<Vec<_>>()
        .join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn col(name: &str, ty: &str, null: &str, key: &str) -> ColumnInfo {
        ColumnInfo::new(name, ty, null, key, None, "", "", "")
    }
    fn names(n: &[&str]) -> Vec<String> {
        n.iter().map(|s| s.to_string()).collect()
    }
    fn row(v: &[Option<&str>]) -> Vec<Option<String>> {
        v.iter().map(|o| o.map(str::to_string)).collect()
    }

    #[test]
    fn page_query_as_the_console_shows_it() {
        assert_eq!(
            page("lampion_demo", "customers", "", None, 0).unwrap(),
            "SELECT * FROM `lampion_demo`.`customers` LIMIT 500 OFFSET 0; SELECT COUNT(*) FROM `lampion_demo`.`customers`;"
        );
        assert_eq!(
            page("lampion_demo", "customers", " city = 'Budapest' ", Some(("name", false)), 500).unwrap(),
            "SELECT * FROM `lampion_demo`.`customers` WHERE city = 'Budapest' ORDER BY `name` DESC LIMIT 500 OFFSET 500; \
             SELECT COUNT(*) FROM `lampion_demo`.`customers` WHERE city = 'Budapest';"
        );
    }

    #[test]
    fn filter_is_one_condition() {
        assert!(page("d", "t", "1=1; DROP TABLE x", None, 0).is_err());
        assert!(page("d", "t", "name = 'a;b'", None, 0).is_ok());
        // A trailing semicolon is a habit, not a second statement.
        assert_eq!(page("d", "t", "id = 1;", None, 0).unwrap(), "SELECT * FROM `d`.`t` WHERE id = 1 LIMIT 500 OFFSET 0; SELECT COUNT(*) FROM `d`.`t` WHERE id = 1;");
        // A comment ends at a line break instead of hiding LIMIT.
        assert!(page("d", "t", "id = 1 -- note", None, 0).unwrap().contains("-- note\n LIMIT 500"));
    }

    #[test]
    fn row_key_prefers_primary_key_then_unique_not_null() {
        let n = names(&["id", "email", "name"]);
        let pk = [col("id", "int(10) unsigned", "NO", "PRI"), col("email", "varchar(160)", "NO", "UNI"), col("name", "text", "YES", "")];
        assert_eq!(key_columns(&pk, &n), [0]);
        let unique = [col("id", "int", "YES", ""), col("email", "varchar(160)", "NO", "UNI"), col("name", "text", "YES", "")];
        assert_eq!(key_columns(&unique, &n), [1]);
        // A nullable unique column can't identify a row: every column is compared.
        let nullable = [col("id", "int", "YES", ""), col("email", "varchar(160)", "YES", "UNI"), col("name", "text", "YES", "")];
        assert_eq!(key_columns(&nullable, &n), [0, 1, 2]);
        let composite = [col("a", "int", "NO", "PRI"), col("b", "int", "NO", "PRI")];
        assert_eq!(key_columns(&composite, &names(&["b", "a"])), [1, 0]);
    }

    #[test]
    fn float_table_without_key_matches_every_column_null_safe() {
        // The pitfall: no key and a FLOAT value. The match uses every column with <=>, and the caller
        // counts the matches first, so 0 becomes a warning instead of a silent no-op.
        let info = [col("x", "float", "YES", ""), col("label", "varchar(10)", "YES", "")];
        let n = names(&["x", "label"]);
        let r = row(&[Some("1.1"), None]);
        let count = count_row("d", "t", &info, &n, &r).unwrap();
        assert_eq!(count.sql, "SELECT COUNT(*) FROM `d`.`t` WHERE `x` <=> ? AND `label` <=> NULL");
        assert_eq!(count.params, ["1.1"]);
        let update = update_row("d", "t", &info, &n, &r, &[(1, Some("new".into()))]).unwrap();
        assert_eq!(update.sql, "UPDATE `d`.`t` SET `label` = ? WHERE `x` <=> ? AND `label` <=> NULL LIMIT 1");
        assert_eq!(update.params, ["new", "1.1"]);
        assert_eq!(update.display, "UPDATE `d`.`t` SET `label` = 'new' WHERE `x` <=> '1.1' AND `label` <=> NULL LIMIT 1");
    }

    #[test]
    fn set_null_and_quotes() {
        let info = [col("id", "int", "NO", "PRI"), col("name", "varchar(20)", "YES", "")];
        let n = names(&["id", "name"]);
        let q = update_row("d", "t", &info, &n, &row(&[Some("3"), Some("x")]), &[(1, None)]).unwrap();
        assert_eq!(q.display, "UPDATE `d`.`t` SET `name` = NULL WHERE `id` <=> '3' LIMIT 1");
        let q = update_row("d", "t", &info, &n, &row(&[Some("3"), None]), &[(1, Some(r"it's \ok".into()))]).unwrap();
        assert_eq!(q.params, [r"it's \ok", "3"]);
        assert_eq!(q.display, r"UPDATE `d`.`t` SET `name` = 'it''s \\ok' WHERE `id` <=> '3' LIMIT 1");
    }

    #[test]
    fn binary_values_are_hex_literals() {
        let info = [col("h", "varbinary(16)", "YES", ""), col("t", "varchar(16)", "YES", "")];
        let n = names(&["h", "t"]);
        let q = delete_row("d", "t", &info, &n, &row(&[Some("0xA1B2"), Some("0xA1B2")])).unwrap();
        assert_eq!(q.sql, "DELETE FROM `d`.`t` WHERE `h` <=> 0xA1B2 AND `t` <=> ? LIMIT 1");
        assert_eq!(q.params, ["0xA1B2"]);
        assert!(!is_hex("0x") && !is_hex("0xABC") && !is_hex("0xZZ") && is_hex("0X00ff"));
    }

    #[test]
    fn insert_uses_table_order_and_skips_empty_fields() {
        let info = [col("id", "int", "NO", "PRI"), col("name", "varchar(20)", "NO", ""), col("data", "blob", "YES", "")];
        let q = insert_row("d", "t", &info, &[("data".into(), "0x00FF".into()), ("name".into(), "Anna".into())]);
        assert_eq!(q.sql, "INSERT INTO `d`.`t` (`name`, `data`) VALUES (?, 0x00FF)");
        assert_eq!(q.display, "INSERT INTO `d`.`t` (`name`, `data`) VALUES ('Anna', 0x00FF)");
        assert_eq!(insert_row("d", "t", &info, &[]).sql, "INSERT INTO `d`.`t` () VALUES ()");
    }

    #[test]
    fn wrong_row_shape_is_an_error() {
        let info = [col("id", "int", "NO", "PRI")];
        assert!(count_row("d", "t", &info, &names(&["id"]), &row(&[Some("1"), Some("2")])).is_err());
    }

    #[test]
    fn copy_rows_as_insert() {
        let n = names(&["id", "name", "price", "raw"]);
        let rows = vec![row(&[Some("1"), Some("O'Brien"), Some("12.50"), Some("0xFF00")]), row(&[Some("2"), None, Some("n/a"), Some("x")])];
        assert_eq!(
            rows_as_insert("products", &n, &[true, false, true, false], &[false, false, false, true], &rows),
            "INSERT INTO `products` (`id`, `name`, `price`, `raw`) VALUES (1, 'O''Brien', 12.50, 0xFF00);\n\
             INSERT INTO `products` (`id`, `name`, `price`, `raw`) VALUES (2, NULL, 'n/a', 'x');"
        );
    }

    #[test]
    fn column_info_derives_flags() {
        let c = ColumnInfo::new("id", "int(10) unsigned", "NO", "PRI", None, "auto_increment", "", "");
        assert!(c.numeric && c.auto_increment && !c.required && !c.nullable);
        let c = ColumnInfo::new("sku", "varchar(20)", "NO", "UNI", None, "", "", "utf8mb4_general_ci");
        assert!(c.required && c.family == "text");
        let c = ColumnInfo::new("status", "enum('new','it''s')", "NO", "", Some("new"), "", "", "");
        assert!(!c.required);
        assert_eq!(c.enum_values, ["new", "it's"]);
        assert!(ColumnInfo::new("b", "blob", "YES", "", None, "", "", "").binary);
    }
}
