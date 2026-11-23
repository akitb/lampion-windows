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
