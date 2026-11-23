// What kind of value a column holds, from its SQL type (drives colours and icons), and the parts of a
// type the column sheet edits separately: `DECIMAL`, `10,2`, `UNSIGNED`.
import type { Component } from "svelte";
import { Hash, CaseSensitive, Calendar, MemoryStick, List, Braces, MapPin, SquareCheck, SquareDashed } from "@lucide/svelte";

export type Family = "integer" | "decimal" | "text" | "temporal" | "binary" | "enumSet" | "json" | "spatial" | "boolean" | "other";

export function family(type: string): Family {
  const t = type.trim().toLowerCase();
  const any = (list: string[]) => list.some((p) => t.startsWith(p));
  if (t.startsWith("tinyint(1)") || t === "bool" || t === "boolean") return "boolean";
  if (any(["int", "tinyint", "smallint", "mediumint", "bigint", "integer", "bit", "serial"])) return "integer";
  if (any(["decimal", "numeric", "float", "double", "real"])) return "decimal";
  if (any(["date", "time", "year", "timestamp"])) return "temporal";
  if (any(["enum", "set"])) return "enumSet";
  if (t.startsWith("json")) return "json";
  if (any(["blob", "binary", "varbinary", "tinyblob", "mediumblob", "longblob"])) return "binary";
  if (any(["geometry", "point", "linestring", "polygon", "multi"])) return "spatial";
  if (any(["char", "varchar", "text", "tinytext", "mediumtext", "longtext", "uuid", "inet"])) return "text";
  return "other";
}

export const isNumeric = (f: Family) => f === "integer" || f === "decimal";

/** The family's colour (a CSS value; the `--st-*` ones need an `.st` ancestor). */
export const familyColor: Record<Family, string> = {
  integer: "var(--st-blue)",
  decimal: "var(--st-blue)",
  text: "var(--st-green)",
  temporal: "var(--st-orange)",
  binary: "var(--st-gray)",
  spatial: "var(--st-gray)",
  enumSet: "var(--st-purple)",
  json: "var(--st-teal)",
  boolean: "var(--st-pink)",
  other: "var(--text-2)",
};

export const familyIcon: Record<Family, Component<any>> = {
  integer: Hash,
  decimal: Hash,
  text: CaseSensitive,
  temporal: Calendar,
  binary: MemoryStick,
  enumSet: List,
  json: Braces,
  spatial: MapPin,
  boolean: SquareCheck,
  other: SquareDashed,
};

/** Common types for the type menu, grouped like the macOS app. */
export const typePresets = ["INT", "INT UNSIGNED", "BIGINT", "TINYINT(1)", "DECIMAL(10,2)", "FLOAT", "DOUBLE", "VARCHAR(255)", "CHAR(10)", "TEXT", "LONGTEXT", "DATE", "DATETIME", "TIMESTAMP", "TIME", "JSON", "BLOB"];

export const typeGroups: { title: string; types: string[] }[] = (() => {
  const group = (t: string) => {
    const f = family(t);
    if (isNumeric(f) || f === "boolean") return "Numbers";
    if (f === "text" || f === "enumSet") return "Text";
    if (f === "temporal") return "Date & Time";
    return "Other";
  };
  return ["Numbers", "Text", "Date & Time", "Other"].map((title) => ({ title, types: typePresets.filter((t) => group(t) === title) })).filter((g) => g.types.length);
})();

/** A type split into its name, the part in brackets and what follows. */
export interface TypeParts {
  base: string;
  args: string;
  modifiers: string;
}

export function typeParts(type: string): TypeParts {
  const t = type.trim();
  const open = t.indexOf("(");
  const close = t.lastIndexOf(")");
  if (open >= 0 && close > open) return { base: t.slice(0, open).trim(), args: t.slice(open + 1, close), modifiers: t.slice(close + 1).trim() };
  const space = t.indexOf(" ");
  if (space >= 0) return { base: t.slice(0, space), args: "", modifiers: t.slice(space).trim() };
  return { base: t, args: "", modifiers: "" };
}

export function joinType(p: TypeParts): string {
  return p.base + (p.args ? `(${p.args})` : "") + (p.modifiers ? " " + p.modifiers : "");
}

const words = (s: string) => s.split(" ").filter(Boolean);

export function isUnsigned(p: TypeParts): boolean {
  return words(p.modifiers).some((w) => w.toUpperCase() === "UNSIGNED");
}

/** UNSIGNED on or off, written in the type's own case. */
export function withUnsigned(p: TypeParts, on: boolean): TypeParts {
  const rest = words(p.modifiers).filter((w) => w.toUpperCase() !== "UNSIGNED");
  if (on) rest.unshift(p.base === p.base.toLowerCase() ? "unsigned" : "UNSIGNED");
  return { ...p, modifiers: rest.join(" ") };
}
