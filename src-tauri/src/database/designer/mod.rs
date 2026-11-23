//! The Designer's picture of a database (port of the macOS DesignerData): every table and view with its
//! columns, and every foreign key, read in one round trip. `relation` builds the SQL and the warnings of
//! the relation sheet, `positions` keeps where the cards were dragged. Pure functions, unit-tested.

pub mod positions;
pub mod relation;

use serde::Serialize;
use std::collections::{HashMap, HashSet};

/// A column as the Designer shows it.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Column {
    pub name: String,
    #[serde(rename = "type")]
    pub ty: String,
    /// PRI, UNI, MUL or empty.
    pub key: String,
    pub nullable: bool,
}

/// A table or view card.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Table {
    pub name: String,
    pub is_view: bool,
    pub engine: String,
    pub columns: Vec<Column>,
}

impl Table {
    pub fn column(&self, name: &str) -> Option<&Column> {
        self.columns.iter().find(|c| c.name == name)
    }
}

/// One foreign key constraint: `table(columns)` → `ref_table(ref_columns)`. Composite keys have several column pairs.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Relation {
    pub name: String,
    pub table: String,
    pub columns: Vec<String>,
    pub ref_schema: String,
    pub ref_table: String,
    pub ref_columns: Vec<String>,
    pub on_delete: String,
    pub on_update: String,
}

/// Everything the Designer draws for one database.
#[derive(Debug, Clone, PartialEq, Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct Schema {
    pub database: String,
    pub tables: Vec<Table>,
    /// Relations inside this database (drawn as lines).
    pub relations: Vec<Relation>,
    /// Foreign keys that point into another database: shown on the column, not as lines.
    pub external: Vec<Relation>,
}

impl Schema {
    pub fn table(&self, name: &str) -> Option<&Table> {
        self.tables.iter().find(|t| t.name == name)
    }
}

/// Tables, columns and foreign keys of `db`: three statements sent as one round trip. The name goes in as a
/// hex literal, which reads the same in every sql_mode (NO_BACKSLASH_ESCAPES changes what `\'` means).
/// Each information_schema table gets the name as a constant: a join condition alone makes MariaDB read
/// the foreign keys of every database.
pub fn load_sql(db: &str) -> String {
    let s = format!("X'{}'", hex::encode(db));
    format!(
        "SELECT TABLE_NAME, TABLE_TYPE, IFNULL(ENGINE,'') FROM information_schema.TABLES \
         WHERE TABLE_SCHEMA = {s} ORDER BY TABLE_NAME;\n\
         SELECT TABLE_NAME, COLUMN_NAME, COLUMN_TYPE, COLUMN_KEY, IS_NULLABLE FROM information_schema.COLUMNS \
         WHERE TABLE_SCHEMA = {s} ORDER BY TABLE_NAME, ORDINAL_POSITION;\n\
         SELECT k.TABLE_NAME, k.CONSTRAINT_NAME, k.COLUMN_NAME, k.REFERENCED_TABLE_SCHEMA, k.REFERENCED_TABLE_NAME, \
         k.REFERENCED_COLUMN_NAME, r.UPDATE_RULE, r.DELETE_RULE FROM information_schema.KEY_COLUMN_USAGE k \
         JOIN information_schema.REFERENTIAL_CONSTRAINTS r ON r.CONSTRAINT_SCHEMA = k.CONSTRAINT_SCHEMA \
         AND r.CONSTRAINT_NAME = k.CONSTRAINT_NAME AND r.TABLE_NAME = k.TABLE_NAME \
         WHERE k.TABLE_SCHEMA = {s} AND r.CONSTRAINT_SCHEMA = {s} AND k.REFERENCED_TABLE_NAME IS NOT NULL \
         ORDER BY k.TABLE_NAME, k.CONSTRAINT_NAME, k.ORDINAL_POSITION"
    )
}

/// The schema from the three result sets of `load_sql` (cells as text, NULL as "").
pub fn assemble(db: &str, tables: &[Vec<String>], columns: &[Vec<String>], keys: &[Vec<String>]) -> Schema {
    fn cell(row: &[String], i: usize) -> String {
        row.get(i).cloned().unwrap_or_default()
    }
    let mut by_table: HashMap<String, Vec<Column>> = HashMap::new();
    for r in columns {
        by_table.entry(cell(r, 0)).or_default().push(Column {
            name: cell(r, 1),
            ty: cell(r, 2),
            key: cell(r, 3),
            nullable: cell(r, 4).eq_ignore_ascii_case("YES"),
        });
    }
    // A sequence never takes part in a foreign key, so it is no card; information_schema's tables are
    // SYSTEM VIEWs: views, which no relation can be drawn from or to either.
    let tables: Vec<Table> = tables
        .iter()
        .filter(|r| cell(r, 1) != "SEQUENCE")
        .map(|r| Table {
            name: cell(r, 0),
            is_view: matches!(cell(r, 1).as_str(), "VIEW" | "SYSTEM VIEW"),
            engine: cell(r, 2),
            columns: by_table.remove(&cell(r, 0)).unwrap_or_default(),
        })
        .collect();

    // The key columns arrive ordered by table, constraint and position: one group per constraint.
    let (mut relations, mut external) = (Vec::new(), Vec::new());
    for group in keys.chunk_by(|a, b| cell(a, 0) == cell(b, 0) && cell(a, 1) == cell(b, 1)) {
        let first = &group[0];
        let r = Relation {
            name: cell(first, 1),
            table: cell(first, 0),
            columns: group.iter().map(|k| cell(k, 2)).collect(),
            ref_schema: cell(first, 3),
            ref_table: cell(first, 4),
            ref_columns: group.iter().map(|k| cell(k, 5)).collect(),
            on_update: cell(first, 6),
            on_delete: cell(first, 7),
        };
        if r.ref_schema == db {
            relations.push(r);
        } else {
            external.push(r);
        }
    }
    // A relation to a table that is not in the list (dropped meanwhile) cannot be drawn.
    let names: HashSet<&str> = tables.iter().map(|t| t.name.as_str()).collect();
    relations.retain(|r| names.contains(r.table.as_str()) && names.contains(r.ref_table.as_str()));
    Schema { database: db.to_string(), tables, relations, external }
}

#[cfg(test)]
mod tests {
    use super::*;

    pub fn row(cells: &[&str]) -> Vec<String> {
        cells.iter().map(|c| c.to_string()).collect()
    }

    /// customers ← orders (customer_id), categories → categories (self), a view, a key into another database.
    pub fn demo() -> Schema {
        assemble(
            "shop",
            &[
                row(&["categories", "BASE TABLE", "InnoDB"]),
                row(&["customers", "BASE TABLE", "InnoDB"]),
                row(&["order_totals", "VIEW", ""]),
                row(&["orders", "BASE TABLE", "InnoDB"]),
                row(&["logs", "BASE TABLE", "MyISAM"]),
            ],
            &[
                row(&["categories", "id", "int(10) unsigned", "PRI", "NO"]),
                row(&["categories", "parent_id", "int(10) unsigned", "MUL", "YES"]),
                row(&["customers", "id", "int(10) unsigned", "PRI", "NO"]),
                row(&["customers", "email", "varchar(160)", "UNI", "NO"]),
                row(&["customers", "note", "text", "", "YES"]),
                row(&["order_totals", "order_id", "int(10) unsigned", "", "NO"]),
                row(&["orders", "id", "int(10) unsigned", "PRI", "NO"]),
                row(&["orders", "customer_id", "int(10) unsigned", "MUL", "NO"]),
                row(&["orders", "total", "decimal(10,2)", "", "NO"]),
                row(&["orders", "user_id", "bigint(20)", "MUL", "YES"]),
                row(&["logs", "order_id", "int(10) unsigned", "", "YES"]),
            ],
            &[
                row(&["categories", "fk_parent", "parent_id", "shop", "categories", "id", "CASCADE", "SET NULL"]),
                row(&["orders", "fk_orders_customer", "customer_id", "shop", "customers", "id", "RESTRICT", "CASCADE"]),
                row(&["orders", "fk_orders_user", "user_id", "auth", "users", "id", "RESTRICT", "RESTRICT"]),
            ],
        )
    }

    #[test]
    fn assembles_tables_views_and_columns_in_order() {
        let s = demo();
        assert_eq!(s.database, "shop");
        let names: Vec<&str> = s.tables.iter().map(|t| t.name.as_str()).collect();
        assert_eq!(names, ["categories", "customers", "order_totals", "orders", "logs"]);
        let v = s.table("order_totals").unwrap();
        assert!(v.is_view && v.engine.is_empty());
        let o = s.table("orders").unwrap();
        assert!(!o.is_view);
        assert_eq!(o.columns.iter().map(|c| c.name.as_str()).collect::<Vec<_>>(), ["id", "customer_id", "total", "user_id"]);
        let c = o.column("customer_id").unwrap();
        assert_eq!((c.ty.as_str(), c.key.as_str(), c.nullable), ("int(10) unsigned", "MUL", false));
        assert!(o.column("user_id").unwrap().nullable);
    }

    #[test]
    fn splits_relations_from_keys_into_other_databases() {
        let s = demo();
        assert_eq!(s.relations.len(), 2);
        let r = s.relations.iter().find(|r| r.name == "fk_orders_customer").unwrap();
        assert_eq!((r.table.as_str(), r.ref_table.as_str()), ("orders", "customers"));
        assert_eq!((r.on_delete.as_str(), r.on_update.as_str()), ("CASCADE", "RESTRICT"));
        let own = s.relations.iter().find(|r| r.name == "fk_parent").unwrap();
        assert_eq!(own.table, own.ref_table);
        assert_eq!(s.external.len(), 1);
        assert_eq!(s.external[0].ref_schema, "auth");
    }

    #[test]
    fn groups_composite_keys_and_drops_relations_to_missing_tables() {
        let s = assemble(
            "d",
            &[row(&["a", "BASE TABLE", "InnoDB"]), row(&["b", "BASE TABLE", "InnoDB"])],
            &[],
            &[
                row(&["a", "fk_ab", "x", "d", "b", "x", "RESTRICT", "RESTRICT"]),
                row(&["a", "fk_ab", "y", "d", "b", "y", "RESTRICT", "RESTRICT"]),
                row(&["a", "fk_gone", "z", "d", "dropped", "z", "RESTRICT", "RESTRICT"]),
                row(&["b", "fk_ab", "x", "d", "a", "x", "NO ACTION", "CASCADE"]),
            ],
        );
        assert_eq!(s.relations.len(), 2, "same constraint name on another table is another relation");
        assert_eq!(s.relations[0].columns, ["x", "y"]);
        assert_eq!(s.relations[0].ref_columns, ["x", "y"]);
        assert_eq!(s.relations[1].table, "b");
        assert!(s.tables.iter().all(|t| t.columns.is_empty()));
    }

    #[test]
    fn system_views_take_no_relations_and_sequences_are_no_cards() {
        let s = assemble(
            "d",
            &[
                row(&["COLUMNS", "SYSTEM VIEW", "Aria"]),
                row(&["history", "SYSTEM VERSIONED", "InnoDB"]),
                row(&["ids", "SEQUENCE", "InnoDB"]),
                row(&["t", "BASE TABLE", "InnoDB"]),
            ],
            &[row(&["ids", "next_not_cached_value", "bigint(21)", "", "NO"])],
            &[],
        );
        assert_eq!(s.tables.iter().map(|t| t.name.as_str()).collect::<Vec<_>>(), ["COLUMNS", "history", "t"]);
        assert!(s.table("COLUMNS").unwrap().is_view, "information_schema's tables can't be linked");
        assert!(!s.table("history").unwrap().is_view, "a system-versioned table is a table");
    }

    #[test]
    fn tolerates_short_rows() {
        let s = assemble("d", &[row(&["t"])], &[row(&["t", "c"])], &[row(&["t"])]);
        let t = s.table("t").unwrap();
        assert!(!t.is_view);
        assert_eq!(t.columns[0].ty, "");
        assert!(!t.columns[0].nullable);
        assert!(s.relations.is_empty());
        assert_eq!(s.external.len(), 1, "a key with no referenced schema is not drawn");
    }

    #[test]
    fn load_sql_reads_everything_in_one_round_trip() {
        let sql = load_sql("o'b\\");
        assert_eq!(sql.matches("SELECT ").count(), 3);
        assert_eq!(sql.matches(";\n").count(), 2);
        assert_eq!(sql.matches("TABLE_SCHEMA = X'6f27625c'").count(), 3, "quotes and backslashes can't break out");
        assert_eq!(sql.matches("X'6f27625c'").count(), 4);
        assert!(sql.contains("r.CONSTRAINT_SCHEMA = X'6f27625c'"), "only this database's foreign keys are read");
        assert!(sql.contains("REFERENTIAL_CONSTRAINTS"));
    }
}
