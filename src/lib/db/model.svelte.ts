// The Database Manager window (part A): connection, sidebar, Data and Query state, and the actions
// behind them. Port of the macOS DBModel. Shared state for part B lives in dbstate.svelte.ts.
import type { EditorState } from "@codemirror/state";
import { askConfirm, db, dialogs, schemaChanged, server, showError, type DbTab } from "./dbstate.svelte";
import { dbapi, message } from "./dbapi.svelte";
import type { MenuItem } from "./menu.svelte";
import type { ColumnInfo, Grid, TableInfo } from "./types";
import { rows as rowsText } from "./util";

export const PAGE_SIZE = 500;
export const SYSTEM_DATABASES = ["information_schema", "mysql", "performance_schema", "sys"];

function pref(key: string, fallback: boolean): boolean {
  try {
    const v = localStorage.getItem(key);
    return v === null ? fallback : v === "1";
  } catch {
    return fallback;
  }
}

export function savePref(key: string, value: boolean) {
  try {
    localStorage.setItem(key, value ? "1" : "0");
  } catch {
    // Storage unavailable: the choice lasts until the window closes.
  }
}

export const ui = $state({
  connectionError: null as string | null,
  /** Databases opened in the sidebar. */
  expanded: [] as string[],
  /** Size and default collation per loaded database (the header's summary). */
  dbInfo: {} as Record<string, { size: string; collation: string }>,
  /** The sidebar's System section is open (remembered). */
  showSystem: pref("db.showSystemDatabases", false),
  /** The row inspector beside the grid (remembered). */
  showInspector: pref("db.showInspector", false),
  /** The Query tab's result area (remembered). */
  resultsOpen: pref("db.queryResultsOpen", true),
  consoleOpen: false,
  /** The New Row sheet. */
  insertRow: false,
});

class DataState {
  /** Columns of the selected table (types, keys) — the grid's headers and the inspector's fields. */
  info = $state.raw<ColumnInfo[]>([]);
  page = $state.raw<Grid>({ columns: [], rows: [] });
  total = $state<number | null>(null);
  error = $state<string | null>(null);
  filter = $state("");
  orderBy = $state<string | null>(null);
  ascending = $state(true);
  offset = $state(0);
  /** Selected row indexes in `page.rows`. */
  selected = $state.raw<number[]>([]);
  /** Bumped when `page` is replaced. */
  version = $state(0);
}
export const data = new DataState();

class QueryState {
  text = $state("SELECT VERSION();");
  /** The database chosen in the Query bar; null follows the sidebar. */
  database = $state<string | null>(null);
  sets = $state.raw<Grid[]>([]);
  shown = $state(0);
  error = $state<string | null>(null);
  /** Where the error points in the text it ran (UTF-16 offsets). */
  errorRange = $state.raw<[number, number] | null>(null);
  /** "1 result, 7 rows · 0.091 s"; only the time after a failure. */
  message = $state("");
  /** A result had more rows than Lampion keeps; only the first 100,000 came. */
  truncated = $state(false);
  /** Bumped by every run. */
  version = $state(0);
  /** A run is in flight: Run waits for it. */
  running = $state(false);
  history = $state.raw<string[]>([]);
  /** The editor's share of the height while the results are shown. */
  split = $state(0.5);
  /** The editor's state between visits to the tab (text, selection, undo). */
  editor: EditorState | null = null;
  /** Set by the open editor: puts its latest text into `text` (updated only after a pause in typing). */
  flush: (() => void) | null = null;
}
export const query = new QueryState();

export const isSystem = (d: string) => SYSTEM_DATABASES.includes(d);

export function tablesOf(d: string | null) {
  return d ? server.tables[d] : undefined;
}

export function tableInfo(d: string | null, t: string | null) {
  return t ? tablesOf(d)?.find((x) => x.name === t) : undefined;
}

export function availableTabs(): DbTab[] {
  if (!db.database) return ["query"];
  return db.table ? ["data", "structure", "query"] : ["tables", "designer", "query"];
}

// ---- Connection and sidebar ----

/** Connects, lists the databases and reloads what the window shows (the toolbar's Refresh). */
export async function reloadAll() {
  try {
    const info = await dbapi.connect();
    server.version = info.version;
    server.databases = info.databases;
    ui.connectionError = null;
  } catch (e) {
    ui.connectionError = message(e);
    return;
  }
  const stale = new Set([...ui.expanded, ...Object.keys(server.tables)]);
  if (db.database) stale.add(db.database);
  for (const d of stale) {
    if (server.databases.includes(d)) loadTables(d);
    else {
      delete server.tables[d];
      ui.expanded = ui.expanded.filter((x) => x !== d);
    }
  }
  if (db.database && !server.databases.includes(db.database)) select(null);
  else if (db.table) loadTable();
}

/** Uses the root password the user typed and connects (once it works it is kept, encrypted, for the next launch). */
export async function connectWithPassword(password: string) {
  try {
    await dbapi.setPassword(password);
  } catch (e) {
    ui.connectionError = message(e);
    return;
  }
  await reloadAll();
}

/** The latest `loadTables` call per database: an older answer arriving later is dropped. */
const tablesSeq: Record<string, number> = {};

export async function loadTables(d: string) {
  const seq = (tablesSeq[d] = (tablesSeq[d] ?? 0) + 1);
  try {
    const info = await dbapi.tables(d);
    if (tablesSeq[d] !== seq) return;
    server.tables[d] = info.tables;
    ui.dbInfo[d] = { size: info.size, collation: info.collation };
    if (db.database === d && db.table) {
      const t = info.tables.find((x) => x.name === db.table);
      // A dropped table leaves its database selected; one opened before the list came learns if it is a view.
      if (!t) select(d);
      else db.isView = t.view;
    }
  } catch (e) {
    if (!ui.connectionError) console.warn(message(e));
  }
}

/** Opens a database once its tables are known, so its rows don't pop in after the disclosure. */
export async function setExpanded(d: string, open: boolean) {
  if (!open) {
    ui.expanded = ui.expanded.filter((x) => x !== d);
    return;
  }
  if (!server.tables[d]) await loadTables(d);
  else loadTables(d);
  if (!ui.expanded.includes(d)) ui.expanded = [...ui.expanded, d];
}

/** Selects a database (table null) or a table/view; null selects nothing. */
export function select(d: string | null, t: string | null = null) {
  db.database = d;
  db.table = d ? t : null;
  db.isView = !!tableInfo(d, t)?.view;
}

/** Applies the tab rules and loads what the new selection shows. Runs on every selection change. */
export function selectionChanged() {
  const d = db.database;
  const t = db.table;
  if (!d) {
    db.tab = "query";
    return;
  }
  if (!t) {
    // Moving between databases keeps Designer and Query.
    if (db.tab !== "query" && db.tab !== "designer") db.tab = "tables";
    loadTables(d);
    return;
  }
  // Moving between tables keeps Structure and Query; otherwise the data shows.
  if (db.tab !== "query" && db.tab !== "structure") db.tab = "data";
  if (!ui.expanded.includes(d)) ui.expanded = [...ui.expanded, d];
  if (!server.tables[d]) loadTables(d);
  data.filter = "";
  data.orderBy = null;
  data.ascending = true;
  data.offset = 0;
  data.total = null;
  data.info = [];
  data.page = { columns: [], rows: [] };
  data.error = null;
  data.selected = [];
  data.version++;
  loadTable();
}

// ---- Data tab ----

let loadSeq = 0;

/** Loads the selected table's columns, then its rows. */
export async function loadTable() {
  const d = db.database;
  const t = db.table;
  if (!d || !t) return;
  const seq = ++loadSeq;
  try {
    const info = await dbapi.columns(d, t);
    if (seq !== loadSeq) return;
    data.info = info;
  } catch (e) {
    if (seq !== loadSeq) return;
    data.error = message(e);
    return;
  }
  await loadData();
}

/** Loads the current page (filter, sort, offset); `after` runs once the rows are in. */
export async function loadData(after?: () => void) {
  const d = db.database;
  const t = db.table;
  if (!d || !t) return;
  const seq = ++loadSeq;
  try {
    const p = await dbapi.page(d, t, data.filter, data.orderBy, data.ascending, data.offset);
    if (seq !== loadSeq) return;
    data.error = null;
    data.page = { columns: p.columns.length ? p.columns : data.info.map((c) => c.name), rows: p.rows, cut: p.cut };
    data.total = p.total;
  } catch (e) {
    if (seq !== loadSeq) return;
    data.error = message(e);
    data.page = { columns: data.info.map((c) => c.name), rows: [] };
    data.total = null;
  }
  data.selected = [];
  data.version++;
  // A delete asked about rows that are no longer where they were.
  if (dialogs.confirm?.action === deleteAsked) dialogs.confirm = null;
  after?.();
}

export function sortBy(column: string) {
  if (data.orderBy === column) data.ascending = !data.ascending;
  else {
    data.orderBy = column;
    data.ascending = true;
  }
  data.offset = 0;
  loadData();
}

export function clearSort() {
  data.orderBy = null;
  loadData();
}

export function turnPage(delta: number) {
  data.offset = Math.max(0, data.offset + delta * PAGE_SIZE);
  loadData();
}

export function applyFilter() {
  data.offset = 0;
  loadData();
}

/** Columns that identify a row (as the backend matches them): primary key, else a UNIQUE NOT NULL one, else all. */
function keyIndexes(): number[] {
  const cols = data.page.columns;
  const pk = data.info.filter((c) => c.key === "PRI").map((c) => cols.indexOf(c.name));
  if (pk.length && pk.every((i) => i >= 0)) return pk;
  const u = data.info.find((c) => c.key === "UNI" && !c.nullable);
  if (u && cols.includes(u.name)) return [cols.indexOf(u.name)];
  return cols.map((_, i) => i);
}

/** After a reload, selects the row whose key values match `expected` (else the same position). */
function reselect(expected: (string | null)[], fallback: number) {
  const keys = keyIndexes();
  const found = data.page.rows.findIndex((r) => keys.every((k) => r[k] === expected[k]));
  if (found >= 0) data.selected = [found];
  else if (fallback < data.page.rows.length) data.selected = [fallback];
}

/** Writes changed fields of one row (grid edit, Set to NULL, inspector Save) and keeps it selected.
 * `version`: the `data.version` the row index belongs to (a menu opened before a reload); nothing is
 * written once the rows changed. Returns true when saved; a failed save keeps the rows (and the
 * inspector's draft) as they are. */
export async function updateRow(row: number, changes: { column: number; value: string | null }[], version = data.version): Promise<boolean> {
  const d = db.database;
  const t = db.table;
  const original = data.page.rows[row];
  if (!d || !t || !original || !changes.length || version !== data.version) return false;
  const expected = [...original];
  for (const c of changes) expected[c.column] = c.value;
  try {
    await dbapi.updateRow(d, t, data.page.columns, original, changes);
  } catch (e) {
    showError(message(e));
    return false;
  }
  await loadData(() => reselect(expected, row));
  return true;
}

/** The whole value of a cell the page got cut (64 KB); rejects when the row can't be found again. */
export async function wholeCell(row: number, column: number): Promise<string | null> {
  const d = db.database;
  const t = db.table;
  const values = data.page.rows[row];
  if (!d || !t || !values) throw "The row is no longer shown.";
  return dbapi.cell(d, t, data.page.columns, values, column);
}

/** The open delete confirmation's action; a reload closes it (see `loadData`). */
let deleteAsked: (() => void) | null = null;

export function askDeleteSelected() {
  const d = db.database;
  const t = db.table;
  // The rows as they are now: a reload while the question is open must not delete others.
  const columns = data.page.columns;
  const rows = data.selected.map((i) => data.page.rows[i]).filter(Boolean);
  const version = data.version;
  const n = rows.length;
  if (!d || !t || !n) return;
  const action = () => {
    deleteAsked = null;
    if (version === data.version) deleteRows(d, t, columns, rows);
  };
  deleteAsked = action;
  askConfirm({
    title: n === 1 ? "Delete the selected row?" : `Delete the ${n} selected rows?`,
    message: "This can't be undone.",
    button: "Delete",
    action,
  });
}

async function deleteRows(d: string, t: string, columns: string[], rows: (string | null)[][]) {
  try {
    await dbapi.deleteRows(d, t, columns, rows);
  } catch (e) {
    showError(message(e));
  }
  await loadData();
}

/** The New Row sheet's Insert. Returns true when the row was added. */
export async function insertRow(values: Record<string, string>): Promise<boolean> {
  const d = db.database;
  const t = db.table;
  if (!d || !t) return false;
  let ok = true;
  try {
    await dbapi.insertRow(d, t, values);
  } catch (e) {
    ok = false;
    showError(message(e));
  }
  await loadData();
  return ok;
}

// ---- Tables, databases, export/import ----

export function askEmptyTable(d: string, t: string) {
  askConfirm({
    title: `Empty the table “${t}”?`,
    message: "All rows in the table will be deleted. This can't be undone.",
    button: "Empty",
    action: () => ddl(dbapi.emptyTable(d, t)),
  });
}

export function askDropTable(d: string, t: string, view: boolean) {
  askConfirm({
    title: `Delete the ${view ? "view" : "table"} “${t}”?`,
    message: view ? "The view will be permanently deleted. The data it shows is not affected." : "The table and all its data will be permanently deleted.",
    button: "Delete",
    action: () => dropTable(d, t, view),
  });
}

/** Several tables and views from the Tables tab. */
export function askDropTables(d: string, items: { name: string; view: boolean }[]) {
  askConfirm({
    title: `Delete ${items.length} tables and views?`,
    message: `${items.map((i) => i.name).join(", ")} and all their data will be permanently deleted.`,
    button: "Delete",
    action: async () => {
      for (const i of items) await dropTable(d, i.name, i.view);
    },
  });
}

async function dropTable(d: string, t: string, view: boolean) {
  if (db.database === d && db.table === t) select(d);
  await ddl(dbapi.dropTable(d, t, view));
}

export function askDropDatabase(d: string) {
  askConfirm({
    title: `Delete the database “${d}”?`,
    message: "All tables and data in the database will be permanently deleted.",
    button: "Delete",
    action: async () => {
      if (db.database === d) select(null);
      ui.expanded = ui.expanded.filter((x) => x !== d);
      delete server.tables[d];
      await ddl(dbapi.dropDatabase(d));
    },
  });
}

/** Runs a schema change, shows its error, and lets every view reload. */
async function ddl(p: Promise<unknown>) {
  try {
    await p;
  } catch (e) {
    showError(message(e));
  }
  schemaChanged();
}

export async function exportItem(d: string, t: string | null = null) {
  try {
    await dbapi.export(d, t);
  } catch (e) {
    showError(message(e));
  }
}

export async function importInto(d: string | null, confirmed = false) {
  try {
    const r = await dbapi.import(d, confirmed);
    if (r.others.length && d) {
      // A whole-database export switches to its own database: its statements would not run in `d`.
      const names = r.others.map((o) => `“${o}”`).join(", ");
      askConfirm({
        title: `Import “${r.file}” into “${d}”?`,
        message: `The file switches to or creates ${r.others.length === 1 ? "the database" : "the databases"} ${names} (USE / CREATE DATABASE), so its statements run there, not in “${d}”.`,
        button: "Import Anyway",
        action: () => importInto(d, true),
      });
      return;
    }
    if (!r.path) return;
  } catch (e) {
    showError(message(e));
  }
  schemaChanged();
}

/** The actions for one table or view (sidebar and Tables tab). */
export function tableMenu(d: string, t: TableInfo, showOpen = true): MenuItem[] {
  const items: MenuItem[] = [];
  if (showOpen) items.push({ label: "Open", action: () => select(d, t.name) }, { separator: true });
  items.push({ label: "Export…", action: () => exportItem(d, t.name) });
  if (!t.view) {
    items.push({ label: "Empty Table…", action: () => askEmptyTable(d, t.name) });
    items.push({ label: "Table Options…", action: () => tableOptions(d, t.name) });
  }
  items.push({ separator: true }, { label: "Delete…", action: () => askDropTable(d, t.name, t.view) });
  return items;
}

/** The actions for a database (sidebar). */
export function databaseMenu(d: string): MenuItem[] {
  return [
    { label: "New Table…", action: () => newTable(d) },
    { label: "Export…", action: () => exportItem(d) },
    { label: "Import SQL…", action: () => importInto(d) },
    { separator: true },
    { label: "Delete Database…", action: () => askDropDatabase(d) },
  ];
}

export function newDatabase() {
  db.sheet = { kind: "newDatabase", database: db.database ?? "" };
}

export function newTable(d: string) {
  if (db.database !== d) select(d);
  db.sheet = { kind: "newTable", database: d };
}

export function tableOptions(d: string, t: string) {
  select(d, t);
  db.sheet = { kind: "tableOptions", database: d, table: t };
}

// ---- Query tab ----

export async function loadHistory() {
  try {
    query.history = await dbapi.history();
  } catch {
    query.history = [];
  }
}

export async function clearHistory() {
  await dbapi.clearHistory();
  query.history = [];
}

let runSeq = 0;

/** Runs the editor's text in the chosen database (or the sidebar's); one run at a time, in order. */
export async function runQuery() {
  query.flush?.();
  const text = query.text;
  if (!text.trim() || query.running) return;
  const seq = ++runSeq;
  query.running = true;
  try {
    const r = await dbapi.runQuery(query.database ?? db.database, text);
    if (seq !== runSeq) return;
    const time = `${r.seconds.toFixed(3)} s`;
    query.sets = r.sets;
    query.truncated = r.truncated;
    query.shown = 0;
    query.error = r.error;
    query.errorRange = r.error && r.errorFrom !== null && r.errorTo !== null ? [r.errorFrom, r.errorTo] : null;
    query.history = r.history;
    if (r.error) query.message = time;
    else if (!r.sets.length) query.message = r.affected !== null ? `Done – rows affected: ${r.affected} · ${time}` : `Done – 0 rows · ${time}`;
    else {
      const n = r.sets.reduce((a, s) => a + s.rows.length, 0);
      query.message = `${r.sets.length} ${r.sets.length === 1 ? "result" : "results"}, ${rowsText(n)} · ${time}`;
    }
    query.version++;
    // A statement without rows may have created, changed or dropped something: every view reloads.
    if (r.affected !== null) schemaChanged();
  } catch (e) {
    if (seq !== runSeq) return;
    query.sets = [];
    query.error = message(e);
    query.errorRange = null;
    query.message = "";
    query.version++;
  } finally {
    if (seq === runSeq) query.running = false;
  }
}
