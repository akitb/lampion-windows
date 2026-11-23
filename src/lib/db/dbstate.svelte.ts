// Database Manager window state, shared by both database parts (see docs/AGENTS.md).
// Part A (data/query) owns the window and drives this; part B (structure/designer) reads and calls it.

export type DbTab = "query" | "tables" | "designer" | "data" | "structure";

export type SheetKind =
  | "newDatabase" | "newTable" | "tableOptions" | "addColumn" | "editColumn"
  | "addIndex" | "addForeignKey";

export interface Sheet {
  kind: SheetKind;
  database: string;
  table?: string;
  /** Column name for editColumn, etc. */
  column?: string;
}

export const db = $state({
  database: null as string | null,
  table: null as string | null,
  isView: false,
  tab: "query" as DbTab,
  /** Bumped after any change to databases/tables/columns: listeners reload what they show. */
  schemaVersion: 0,
  /** The structure sheet that is open (rendered by StructureSheets.svelte), or null. */
  sheet: null as Sheet | null,
});

/** Opens a table (or view) in the given tab. */
export function openTable(database: string, table: string, tab: DbTab = "data", isView = false) {
  db.database = database;
  db.table = table;
  db.isView = isView;
  db.tab = tab;
}

/** Call after DDL so the sidebar, header and views reload. */
export function schemaChanged() {
  db.schemaVersion++;
}
