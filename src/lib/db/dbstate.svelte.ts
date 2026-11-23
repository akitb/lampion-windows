// Database Manager window state, shared by both database parts (see docs/AGENTS.md).
// Part A (data/query) owns the window and drives this; part B (structure/designer) reads and calls it.
import type { TableInfo } from "./types";

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

/** What the server has, as the sidebar shows it (filled by part A; read-only for others). */
export const server = $state({
  /** "MariaDB 11.4.2"; empty until connected. */
  version: "",
  databases: [] as string[],
  /** Tables and views per database, loaded when a database is opened in the sidebar. */
  tables: {} as Record<string, TableInfo[]>,
});

export interface Confirm {
  title: string;
  message: string;
  /** The destructive button, e.g. "Delete". */
  button: string;
  action: () => void;
}

/** The window's alert and confirmation (native <dialog>s rendered by the window). */
export const dialogs = $state({
  alert: null as string | null,
  confirm: null as Confirm | null,
});

/** Shows a server error (or any message) in the window's Error alert. */
export function showError(message: string) {
  dialogs.alert = message;
}

/** Asks before a destructive action, like the macOS confirmation alerts. */
export function askConfirm(c: Confirm) {
  dialogs.confirm = c;
}
