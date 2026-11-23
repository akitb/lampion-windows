// Small helpers for the Database Manager views.
import { writeText } from "@tauri-apps/plugin-clipboard-manager";

export async function copy(text: string) {
  await writeText(text);
}

let ctx: CanvasRenderingContext2D | null = null;

/** Text width in px for a CSS font, e.g. "600 11.5px Segoe UI". */
export function measure(text: string, font: string): number {
  ctx ??= document.createElement("canvas").getContext("2d");
  if (!ctx) return text.length * 7;
  ctx.font = font;
  return ctx.measureText(text).width;
}

/** Shortens text in the middle ("informati…_schema") so it fits `width` px. */
export function middle(text: string, width: number, font: string): string {
  if (measure(text, font) <= width) return text;
  const chars = [...text];
  let lo = 0;
  let hi = chars.length;
  // Largest number of kept characters that fits, split evenly around the ellipsis.
  while (lo < hi) {
    const mid = Math.ceil((lo + hi) / 2);
    const head = Math.ceil(mid / 2);
    const s = chars.slice(0, head).join("") + "…" + chars.slice(chars.length - (mid - head)).join("");
    if (measure(s, font) <= width) lo = mid;
    else hi = mid - 1;
  }
  const head = Math.ceil(lo / 2);
  return chars.slice(0, head).join("") + "…" + chars.slice(chars.length - (lo - head)).join("");
}

/** Short, single-line values can be edited in place; longer or multi-line ones only in the inspector. */
export function isSimple(text: string): boolean {
  return text.length <= 2000 && !text.includes("\n") && !text.includes("\r");
}

/** A value on one line: line breaks shown as "⏎", cut at 300 characters. */
export function flatten(text: string): string {
  if (isSimple(text)) return text;
  return text.slice(0, 300).replaceAll("\r", "").replaceAll("\n", " ⏎ ");
}

/** 1234 → "1 234" / "1,234" (the system's grouping). */
export function count(n: number): string {
  return n.toLocaleString();
}

/** "1 row", "12 rows". */
export function rows(n: number): string {
  return n === 1 ? "1 row" : `${count(n)} rows`;
}

/** The first non-empty line of a query, shortened for a menu. */
export function summary(query: string): string {
  const line = query.split(/\r?\n/).map((l) => l.trim()).find((l) => l) ?? query;
  const more = line !== query.trim();
  if (line.length > 70) return line.slice(0, 70) + "…";
  return more ? line + " …" : line;
}

/** Whether a reduced-motion preference is set (animations become fades or nothing). */
export function reducedMotion(): boolean {
  return matchMedia("(prefers-reduced-motion: reduce)").matches;
}
