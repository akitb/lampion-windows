//! Database commands: sidebar, tables, data grid, row editing, query, console, export/import.
//! Register each in lib.rs under "database (data, query, sidebar)".

// The data/query logic lives in `database/data/` (this file is the part's entry point).
#[path = "data/mod.rs"]
pub mod data;

use crate::core::dialog::{pick, Kind};
use crate::core::{activity, paths, shell};
use crate::database::client;
use data::run::{self, Session};
use data::sql::{self, ColumnInfo};
use data::{complete, format, history, marks, split};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::process::Stdio;
use std::time::Instant;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ServerInfo {
    /// "MariaDB 11.4.2"
    pub version: String,
    pub databases: Vec<String>,
}

/// Connects (or reconnects) and lists the databases. An error starting with "ERROR 1045" means the
/// root password is needed (`db_set_password`).
#[tauri::command]
pub async fn db_connect() -> Result<ServerInfo, String> {
    let v = run::quiet("SELECT VERSION()", &[]).await?;
    let raw = v.rows.first().and_then(|r| r.first().cloned().flatten()).unwrap_or_default();
    let version = format!("MariaDB {}", raw.split('-').next().unwrap_or(&raw));
    let databases: Vec<String> = run::quiet("SHOW DATABASES", &[]).await?.rows.into_iter().filter_map(|r| r.into_iter().next().flatten()).collect();
    complete::set_databases(&databases);
    Ok(ServerInfo { version, databases })
}

/// Uses `password` for root from now on (kept in memory only) and connects again.
#[tauri::command]
pub async fn db_set_password(password: String) -> Result<ServerInfo, String> {
    client::configure(client::port(), (!password.is_empty()).then_some(password));
    db_connect().await
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TableInfo {
    pub name: String,
    pub view: bool,
    pub engine: String,
    pub collation: String,
    /// The server's estimate; `None` for views.
    pub rows: Option<u64>,
    /// "1.2k"
    pub rows_short: Option<String>,
    pub bytes: u64,
    /// "48 KB"
    pub size: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DatabaseInfo {
    pub tables: Vec<TableInfo>,
    /// Data and index size of every table together.
    pub size: String,
    /// The database's default collation.
    pub collation: String,
}

/// The tables and views of a database (also refreshes the editor's completion for it).
#[tauri::command]
pub async fn db_tables(database: String) -> Result<DatabaseInfo, String> {
    let p = [database.clone()];
    let list = run::quiet(
        "SELECT TABLE_NAME, TABLE_TYPE, IFNULL(ENGINE,''), IFNULL(TABLE_ROWS,''), IFNULL(DATA_LENGTH+INDEX_LENGTH,0), \
         IFNULL(TABLE_COLLATION,'') FROM information_schema.TABLES WHERE TABLE_SCHEMA = ? ORDER BY TABLE_NAME",
        &p,
    )
    .await?;
    let tables: Vec<TableInfo> = list
        .rows
        .into_iter()
        .map(|r| {
            let v: Vec<String> = r.into_iter().map(Option::unwrap_or_default).chain(std::iter::repeat(String::new())).take(6).collect();
            let rows = v[3].trim().parse::<u64>().ok();
            let bytes = v[4].trim().parse::<u64>().unwrap_or(0);
            TableInfo {
                name: v[0].clone(),
                view: v[1] == "VIEW",
                engine: v[2].clone(),
                collation: v[5].clone(),
                rows,
                rows_short: rows.map(format::compact_count),
                bytes,
                size: format::byte_size(bytes),
            }
        })
        .collect();
    let columns = run::quiet(
        "SELECT TABLE_NAME, COLUMN_NAME FROM information_schema.COLUMNS WHERE TABLE_SCHEMA = ? ORDER BY TABLE_NAME, ORDINAL_POSITION",
        &p,
    )
    .await?;
    let mut by_table: HashMap<String, Vec<String>> = HashMap::new();
    for r in columns.rows {
        if let [Some(t), Some(c), ..] = r.as_slice() {
            by_table.entry(t.clone()).or_default().push(c.clone());
        }
    }
    complete::set_tables(&database, tables.iter().map(|t| (t.name.clone(), by_table.remove(&t.name).unwrap_or_default())).collect());
    let collation = run::quiet("SELECT DEFAULT_COLLATION_NAME FROM information_schema.SCHEMATA WHERE SCHEMA_NAME = ?", &p)
        .await?
        .rows
        .into_iter()
        .next()
        .and_then(|r| r.into_iter().next().flatten())
        .unwrap_or_default();
    let size = format::byte_size(tables.iter().map(|t| t.bytes).sum());
    Ok(DatabaseInfo { tables, size, collation })
}

async fn load_columns(database: &str, table: &str) -> Result<Vec<ColumnInfo>, String> {
    let g = run::quiet(&format!("SHOW FULL COLUMNS FROM {}", client::qualified(database, table)), &[]).await?;
    let at = |name: &str| g.columns.iter().position(|c| c == name);
    let (field, ty, coll, null, key, def, extra, comment) =
        (at("Field"), at("Type"), at("Collation"), at("Null"), at("Key"), at("Default"), at("Extra"), at("Comment"));
    let get = |r: &[Option<String>], i: Option<usize>| i.and_then(|i| r.get(i).cloned().flatten());
    Ok(g.rows
        .iter()
        .map(|r| {
            let s = |i| get(r, i).unwrap_or_default();
            ColumnInfo::new(&s(field), &s(ty), &s(null), &s(key), get(r, def).as_deref(), &s(extra), &s(comment), &s(coll))
        })
        .collect())
}

/// The columns of a table or view, for the grid headers, the row inspector and the insert sheet.
#[tauri::command]
pub async fn db_columns(database: String, table: String) -> Result<Vec<ColumnInfo>, String> {
    load_columns(&database, &table).await
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Page {
    pub columns: Vec<String>,
    pub rows: Vec<Vec<Option<String>>>,
    /// Rows matching the filter.
    pub total: Option<u64>,
}

/// One page (500 rows) of a table, filtered and sorted, and the number of matching rows.
#[tauri::command]
pub async fn db_page(database: String, table: String, filter: String, order_by: Option<String>, ascending: bool, offset: u64) -> Result<Page, String> {
    let text = sql::page(&database, &table, &filter, order_by.as_deref().map(|c| (c, ascending)), offset)?;
    let mut s = Session::open().await?;
    let r = s.text(&text).await;
    s.log(r.as_ref().err());
    let mut grids = r?.into_iter().filter_map(|o| o.grid);
    let page = grids.next().unwrap_or_default();
    let total = grids.next().and_then(|g| g.rows.first()?.first()?.as_deref()?.parse().ok());
    Ok(Page { columns: page.columns, rows: page.rows, total })
}

#[derive(Deserialize)]
pub struct CellChange {
    /// Index into the page's columns.
    pub column: usize,
    pub value: Option<String>,
}

fn not_found(keyed: bool) -> String {
    if keyed {
        "The change was not saved: the row is no longer in the table (it was changed or deleted elsewhere). Reload the rows and try again.".into()
    } else {
        "The change was not saved: Lampion could not find this row again. The table has no primary key, and some of its values \
         (for example FLOAT numbers) can't be matched exactly. Add a primary key in Structure to edit rows safely."
            .into()
    }
}

fn keyed(info: &[ColumnInfo], columns: &[String]) -> bool {
    sql::key_columns(info, columns).len() < columns.len() || info.iter().any(|c| c.key == "PRI")
}

/// Changes cells of one row. `row` holds the values as loaded (in `columns` order); the row is found
/// by its key (see `sql::key_columns`) and counted first, so a row that can't be found is reported.
#[tauri::command]
pub async fn db_update_row(database: String, table: String, columns: Vec<String>, row: Vec<Option<String>>, changes: Vec<CellChange>) -> Result<(), String> {
    let info = load_columns(&database, &table).await?;
    let changes: Vec<(usize, Option<String>)> = changes.into_iter().map(|c| (c.column, c.value)).collect();
    let count = sql::count_row(&database, &table, &info, &columns, &row)?;
    let update = sql::update_row(&database, &table, &info, &columns, &row, &changes)?;
    let mut s = Session::open().await?;
    let result = match s.run(&count).await {
        Ok(o) if run::first_number(&o) == Some(0) => {
            s.log(None);
            return Err(not_found(keyed(&info, &columns)));
        }
        Ok(_) => s.run(&update).await.map(|_| ()),
        Err(e) => Err(e),
    };
    s.log(result.as_ref().err());
    result
}

/// Adds a row; fields left out get their default value.
#[tauri::command]
pub async fn db_insert_row(database: String, table: String, values: HashMap<String, String>) -> Result<(), String> {
    let info = load_columns(&database, &table).await?;
    let values: Vec<(String, String)> = values.into_iter().collect();
    let q = sql::insert_row(&database, &table, &info, &values);
    let mut s = Session::open().await?;
    let r = s.run(&q).await;
    s.log(r.as_ref().err());
    r.map(|_| ())
}

/// Deletes rows (each as loaded, in `columns` order). Rows that can't be found again are reported.
#[tauri::command]
pub async fn db_delete_rows(database: String, table: String, columns: Vec<String>, rows: Vec<Vec<Option<String>>>) -> Result<(), String> {
    let info = load_columns(&database, &table).await?;
    let statements = rows.iter().map(|r| sql::delete_row(&database, &table, &info, &columns, r)).collect::<Result<Vec<_>, _>>()?;
    let mut s = Session::open().await?;
    let mut missing = 0;
    let mut result = Ok(());
    for q in &statements {
        match s.run(q).await {
            Ok(o) if run::affected(&o) == 0 => missing += 1,
            Ok(_) => {}
            Err(e) => {
                result = Err(e);
                break;
            }
        }
    }
    s.log(result.as_ref().err());
    result?;
    match missing {
        0 => Ok(()),
        1 if statements.len() == 1 => Err(not_found(keyed(&info, &columns)).replace("change was not saved", "row was not deleted")),
        n => Err(format!("{n} of the {} rows were not deleted: Lampion could not find them again. Reload the rows and try again.", statements.len())),
    }
}

async fn change(sql: String) -> Result<(), String> {
    let mut s = Session::open().await?;
    let r = s.text(&sql).await;
    s.log(r.as_ref().err());
    r.map(|_| ())
}

/// Deletes every row (TRUNCATE).
#[tauri::command]
pub async fn db_empty_table(database: String, table: String) -> Result<(), String> {
    change(format!("TRUNCATE TABLE {}", client::qualified(&database, &table))).await
}

#[tauri::command]
pub async fn db_drop_table(database: String, table: String, view: bool) -> Result<(), String> {
    change(format!("DROP {} {}", if view { "VIEW" } else { "TABLE" }, client::qualified(&database, &table))).await
}

#[tauri::command]
pub async fn db_drop_database(database: String) -> Result<(), String> {
    change(format!("DROP DATABASE {}", client::ident(&database))).await
}

#[derive(Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct QueryRun {
    /// Every result set with columns, in order.
    pub sets: Vec<run::Grid>,
    /// Rows changed by the statements that returned no rows (`None` when every statement returned rows).
    pub affected: Option<u64>,
    pub error: Option<String>,
    /// Where the error points in the text (UTF-16 offsets, as the editor counts).
    pub error_from: Option<usize>,
    pub error_to: Option<usize>,
    pub seconds: f64,
    /// The history with this query first.
    pub history: Vec<String>,
}

fn utf16(text: &str, byte: usize) -> usize {
    text[..byte].encode_utf16().count()
}

/// Runs the Query tab's text: statement by statement on one connection (in `database` if given),
/// every result set kept, stopping at the first error.
#[tauri::command]
pub async fn db_run_query(database: Option<String>, text: String) -> Result<QueryRun, String> {
    let started = Instant::now();
    let mut out = QueryRun { history: history::add(&text), ..Default::default() };
    let spans = split::split(&text);
    if spans.is_empty() {
        out.error = Some("Query was empty".into());
        return Ok(out);
    }
    let shown = history::normalize(&text);
    let mut conn = match client::conn().await {
        Ok(c) => c,
        Err(e) => {
            client::log_sql(&shown, Some(&e), started);
            out.error = Some(e);
            return Ok(out);
        }
    };
    if let Some(db) = database.as_deref().filter(|d| !d.is_empty()) {
        if let Err(e) = run::statement(&mut conn, &format!("USE {}", client::ident(db)), &[]).await {
            out.error = Some(e);
        }
    }
    for span in spans {
        if out.error.is_some() {
            break;
        }
        let stmt = &text[span.start..span.end];
        match run::statement(&mut conn, stmt, &[]).await {
            Ok(outcomes) => {
                for o in outcomes {
                    match o.grid {
                        Some(g) => out.sets.push(g),
                        None => out.affected = Some(out.affected.unwrap_or(0) + o.affected),
                    }
                }
            }
            Err(e) => {
                if let Some((a, b)) = marks::error_range(&e, stmt) {
                    out.error_from = Some(utf16(&text, span.start + a));
                    out.error_to = Some(utf16(&text, span.start + b));
                }
                out.error = Some(e);
            }
        }
    }
    // The user's SQL may change the session (USE, variables, locks): never hand it back to the pool.
    let _ = conn.disconnect().await;
    client::log_sql(&shown, out.error.as_ref(), started);
    out.seconds = started.elapsed().as_secs_f64();
    Ok(out)
}

#[tauri::command]
pub fn db_history() -> Vec<String> {
    history::all()
}

#[tauri::command]
pub fn db_history_clear() {
    history::clear()
}

/// Suggestions at the caret. `before`: the editor text up to the caret; `text`: all of it (for aliases).
#[tauri::command]
pub async fn db_complete(database: Option<String>, table: Option<String>, before: String, text: String) -> complete::Completion {
    complete::at_caret(database.as_deref(), table.as_deref(), &before, &text)
}

#[derive(Serialize)]
pub struct SyntaxWords {
    pub keywords: Vec<&'static str>,
    pub functions: &'static [&'static str],
    pub types: &'static [&'static str],
}

/// The words the editor highlights.
#[tauri::command]
pub fn db_syntax() -> SyntaxWords {
    SyntaxWords { keywords: complete::highlight_keywords(), functions: complete::FUNCTIONS, types: complete::TYPES }
}

/// Unclosed quotes and unbalanced parentheses in the editor (UTF-16 ranges).
#[tauri::command]
pub async fn db_syntax_marks(text: String) -> Vec<(usize, usize)> {
    marks::syntax_marks(&text).into_iter().map(|(a, b)| (utf16(&text, a), utf16(&text, b))).collect()
}

/// "Copy Row as INSERT" text for rows of a grid.
#[tauri::command]
pub fn db_insert_text(table: String, columns: Vec<String>, numeric: Vec<bool>, binary: Vec<bool>, rows: Vec<Vec<Option<String>>>) -> String {
    sql::rows_as_insert(&table, &columns, &numeric, &binary, &rows)
}

/// mariadb-dump.exe / mariadb.exe with the connection Lampion uses (no console window).
fn tool(exe: &std::path::Path, database: Option<&str>) -> std::process::Command {
    let mut c = shell::command(exe);
    c.args(["--host=127.0.0.1", &format!("--port={}", client::port()), "--user=root", "--protocol=TCP", "--default-character-set=utf8mb4"]);
    if let Some(d) = database {
        c.arg(format!("--database={d}"));
    }
    if let Some(pw) = client::password() {
        c.env("MYSQL_PWD", pw);
    }
    c
}

/// The mariadb client's error line, like the macOS app shows it.
fn tool_error(stderr: &str) -> String {
    let lines: Vec<&str> = stderr.lines().map(str::trim).filter(|l| !l.is_empty()).collect();
    if let Some(e) = lines.iter().rev().find(|l| l.starts_with("ERROR")) {
        if e.contains("2002") || e.contains("2003") || e.contains("Can't connect") {
            return format!("Can't connect to the database. Is MariaDB running? ({e})");
        }
        return e.to_string();
    }
    if lines.is_empty() { "Unknown error".into() } else { lines.join("\n") }
}

fn missing(exe: &std::path::Path) -> Option<String> {
    (!exe.exists()).then(|| format!("{} was not found. Install the servers from the Lampion main window first.", exe.display()))
}

async fn choose(kind: Kind, title: &'static str, filter: (&'static str, &'static str), name: Option<String>) -> Option<String> {
    let backups = paths::backups();
    let start = backups.is_dir().then(|| backups.to_string_lossy().into_owned());
    tauri::async_runtime::spawn_blocking(move || pick(kind, title, start.as_deref(), Some(filter), name.as_deref())).await.ok().flatten()
}

/// Saves a database (or one table) to an SQL file the user picks (in `backups\` by default).
/// `None`: cancelled.
#[tauri::command]
pub async fn db_export(database: String, table: Option<String>) -> Result<Option<String>, String> {
    let exe = paths::mariadb_dump_exe();
    if let Some(e) = missing(&exe) {
        return Err(e);
    }
    let name = match &table {
        Some(t) => format!("{database}-{t}.sql"),
        None => format!("{database}.sql"),
    };
    let Some(path) = choose(Kind::SaveFile, "Export to SQL File", ("SQL file", "*.sql"), Some(name)).await else { return Ok(None) };
    let what = format!("{database}{}", table.as_deref().map(|t| format!(".{t}")).unwrap_or_default());
    let started = Instant::now();
    let existed = std::path::Path::new(&path).exists();
    let mut cmd = tool(&exe, None);
    cmd.args([format!("--result-file={path}"), "--routines".into()]);
    match &table {
        Some(t) => cmd.args([database.as_str(), t.as_str()]),
        None => cmd.args(["--databases", database.as_str()]),
    };
    let out = tauri::async_runtime::spawn_blocking(move || cmd.stdin(Stdio::null()).output()).await.map_err(|e| e.to_string())?;
    let label = format!("/* Export: {what} → {path} */");
    match out {
        Ok(o) if o.status.success() => {
            client::log_sql(&label, None, started);
            client::log_sql("/* Export done */", None, started);
            activity::ok("Database", &format!("Exported {what} to {path}"));
            Ok(Some(path))
        }
        other => {
            let e = match other {
                Ok(o) => tool_error(&String::from_utf8_lossy(&o.stderr)),
                Err(e) => e.to_string(),
            };
            if !existed {
                let _ = std::fs::remove_file(&path);
            }
            client::log_sql(&label, Some(&e), started);
            activity::error("Database", &format!("Export of {what} failed: {e}"));
            Err(e)
        }
    }
}

/// Runs an SQL file the user picks, in `database` if given (else the file must choose one).
/// `None`: cancelled.
#[tauri::command]
pub async fn db_import(database: Option<String>) -> Result<Option<String>, String> {
    let exe = paths::mariadb_exe();
    if let Some(e) = missing(&exe) {
        return Err(e);
    }
    let Some(path) = choose(Kind::OpenFile, "Import SQL File", ("SQL files", "*.sql"), None).await else { return Ok(None) };
    let file = std::fs::File::open(&path).map_err(|e| format!("Can't open {path}: {e}"))?;
    let name = std::path::Path::new(&path).file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_else(|| path.clone());
    let label = format!("/* Import: {name}{} */", database.as_deref().map(|d| format!(" → {d}")).unwrap_or_default());
    let started = Instant::now();
    let mut cmd = tool(&exe, database.as_deref());
    let out = tauri::async_runtime::spawn_blocking(move || cmd.stdin(Stdio::from(file)).output()).await.map_err(|e| e.to_string())?;
    match out {
        Ok(o) if o.status.success() => {
            client::log_sql(&label, None, started);
            client::log_sql("/* Import done */", None, started);
            activity::ok("Database", &format!("Imported {name}{}", database.as_deref().map(|d| format!(" into {d}")).unwrap_or_default()));
            Ok(Some(path))
        }
        other => {
            let e = match other {
                Ok(o) => tool_error(&String::from_utf8_lossy(&o.stderr)),
                Err(e) => e.to_string(),
            };
            client::log_sql(&label, Some(&e), started);
            activity::error("Database", &format!("Import of {name} failed: {e}"));
            Err(e)
        }
    }
}

/// Shows the main window (the connection error's "Open Lampion").
#[tauri::command]
pub fn db_show_main_window(app: tauri::AppHandle) -> Result<(), String> {
    use tauri::Manager;
    let w = app.get_webview_window("main").ok_or("The main window is closed.")?;
    let _ = w.show();
    let _ = w.unminimize();
    w.set_focus().map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tool_errors_read_like_the_client() {
        assert_eq!(tool_error("ERROR 1064 (42000) at line 5: You have an error\n"), "ERROR 1064 (42000) at line 5: You have an error");
        assert!(tool_error("ERROR 2002 (HY000): Can't connect to server on '127.0.0.1' (10061)").starts_with("Can't connect to the database."));
        assert_eq!(tool_error("mariadb-dump: Got error: 1049: \"Unknown database 'x'\" when selecting the database\n"),
            "mariadb-dump: Got error: 1049: \"Unknown database 'x'\" when selecting the database");
        assert_eq!(tool_error(""), "Unknown error");
    }

    #[test]
    fn utf16_offsets() {
        assert_eq!(utf16("árvíz", 2), 1);
        assert_eq!(utf16("a😀b", 5), 3);
    }

    #[test]
    fn keyed_tables_get_the_plain_message() {
        let id = ColumnInfo::new("id", "int", "NO", "PRI", None, "", "", "");
        let x = ColumnInfo::new("x", "float", "YES", "", None, "", "", "");
        let cols = vec!["id".to_string(), "x".to_string()];
        assert!(keyed(&[id, x.clone()], &cols));
        assert!(!keyed(&[x], &["x".to_string()]));
    }
}
