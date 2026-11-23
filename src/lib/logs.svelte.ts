// The Lampion Activity feed behind the Logs badge (`app.logErrors`: errors in it since launch) and the
// Logs page's chosen source, which stays when the page is left. Owned by the logs part.
import { call, core, on } from "./api";
import { app } from "./state.svelte";
import { activityRow, isSource, type Row } from "./logs/rows";
import type { ActivityLine, LogSource } from "./types";

/** As many lines as the core keeps in memory. */
const MAX_ACTIVITY = 2000;

class Logs {
  /** Lampion Activity, oldest first. Replaced (never changed in place) so the page can tell. */
  activity = $state.raw<Row[]>([]);
  source = $state<LogSource>("activity");
}

export const logs = new Logs();

function add(lines: ActivityLine[]) {
  if (!lines.length) return;
  const all = [...logs.activity, ...lines.map(activityRow)];
  logs.activity = all.length > MAX_ACTIVITY ? all.slice(-MAX_ACTIVITY) : all;
  app.logErrors = logs.activity.reduce((n, r) => n + (r.level === "error" ? 1 : 0), 0);
}

const same = (a: ActivityLine, b: ActivityLine) => a.time === b.time && a.module === b.module && a.level === b.level && a.text === b.text;

/** How many of the lines that arrived while loading are already at the end of the loaded list. */
function overlap(loaded: ActivityLine[], early: ActivityLine[]) {
  for (let k = Math.min(loaded.length, early.length); k > 0; k--) {
    if (early.slice(0, k).every((l, i) => same(loaded[loaded.length - k + i], l))) return k;
  }
  return 0;
}

let started = false;

/** Starts the activity feed (and with it the badge) and listens for `logs:show` from other pages. Once. */
export function startLogBadge(): void {
  if (started) return;
  started = true;
  window.addEventListener("logs:show", (e) => {
    const source = (e as CustomEvent<{ source?: unknown }>).detail?.source;
    if (isSource(source)) logs.source = source;
  });
  let early: ActivityLine[] | null = [];
  void on<ActivityLine>("activity://line", (line) => (early ? early.push(line) : add([line])));
  const loaded = (all: ActivityLine[]) => {
    const waiting = early ?? [];
    early = null;
    add(all);
    add(waiting.slice(overlap(all, waiting)));
  };
  core.activityLines().then(loaded, () => loaded([]));
}

/** Empties the Lampion Activity list (the file on disk is kept). */
export async function clearActivity() {
  await call("logs_clear", { source: "activity" });
  logs.activity = [];
  app.logErrors = 0;
}
