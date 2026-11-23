// What the Logs page draws: one row per line, the same for Lampion Activity and the server logs.
import type { ActivityLine, Level, LogLine, LogSource } from "$lib/types";

export interface Row {
  id: number;
  level: Level;
  time: string | null;
  tag: string | null;
  /** Shown text (the line without its time and tag). */
  message: string;
  /** The line as written: copied, and searched. */
  raw: string;
  /** `raw` in lowercase, made once, so searching 5000 lines stays quick. */
  lower: string;
}

export function fileRow(l: LogLine): Row {
  return { id: l.id, level: l.level, time: l.time, tag: l.tag, message: l.message ?? l.text, raw: l.text, lower: l.text.toLowerCase() };
}

let activityId = 0;
export function activityRow(l: ActivityLine): Row {
  const raw = `${l.time} [${l.module}] ${l.text}`;
  return { id: ++activityId, level: l.level, time: l.time, tag: l.module, message: l.text, raw, lower: raw.toLowerCase() };
}

export const sourceTitles: Record<LogSource, string> = {
  activity: "Lampion Activity",
  apacheError: "Apache Errors",
  apacheAccess: "Apache Requests",
  php: "PHP Errors",
  mariadb: "MariaDB",
  mariadbSlow: "Slow Queries",
  mariadbGeneral: "All Queries",
};

export const sources = Object.keys(sourceTitles) as LogSource[];

export function isSource(s: unknown): s is LogSource {
  return typeof s === "string" && s in sourceTitles;
}

/** Shown when the file does not exist yet. */
export function missingHint(s: LogSource): string {
  switch (s) {
    case "php":
      return "Turn on “Separate PHP error log” in PHP settings. Until then PHP errors go to Apache Errors.";
    case "mariadbSlow":
      return "Turn on the slow query log in MariaDB settings.";
    case "mariadbGeneral":
      return "Turn on “Log every query” in MariaDB settings.";
    default:
      return "The file is created the first time the server writes to it.";
  }
}

const palette = ["--log-blue", "--log-teal", "--log-indigo", "--log-brown", "--log-cyan", "--log-mint"];

/** The tag's colour: HTTP status class, a known module or level, or a stable colour from the text. */
export function tagColor(tag: string): string {
  if (/^\d+$/.test(tag)) {
    const code = Number(tag);
    if (code >= 100 && code <= 599) return code >= 500 ? "var(--red)" : code >= 400 ? "var(--log-orange)" : code >= 300 ? "var(--log-blue)" : "var(--green)";
  }
  switch (tag.toLowerCase()) {
    case "apache": return "var(--tint-apache)";
    case "mysql": case "mariadb": return "var(--tint-mariadb)";
    case "php": return "var(--tint-php)";
    case "setup": return "var(--log-purple)";
    case "test": return "var(--log-pink)";
    case "error": case "fatal error": case "parse error": return "var(--red)";
    case "warning": case "warn": case "deprecated": case "notice": return "var(--log-orange)";
  }
  let sum = 0;
  for (const ch of tag) sum += ch.codePointAt(0) ?? 0;
  return `var(${palette[sum % palette.length]})`;
}

/** Text split around the (case-insensitive) matches of `q`, for highlighting. */
export function marks(text: string, q: string): { text: string; hit: boolean }[] {
  const lower = text.toLowerCase();
  const ql = q.toLowerCase();
  // Lowercasing changed the length (rare letters): no highlight rather than a wrong one.
  if (!ql || lower.length !== text.length) return [{ text, hit: false }];
  const out: { text: string; hit: boolean }[] = [];
  let i = 0;
  for (let j = lower.indexOf(ql); j >= 0; j = lower.indexOf(ql, j + ql.length)) {
    if (j > i) out.push({ text: text.slice(i, j), hit: false });
    out.push({ text: text.slice(j, j + ql.length), hit: true });
    i = j + ql.length;
  }
  if (i < text.length) out.push({ text: text.slice(i), hit: false });
  return out;
}

/** Like macOS: "749 bytes", "2 KB", "1.5 MB" (1 KB = 1000 bytes). */
export function fileSize(n: number): string {
  if (n === 0) return "Zero KB";
  if (n < 1000) return `${n} ${n === 1 ? "byte" : "bytes"}`;
  if (n < 1e6) return `${Math.round(n / 1e3)} KB`;
  if (n < 1e9) return `${(n / 1e6).toFixed(1)} MB`;
  return `${(n / 1e9).toFixed(2)} GB`;
}
