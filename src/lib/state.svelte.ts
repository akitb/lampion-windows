// Main-window state shared between the shell and the pages (Svelte 5 runes).
// Each feature owns its own fields: servers → `servers`; config pages → `unsaved`; logs → `logErrors`.
import type { Pane, ServerId, ServerStatus } from "./types";

const stopped = (port: number): ServerStatus => ({ state: "stopped", pids: [], ports: [], port, version: null });

export const app = $state({
  pane: "overview" as Pane,
  /** Latest status per server; replaced only when the core reports a real change. */
  servers: { apache: stopped(8080), mariadb: stopped(3306) } as Record<ServerId, ServerStatus>,
  /** Settings pages with unsaved edits (orange pencil in the sidebar). */
  unsaved: [] as Pane[],
  /** Errors in the activity log since launch (badge on Logs). */
  logErrors: 0,
});

export function setUnsaved(pane: Pane, dirty: boolean) {
  const has = app.unsaved.includes(pane);
  if (dirty && !has) app.unsaved.push(pane);
  if (!dirty && has) app.unsaved = app.unsaved.filter((p) => p !== pane);
}
