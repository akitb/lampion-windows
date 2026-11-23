// Types shared across pages. Keep in sync with the Rust structs (serde camelCase).

export interface AppInfo {
  version: string;
  lampionRoot: string;
  args: string[];
}

export interface Settings {
  lampionRoot: string;
  openAtLogin: boolean;
  startServersOnOpen: boolean;
  stopServersOnQuit: boolean;
  checkForUpdates: boolean;
  skippedVersion: string | null;
  rootNoPassword: boolean | null;
  phpmyadminAutoLogin: boolean | null;
  useServices: boolean;
  [extra: string]: unknown;
}

export type Level = "info" | "ok" | "warn" | "error";

export interface ActivityLine {
  time: string;
  module: string;
  level: Level;
  text: string;
}

export type ServerId = "apache" | "mariadb";

/** What the sidebar, toolbar and Overview show for one server (sent as `servers://status`). */
export type ServerState = "running" | "stopped" | "busy" | "notInstalled";

export interface ServerStatus {
  state: ServerState;
  pids: number[];
  ports: number[];
  /** Configured port (shown even when stopped). */
  port: number;
  version: string | null;
}

export type Pane = "overview" | "logs" | "apache" | "php" | "mariadb" | "general";

// ---- config ----

/** A settings field the user has to fix (`field` is the settings key). */
export interface ConfigProblem {
  field: string;
  message: string;
}

export interface ConfigCheck {
  /** Saving is refused while there are problems. */
  problems: ConfigProblem[];
  /** Shown, but saving is allowed. */
  warnings: ConfigProblem[];
}

export interface SaveOutcome {
  /** Not saved: these fields are invalid. */
  problems: ConfigProblem[];
  /** The file was written (false: it already had these values). */
  changed: boolean;
}

/** `config_restore` / `config_edit_file` targets. */
export type ConfigFileId = "apache" | "vhosts" | "php" | "mariadb" | "phpmyadmin";

export interface ApacheSettings {
  port: string;
  documentRoot: string;
  serverAdmin: string;
  allowOverride: boolean;
  directoryListing: boolean;
  logLevel: string;
  /** Feature id → on. */
  modules: Record<string, boolean>;
}

export interface ApacheFeature {
  id: string;
  title: string;
  detail: string;
  modules: string[];
  needs: string[];
}

export interface ApacheForm {
  settings: ApacheSettings;
  file: string;
  hasBackup: boolean;
  features: ApacheFeature[];
  logLevels: string[];
}

export interface PhpSettings {
  memoryLimit: string;
  uploadMaxFilesize: string;
  postMaxSize: string;
  maxExecutionTime: string;
  maxInputTime: string;
  maxInputVars: string;
  displayErrors: boolean;
  displayStartupErrors: boolean;
  errorReporting: string;
  logErrors: boolean;
  separateErrorLog: boolean;
  /** Empty: not set. */
  timezone: string;
  shortOpenTag: boolean;
  opcache: boolean;
  sessionLifetime: string;
}

export interface PhpForm {
  settings: PhpSettings;
  file: string;
  hasBackup: boolean;
  /** The file "Separate PHP error log" uses. */
  errorLog: string;
  systemTimezone: string | null;
}

export interface PhpModules {
  version: string;
  extensions: string[];
}

export type SqlMode = "default" | "strict" | "relaxed" | "custom";

/** Empty text fields mean MariaDB's built-in default. */
export interface MariadbSettings {
  port: string;
  localOnly: boolean;
  maxConnections: string;
  maxAllowedPacket: string;
  bufferPoolSize: string;
  characterSet: string;
  collation: string;
  sqlMode: SqlMode;
  customSqlMode: string;
  slowQueryLog: boolean;
  longQueryTime: string;
  generalLog: boolean;
}

export interface Charset {
  charset: string;
  collations: string[];
}

export interface MariadbForm {
  settings: MariadbSettings;
  file: string;
  hasBackup: boolean;
  /** MariaDB's built-in values, shown greyed in empty fields. */
  defaults: MariadbSettings;
  charsets: Charset[];
}

/** A "Copy Settings" text. */
export interface Snippet {
  id: string;
  title: string;
  text: string;
}

export interface ConfigBackup {
  id: ConfigFileId;
  name: string;
  file: string;
  backup: string;
}

/** Ports and paths read fresh from httpd.conf, my.ini and php.ini (also the `config://locations` event). */
export interface Locations {
  apachePort: number;
  documentRoot: string;
  apacheErrorLog: string;
  apacheAccessLog: string;
  mariadbPort: number;
  mariadbDatadir: string;
  mariadbErrorLog: string | null;
  phpErrorLog: string | null;
  slowQueryLog: string | null;
  generalQueryLog: string | null;
}

export interface LampionFolders {
  root: string;
  bin: string;
  logs: string;
  appLog: string;
}

// ---- servers ----

/** The setup banner (`servers://setup`, `setup_state`). */
export type SetupPhase = "ready" | "chooseFolder" | "needsRuntime" | "running" | "failed" | "missing";

export interface SetupState {
  phase: SetupPhase;
  /** The Lampion folder (the one being chosen, or the current one). */
  root: string;
  /** What runs now, or what is wrong. */
  title: string;
  detail: string;
  /** 0…1 while downloading or unpacking; null = no measurable progress. */
  progress: number | null;
  /** Missing parts by name ("Apache", "the configuration", …). */
  missing: string[];
  canCancel: boolean;
  /** Nothing is installed in the folder yet, so another one may be chosen. */
  canChangeRoot: boolean;
}

/** The Overview's PHP card (`php_info`). */
export interface PhpInfo {
  installed: boolean;
  version: string | null;
  /** httpd.conf loads PHP and Apache runs. */
  linked: boolean;
  memoryLimit: string | null;
  uploadMaxFilesize: string | null;
}

// ---- logs ----

/** The Logs page's sources; also the `detail.source` of the `logs:show` window event. */
export type LogSource = "activity" | "apacheError" | "apacheAccess" | "php" | "mariadb" | "mariadbSlow" | "mariadbGeneral";

/** One line of a server log, split into time · tag · message. */
export interface LogLine {
  id: number;
  level: Level;
  time: string | null;
  tag: string | null;
  /** The line without its time and tag; null: show `text`. */
  message: string | null;
  /** The line as written (copied and searched). */
  text: string;
}

/** A server log's state (`logs_follow`, `logs_reload`, `logs_clear`) and its changes (`logs://update`). */
export interface LogUpdate {
  source: LogSource;
  /** Grows with every update; anything not newer than what the page has is ignored. */
  seq: number;
  /** `lines` replace the list instead of adding to it. */
  reset: boolean;
  lines: LogLine[];
  exists: boolean;
  size: number;
  path: string;
}

/** A source chip (`logs_sources`); Lampion Activity's path is Lampion.log. */
export interface LogSourceInfo {
  source: LogSource;
  path: string;
  exists: boolean;
}

// ---- app ----

export interface Release {
  version: string;
  /** The release on GitHub. */
  page: string;
  /** Only the "What's new" part of the release notes (Markdown). */
  notes: string;
}

/** The update banner (`update_state`, `update://state`). */
export type UpdateState =
  | { state: "idle" }
  | { state: "checking" }
  | { state: "upToDate" }
  | { state: "available"; release: Release }
  | { state: "downloading"; release: Release; progress: number }
  | { state: "installing"; release: Release }
  | { state: "failed"; message: string; /** The releases page ("Download Manually"). */ page: string };

// ---- database structure ----

/** A column as SHOW FULL COLUMNS describes it (`db_structure_load`). */
export interface StructureColumn {
  name: string;
  type: string;
  collation: string;
  nullable: boolean;
  /** "PRI", "UNI", "MUL" or "". */
  key: string;
  /** null: no default (or NULL); "" is an empty text default. */
  default: string | null;
  extra: string;
  comment: string;
}

/** One index with its columns in index order. */
export interface StructureIndex {
  name: string;
  columns: string[];
  unique: boolean;
  /** BTREE, HASH, FULLTEXT, SPATIAL. */
  type: string;
}

export interface StructureForeignKey {
  name: string;
  columns: string[];
  refSchema: string;
  refTable: string;
  refColumns: string[];
  onUpdate: string;
  onDelete: string;
}

/** Everything the Structure tab shows about a table or view (`db_structure_load`). */
export interface TableStructure {
  view: boolean;
  engine: string;
  collation: string;
  /** The server's estimate; null for views. */
  rows: number | null;
  bytes: number;
  /** "48 KB" */
  size: string;
  columns: StructureColumn[];
  indexes: StructureIndex[];
  foreignKeys: StructureForeignKey[];
  createSql: string;
}

/** A column as the Add/Edit Column sheet and New Table edit it. */
export interface ColumnDraft {
  name: string;
  type: string;
  nullable: boolean;
  /** Empty: no default. NULL, numbers, quoted text and CURRENT_TIMESTAMP are used as written; other text is quoted. */
  defaultText: string;
  autoIncrement: boolean;
  /** New Table only. */
  primaryKey: boolean;
  /** ON UPDATE CURRENT_TIMESTAMP */
  onUpdateTimestamp: boolean;
  comment: string;
}

/** Where a saved column goes ("unchanged": at the end when adding, where it was when editing). */
export type ColumnPosition = { kind: "unchanged" } | { kind: "first" } | { kind: "after"; column: string };

export type IndexKind = "index" | "unique" | "primary" | "fulltext";

export interface ForeignKeyDraft {
  /** Empty: the server names it. */
  name: string;
  column: string;
  refTable: string;
  refColumn: string;
  onDelete: string;
  onUpdate: string;
}

/** What the sheets' pickers offer (`db_structure_choices`). */
export interface StructureChoices {
  engines: string[];
  collations: { name: string; detail: string }[];
  defaultCollation: string;
  rules: string[];
}

/** `db_structure_check_name`: the name's problem, and whether it needs no quoting. */
export interface NameCheck {
  problem: string | null;
  plain: boolean;
}
