// The only way the UI talks to the Rust core: commands (`call`) and events (`on`).
import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import type { ActivityLine, AppInfo, Settings } from "./types";

/** Runs a Rust command. Errors arrive as the command's `Err(String)` text. */
export function call<T = void>(cmd: string, args?: Record<string, unknown>): Promise<T> {
  return invoke<T>(cmd, args);
}

/** Listens to an event from the core; returns the unsubscribe function. */
export function on<T>(event: string, handler: (payload: T) => void): Promise<UnlistenFn> {
  return listen<T>(event, (e) => handler(e.payload));
}

export const core = {
  appInfo: () => call<AppInfo>("app_info"),
  getSettings: () => call<Settings>("get_settings"),
  setSettings: (patch: Partial<Settings>) => call<Settings>("set_settings", { patch }),
  activityLines: () => call<ActivityLine[]>("activity_lines"),
  openPath: (path: string) => call("open_path", { path }),
  openUrl: (url: string) => call("open_url", { url }),
  openTerminal: (dir: string) => call("open_terminal", { dir }),
  openDatabaseWindow: () => call("open_database_window"),
  /** Native Windows dialog; resolves to the chosen path or null when cancelled. */
  pickPath: (opts: { kind: "folder" | "open" | "save"; title: string; startDir?: string; filter?: [string, string]; defaultName?: string }) =>
    call<string | null>("pick_path", { kind: opts.kind, title: opts.title, startDir: opts.startDir ?? null, filter: opts.filter ?? null, defaultName: opts.defaultName ?? null }),
};
