// Types of the Database Manager's data/query commands (Rust: src-tauri/src/database/commands.rs, serde camelCase).

export interface ServerInfo {
  /** "MariaDB 11.4.2" */
  version: string;
  databases: string[];
}

export interface TableInfo {
  name: string;
  view: boolean;
  engine: string;
  collation: string;
  /** The server's estimate; null for views. */
  rows: number | null;
  /** "1.2k" */
  rowsShort: string | null;
  bytes: number;
  /** "48 KB" */
  size: string;
}

export interface DatabaseInfo {
  tables: TableInfo[];
  size: string;
  collation: string;
}

export type TypeFamily = "integer" | "decimal" | "text" | "temporal" | "binary" | "enumSet" | "json" | "spatial" | "boolean" | "other";

/** A column from SHOW FULL COLUMNS, with derived flags. */
export interface ColumnInfo {
  name: string;
  type: string;
  nullable: boolean;
  /** "PRI", "UNI", "MUL" or "". */
  key: string;
  default: string | null;
  extra: string;
  comment: string;
  collation: string;
  family: TypeFamily;
  numeric: boolean;
  binary: boolean;
  autoIncrement: boolean;
  /** NOT NULL, no default, not automatic: an insert needs a value. */
  required: boolean;
  enumValues: string[];
}

/** Rows as text; null is SQL NULL. */
export interface Grid {
  columns: string[];
  rows: (string | null)[][];
}

export interface Page extends Grid {
  total: number | null;
}

export interface QueryRun {
  sets: Grid[];
  affected: number | null;
  error: string | null;
  errorFrom: number | null;
  errorTo: number | null;
  seconds: number;
  history: string[];
}

export interface Completion {
  prefixLen: number;
  qualified: boolean;
  items: string[];
}

export interface SyntaxWords {
  keywords: string[];
  functions: string[];
  types: string[];
}

/** The `database://sql` event: one console entry. */
export interface SqlLogEntry {
  sql: string;
  error: string | null;
  millis: number;
}
