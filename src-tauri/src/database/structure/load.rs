//! A table as the Structure tab shows it, put together from the server's answers: the table's status,
//! SHOW FULL COLUMNS, SHOW INDEX, SHOW CREATE TABLE and its foreign keys.

use crate::database::commands::data::format::byte_size;
use crate::database::commands::data::run::Grid;
use serde::Serialize;

/// Type, engine, collation, estimated rows and size of one table (parameters: database, table).
pub const STATUS: &str = "SELECT TABLE_TYPE, IFNULL(ENGINE,''), IFNULL(TABLE_COLLATION,''), TABLE_ROWS, \
     IFNULL(DATA_LENGTH+INDEX_LENGTH,0) FROM information_schema.TABLES WHERE TABLE_SCHEMA = ? AND TABLE_NAME = ?";

/// The foreign keys of one table, a row per column (parameters: database, table).
pub const FOREIGN_KEYS: &str = "SELECT k.CONSTRAINT_NAME, k.COLUMN_NAME, k.REFERENCED_TABLE_SCHEMA, k.REFERENCED_TABLE_NAME, \
     k.REFERENCED_COLUMN_NAME, r.UPDATE_RULE, r.DELETE_RULE FROM information_schema.KEY_COLUMN_USAGE k \
     JOIN information_schema.REFERENTIAL_CONSTRAINTS r ON r.CONSTRAINT_SCHEMA = k.CONSTRAINT_SCHEMA \
     AND r.CONSTRAINT_NAME = k.CONSTRAINT_NAME AND r.TABLE_NAME = k.TABLE_NAME \
     WHERE k.TABLE_SCHEMA = ? AND k.TABLE_NAME = ? AND k.REFERENCED_TABLE_NAME IS NOT NULL \
     ORDER BY k.CONSTRAINT_NAME, k.ORDINAL_POSITION";

/// The column names of one table, in order (parameters: database, table).
pub const COLUMN_NAMES: &str =
    "SELECT COLUMN_NAME FROM information_schema.COLUMNS WHERE TABLE_SCHEMA = ? AND TABLE_NAME = ? ORDER BY ORDINAL_POSITION";

#[derive(Debug, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Column {
    pub name: String,
    #[serde(rename = "type")]
    pub sql_type: String,
    pub collation: String,
    pub nullable: bool,
    /// "PRI", "UNI", "MUL" or "".
    pub key: String,
    /// None: no default (or NULL); "" is an empty text default.
    pub default: Option<String>,
    pub extra: String,
    pub comment: String,
}

/// One index with its columns in index order.
#[derive(Debug, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Index {
    pub name: String,
    pub columns: Vec<String>,
    pub unique: bool,
    /// BTREE, HASH, FULLTEXT, SPATIAL.
    #[serde(rename = "type")]
    pub index_type: String,
}

/// One foreign key constraint (multi-column keys arrive as a row per column).
#[derive(Debug, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ForeignKey {
    pub name: String,
    pub columns: Vec<String>,
    pub ref_schema: String,
    pub ref_table: String,
    pub ref_columns: Vec<String>,
    pub on_update: String,
    pub on_delete: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TableStructure {
    pub view: bool,
    pub engine: String,
    pub collation: String,
    /// The server's estimate; None for views.
    pub rows: Option<u64>,
    pub bytes: u64,
    /// "48 KB"
    pub size: String,
    pub columns: Vec<Column>,
    pub indexes: Vec<Index>,
    pub foreign_keys: Vec<ForeignKey>,
    /// SHOW CREATE TABLE's statement (CREATE VIEW for a view).
    pub create_sql: String,
}

/// A cell by its column's name; None for NULL or a missing column.
fn cell<'a>(g: &Grid, row: &'a [Option<String>], name: &str) -> Option<&'a str> {
    let i = g.columns.iter().position(|c| c.eq_ignore_ascii_case(name))?;
    row.get(i)?.as_deref()
}

/// A cell by position; "" for NULL.
fn at(row: Option<&Vec<Option<String>>>, i: usize) -> String {
    row.and_then(|r| r.get(i).cloned().flatten()).unwrap_or_default()
}

/// SHOW FULL COLUMNS
pub fn columns(g: &Grid) -> Vec<Column> {
    g.rows
        .iter()
        .map(|r| {
            let s = |name| cell(g, r, name).unwrap_or_default().to_string();
            Column {
                name: s("Field"),
                sql_type: s("Type"),
                collation: s("Collation"),
                nullable: s("Null").eq_ignore_ascii_case("YES"),
                key: s("Key"),
                default: cell(g, r, "Default").map(str::to_string),
                extra: s("Extra"),
                comment: s("Comment"),
            }
        })
        .collect()
}

/// SHOW INDEX: a row per indexed column, grouped by index.
pub fn indexes(g: &Grid) -> Vec<Index> {
    let mut out: Vec<Index> = Vec::new();
    for r in &g.rows {
        let (Some(name), Some(column)) = (cell(g, r, "Key_name"), cell(g, r, "Column_name")) else { continue };
        match out.iter_mut().find(|i| i.name == name) {
            Some(i) => i.columns.push(column.into()),
            None => out.push(Index {
                name: name.into(),
                columns: vec![column.into()],
                unique: cell(g, r, "Non_unique") == Some("0"),
                index_type: cell(g, r, "Index_type").unwrap_or_default().into(),
            }),
        }
    }
    out
}

/// `FOREIGN_KEYS`, grouped by constraint.
pub fn foreign_keys(g: &Grid) -> Vec<ForeignKey> {
    let mut out: Vec<ForeignKey> = Vec::new();
    for r in &g.rows {
        let v = |i| at(Some(r), i);
        match out.iter_mut().find(|f| f.name == v(0)) {
            Some(f) => {
                f.columns.push(v(1));
                f.ref_columns.push(v(4));
            }
            None => out.push(ForeignKey {
                name: v(0),
                columns: vec![v(1)],
                ref_schema: v(2),
                ref_table: v(3),
                ref_columns: vec![v(4)],
                on_update: v(5),
                on_delete: v(6),
            }),
        }
    }
    out
}

/// The five answers as one table.
pub fn structure(status: &Grid, columns_g: &Grid, indexes_g: &Grid, create: &Grid, fks: &Grid) -> TableStructure {
    let s = status.rows.first();
    let bytes = at(s, 4).trim().parse().unwrap_or(0);
    TableStructure {
        view: at(s, 0) == "VIEW",
        engine: at(s, 1),
        collation: at(s, 2),
        rows: at(s, 3).trim().parse().ok(),
        bytes,
        size: byte_size(bytes),
        columns: columns(columns_g),
        indexes: indexes(indexes_g),
        foreign_keys: foreign_keys(fks),
        create_sql: at(create.rows.first(), 1),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn grid(columns: &[&str], rows: &[&[Option<&str>]]) -> Grid {
        Grid {
            columns: columns.iter().map(|c| c.to_string()).collect(),
            rows: rows.iter().map(|r| r.iter().map(|v| v.map(str::to_string)).collect()).collect(),
        }
    }

    #[test]
    fn full_columns_keep_null_and_empty_defaults_apart() {
        let g = grid(
            &["Field", "Type", "Collation", "Null", "Key", "Default", "Extra", "Privileges", "Comment"],
            &[
                &[Some("id"), Some("int(10) unsigned"), None, Some("NO"), Some("PRI"), None, Some("auto_increment"), Some("select"), Some("")],
                &[Some("status"), Some("enum('new','paid')"), Some("utf8mb4_hungarian_ci"), Some("NO"), Some("MUL"), Some("new"), Some(""), Some(""), Some("Order state")],
                &[Some("note"), Some("text"), Some("utf8mb4_hungarian_ci"), Some("YES"), Some(""), None, Some(""), Some(""), Some("")],
                &[Some("code"), Some("varchar(5)"), Some("utf8mb4_bin"), Some("NO"), Some("UNI"), Some(""), Some(""), Some(""), Some("")],
            ],
        );
        let c = columns(&g);
        assert_eq!(c.len(), 4);
        assert_eq!(
            c[0],
            Column {
                name: "id".into(),
                sql_type: "int(10) unsigned".into(),
                collation: String::new(),
                nullable: false,
                key: "PRI".into(),
                default: None,
                extra: "auto_increment".into(),
                comment: String::new(),
            }
        );
        assert_eq!((c[1].default.as_deref(), c[1].comment.as_str()), (Some("new"), "Order state"));
        assert!(c[2].nullable && c[2].default.is_none());
        assert_eq!(c[3].default.as_deref(), Some(""));
    }

    #[test]
    fn index_rows_are_grouped_in_column_order() {
        let g = grid(
            &["Table", "Non_unique", "Key_name", "Seq_in_index", "Column_name", "Collation", "Cardinality", "Sub_part", "Packed", "Null", "Index_type"],
            &[
                &[Some("t"), Some("0"), Some("PRIMARY"), Some("1"), Some("id"), Some("A"), Some("8"), None, None, Some(""), Some("BTREE")],
                &[Some("t"), Some("1"), Some("idx_two"), Some("1"), Some("status"), Some("A"), Some("2"), None, None, Some(""), Some("BTREE")],
                &[Some("t"), Some("1"), Some("idx_two"), Some("2"), Some("ordered_at"), Some("A"), Some("8"), None, None, Some(""), Some("BTREE")],
                &[Some("t"), Some("0"), Some("uq_code"), Some("1"), Some("code"), Some("A"), Some("8"), None, None, Some(""), Some("BTREE")],
                &[Some("t"), Some("1"), Some("ft_note"), Some("1"), Some("note"), None, None, None, None, Some("YES"), Some("FULLTEXT")],
                &[Some("t"), Some("1"), Some("broken"), Some("1"), None, None, None, None, None, Some(""), Some("BTREE")],
            ],
        );
        let i = indexes(&g);
        let summary: Vec<(&str, Vec<&str>, bool, &str)> =
            i.iter().map(|x| (x.name.as_str(), x.columns.iter().map(String::as_str).collect(), x.unique, x.index_type.as_str())).collect();
        assert_eq!(
            summary,
            vec![
                ("PRIMARY", vec!["id"], true, "BTREE"),
                ("idx_two", vec!["status", "ordered_at"], false, "BTREE"),
                ("uq_code", vec!["code"], true, "BTREE"),
                ("ft_note", vec!["note"], false, "FULLTEXT"),
            ]
        );
    }

    #[test]
    fn foreign_key_rows_become_one_key_per_constraint() {
        let g = grid(
            &["CONSTRAINT_NAME", "COLUMN_NAME", "REFERENCED_TABLE_SCHEMA", "REFERENCED_TABLE_NAME", "REFERENCED_COLUMN_NAME", "UPDATE_RULE", "DELETE_RULE"],
            &[
                &[Some("fk_item_order"), Some("order_id"), Some("shop"), Some("orders"), Some("id"), Some("RESTRICT"), Some("CASCADE")],
                &[Some("fk_two"), Some("a"), Some("other"), Some("pairs"), Some("x"), Some("CASCADE"), Some("SET NULL")],
                &[Some("fk_two"), Some("b"), Some("other"), Some("pairs"), Some("y"), Some("CASCADE"), Some("SET NULL")],
            ],
        );
        let f = foreign_keys(&g);
        assert_eq!(f.len(), 2);
        assert_eq!(
            f[0],
            ForeignKey {
                name: "fk_item_order".into(),
                columns: vec!["order_id".into()],
                ref_schema: "shop".into(),
                ref_table: "orders".into(),
                ref_columns: vec!["id".into()],
                on_update: "RESTRICT".into(),
                on_delete: "CASCADE".into(),
            }
        );
        assert_eq!((f[1].columns.clone(), f[1].ref_columns.clone()), (vec!["a".to_string(), "b".into()], vec!["x".to_string(), "y".into()]));
        assert_eq!((f[1].ref_schema.as_str(), f[1].on_delete.as_str()), ("other", "SET NULL"));
    }

    #[test]
    fn status_and_create_statement() {
        let status = grid(&["TABLE_TYPE", "e", "c", "TABLE_ROWS", "s"], &[&[Some("BASE TABLE"), Some("InnoDB"), Some("utf8mb4_hungarian_ci"), Some("8"), Some("49152")]]);
        let create = grid(&["Table", "Create Table"], &[&[Some("orders"), Some("CREATE TABLE `orders` (\n  `id` int(10)\n)")]]);
        let empty = Grid::default();
        let t = structure(&status, &empty, &empty, &create, &empty);
        assert!(!t.view);
        assert_eq!((t.engine.as_str(), t.collation.as_str(), t.rows, t.bytes, t.size.as_str()), ("InnoDB", "utf8mb4_hungarian_ci", Some(8), 49152, "48 KB"));
        assert_eq!(t.create_sql, "CREATE TABLE `orders` (\n  `id` int(10)\n)");

        let view = grid(&["TABLE_TYPE", "e", "c", "TABLE_ROWS", "s"], &[&[Some("VIEW"), Some(""), Some(""), None, Some("0")]]);
        let create = grid(&["View", "Create View", "character_set_client", "collation_connection"], &[&[Some("v"), Some("CREATE VIEW `v` AS select 1"), Some("utf8mb4"), Some("utf8mb4_general_ci")]]);
        let v = structure(&view, &empty, &empty, &create, &empty);
        assert!(v.view && v.rows.is_none() && v.engine.is_empty());
        assert_eq!(v.create_sql, "CREATE VIEW `v` AS select 1");

        // A table that is gone: nothing, but no panic.
        let gone = structure(&empty, &empty, &empty, &empty, &empty);
        assert!(gone.columns.is_empty() && gone.create_sql.is_empty() && gone.rows.is_none());
    }
}
