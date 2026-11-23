//! Database commands: the Designer (schema for the ER diagram, relation add/drop, layout persistence).
//! Register each in lib.rs under "database (designer)".

#[path = "designer/mod.rs"]
pub mod designer;

use crate::database::client;
use designer::positions::{self, Positions};
use designer::relation::{self, NewRelation};
use designer::Schema;
use mysql_async::prelude::*;
use mysql_async::Row;
use once_cell::sync::Lazy;
use serde::Serialize;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Mutex;

/// The schema last read per database; the relation sheet's warnings and suggested names use it.
static SCHEMAS: Lazy<Mutex<HashMap<String, Schema>>> = Lazy::new(Default::default);

/// Reads the schema in one round trip. Quiet: background reads stay out of the SQL console, as on macOS.
async fn read(database: &str) -> Result<Schema, String> {
    let mut conn = client::conn().await?;
    let mut result = conn.query_iter(designer::load_sql(database)).await.map_err(client::err)?;
    let mut sets: Vec<Vec<Vec<String>>> = Vec::new();
    loop {
        let rows: Vec<Row> = result.collect().await.map_err(client::err)?;
        sets.push(rows.iter().map(|r| (0..r.len()).map(|i| r.as_ref(i).and_then(client::text).unwrap_or_default()).collect()).collect());
        if result.is_empty() {
            break;
        }
    }
    drop(result);
    sets.resize(3, Vec::new());
    let schema = designer::assemble(database, &sets[0], &sets[1], &sets[2]);
    SCHEMAS.lock().unwrap().insert(database.to_string(), schema.clone());
    Ok(schema)
}

fn positions_file(database: &str) -> PathBuf {
    crate::core::paths::app_data().join("designer").join(positions::file_name(database))
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DesignerLoad {
    pub schema: Schema,
    /// Where the user dragged the cards (empty: lay them out automatically).
    pub positions: Positions,
}

/// The tables, columns and foreign keys of a database, and the card positions saved for it.
#[tauri::command]
pub async fn db_designer_schema(database: String) -> Result<DesignerLoad, String> {
    let schema = read(&database).await?;
    Ok(DesignerLoad { schema, positions: positions::read(&positions_file(&database)) })
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RelationPreview {
    pub sql: String,
    /// Problems MariaDB will most likely refuse, most important first.
    pub warnings: Vec<String>,
    pub suggested_name: String,
}

/// What the relation sheet shows: the exact SQL (with `name`, or the suggested name when it is null),
/// the warnings for the chosen rules, and a free constraint name.
#[tauri::command]
pub async fn db_designer_preview(
    database: String,
    relation: NewRelation,
    on_delete: String,
    on_update: String,
    name: Option<String>,
) -> Result<RelationPreview, String> {
    let cache = SCHEMAS.lock().unwrap();
    let empty = Schema::default();
    let schema = cache.get(&database).unwrap_or(&empty);
    let suggested_name = relation::suggested_name(schema, &relation.table, &relation.column);
    let sql = relation::add_sql(&database, &relation, &on_delete, &on_update, name.as_deref().unwrap_or(&suggested_name))?;
    let warnings = relation::warnings(schema, &relation, &on_delete, &on_update);
    Ok(RelationPreview { sql, warnings, suggested_name })
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RelationAdded {
    pub schema: Schema,
    /// False when MariaDB took the statement but kept no foreign key (MyISAM and other engines ignore
    /// them); the index it made for the column is in `schema` all the same.
    pub created: bool,
}

/// Adds the foreign key and returns the fresh schema. MariaDB's error comes back as the error text.
#[tauri::command]
pub async fn db_designer_add_relation(
    database: String,
    relation: NewRelation,
    on_delete: String,
    on_update: String,
    name: String,
) -> Result<RelationAdded, String> {
    let sql = relation::add_sql(&database, &relation, &on_delete, &on_update, &name)?;
    client::query(&sql).await?;
    let schema = read(&database).await?;
    Ok(RelationAdded { created: relation::created(&schema, &relation), schema })
}

/// Drops a foreign key (the columns and their data stay) and returns the fresh schema.
#[tauri::command]
pub async fn db_designer_drop_relation(database: String, table: String, name: String) -> Result<Schema, String> {
    client::query(&relation::drop_sql(&database, &table, &name)).await?;
    read(&database).await
}

/// Saves where the cards are; an empty map forgets the positions (Auto Arrange).
#[tauri::command]
pub async fn db_designer_save_positions(database: String, positions: Positions) -> Result<(), String> {
    positions::write(&positions_file(&database), &positions)
}
