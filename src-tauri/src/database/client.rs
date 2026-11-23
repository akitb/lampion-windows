//! The MariaDB connection (native driver, TCP 127.0.0.1, root). Shared by every database command.
//! Values come back as text (`None` = SQL NULL), which is what the grid, inspector and exports need.

use mysql_async::prelude::*;
use mysql_async::{Conn, Opts, OptsBuilder, Pool, Row, Value};
use once_cell::sync::Lazy;
use serde::Serialize;
use std::sync::Mutex;

struct Target {
    port: u16,
    password: Option<String>,
}

static TARGET: Lazy<Mutex<Target>> = Lazy::new(|| Mutex::new(Target { port: 3306, password: None }));
static POOL: Lazy<Mutex<Option<Pool>>> = Lazy::new(|| Mutex::new(None));

/// Points the client at a port / password (call when MariaDB's port or root access changes).
pub fn configure(port: u16, password: Option<String>) {
    {
        let mut t = TARGET.lock().unwrap();
        if t.port == port && t.password == password {
            return;
        }
        t.port = port;
        t.password = password;
    }
    // TARGET is released before POOL is taken: conn() holds POOL while opts() locks TARGET.
    if let Some(p) = POOL.lock().unwrap().take() {
        tokio::spawn(async move { let _ = p.disconnect().await; });
    }
}

/// Follows a new MariaDB port and keeps the root password the user set (if any).
pub fn set_port(port: u16) {
    let password = TARGET.lock().unwrap().password.clone();
    configure(port, password);
}

pub fn port() -> u16 {
    TARGET.lock().unwrap().port
}

/// The root password the user gave (in memory only), for mariadb-dump / mariadb.exe.
pub fn password() -> Option<String> {
    TARGET.lock().unwrap().password.clone()
}

fn opts() -> Opts {
    let t = TARGET.lock().unwrap();
    OptsBuilder::default()
        .ip_or_hostname("127.0.0.1")
        .tcp_port(t.port)
        .user(Some("root"))
        .pass(t.password.clone().or(Some(String::new())))
        .prefer_socket(false)
        .into()
}

/// A pooled connection (reconnects after `configure`).
pub async fn conn() -> Result<Conn, String> {
    let pool = {
        let mut g = POOL.lock().unwrap();
        g.get_or_insert_with(|| Pool::new(opts())).clone()
    };
    pool.get_conn().await.map_err(err)
}

/// A MariaDB error as the UI shows it: "ERROR 1146 (42S02): Table 'x.y' doesn't exist".
pub fn err(e: mysql_async::Error) -> String {
    match e {
        mysql_async::Error::Server(s) => format!("ERROR {} ({}): {}", s.code, s.state, s.message),
        other => other.to_string(),
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Column {
    pub name: String,
    /// The driver's column type, e.g. "MYSQL_TYPE_VAR_STRING" (use SHOW COLUMNS for the declared type).
    pub type_name: String,
    pub binary: bool,
}

#[derive(Debug, Clone, Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct ResultSet {
    pub columns: Vec<Column>,
    pub rows: Vec<Vec<Option<String>>>,
    pub affected_rows: u64,
}

/// One value as text. Binary data that is not UTF-8 becomes `0x…` hex.
pub fn text(v: &Value) -> Option<String> {
    match v {
        Value::NULL => None,
        Value::Bytes(b) => Some(match std::str::from_utf8(b) {
            Ok(s) => s.to_string(),
            Err(_) => format!("0x{}", hex::encode_upper(b)),
        }),
        Value::Int(i) => Some(i.to_string()),
        Value::UInt(u) => Some(u.to_string()),
        Value::Float(f) => Some(f.to_string()),
        Value::Double(d) => Some(d.to_string()),
        Value::Date(y, mo, d, h, mi, s, us) => Some(if *h == 0 && *mi == 0 && *s == 0 && *us == 0 {
            format!("{y:04}-{mo:02}-{d:02}")
        } else if *us == 0 {
            format!("{y:04}-{mo:02}-{d:02} {h:02}:{mi:02}:{s:02}")
        } else {
            format!("{y:04}-{mo:02}-{d:02} {h:02}:{mi:02}:{s:02}.{us:06}")
        }),
        Value::Time(neg, d, h, mi, s, us) => {
            let hours = *d * 24 + u32::from(*h);
            let sign = if *neg { "-" } else { "" };
            Some(if *us == 0 {
                format!("{sign}{hours:02}:{mi:02}:{s:02}")
            } else {
                format!("{sign}{hours:02}:{mi:02}:{s:02}.{us:06}")
            })
        }
    }
}

fn rows_to_set(rows: Vec<Row>) -> ResultSet {
    let columns = rows
        .first()
        .map(|r| {
            r.columns_ref()
                .iter()
                .map(|c| Column {
                    name: c.name_str().into_owned(),
                    type_name: format!("{:?}", c.column_type()),
                    binary: c.flags().contains(mysql_async::consts::ColumnFlags::BINARY_FLAG)
                        && c.character_set() == 63,
                })
                .collect()
        })
        .unwrap_or_default();
    let rows = rows
        .iter()
        .map(|r| (0..r.len()).map(|i| r.as_ref(i).and_then(text)).collect())
        .collect();
    ResultSet { columns, rows, affected_rows: 0 }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SqlLogEntry {
    pub sql: String,
    pub error: Option<String>,
    pub millis: u64,
}

/// The console shows at most this much of a statement or error (a pasted dump or a long value stays here).
const LOG_MAX: usize = 4096;

/// `s`, cut to `LOG_MAX` bytes on a character boundary.
fn log_text(s: &str) -> String {
    if s.len() <= LOG_MAX {
        return s.to_string();
    }
    let mut end = LOG_MAX;
    while !s.is_char_boundary(end) {
        end -= 1;
    }
    format!("{}…", &s[..end])
}

/// Every statement Lampion runs goes to the SQL console (`database://sql`).
pub fn log_sql(sql: &str, error: Option<&String>, started: std::time::Instant) {
    crate::core::emit(
        "database://sql",
        SqlLogEntry { sql: log_text(sql), error: error.map(|e| log_text(e)), millis: started.elapsed().as_millis() as u64 },
    );
}

/// Runs one statement and returns its rows as text (text protocol, so any SQL works).
pub async fn query(sql: &str) -> Result<ResultSet, String> {
    let started = std::time::Instant::now();
    let r = query_inner(sql).await;
    log_sql(sql, r.as_ref().err(), started);
    r
}

async fn query_inner(sql: &str) -> Result<ResultSet, String> {
    let mut c = conn().await?;
    let mut result = c.query_iter(sql).await.map_err(err)?;
    let columns: Vec<Column> = result
        .columns_ref()
        .iter()
        .map(|c| Column {
            name: c.name_str().into_owned(),
            type_name: format!("{:?}", c.column_type()),
            binary: c.character_set() == 63
                && c.flags().contains(mysql_async::consts::ColumnFlags::BINARY_FLAG),
        })
        .collect();
    let rows: Vec<Row> = result.collect().await.map_err(err)?;
    let affected = result.affected_rows();
    drop(result);
    let mut set = rows_to_set(rows);
    if set.columns.is_empty() {
        set.columns = columns;
    }
    set.affected_rows = affected;
    Ok(set)
}

/// Runs a statement with `?` parameters (binary protocol); values as text, `None` = NULL.
pub async fn exec(sql: &str, params: Vec<Option<String>>) -> Result<ResultSet, String> {
    let started = std::time::Instant::now();
    let r = exec_inner(sql, params).await;
    log_sql(sql, r.as_ref().err(), started);
    r
}

async fn exec_inner(sql: &str, params: Vec<Option<String>>) -> Result<ResultSet, String> {
    let mut c = conn().await?;
    let params: Vec<Value> = params
        .into_iter()
        .map(|p| p.map(|s| Value::Bytes(s.into_bytes())).unwrap_or(Value::NULL))
        .collect();
    let rows: Vec<Row> = c.exec(sql, params).await.map_err(err)?;
    let affected = c.affected_rows();
    let mut set = rows_to_set(rows);
    set.affected_rows = affected;
    Ok(set)
}

/// An identifier in backticks, with backticks doubled.
pub fn ident(name: &str) -> String {
    format!("`{}`", name.replace('`', "``"))
}

/// `db`.`table`
pub fn qualified(db: &str, table: &str) -> String {
    format!("{}.{}", ident(db), ident(table))
}

/// A string literal in single quotes, with `'` and `\` escaped.
pub fn literal(value: &str) -> String {
    format!("'{}'", value.replace('\\', "\\\\").replace('\'', "\\'"))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn quoting() {
        assert_eq!(ident("a`b"), "`a``b`");
        assert_eq!(qualified("d", "t"), "`d`.`t`");
        assert_eq!(literal(r"it's \x"), r"'it\'s \\x'");
    }

    #[test]
    fn console_lines_stay_small() {
        assert_eq!(log_text("SELECT 1"), "SELECT 1");
        let long = format!("{}ő", "a".repeat(LOG_MAX - 1));
        // The two-byte "ő" doesn't fit: cut before it.
        assert_eq!(log_text(&long), format!("{}…", "a".repeat(LOG_MAX - 1)));
    }
}
