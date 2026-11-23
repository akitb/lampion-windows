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
