// The data/query commands (Rust: database/commands.rs). Every call counts as "busy" while it runs,
// so the Refresh button can pulse during slow work.
import { call } from "$lib/api";
import type { ColumnInfo, Completion, DatabaseInfo, Page, QueryRun, ServerInfo, SyntaxWords } from "./types";

/** Statements in flight. Only the busy indicators read it, so changes don't redraw the window. */
export const activity = $state({ busy: 0 });

async function track<T>(p: Promise<T>): Promise<T> {
  activity.busy++;
  try {
    return await p;
  } finally {
    activity.busy--;
  }
}

/** Command errors arrive as strings; anything else becomes one. */
export function message(e: unknown): string {
  return typeof e === "string" ? e : e instanceof Error ? e.message : String(e);
}

export const dbapi = {
  connect: () => track(call<ServerInfo>("db_connect")),
  setPassword: (password: string) => track(call<ServerInfo>("db_set_password", { password })),
  tables: (database: string) => track(call<DatabaseInfo>("db_tables", { database })),
  columns: (database: string, table: string) => track(call<ColumnInfo[]>("db_columns", { database, table })),
  page: (database: string, table: string, filter: string, orderBy: string | null, ascending: boolean, offset: number) =>
    track(call<Page>("db_page", { database, table, filter, orderBy, ascending, offset })),
  updateRow: (database: string, table: string, columns: string[], row: (string | null)[], changes: { column: number; value: string | null }[]) =>
    track(call("db_update_row", { database, table, columns, row, changes })),
  insertRow: (database: string, table: string, values: Record<string, string>) => track(call("db_insert_row", { database, table, values })),
  deleteRows: (database: string, table: string, columns: string[], rows: (string | null)[][]) =>
    track(call("db_delete_rows", { database, table, columns, rows })),
  emptyTable: (database: string, table: string) => track(call("db_empty_table", { database, table })),
  dropTable: (database: string, table: string, view: boolean) => track(call("db_drop_table", { database, table, view })),
  dropDatabase: (database: string) => track(call("db_drop_database", { database })),
  runQuery: (database: string | null, text: string) => track(call<QueryRun>("db_run_query", { database, text })),
  history: () => call<string[]>("db_history"),
  clearHistory: () => call("db_history_clear"),
  complete: (database: string | null, table: string | null, before: string, text: string) =>
    call<Completion>("db_complete", { database, table, before, text }),
  syntax: () => call<SyntaxWords>("db_syntax"),
  syntaxMarks: (text: string) => call<[number, number][]>("db_syntax_marks", { text }),
  insertText: (table: string, columns: string[], numeric: boolean[], binary: boolean[], rows: (string | null)[][]) =>
    call<string>("db_insert_text", { table, columns, numeric, binary, rows }),
  /** Resolves to the saved file, or null when the save dialog was cancelled. */
  export: (database: string, table: string | null) => track(call<string | null>("db_export", { database, table })),
  /** Resolves to the imported file, or null when cancelled. */
  import: (database: string | null) => track(call<string | null>("db_import", { database })),
  showMainWindow: () => call("db_show_main_window"),
};
