//! Every statement of the Structure tab and its sheets, built as text so the SQL console shows exactly
//! what ran: identifiers in backticks (backtick doubled), values quoted. Engines, collations, rules and
//! operations go into the SQL as bare words, so they are checked here first.

use crate::database::client::{ident, literal, qualified};
use crate::database::commands::data::format::family;
use once_cell::sync::Lazy;
use regex::Regex;
use serde::{Deserialize, Serialize};

/// The engines the sheets offer (Table Options adds the table's own when it is another one).
pub const ENGINES: [&str; 4] = ["InnoDB", "MyISAM", "Aria", "MEMORY"];
/// What a foreign key does when the referenced row changes.
pub const RULES: [&str; 4] = ["RESTRICT", "CASCADE", "SET NULL", "NO ACTION"];
/// Table maintenance (Table Options).
pub const OPERATIONS: [&str; 4] = ["OPTIMIZE", "ANALYZE", "CHECK", "REPAIR"];

const MODERN: &str = "utf8mb4_uca1400_ai_ci";
const COLLATIONS: [(&str, &str); 5] = [
    (MODERN, "Modern Unicode sorting, ignores case and accents. The MariaDB 11 default."),
    ("utf8mb4_unicode_ci", "Unicode sorting by an older standard. Also works on MySQL."),
    ("utf8mb4_general_ci", "Fast, simplified sorting. The classic default."),
    ("utf8mb4_hungarian_ci", "Hungarian alphabet: á, é, ő, ű and cs, sz, zs sort correctly."),
    ("utf8mb4_bin", "Compares bytes: case- and accent-sensitive."),
];

#[derive(Debug, Serialize)]
pub struct Collation {
    pub name: &'static str,
    pub detail: &'static str,
}

/// What the sheets' pickers offer on this server.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Choices {
    pub engines: [&'static str; 4],
    pub collations: Vec<Collation>,
    pub default_collation: &'static str,
    pub rules: [&'static str; 4],
}

/// uca1400 collations exist from MariaDB 10.10. An unknown version counts as a current MariaDB.
pub fn supports_uca1400(version: &str) -> bool {
    let digits = version.trim_start_matches(|c: char| !c.is_ascii_digit());
    let parts: Vec<u32> = digits
        .split('.')
        .take(2)
        .map_while(|p| p.chars().take_while(char::is_ascii_digit).collect::<String>().parse().ok())
        .collect();
    match parts[..] {
        [major, minor] => major > 10 || (major == 10 && minor >= 10),
        _ => true,
    }
}

/// `version` as the server reports it ("MariaDB 11.4.2"; empty when unknown).
pub fn choices(version: &str) -> Choices {
    let modern = supports_uca1400(version);
    Choices {
        engines: ENGINES,
        collations: COLLATIONS.iter().filter(|(n, _)| modern || *n != MODERN).map(|&(name, detail)| Collation { name, detail }).collect(),
        default_collation: if modern { MODERN } else { "utf8mb4_general_ci" },
        rules: RULES,
    }
}

// ---- names ----

static PLAIN: Lazy<Regex> = Lazy::new(|| Regex::new("^[A-Za-z0-9_]+$").unwrap());

/// What is wrong with a database or table name (`kind`: "Database" or "Table"), or None.
/// An empty name is no problem yet: the sheet just can't be confirmed.
pub fn name_problem(raw: &str, kind: &str, existing: &[String]) -> Option<String> {
    let name = raw.trim();
    if name.is_empty() {
        return None;
    }
    if name.chars().count() > 64 {
        return Some(format!("{kind} names can be at most 64 characters long."));
    }
    if name.contains(['/', '\\', '.']) {
        return Some("The name can't contain / \\ or a dot.".into());
    }
    if existing.iter().any(|e| e.to_lowercase() == name.to_lowercase()) {
        return Some(format!("A {} named “{name}” already exists.", kind.to_lowercase()));
    }
    None
}

/// Letters, digits and underscores only: no quoting needed in SQL and PHP.
pub fn is_plain(raw: &str) -> bool {
    PLAIN.is_match(raw.trim())
}

#[derive(Debug, Serialize, PartialEq)]
pub struct NameCheck {
    pub problem: Option<String>,
    pub plain: bool,
}

pub fn check_name(raw: &str, kind: &str, existing: &[String]) -> NameCheck {
    NameCheck { problem: name_problem(raw, kind, existing), plain: raw.trim().is_empty() || is_plain(raw) }
}

/// The trimmed name, or why it can't be used.
fn valid_name(raw: &str, kind: &str, existing: &[String]) -> Result<String, String> {
    let name = raw.trim();
    if name.is_empty() {
        return Err(format!("The {} needs a name.", kind.to_lowercase()));
    }
    name_problem(name, kind, existing).map_or(Ok(name.to_string()), Err)
}

/// An engine or collation, which goes into the SQL unquoted.
fn word<'a>(value: &'a str, what: &str) -> Result<&'a str, String> {
    if PLAIN.is_match(value) {
        Ok(value)
    } else {
        Err(format!("“{value}” is not a valid {what}."))
    }
}

/// `utf8mb4_hungarian_ci` → `utf8mb4`.
pub fn charset(collation: &str) -> &str {
    collation.split('_').next().unwrap_or("utf8mb4")
}

// ---- columns ----

static NUMBER: Lazy<Regex> = Lazy::new(|| Regex::new(r"^[+-]?(\d+\.?\d*|\.\d+)([eE][+-]?\d+)?$").unwrap());
static FUNCTION: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"(?i)^(current_timestamp|now|localtimestamp|current_date|curdate|current_time|curtime|uuid)(\(\d*\))?$").unwrap()
});

/// The base type, lowercase: `VARCHAR(20) BINARY` → `varchar`.
fn base_type(sql_type: &str) -> String {
    let t = sql_type.trim().to_ascii_lowercase();
    t.split(|c: char| c == '(' || c.is_whitespace()).next().unwrap_or("").to_string()
}

/// Types whose values are strings: a number typed as their default means that text (`007` stays `'007'`).
fn holds_text(sql_type: &str) -> bool {
    matches!(family(sql_type), "text" | "enumSet" | "binary")
}

/// The text typed into a "Default" field as SQL, or None for no default: NULL, TRUE/FALSE, quoted text,
/// an expression in parentheses and the time/uuid functions are used as written, numbers too unless the
/// column holds text; anything else is quoted.
pub fn default_clause(text: &str, sql_type: &str) -> Option<String> {
    let t = text.trim();
    if t.is_empty() {
        return None;
    }
    let keyword = ["NULL", "TRUE", "FALSE"].contains(&t.to_ascii_uppercase().as_str());
    let quoted = t.len() >= 2 && t.starts_with('\'') && t.ends_with('\'');
    let expression = t.len() >= 2 && t.starts_with('(') && t.ends_with(')');
    let number = NUMBER.is_match(t) && !holds_text(sql_type);
    if keyword || quoted || expression || number || FUNCTION.is_match(t) {
        Some(t.to_string())
    } else {
        Some(literal(t))
    }
}

/// A column's default as information_schema.COLUMNS writes it (SQL: literals quoted, `NULL` for none),
/// put the way the Default field holds it, so saving the column unchanged keeps the default exactly.
pub fn sheet_default(column_default: Option<&str>, sql_type: &str) -> String {
    match column_default.map(str::trim) {
        None | Some("NULL") | Some("") => String::new(),
        Some(v) if default_clause(v, sql_type).as_deref() == Some(v) => v.to_string(),
        Some(v) => format!("({v})"),
    }
}

/// `ON UPDATE CURRENT_TIMESTAMP` with the column's fractional seconds (MariaDB requires at least those).
fn on_update(sql_type: &str) -> String {
    let t = sql_type.trim().to_ascii_lowercase();
    let digits = ["timestamp(", "datetime("].iter().find_map(|p| t.strip_prefix(*p)).map(|r| r.chars().take_while(char::is_ascii_digit).collect::<String>());
    match digits.filter(|d| !d.is_empty() && d != "0") {
        Some(d) => format!("ON UPDATE CURRENT_TIMESTAMP({d})"),
        None => "ON UPDATE CURRENT_TIMESTAMP".into(),
    }
}

/// A column as the Add/Edit Column sheet and New Table edit it.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct ColumnDraft {
    pub name: String,
    #[serde(rename = "type")]
    pub sql_type: String,
    pub nullable: bool,
    /// See `default_clause`; empty means no default.
    pub default_text: String,
    pub auto_increment: bool,
    /// New Table only: the column is part of the primary key.
    pub primary_key: bool,
    /// ON UPDATE CURRENT_TIMESTAMP (with the type's precision).
    pub on_update_timestamp: bool,
    pub comment: String,
    /// Edit Column keeps what the sheet doesn't show: the column's collation (with its character set;
    /// written for text types only, so a new type without one drops it)…
    pub collation: String,
    /// …and INVISIBLE (left out of `SELECT *`).
    pub invisible: bool,
}

impl ColumnDraft {
    /// The collation clause, unless the type doesn't take one or already names one.
    fn collation_clause(&self) -> Option<String> {
        let t = self.sql_type.to_ascii_lowercase();
        let text = matches!(base_type(&t).as_str(), "char" | "varchar" | "tinytext" | "text" | "mediumtext" | "longtext" | "enum" | "set");
        let named = t.contains("collate") || t.contains("character set") || t.contains("charset");
        (text && !named && PLAIN.is_match(&self.collation)).then(|| format!("CHARACTER SET {} COLLATE {}", charset(&self.collation), self.collation))
    }

    /// Everything after the column name in a column definition.
    pub fn definition(&self) -> String {
        let mut parts = vec![self.sql_type.trim().to_string()];
        parts.extend(self.collation_clause());
        parts.push((if self.nullable { "NULL" } else { "NOT NULL" }).to_string());
        if self.auto_increment {
            parts.push("AUTO_INCREMENT".into());
        } else if let Some(d) = default_clause(&self.default_text, &self.sql_type) {
            parts.push(format!("DEFAULT {d}"));
        }
        if self.on_update_timestamp {
            parts.push(on_update(&self.sql_type));
        }
        if self.invisible {
            parts.push("INVISIBLE".into());
        }
        if !self.comment.is_empty() {
            parts.push(format!("COMMENT {}", literal(&self.comment)));
        }
        parts.join(" ")
    }

    /// `name` and the definition (the sheet's preview); None until both name and type are filled in.
    pub fn sql(&self) -> Option<String> {
        (!self.name.trim().is_empty() && !self.sql_type.trim().is_empty()).then(|| format!("{} {}", ident(self.name.trim()), self.definition()))
    }
}

/// Where a saved column goes.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum Position {
    /// At the end when adding, where it was when editing.
    #[default]
    Unchanged,
    First,
    After { column: String },
}

/// Adds a column (`replacing` None) or changes the column `replacing`.
pub fn save_column(db: &str, table: &str, column: &ColumnDraft, replacing: Option<&str>, position: &Position) -> Result<String, String> {
    let Some(definition) = column.sql() else {
        return Err("The column needs a name and a type.".into());
    };
    if !column.collation.is_empty() {
        word(&column.collation, "collation")?;
    }
    let what = match replacing {
        Some(old) => format!("CHANGE COLUMN {} ", ident(old)),
        None => "ADD COLUMN ".into(),
    };
    let place = match position {
        Position::Unchanged => String::new(),
        Position::First => " FIRST".into(),
        Position::After { column } => format!(" AFTER {}", ident(column)),
    };
    Ok(format!("ALTER TABLE {} {what}{definition}{place}", qualified(db, table)))
}

pub fn drop_column(db: &str, table: &str, column: &str) -> String {
    format!("ALTER TABLE {} DROP COLUMN {}", qualified(db, table), ident(column))
}

// ---- indexes ----

#[derive(Debug, Clone, Copy, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum IndexKind {
    Index,
    Unique,
    Primary,
    Fulltext,
}

/// `columns` in index order; an empty `name` lets the server choose (PRIMARY KEY has none).
pub fn add_index(db: &str, table: &str, kind: IndexKind, name: &str, columns: &[String]) -> Result<String, String> {
    if columns.is_empty() {
        return Err("Choose at least one column.".into());
    }
    let cols = columns.iter().map(|c| ident(c)).collect::<Vec<_>>().join(", ");
    let label = if name.trim().is_empty() { String::new() } else { format!("{} ", ident(name.trim())) };
    let what = match kind {
        IndexKind::Primary => "PRIMARY KEY ".to_string(),
        IndexKind::Unique => format!("UNIQUE INDEX {label}"),
        IndexKind::Fulltext => format!("FULLTEXT INDEX {label}"),
        IndexKind::Index => format!("INDEX {label}"),
    };
    Ok(format!("ALTER TABLE {} ADD {what}({cols})", qualified(db, table)))
}

/// The index named PRIMARY is the primary key.
pub fn drop_index(db: &str, table: &str, name: &str) -> String {
    let what = if name == "PRIMARY" { "PRIMARY KEY".to_string() } else { format!("INDEX {}", ident(name)) };
    format!("ALTER TABLE {} DROP {what}", qualified(db, table))
}

// ---- foreign keys ----

/// A link from a column of this table to a column of another table in the same database.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct ForeignKeyDraft {
    /// Empty: the server names it.
    pub name: String,
    pub column: String,
    pub ref_table: String,
    pub ref_column: String,
    pub on_delete: String,
    pub on_update: String,
}

fn rule(value: &str) -> Result<&str, String> {
    RULES.iter().find(|r| **r == value).copied().ok_or_else(|| format!("“{value}” is not a foreign key rule."))
}

pub fn add_foreign_key(db: &str, table: &str, fk: &ForeignKeyDraft) -> Result<String, String> {
    if fk.column.is_empty() || fk.ref_table.is_empty() || fk.ref_column.is_empty() {
        return Err("Choose the column and the table and column it refers to.".into());
    }
    let constraint = if fk.name.trim().is_empty() { String::new() } else { format!("CONSTRAINT {} ", ident(fk.name.trim())) };
    Ok(format!(
        "ALTER TABLE {} ADD {constraint}FOREIGN KEY ({}) REFERENCES {} ({}) ON DELETE {} ON UPDATE {}",
        qualified(db, table),
        ident(&fk.column),
        qualified(db, &fk.ref_table),
        ident(&fk.ref_column),
        rule(&fk.on_delete)?,
        rule(&fk.on_update)?
    ))
}

pub fn drop_foreign_key(db: &str, table: &str, name: &str) -> String {
    format!("ALTER TABLE {} DROP FOREIGN KEY {}", qualified(db, table), ident(name))
}

// ---- table options ----

/// Renames the table and/or moves it to another database.
pub fn rename_table(db: &str, table: &str, new_db: &str, new_name: &str) -> Result<String, String> {
    let name = valid_name(new_name, "Table", &[])?;
    if new_db.is_empty() {
        return Err("Choose a database.".into());
    }
    if new_db == db && name == table {
        return Err("The table already has this name.".into());
    }
    Ok(format!("RENAME TABLE {} TO {}", qualified(db, table), qualified(new_db, &name)))
}

/// A copy in the same database: structure and indexes, and the rows when `data` lists the columns to copy
/// (by name: `SELECT *` would leave INVISIBLE columns out, and generated ones can't be written).
pub fn copy_table(db: &str, table: &str, new_name: &str, data: Option<&[String]>) -> Result<Vec<String>, String> {
    let name = valid_name(new_name, "Table", &[])?;
    let target = qualified(db, &name);
    let source = qualified(db, table);
    let mut out = vec![format!("CREATE TABLE {target} LIKE {source}")];
    if let Some(columns) = data.filter(|c| !c.is_empty()) {
        let list = columns.iter().map(|c| ident(c)).collect::<Vec<_>>().join(", ");
        out.push(format!("INSERT INTO {target} ({list}) SELECT {list} FROM {source}"));
    }
    Ok(out)
}

/// Removes the copy `copy_table` made when filling it failed.
pub fn drop_copy(db: &str, new_name: &str) -> String {
    format!("DROP TABLE {}", qualified(db, new_name.trim()))
}

/// A new engine and/or collation (the data is converted to the collation's character set); None: nothing to change.
pub fn table_options(db: &str, table: &str, engine: Option<&str>, collation: Option<&str>) -> Result<Option<String>, String> {
    let mut options = Vec::new();
    if let Some(e) = engine {
        options.push(format!("ENGINE = {}", word(e, "engine")?));
    }
    if let Some(c) = collation {
        let c = word(c, "collation")?;
        options.push(format!("CONVERT TO CHARACTER SET {} COLLATE {c}", charset(c)));
    }
    Ok((!options.is_empty()).then(|| format!("ALTER TABLE {} {}", qualified(db, table), options.join(", "))))
}

/// OPTIMIZE, ANALYZE, CHECK or REPAIR.
pub fn maintenance(db: &str, table: &str, operation: &str) -> Result<String, String> {
    if !OPERATIONS.contains(&operation) {
        return Err(format!("“{operation}” is not a table operation."));
    }
    Ok(format!("{operation} TABLE {}", qualified(db, table)))
}

/// The server's answer to a maintenance statement: "Optimize finished." and a line per message.
pub fn maintenance_report(operation: &str, messages: &[(String, String)]) -> String {
    let mut name = operation.to_lowercase();
    if let Some(first) = name.get_mut(0..1) {
        first.make_ascii_uppercase();
    }
    let mut out = format!("{name} finished.");
    for (kind, text) in messages {
        out.push_str(&format!("\n{kind}: {text}"));
    }
    out
}

// ---- new table, new database ----

/// CREATE TABLE from the New Table sheet. Rows without a name or type are ignored; the primary key
/// columns become one PRIMARY KEY (…) clause.
pub fn create_table(db: &str, name: &str, columns: &[ColumnDraft], engine: &str, collation: &str) -> Result<String, String> {
    let name = valid_name(name, "Table", &[])?;
    let usable: Vec<ColumnDraft> = columns
        .iter()
        .filter(|c| !c.name.trim().is_empty() && !c.sql_type.trim().is_empty())
        // A key column can't be NULL; the sheet turns NULL off too.
        .map(|c| ColumnDraft { nullable: c.nullable && !c.primary_key, ..c.clone() })
        .collect();
    if usable.is_empty() {
        return Err("Add at least one column with a name and a type.".into());
    }
    for (i, c) in usable.iter().enumerate() {
        let n = c.name.trim().to_lowercase();
        if usable[..i].iter().any(|o| o.name.trim().to_lowercase() == n) {
            return Err(format!("Two columns are named “{}”.", c.name.trim()));
        }
    }
    let mut defs: Vec<String> = usable.iter().map(|c| format!("  {} {}", ident(c.name.trim()), c.definition())).collect();
    let keys: Vec<String> = usable.iter().filter(|c| c.primary_key).map(|c| ident(c.name.trim())).collect();
    if !keys.is_empty() {
        defs.push(format!("  PRIMARY KEY ({})", keys.join(", ")));
    }
    let engine = word(engine, "engine")?;
    let collation = word(collation, "collation")?;
    Ok(format!(
        "CREATE TABLE {} (\n{}\n) ENGINE={engine} DEFAULT CHARSET={} COLLATE={collation}",
        qualified(db, &name),
        defs.join(",\n"),
        charset(collation)
    ))
}

pub fn create_database(name: &str, collation: &str) -> Result<String, String> {
    let name = valid_name(name, "Database", &[])?;
    let collation = word(collation, "collation")?;
    Ok(format!("CREATE DATABASE {} CHARACTER SET {} COLLATE {collation}", ident(&name), charset(collation)))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn draft(name: &str, sql_type: &str) -> ColumnDraft {
        ColumnDraft { name: name.into(), sql_type: sql_type.into(), nullable: true, ..Default::default() }
    }

    #[test]
    fn defaults_are_quoted_only_when_they_are_text() {
        let d = |t: &str| default_clause(t, "INT");
        assert_eq!(d(""), None);
        assert_eq!(d("   "), None);
        for asis in ["NULL", "null", "TRUE", "false", "0", "-1", "0.00", "+3.5", ".5", "1e3", "'x'", "''", "CURRENT_TIMESTAMP", "current_timestamp()", "now()", "CURRENT_TIMESTAMP(3)", "curdate", "uuid()", "(1 + 1)"] {
            assert_eq!(d(asis).as_deref(), Some(asis), "{asis}");
        }
        assert_eq!(d(" 42 ").as_deref(), Some("42"));
        assert_eq!(d("new").as_deref(), Some("'new'"));
        assert_eq!(d("it's").as_deref(), Some(r"'it\'s'"));
        assert_eq!(d(r"C:\www").as_deref(), Some(r"'C:\\www'"));
        // Not numbers, not functions: quoted (a bare `inf` or `now(x)` would be invalid SQL).
        assert_eq!(d("inf").as_deref(), Some("'inf'"));
        assert_eq!(d("1.2.3").as_deref(), Some("'1.2.3'"));
        assert_eq!(d("now(x)").as_deref(), Some("'now(x)'"));
        assert_eq!(d("'").as_deref(), Some(r"'\''"));
        // A number for a text column is that text: 007 would be stored as '7'.
        for ty in ["VARCHAR(10)", "char(3)", "TEXT", "enum('1','2')", "varbinary(4)"] {
            assert_eq!(default_clause("007", ty).as_deref(), Some("'007'"), "{ty}");
        }
        assert_eq!(default_clause("1.50", "decimal(5,2)").as_deref(), Some("1.50"));
    }

    #[test]
    fn edit_column_keeps_the_default_exactly() {
        // information_schema.COLUMNS quotes text defaults; the sheet gets them so they save unchanged.
        let cases = [
            (Some("'NULL'"), "varchar(10)", "'NULL'"), // the text NULL, not SQL NULL
            (Some("'007'"), "varchar(10)", "'007'"),
            (Some("NULL"), "varchar(10)", ""),
            (None, "int(11)", ""),
            (Some("0"), "int(11)", "0"),
            (Some("''"), "text", "''"),
            (Some(r"'it\'s'"), "varchar(9)", r"'it\'s'"),
            (Some("current_timestamp(3)"), "timestamp(3)", "current_timestamp(3)"),
            (Some("b'101'"), "bit(3)", "(b'101')"),
            (Some("concat('a','b')"), "varchar(9)", "(concat('a','b'))"),
        ];
        for (from, ty, want) in cases {
            let text = sheet_default(from, ty);
            assert_eq!(text, want, "{from:?}");
            // Saved unchanged, the field goes into the SQL as it is.
            assert_eq!(default_clause(&text, ty), (!text.is_empty()).then(|| text.clone()), "{from:?}");
        }
    }

    #[test]
    fn edit_column_keeps_collation_invisible_and_update_precision() {
        let mut c = draft("name", "varchar(20)");
        c.collation = "latin1_swedish_ci".into();
        c.invisible = true;
        assert_eq!(c.definition(), "varchar(20) CHARACTER SET latin1 COLLATE latin1_swedish_ci NULL INVISIBLE");
        // A type without a collation drops it; a type that names its own keeps that one.
        c.sql_type = "INT".into();
        assert_eq!(c.definition(), "INT NULL INVISIBLE");
        c.sql_type = "VARCHAR(5) COLLATE utf8mb4_bin".into();
        assert_eq!(c.definition(), "VARCHAR(5) COLLATE utf8mb4_bin NULL INVISIBLE");
        c.sql_type = "enum('a','b')".into();
        c.collation = "utf8mb4_bin".into();
        c.invisible = false;
        assert_eq!(c.definition(), "enum('a','b') CHARACTER SET utf8mb4 COLLATE utf8mb4_bin NULL");
        // The collation goes in unquoted: only plain words.
        c.collation = "x; DROP".into();
        assert_eq!(c.definition(), "enum('a','b') NULL");
        assert!(save_column("d", "t", &c, Some("e"), &Position::Unchanged).is_err());

        let mut t = draft("changed", "TIMESTAMP(3)");
        t.nullable = false;
        t.default_text = "current_timestamp(3)".into();
        t.on_update_timestamp = true;
        assert_eq!(t.definition(), "TIMESTAMP(3) NOT NULL DEFAULT current_timestamp(3) ON UPDATE CURRENT_TIMESTAMP(3)");
        t.sql_type = "datetime(6)".into();
        assert!(t.definition().ends_with("ON UPDATE CURRENT_TIMESTAMP(6)"));
        t.sql_type = "timestamp".into();
        assert!(t.definition().ends_with("ON UPDATE CURRENT_TIMESTAMP"));
    }

    #[test]
    fn column_definitions() {
        let mut c = draft("price", "decimal(10,2)");
        c.nullable = false;
        assert_eq!(c.sql().as_deref(), Some("`price` decimal(10,2) NOT NULL"));
        c.default_text = "0".into();
        assert_eq!(c.definition(), "decimal(10,2) NOT NULL DEFAULT 0");

        let mut t = draft("updated_at", " TIMESTAMP ");
        t.default_text = "CURRENT_TIMESTAMP".into();
        t.on_update_timestamp = true;
        t.comment = "Last change, it's automatic".into();
        assert_eq!(t.definition(), r"TIMESTAMP NULL DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP COMMENT 'Last change, it\'s automatic'");

        // Auto increment has no default, even when one is typed.
        let mut id = draft(" id ", "INT UNSIGNED");
        id.nullable = false;
        id.auto_increment = true;
        id.default_text = "5".into();
        assert_eq!(id.sql().as_deref(), Some("`id` INT UNSIGNED NOT NULL AUTO_INCREMENT"));

        assert_eq!(draft("a`b", "TEXT").sql().as_deref(), Some("`a``b` TEXT NULL"));
        assert_eq!(draft("", "TEXT").sql(), None);
        assert_eq!(draft("x", "  ").sql(), None);
    }

    #[test]
    fn add_change_and_move_columns() {
        let c = draft("note", "TEXT");
        assert_eq!(save_column("shop", "orders", &c, None, &Position::Unchanged).unwrap(), "ALTER TABLE `shop`.`orders` ADD COLUMN `note` TEXT NULL");
        assert_eq!(save_column("shop", "orders", &c, None, &Position::First).unwrap(), "ALTER TABLE `shop`.`orders` ADD COLUMN `note` TEXT NULL FIRST");
        assert_eq!(
            save_column("shop", "orders", &c, Some("old note"), &Position::After { column: "total".into() }).unwrap(),
            "ALTER TABLE `shop`.`orders` CHANGE COLUMN `old note` `note` TEXT NULL AFTER `total`"
        );
        assert_eq!(save_column("shop", "orders", &c, Some("note"), &Position::Unchanged).unwrap(), "ALTER TABLE `shop`.`orders` CHANGE COLUMN `note` `note` TEXT NULL");
        assert!(save_column("shop", "orders", &draft(" ", "TEXT"), None, &Position::Unchanged).is_err());
        assert_eq!(drop_column("shop", "orders", "note"), "ALTER TABLE `shop`.`orders` DROP COLUMN `note`");
    }

    #[test]
    fn positions_come_from_the_sheet_as_tagged_objects() {
        let p: Position = serde_json::from_str(r#"{"kind":"after","column":"id"}"#).unwrap();
        assert!(matches!(p, Position::After { column } if column == "id"));
        assert!(matches!(serde_json::from_str::<Position>(r#"{"kind":"first"}"#).unwrap(), Position::First));
        assert!(matches!(serde_json::from_str::<Position>(r#"{"kind":"unchanged"}"#).unwrap(), Position::Unchanged));
        let c: ColumnDraft = serde_json::from_str(r#"{"name":"n","type":"INT","nullable":false,"defaultText":"","autoIncrement":true,"primaryKey":true,"onUpdateTimestamp":false,"comment":""}"#).unwrap();
        assert_eq!(c.sql().as_deref(), Some("`n` INT NOT NULL AUTO_INCREMENT"));
    }

    #[test]
    fn indexes_and_primary_keys() {
        let cols = vec!["status".to_string(), "ordered_at".to_string()];
        assert_eq!(add_index("d", "t", IndexKind::Index, "", &cols).unwrap(), "ALTER TABLE `d`.`t` ADD INDEX (`status`, `ordered_at`)");
        assert_eq!(add_index("d", "t", IndexKind::Index, " idx_status ", &cols[..1]).unwrap(), "ALTER TABLE `d`.`t` ADD INDEX `idx_status` (`status`)");
        assert_eq!(add_index("d", "t", IndexKind::Unique, "uq", &cols).unwrap(), "ALTER TABLE `d`.`t` ADD UNIQUE INDEX `uq` (`status`, `ordered_at`)");
        assert_eq!(add_index("d", "t", IndexKind::Fulltext, "", &cols[..1]).unwrap(), "ALTER TABLE `d`.`t` ADD FULLTEXT INDEX (`status`)");
        // A primary key never takes a name.
        assert_eq!(add_index("d", "t", IndexKind::Primary, "pk", &cols[..1]).unwrap(), "ALTER TABLE `d`.`t` ADD PRIMARY KEY (`status`)");
        assert!(add_index("d", "t", IndexKind::Index, "x", &[]).is_err());
        assert_eq!(drop_index("d", "t", "PRIMARY"), "ALTER TABLE `d`.`t` DROP PRIMARY KEY");
        assert_eq!(drop_index("d", "t", "idx_status"), "ALTER TABLE `d`.`t` DROP INDEX `idx_status`");
        let kind: IndexKind = serde_json::from_str("\"fulltext\"").unwrap();
        assert_eq!(kind, IndexKind::Fulltext);
    }

    #[test]
    fn foreign_keys() {
        let mut fk = ForeignKeyDraft {
            name: String::new(),
            column: "customer_id".into(),
            ref_table: "customers".into(),
            ref_column: "id".into(),
            on_delete: "CASCADE".into(),
            on_update: "RESTRICT".into(),
        };
        assert_eq!(
            add_foreign_key("shop", "orders", &fk).unwrap(),
            "ALTER TABLE `shop`.`orders` ADD FOREIGN KEY (`customer_id`) REFERENCES `shop`.`customers` (`id`) ON DELETE CASCADE ON UPDATE RESTRICT"
        );
        fk.name = " fk_order_customer ".into();
        fk.on_delete = "SET NULL".into();
        fk.on_update = "NO ACTION".into();
        assert_eq!(
            add_foreign_key("shop", "orders", &fk).unwrap(),
            "ALTER TABLE `shop`.`orders` ADD CONSTRAINT `fk_order_customer` FOREIGN KEY (`customer_id`) REFERENCES `shop`.`customers` (`id`) ON DELETE SET NULL ON UPDATE NO ACTION"
        );
        // Rules go into the SQL as words: only the four known ones.
        fk.on_update = "CASCADE; DROP TABLE x".into();
        assert!(add_foreign_key("shop", "orders", &fk).is_err());
        fk.on_update = "RESTRICT".into();
        fk.ref_column.clear();
        assert!(add_foreign_key("shop", "orders", &fk).is_err());
        assert_eq!(drop_foreign_key("shop", "orders", "fk_order_customer"), "ALTER TABLE `shop`.`orders` DROP FOREIGN KEY `fk_order_customer`");
    }

    #[test]
    fn rename_move_and_copy() {
        assert_eq!(rename_table("shop", "orders", "shop", " sales ").unwrap(), "RENAME TABLE `shop`.`orders` TO `shop`.`sales`");
        assert_eq!(rename_table("shop", "orders", "archive", "orders").unwrap(), "RENAME TABLE `shop`.`orders` TO `archive`.`orders`");
        assert!(rename_table("shop", "orders", "shop", "orders").is_err());
        assert!(rename_table("shop", "orders", "shop", "  ").is_err());
        assert!(rename_table("shop", "orders", "shop", "a.b").is_err());
        assert!(rename_table("shop", "orders", "", "x").is_err());
        assert_eq!(copy_table("shop", "orders", "orders_copy", None).unwrap(), ["CREATE TABLE `shop`.`orders_copy` LIKE `shop`.`orders`"]);
        // The columns by name: an INVISIBLE one too, a generated one not (the caller leaves it out).
        let cols = ["id".to_string(), "hidden".into()];
        assert_eq!(
            copy_table("shop", "orders", " orders_copy ", Some(&cols)).unwrap(),
            ["CREATE TABLE `shop`.`orders_copy` LIKE `shop`.`orders`", "INSERT INTO `shop`.`orders_copy` (`id`, `hidden`) SELECT `id`, `hidden` FROM `shop`.`orders`"]
        );
        assert!(copy_table("shop", "orders", "", Some(&cols)).is_err());
        assert_eq!(drop_copy("shop", " orders_copy "), "DROP TABLE `shop`.`orders_copy`");
    }

    #[test]
    fn engine_collation_and_maintenance() {
        assert_eq!(table_options("d", "t", None, None).unwrap(), None);
        assert_eq!(table_options("d", "t", Some("Aria"), None).unwrap().as_deref(), Some("ALTER TABLE `d`.`t` ENGINE = Aria"));
        assert_eq!(
            table_options("d", "t", Some("InnoDB"), Some("utf8mb4_hungarian_ci")).unwrap().as_deref(),
            Some("ALTER TABLE `d`.`t` ENGINE = InnoDB, CONVERT TO CHARACTER SET utf8mb4 COLLATE utf8mb4_hungarian_ci")
        );
        assert_eq!(
            table_options("d", "t", None, Some("latin2_general_ci")).unwrap().as_deref(),
            Some("ALTER TABLE `d`.`t` CONVERT TO CHARACTER SET latin2 COLLATE latin2_general_ci")
        );
        assert!(table_options("d", "t", Some("InnoDB; DROP TABLE t"), None).is_err());
        assert!(table_options("d", "t", None, Some("utf8mb4 bin")).is_err());

        for op in OPERATIONS {
            assert_eq!(maintenance("d", "t", op).unwrap(), format!("{op} TABLE `d`.`t`"));
        }
        assert!(maintenance("d", "t", "DROP").is_err());
        assert!(maintenance("d", "t", "optimize").is_err());
        let msgs = vec![("note".to_string(), "Table does not support optimize, doing recreate + analyze instead".to_string()), ("status".to_string(), "OK".to_string())];
        assert_eq!(maintenance_report("OPTIMIZE", &msgs), "Optimize finished.\nnote: Table does not support optimize, doing recreate + analyze instead\nstatus: OK");
        assert_eq!(maintenance_report("CHECK", &[]), "Check finished.");
    }

    #[test]
    fn create_table_from_the_designer_rows() {
        let mut id = draft("id", "INT UNSIGNED");
        id.nullable = true; // the key turns NULL off
        id.auto_increment = true;
        id.primary_key = true;
        let mut name = draft("name", "VARCHAR(255)");
        name.default_text = "none yet".into();
        let unnamed = draft("  ", "TEXT");
        let sql = create_table("shop", " tags ", &[id.clone(), name.clone(), unnamed], "InnoDB", "utf8mb4_uca1400_ai_ci").unwrap();
        assert_eq!(
            sql,
            "CREATE TABLE `shop`.`tags` (\n  `id` INT UNSIGNED NOT NULL AUTO_INCREMENT,\n  `name` VARCHAR(255) NULL DEFAULT 'none yet',\n  PRIMARY KEY (`id`)\n) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_uca1400_ai_ci"
        );
        // Two key columns: one composite key, in row order.
        let mut a = draft("order_id", "INT");
        a.primary_key = true;
        let mut b = draft("product_id", "INT");
        b.primary_key = true;
        assert!(create_table("d", "t", &[a.clone(), b], "Aria", "utf8mb4_bin").unwrap().contains("  PRIMARY KEY (`order_id`, `product_id`)\n) ENGINE=Aria DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_bin"));
        // No key: no PRIMARY KEY clause.
        assert!(!create_table("d", "t", &[name.clone()], "InnoDB", "utf8mb4_bin").unwrap().contains("PRIMARY KEY"));
        assert!(create_table("d", "t", &[draft("", "")], "InnoDB", "utf8mb4_bin").is_err());
        assert_eq!(create_table("d", "t", &[draft("Name", "TEXT"), draft("name ", "INT")], "InnoDB", "utf8mb4_bin").unwrap_err(), "Two columns are named “name”.");
        assert!(create_table("d", "", &[name.clone()], "InnoDB", "utf8mb4_bin").is_err());
        assert!(create_table("d", "t", &[name], "InnoDB", "x'y").is_err());
    }

    #[test]
    fn create_database_with_a_collation() {
        assert_eq!(create_database(" my_project ", "utf8mb4_hungarian_ci").unwrap(), "CREATE DATABASE `my_project` CHARACTER SET utf8mb4 COLLATE utf8mb4_hungarian_ci");
        assert_eq!(create_database("a`b", "utf8mb4_bin").unwrap(), "CREATE DATABASE `a``b` CHARACTER SET utf8mb4 COLLATE utf8mb4_bin");
        assert!(create_database("", "utf8mb4_bin").is_err());
        assert!(create_database("x/y", "utf8mb4_bin").is_err());
    }

    #[test]
    fn names_are_checked_before_they_are_sent() {
        let existing = vec!["Orders".to_string(), "customers".to_string()];
        assert_eq!(name_problem("", "Table", &existing), None);
        assert_eq!(name_problem("order_items", "Table", &existing), None);
        assert_eq!(name_problem(" orders ", "Table", &existing).as_deref(), Some("A table named “orders” already exists."));
        assert_eq!(name_problem("CUSTOMERS", "Database", &existing).as_deref(), Some("A database named “CUSTOMERS” already exists."));
        assert_eq!(name_problem(&"a".repeat(65), "Database", &[]).as_deref(), Some("Database names can be at most 64 characters long."));
        assert_eq!(name_problem(&"á".repeat(64), "Database", &[]), None);
        for bad in ["a/b", r"a\b", "a.b"] {
            assert_eq!(name_problem(bad, "Table", &[]).as_deref(), Some("The name can't contain / \\ or a dot."), "{bad}");
        }
        assert!(is_plain("my_project_2") && !is_plain("my project") && !is_plain("ügyfelek") && !is_plain(""));
        assert_eq!(check_name("my project", "Database", &[]), NameCheck { problem: None, plain: false });
        assert_eq!(check_name("", "Database", &[]), NameCheck { problem: None, plain: true });
    }

    #[test]
    fn collations_follow_the_server_version() {
        assert!(supports_uca1400("MariaDB 11.4.2") && supports_uca1400("MariaDB 13.0.2") && supports_uca1400("10.11.6-MariaDB"));
        assert!(supports_uca1400("10.10.1") && !supports_uca1400("10.6.18-MariaDB") && !supports_uca1400("MariaDB 10.4"));
        assert!(supports_uca1400("") && supports_uca1400("MariaDB"));
        let new = choices("MariaDB 11.4.2");
        assert_eq!(new.default_collation, "utf8mb4_uca1400_ai_ci");
        assert_eq!(new.collations.len(), 5);
        let old = choices("10.6.18-MariaDB");
        assert_eq!(old.default_collation, "utf8mb4_general_ci");
        assert!(old.collations.iter().all(|c| c.name != "utf8mb4_uca1400_ai_ci"));
        assert_eq!(charset("utf8mb4_hungarian_ci"), "utf8mb4");
        assert_eq!(charset("latin1_swedish_ci"), "latin1");
    }
}
