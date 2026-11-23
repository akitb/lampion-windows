//! A relation drawn on the diagram (port of DesignerRelationSheet): the exact SQL, the problems MariaDB
//! will most likely refuse (shown before anything is sent), a free constraint name, and the check that
//! the foreign key really exists afterwards.

use super::Schema;
use crate::database::client::{ident, qualified};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;

/// ON DELETE / ON UPDATE choices, as the sheet offers them.
pub const RULES: [&str; 4] = ["RESTRICT", "CASCADE", "SET NULL", "NO ACTION"];

/// `table.column` becomes a foreign key that refers to `ref_table.ref_column`.
#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NewRelation {
    pub table: String,
    pub column: String,
    pub ref_table: String,
    pub ref_column: String,
}

fn rule(value: &str) -> Result<&'static str, String> {
    RULES.iter().copied().find(|r| *r == value).ok_or_else(|| format!("“{value}” is not a referential action."))
}

/// `ALTER TABLE db.t ADD CONSTRAINT fk FOREIGN KEY (col) REFERENCES db.r (rcol) ON DELETE … ON UPDATE …`;
/// without a name MariaDB picks one. The rules are checked, since they are written into the SQL as words.
pub fn add_sql(db: &str, r: &NewRelation, on_delete: &str, on_update: &str, name: &str) -> Result<String, String> {
    let (on_delete, on_update) = (rule(on_delete)?, rule(on_update)?);
    let name = name.trim();
    let add = if name.is_empty() { "ADD".to_string() } else { format!("ADD CONSTRAINT {}", ident(name)) };
    Ok(format!(
        "ALTER TABLE {}\n  {add} FOREIGN KEY ({})\n  REFERENCES {} ({})\n  ON DELETE {on_delete} ON UPDATE {on_update};",
        qualified(db, &r.table),
        ident(&r.column),
        qualified(db, &r.ref_table),
        ident(&r.ref_column),
    ))
}

/// `ALTER TABLE db.t DROP FOREIGN KEY fk;` (the columns, their data and the index stay).
pub fn drop_sql(db: &str, table: &str, name: &str) -> String {
    format!("ALTER TABLE {} DROP FOREIGN KEY {};", qualified(db, table), ident(name))
}

const INTEGERS: [&str; 6] = ["int", "tinyint", "smallint", "mediumint", "bigint", "integer"];

/// The part of a type that must match for a foreign key: integer kind and signedness, or the type itself.
/// Display widths (`int(10)`) and string lengths may differ, and InnoDB pairs CHAR with VARCHAR and
/// BINARY with VARBINARY.
pub fn fk_comparable(raw: &str) -> String {
    let t = raw.trim().to_lowercase();
    if let Some(open) = t.find('(') {
        let head = &t[..open];
        if INTEGERS.contains(&head) {
            let kind = if head == "integer" { "int" } else { head };
            return format!("{kind}{}", if t.contains("unsigned") { " unsigned" } else { "" });
        }
        if head == "char" || head == "varchar" {
            return "char".into();
        }
        if head == "binary" || head == "varbinary" {
            return "binary".into();
        }
    }
    t.replace("integer", "int")
}

/// Problems MariaDB will most likely refuse, found before anything is sent.
pub fn warnings(s: &Schema, r: &NewRelation, on_delete: &str, on_update: &str) -> Vec<String> {
    let child = s.table(&r.table);
    let parent = s.table(&r.ref_table);
    let fk = child.and_then(|t| t.column(&r.column));
    let referenced = parent.and_then(|t| t.column(&r.ref_column));
    let mut w = Vec::new();
    if let (Some(a), Some(b)) = (fk, referenced) {
        if fk_comparable(&a.ty) != fk_comparable(&b.ty) {
            w.push(format!("The column types differ ({} and {}). MariaDB will most likely refuse.", a.ty, b.ty));
        }
    }
    if referenced.is_some_and(|b| b.key.is_empty()) {
        w.push(format!(
            "“{}.{}” has no index. The referenced column needs one (a primary key works).",
            r.ref_table, r.ref_column
        ));
    }
    if fk.is_some_and(|a| !a.nullable) && (on_delete == "SET NULL" || on_update == "SET NULL") {
        w.push(format!("SET NULL needs “{}” to allow NULL, and it is NOT NULL.", r.column));
    }
    if let Some(t) = [child, parent].into_iter().flatten().find(|t| !t.engine.is_empty() && !t.engine.eq_ignore_ascii_case("InnoDB")) {
        w.push(format!("“{}” uses {}, which ignores foreign keys. Change it to InnoDB in Table Options first.", t.name, t.engine));
    }
    if let Some(e) = s.relations.iter().find(|e| {
        e.table == r.table && e.columns == [r.column.as_str()] && e.ref_table == r.ref_table && e.ref_columns == [r.ref_column.as_str()]
    }) {
        w.push(format!("This relation already exists as “{}”.", e.name));
    }
    w
}

/// A free constraint name such as `fk_orders_customer_id` (InnoDB wants them unique per database).
pub fn suggested_name(s: &Schema, table: &str, column: &str) -> String {
    let base: String = format!("fk_{table}_{column}").chars().take(60).collect();
    let used: HashSet<String> = s.relations.iter().chain(&s.external).map(|r| r.name.to_lowercase()).collect();
    let mut name = base.clone();
    let mut n = 2;
    while used.contains(&name.to_lowercase()) {
        name = format!("{base}_{n}");
        n += 1;
    }
    name
}

/// Whether the freshly read schema has the relation that was just added.
pub fn created(s: &Schema, r: &NewRelation) -> bool {
    s.relations.iter().any(|e| e.table == r.table && e.columns.contains(&r.column) && e.ref_table == r.ref_table)
}

#[cfg(test)]
mod tests {
    use super::super::tests::demo;
    use super::*;

    fn rel(table: &str, column: &str, ref_table: &str, ref_column: &str) -> NewRelation {
        NewRelation { table: table.into(), column: column.into(), ref_table: ref_table.into(), ref_column: ref_column.into() }
    }

    #[test]
    fn add_sql_is_qualified_and_quoted() {
        let r = rel("orders", "customer_id", "customers", "id");
        assert_eq!(
            add_sql("shop", &r, "CASCADE", "RESTRICT", " fk_orders_customer_id ").unwrap(),
            "ALTER TABLE `shop`.`orders`\n  ADD CONSTRAINT `fk_orders_customer_id` FOREIGN KEY (`customer_id`)\n  \
             REFERENCES `shop`.`customers` (`id`)\n  ON DELETE CASCADE ON UPDATE RESTRICT;"
        );
        let odd = rel("a`b", "c", "d", "e`");
        let sql = add_sql("x", &odd, "SET NULL", "NO ACTION", "").unwrap();
        assert!(sql.starts_with("ALTER TABLE `x`.`a``b`\n  ADD FOREIGN KEY (`c`)"), "no name: MariaDB names it");
        assert!(sql.contains("(`e```)") && sql.ends_with("ON DELETE SET NULL ON UPDATE NO ACTION;"));
    }

    #[test]
    fn add_sql_refuses_unknown_rules() {
        let r = rel("a", "b", "c", "d");
        assert!(add_sql("x", &r, "CASCADE; DROP TABLE a", "RESTRICT", "").is_err());
        assert!(add_sql("x", &r, "RESTRICT", "cascade", "").is_err());
    }

    #[test]
    fn drop_sql_names_the_constraint() {
        assert_eq!(drop_sql("shop", "orders", "fk`1"), "ALTER TABLE `shop`.`orders` DROP FOREIGN KEY `fk``1`;");
    }

    #[test]
    fn comparable_types_ignore_widths_and_lengths() {
        assert_eq!(fk_comparable("int(10) unsigned"), fk_comparable("int unsigned"));
        assert_eq!(fk_comparable("INT(11)"), fk_comparable("int"));
        assert_eq!(fk_comparable("integer(11)"), fk_comparable("int"));
        assert_ne!(fk_comparable("int(10) unsigned"), fk_comparable("int(11)"));
        assert_ne!(fk_comparable("int unsigned"), fk_comparable("bigint unsigned"));
        assert_eq!(fk_comparable("varchar(64)"), fk_comparable("varchar(255)"));
        assert_eq!(fk_comparable("varchar(64)"), fk_comparable("CHAR(36)"), "InnoDB links CHAR and VARCHAR");
        assert_eq!(fk_comparable("binary(16)"), fk_comparable("varbinary(255)"));
        assert_ne!(fk_comparable("char(36)"), fk_comparable("binary(36)"));
        assert_ne!(fk_comparable("decimal(10,2)"), fk_comparable("decimal(12,2)"));
    }

    #[test]
    fn no_warnings_for_a_good_relation() {
        assert!(warnings(&demo(), &rel("orders", "id", "customers", "id"), "CASCADE", "CASCADE").is_empty());
    }

    #[test]
    fn warns_about_type_mismatch() {
        let w = warnings(&demo(), &rel("orders", "total", "customers", "id"), "RESTRICT", "RESTRICT");
        assert_eq!(w, ["The column types differ (decimal(10,2) and int(10) unsigned). MariaDB will most likely refuse."]);
    }

    #[test]
    fn warns_about_a_target_without_index() {
        let w = warnings(&demo(), &rel("orders", "id", "customers", "note"), "RESTRICT", "RESTRICT");
        assert!(w.contains(&"“customers.note” has no index. The referenced column needs one (a primary key works).".to_string()));
    }

    #[test]
    fn warns_about_set_null_on_not_null() {
        let r = rel("orders", "customer_id", "customers", "id");
        let w = warnings(&demo(), &r, "RESTRICT", "SET NULL");
        assert!(w.contains(&"SET NULL needs “customer_id” to allow NULL, and it is NOT NULL.".to_string()));
        let nullable = rel("categories", "parent_id", "customers", "id");
        assert!(warnings(&demo(), &nullable, "SET NULL", "SET NULL").iter().all(|w| !w.starts_with("SET NULL")));
    }

    #[test]
    fn warns_about_a_table_that_is_not_innodb() {
        let w = warnings(&demo(), &rel("logs", "order_id", "orders", "id"), "RESTRICT", "RESTRICT");
        assert_eq!(w, ["“logs” uses MyISAM, which ignores foreign keys. Change it to InnoDB in Table Options first."]);
        let signed = warnings(&demo(), &rel("orders", "user_id", "orders", "id"), "RESTRICT", "RESTRICT");
        assert_eq!(signed.len(), 1, "bigint and int unsigned differ: {signed:?}");
    }

    #[test]
    fn warns_about_an_existing_relation() {
        let w = warnings(&demo(), &rel("orders", "customer_id", "customers", "id"), "RESTRICT", "RESTRICT");
        assert_eq!(w, ["This relation already exists as “fk_orders_customer”."]);
    }

    #[test]
    fn suggests_a_free_name() {
        let s = demo();
        assert_eq!(suggested_name(&s, "orders", "customer_id"), "fk_orders_customer_id");
        assert_eq!(suggested_name(&s, "orders", "customer"), "fk_orders_customer_2", "fk_orders_customer is taken");
        let mut s2 = s.clone();
        s2.external[0].name = "FK_ORDERS_CUSTOMER_2".into();
        assert_eq!(suggested_name(&s2, "orders", "customer"), "fk_orders_customer_3", "names compare without case");
        let long = suggested_name(&s, &"t".repeat(50), &"c".repeat(50));
        assert_eq!(long.chars().count(), 60, "MariaDB allows 64 characters, so a counter still fits");
    }

    #[test]
    fn checks_that_the_relation_was_created() {
        let s = demo();
        assert!(created(&s, &rel("orders", "customer_id", "customers", "id")));
        assert!(!created(&s, &rel("logs", "order_id", "orders", "id")));
    }
}
