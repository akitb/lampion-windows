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

/// Rows of one result set, cells as text (`None` = NULL).
#[derive(Debug, Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Grid {
    pub columns: Vec<String>,
    pub rows: Vec<Vec<Option<String>>>,
}

/// One result of a statement: a grid when it returned columns, else the affected-row count.
#[derive(Debug, Default)]
pub struct Outcome {
    pub grid: Option<Grid>,
    pub affected: u64,
}

async fn drain<P: Protocol>(mut r: QueryResult<'_, 'static, P>) -> Result<Vec<Outcome>, mysql_async::Error> {
    let mut out = Vec::new();
    loop {
        let columns = r.columns().map(|c| c.to_vec()).unwrap_or_default();
        let affected = r.affected_rows();
        let rows: Vec<Row> = r.collect().await?;
        if columns.is_empty() {
            out.push(Outcome { grid: None, affected });
        } else {
            let binary: Vec<bool> = columns.iter().map(format::is_binary_column).collect();
            let rows = rows
                .iter()
                .map(|row| (0..row.len()).map(|i| row.as_ref(i).and_then(|v| format::cell(v, binary.get(i) == Some(&true)))).collect())
                .collect();
            out.push(Outcome { grid: Some(Grid { columns: columns.iter().map(|c| c.name_str().into_owned()).collect(), rows }), affected: 0 });
        }
        if r.is_empty() {
            return Ok(out);
        }
    }
}

/// Runs one statement (text protocol without parameters, so any SQL works).
pub async fn statement(conn: &mut Conn, sql: &str, params: &[String]) -> Result<Vec<Outcome>, String> {
    if params.is_empty() {
        let r = conn.query_iter(sql).await.map_err(client::err)?;
        drain(r).await.map_err(client::err)
    } else {
        let values: Vec<Value> = params.iter().map(|s| Value::Bytes(s.clone().into_bytes())).collect();
        let r = conn.exec_iter(sql, values).await.map_err(client::err)?;
        drain(r).await.map_err(client::err)
    }
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

    pub async fn run(&mut self, q: &Built) -> Result<Vec<Outcome>, String> {
        self.shown.push(q.display.trim_end_matches(';').to_string());
        statement(&mut self.conn, &q.sql, &q.params).await
    }

    pub async fn text(&mut self, sql: &str) -> Result<Vec<Outcome>, String> {
        self.run(&Built { sql: sql.to_string(), params: Vec::new(), display: sql.to_string() }).await
    }

    /// Sends the console line: every statement run, and the error if the last one failed.
    pub fn log(&self, error: Option<&String>) {
        if !self.shown.is_empty() {
            client::log_sql(&format!("{};", self.shown.join("; ")), error, self.started);
        }
    }
}
