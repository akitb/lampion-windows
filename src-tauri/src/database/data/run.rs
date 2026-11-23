//! Running statements for the data/query part: every result set of a statement, cells as text
//! (binary as hex), and one SQL console line per user action. Sidebar and metadata loads are quiet,
//! so the console shows what the user's actions sent (like the macOS app).

use super::format;
use super::sql::Built;
use crate::database::client;
use mysql_async::prelude::*;
use mysql_async::{Conn, QueryResult, Row, Value};
use serde::Serialize;
use std::time::Instant;

/// At most this many rows of one result set are kept, so a `SELECT *` on a huge table can't exhaust memory.
pub const MAX_ROWS: usize = 100_000;
/// Grid results also stop at about this many bytes of cell text.
pub const MAX_BYTES: usize = 64 * 1024 * 1024;

/// Changes (row edits, structure) run strict: a value that doesn't fit fails instead of being stored as
/// 0 or '' with a warning, as Lampion's "Relaxed" mode would. `literal()` escapes with backslashes, so
/// NO_BACKSLASH_ESCAPES goes. The pool resets the session when the connection goes back.
pub const STRICT: &str = "SET SESSION sql_mode = CONCAT(REPLACE(@@sql_mode, 'NO_BACKSLASH_ESCAPES', ''), ',STRICT_ALL_TABLES')";

/// Rows of one result set, cells as text (`None` = NULL).
#[derive(Debug, Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Grid {
    pub columns: Vec<String>,
    pub rows: Vec<Vec<Option<String>>>,
    /// `[row, column]` of the cells cut to `format::MAX_CELL` (grid results only).
    pub cut: Vec<[usize; 2]>,
}

/// One result of a statement: a grid when it returned columns, else the affected-row count.
#[derive(Debug, Default)]
pub struct Outcome {
    pub grid: Option<Grid>,
    pub affected: u64,
    /// More rows came than `MAX_ROWS` (or `MAX_BYTES`); only the first ones are in `grid`.
    pub truncated: bool,
}

/// `cut`: long cells are cut for the grid and the result stops at `MAX_BYTES`; otherwise values are whole.
async fn drain<P: Protocol>(mut r: QueryResult<'_, 'static, P>, cut: bool) -> Result<Vec<Outcome>, mysql_async::Error> {
    let max_cell = if cut { format::MAX_CELL } else { usize::MAX };
    let max_bytes = if cut { MAX_BYTES } else { usize::MAX };
    let mut out = Vec::new();
    loop {
        let columns = r.columns().map(|c| c.to_vec()).unwrap_or_default();
        let affected = r.affected_rows();
        let binary: Vec<bool> = columns.iter().map(format::is_binary_column).collect();
        let mut truncated = false;
        let mut cuts: Vec<[usize; 2]> = Vec::new();
        let mut bytes = 0usize;
        // Rows are converted as they arrive; past the limits they are read and dropped.
        let rows = r
            .reduce(Vec::new(), |mut acc: Vec<Vec<Option<String>>>, row: Row| {
                if acc.len() < MAX_ROWS && bytes < max_bytes {
                    let n = acc.len();
                    let cells = (0..row.len())
                        .map(|i| {
                            let (v, was_cut) = row.as_ref(i).map_or((None, false), |v| format::grid_cell(v, binary.get(i) == Some(&true), max_cell));
                            if was_cut {
                                cuts.push([n, i]);
                            }
                            bytes = bytes.saturating_add(v.as_ref().map_or(0, String::len));
                            v
                        })
                        .collect();
                    acc.push(cells);
                } else {
                    truncated = true;
                }
                acc
            })
            .await?;
        if columns.is_empty() {
            out.push(Outcome { grid: None, affected, truncated: false });
        } else {
            let columns = columns.iter().map(|c| c.name_str().into_owned()).collect();
            out.push(Outcome { grid: Some(Grid { columns, rows, cut: cuts }), affected: 0, truncated });
        }
        if r.is_empty() {
            return Ok(out);
        }
    }
}

/// Runs one statement (text protocol without parameters, so any SQL works); values whole.
pub async fn statement(conn: &mut Conn, sql: &str, params: &[String]) -> Result<Vec<Outcome>, String> {
    if params.is_empty() {
        let r = conn.query_iter(sql).await.map_err(client::err)?;
        drain(r, false).await.map_err(client::err)
    } else {
        let values: Vec<Value> = params.iter().map(|s| Value::Bytes(s.clone().into_bytes())).collect();
        let r = conn.exec_iter(sql, values).await.map_err(client::err)?;
        drain(r, false).await.map_err(client::err)
    }
}

/// `statement` for a grid: long cells cut (`format::MAX_CELL`), at most `MAX_BYTES` of rows.
pub async fn statement_cut(conn: &mut Conn, sql: &str) -> Result<Vec<Outcome>, String> {
    let r = conn.query_iter(sql).await.map_err(client::err)?;
    drain(r, true).await.map_err(client::err)
}

/// A query the SQL console doesn't show; its first result set.
pub async fn quiet(sql: &str, params: &[String]) -> Result<Grid, String> {
    let mut c = client::conn().await?;
    Ok(statement(&mut c, sql, params).await?.into_iter().find_map(|o| o.grid).unwrap_or_default())
}

/// The first number of the first result set (e.g. a `COUNT(*)`).
pub fn first_number(outcomes: &[Outcome]) -> Option<u64> {
    let g = outcomes.iter().find_map(|o| o.grid.as_ref())?;
    g.rows.first()?.first()?.as_deref()?.trim().parse().ok()
}

pub fn affected(outcomes: &[Outcome]) -> u64 {
    outcomes.iter().map(|o| o.affected).sum()
}

/// The statements of one user action on one connection, logged as one console line.
pub struct Session {
    pub conn: Conn,
    shown: Vec<String>,
    started: Instant,
}

impl Session {
    pub async fn open() -> Result<Self, String> {
        Ok(Session { conn: client::conn().await?, shown: Vec::new(), started: Instant::now() })
    }

    /// A session for changes, in `STRICT` mode (the SET is not shown in the console).
    pub async fn strict() -> Result<Self, String> {
        let mut s = Self::open().await?;
        statement(&mut s.conn, STRICT, &[]).await?;
        Ok(s)
    }

    pub async fn run(&mut self, q: &Built) -> Result<Vec<Outcome>, String> {
        self.shown.push(q.display.trim_end_matches(';').to_string());
        statement(&mut self.conn, &q.sql, &q.params).await
    }

    pub async fn text(&mut self, sql: &str) -> Result<Vec<Outcome>, String> {
        self.run(&Built { sql: sql.to_string(), params: Vec::new(), display: sql.to_string() }).await
    }

    /// `text` for a grid: long cells cut (see `statement_cut`).
    pub async fn grid(&mut self, sql: &str) -> Result<Vec<Outcome>, String> {
        self.shown.push(sql.trim_end_matches(';').to_string());
        statement_cut(&mut self.conn, sql).await
    }

    /// Fails as running `sql` would unless it is exactly one valid statement: the server parses it as a
    /// prepared statement, which can't hold a second one. So text pasted into `sql` (the Data tab's
    /// filter) can't add a statement, whatever the session's sql_mode does to quotes and backslashes.
    pub async fn single(&mut self, sql: &str) -> Result<(), String> {
        let r = match self.conn.prep(sql).await {
            Ok(stmt) => self.conn.close(stmt).await,
            Err(e) => Err(e),
        };
        if r.is_err() {
            self.shown.push(sql.trim_end_matches(';').to_string());
        }
        r.map_err(client::err)
    }

    /// Sends the console line: every statement run, and the error if the last one failed.
    pub fn log(&self, error: Option<&String>) {
        if !self.shown.is_empty() {
            client::log_sql(&format!("{};", self.shown.join("; ")), error, self.started);
        }
    }
}
