//! Database commands: the Structure tab and its sheets (columns, indexes, foreign keys, Table Options,
//! New Table, New Database). Register each in lib.rs under "database (structure)".
//! Loads are quiet; every change is one SQL console line (like the macOS app), and a failure comes back
//! as the server's error text for the sheet to show.

#[path = "structure/mod.rs"]
pub mod structure;

use crate::database::commands::data::run::{self, Session};
use structure::load::{self, TableStructure};
use structure::sql::{self, Choices, ColumnDraft, ForeignKeyDraft, IndexKind, NameCheck, Position};

/// `literal()` escapes with backslashes: under NO_BACKSLASH_ESCAPES a comment or default with `\` would be
/// stored differently than typed. The pool resets the session when the connection goes back.
const BACKSLASH_ESCAPES: &str = "SET SESSION sql_mode = REPLACE(@@sql_mode, 'NO_BACKSLASH_ESCAPES', '')";

/// Runs the statements of one change on one connection, as one console line.
async fn change(statements: &[String]) -> Result<(), String> {
    let mut s = Session::open().await?;
    run::statement(&mut s.conn, BACKSLASH_ESCAPES, &[]).await?;
    let mut result = Ok(());
    for q in statements {
        if let Err(e) = s.text(q).await {
            result = Err(e);
            break;
        }
    }
    s.log(result.as_ref().err());
    result
}

/// Everything the Structure tab shows about a table or view.
#[tauri::command]
pub async fn db_structure_load(database: String, table: String) -> Result<TableStructure, String> {
    let fq = crate::database::client::qualified(&database, &table);
    let p = [database, table];
    let status = run::quiet(load::STATUS, &p).await?;
    let columns = run::quiet(&format!("SHOW FULL COLUMNS FROM {fq}"), &[]).await?;
    let indexes = run::quiet(&format!("SHOW INDEX FROM {fq}"), &[]).await?;
    let create = run::quiet(&format!("SHOW CREATE TABLE {fq}"), &[]).await?;
    let foreign_keys = run::quiet(load::FOREIGN_KEYS, &p).await?;
    Ok(load::structure(&status, &columns, &indexes, &create, &foreign_keys))
}

/// The column names of a table (Add Foreign Key's "Refers to" column).
#[tauri::command]
pub async fn db_structure_columns(database: String, table: String) -> Result<Vec<String>, String> {
    let g = run::quiet(load::COLUMN_NAMES, &[database, table]).await?;
    Ok(g.rows.into_iter().filter_map(|r| r.into_iter().next().flatten()).collect())
}

/// Engines, collations (with a line about each) and foreign key rules for the sheets.
/// `version`: the server's, e.g. "MariaDB 11.4.2" (uca1400 collations need 10.10).
#[tauri::command]
pub fn db_structure_choices(version: String) -> Choices {
    sql::choices(&version)
}

/// The Add/Edit Column sheet's preview: `name` and the definition, or None while either is empty.
#[tauri::command]
pub fn db_structure_column_sql(column: ColumnDraft) -> Option<String> {
    column.sql()
}

/// A database or table name's problem (`kind`: "Database" or "Table") and whether it needs no quoting.
#[tauri::command]
pub fn db_structure_check_name(kind: String, name: String, existing: Vec<String>) -> NameCheck {
    sql::check_name(&name, &kind, &existing)
}

/// Adds a column (`replacing` null) or changes the column `replacing`; `position` moves it.
#[tauri::command]
pub async fn db_structure_save_column(
    database: String,
    table: String,
    column: ColumnDraft,
    replacing: Option<String>,
    position: Position,
) -> Result<(), String> {
    change(&[sql::save_column(&database, &table, &column, replacing.as_deref(), &position)?]).await
}

#[tauri::command]
pub async fn db_structure_drop_column(database: String, table: String, column: String) -> Result<(), String> {
    change(&[sql::drop_column(&database, &table, &column)]).await
}

/// `columns` in index order; an empty `name` lets the server name it.
#[tauri::command]
pub async fn db_structure_add_index(database: String, table: String, kind: IndexKind, name: String, columns: Vec<String>) -> Result<(), String> {
    change(&[sql::add_index(&database, &table, kind, &name, &columns)?]).await
}

/// Deletes an index ("PRIMARY": the primary key).
#[tauri::command]
pub async fn db_structure_drop_index(database: String, table: String, name: String) -> Result<(), String> {
    change(&[sql::drop_index(&database, &table, &name)]).await
}

#[tauri::command]
pub async fn db_structure_add_foreign_key(database: String, table: String, key: ForeignKeyDraft) -> Result<(), String> {
    change(&[sql::add_foreign_key(&database, &table, &key)?]).await
}

#[tauri::command]
pub async fn db_structure_drop_foreign_key(database: String, table: String, name: String) -> Result<(), String> {
    change(&[sql::drop_foreign_key(&database, &table, &name)]).await
}

/// Renames the table and/or moves it to `new_database`; resolves to the new name.
#[tauri::command]
pub async fn db_structure_rename_table(database: String, table: String, new_database: String, new_name: String) -> Result<String, String> {
    change(&[sql::rename_table(&database, &table, &new_database, &new_name)?]).await?;
    Ok(new_name.trim().to_string())
}

/// Copies the table in its database (structure and indexes, and the rows when `with_data`);
/// resolves to the message for the "Done" alert.
#[tauri::command]
pub async fn db_structure_copy_table(database: String, table: String, new_name: String, with_data: bool) -> Result<String, String> {
    change(&sql::copy_table(&database, &table, &new_name, with_data)?).await?;
    Ok(format!("Copied to “{}”.", new_name.trim()))
}

/// Changes the engine and/or the collation (null: keep).
#[tauri::command]
pub async fn db_structure_set_options(database: String, table: String, engine: Option<String>, collation: Option<String>) -> Result<(), String> {
    match sql::table_options(&database, &table, engine.as_deref(), collation.as_deref())? {
        Some(q) => change(&[q]).await,
        None => Ok(()),
    }
}

/// OPTIMIZE, ANALYZE, CHECK or REPAIR; resolves to the server's report ("Optimize finished.\nstatus: OK").
#[tauri::command]
pub async fn db_structure_maintenance(database: String, table: String, operation: String) -> Result<String, String> {
    let q = sql::maintenance(&database, &table, &operation)?;
    let mut s = Session::open().await?;
    let r = s.text(&q).await;
    s.log(r.as_ref().err());
    let grid = r?.into_iter().find_map(|o| o.grid).unwrap_or_default();
    let at = |name: &str| grid.columns.iter().position(|c| c.eq_ignore_ascii_case(name));
    let (kind, text) = (at("Msg_type"), at("Msg_text"));
    let get = |row: &[Option<String>], i: Option<usize>| i.and_then(|i| row.get(i).cloned().flatten()).unwrap_or_default();
    let messages: Vec<(String, String)> = grid.rows.iter().map(|r| (get(r, kind), get(r, text))).collect();
    Ok(sql::maintenance_report(&operation, &messages))
}

/// Creates a table from the New Table sheet's rows; resolves to its name.
#[tauri::command]
pub async fn db_create_table(database: String, name: String, columns: Vec<ColumnDraft>, engine: String, collation: String) -> Result<String, String> {
    change(&[sql::create_table(&database, &name, &columns, &engine, &collation)?]).await?;
    Ok(name.trim().to_string())
}

/// Creates a database (character set utf8mb4); resolves to its name.
#[tauri::command]
pub async fn db_create_database(name: String, collation: String) -> Result<String, String> {
    change(&[sql::create_database(&name, &collation)?]).await?;
    Ok(name.trim().to_string())
}
