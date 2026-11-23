// The structure commands (Rust: database/structure_commands.rs). Calls that reach the server count as
// "busy" for the window's Refresh pulse, like part A's calls.
import { untrack } from "svelte";
import { call } from "$lib/api";
import { activity } from "../dbapi.svelte";
import type { ColumnDraft, ColumnPosition, ForeignKeyDraft, IndexKind, NameCheck, StructureChoices, TableStructure } from "$lib/types";

async function track<T>(p: Promise<T>): Promise<T> {
  // Not a dependency of the effect that calls this (it would run again whenever the count changes).
  untrack(() => activity.busy++);
  try {
    return await p;
  } finally {
    activity.busy--;
  }
}

export const structure = {
  load: (database: string, table: string) => track(call<TableStructure>("db_structure_load", { database, table })),
  columns: (database: string, table: string) => track(call<string[]>("db_structure_columns", { database, table })),
  choices: (version: string) => call<StructureChoices>("db_structure_choices", { version }),
  columnSql: (column: ColumnDraft) => call<string | null>("db_structure_column_sql", { column }),
  checkName: (kind: "Database" | "Table", name: string, existing: string[]) => call<NameCheck>("db_structure_check_name", { kind, name, existing }),
  saveColumn: (database: string, table: string, column: ColumnDraft, replacing: string | null, position: ColumnPosition) =>
    track(call("db_structure_save_column", { database, table, column, replacing, position })),
  dropColumn: (database: string, table: string, column: string) => track(call("db_structure_drop_column", { database, table, column })),
  addIndex: (database: string, table: string, kind: IndexKind, name: string, columns: string[]) =>
    track(call("db_structure_add_index", { database, table, kind, name, columns })),
  dropIndex: (database: string, table: string, name: string) => track(call("db_structure_drop_index", { database, table, name })),
  addForeignKey: (database: string, table: string, key: ForeignKeyDraft) => track(call("db_structure_add_foreign_key", { database, table, key })),
  dropForeignKey: (database: string, table: string, name: string) => track(call("db_structure_drop_foreign_key", { database, table, name })),
  /** Resolves to the new name. */
  renameTable: (database: string, table: string, newDatabase: string, newName: string) =>
    track(call<string>("db_structure_rename_table", { database, table, newDatabase, newName })),
  /** Resolves to the "Done" message. */
  copyTable: (database: string, table: string, newName: string, withData: boolean) =>
    track(call<string>("db_structure_copy_table", { database, table, newName, withData })),
  setOptions: (database: string, table: string, engine: string | null, collation: string | null) =>
    track(call("db_structure_set_options", { database, table, engine, collation })),
  /** Resolves to the server's report. */
  maintenance: (database: string, table: string, operation: string) => track(call<string>("db_structure_maintenance", { database, table, operation })),
  /** Resolves to the table's name. */
  createTable: (database: string, name: string, columns: ColumnDraft[], engine: string, collation: string) =>
    track(call<string>("db_create_table", { database, name, columns, engine, collation })),
  /** Resolves to the database's name. */
  createDatabase: (name: string, collation: string) => track(call<string>("db_create_database", { name, collation })),
};
