// The Designer's geometry and words as pure functions (port of the macOS DesignerGeometry, DesignerData and
// the DesignerStore helpers): card sizes, where the relation lines run, the automatic layout, hit tests, zoom
// and pan, and the schema cache. Everything is in "world" points (the diagram at 100 %); the canvas scales
// and pans the world. Only type imports, so `node --test` loads this file as is (diagram.test.mjs).
import type { DesignerColumn, DesignerNewRelation, DesignerRelation, DesignerSchema, DesignerTable } from "$lib/types";

export interface Point { x: number; y: number }
export interface Size { w: number; h: number }
export interface Rect { x: number; y: number; w: number; h: number }

export const METRICS = {
  headerHeight: 36,
  rowHeight: 22,
  bottomPadding: 6,
  minWidth: 170,
  maxWidth: 340,
  /** Above this many columns a card shows the first ones and the key columns, then "+ N more". */
  maxRows: 24,
  firstRows: 16,
  iconSlot: 30,
  padding: 10,
  handleRadius: 5,
  arrowLength: 8,
} as const;
const M = METRICS;

// ---- Types and words ----

const INTEGERS = ["int", "tinyint", "smallint", "mediumint", "bigint", "integer"];

/** `int(10) unsigned` → `int unsigned`, `varchar(255)` stays: integer display widths are noise in a diagram. */
export function compactType(raw: string): string {
  let t = raw.toLowerCase();
  const open = t.indexOf("(");
  const close = open < 0 ? -1 : t.indexOf(")", open);
  if (close >= 0 && INTEGERS.includes(t.slice(0, open)) && !t.startsWith("tinyint(1)")) t = t.slice(0, open) + t.slice(close + 1);
  if (t.startsWith("enum(") || t.startsWith("set(")) t = t.slice(0, t.indexOf("("));
  t = t.replaceAll(" zerofill", "");
  const chars = [...t];
  return chars.length > 22 ? chars.slice(0, 21).join("") + "…" : t;
}

export type TypeFamily = "integer" | "decimal" | "text" | "temporal" | "binary" | "enumSet" | "json" | "spatial" | "boolean" | "other";

/** What kind of value a column holds (the chip colour); the same rules as the Data tab's. */
export function typeFamily(raw: string): TypeFamily {
  const t = raw.trim().toLowerCase();
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

/** A relation's identity on the diagram (constraint names are unique per table). */
export const relationId = (r: DesignerRelation) => r.table + "\u0001" + r.name;

/** `orders.customer_id → customers.id`; composite keys `a.(x, y) → b.(x, y)`. */
export function ends(r: DesignerRelation): string {
  const side = (table: string, columns: string[]) => (columns.length === 1 ? `${table}.${columns[0]}` : `${table}.(${columns.join(", ")})`);
  return `${side(r.table, r.columns)} → ${side(r.refTable, r.refColumns)}`;
}

/** `orders.customer_id → customers.id · ON DELETE CASCADE` */
export function summary(r: DesignerRelation): string {
  let text = `${ends(r)} · ON DELETE ${r.onDelete}`;
  if (r.onUpdate !== "RESTRICT" && r.onUpdate !== "NO ACTION") text += ` · ON UPDATE ${r.onUpdate}`;
  return text;
}

const plural = (n: number, word: string) => `${n} ${word}${n === 1 ? "" : "s"}`;

/** "8 tables · 1 view · 8 relations", plus "· 3 hidden" while "Relations only" hides tables. */
export function countText(s: DesignerSchema, visible: number, relationsOnly: boolean): string {
  const views = s.tables.filter((t) => t.isView).length;
  const parts = [plural(s.tables.length - views, "table")];
  if (views > 0) parts.push(plural(views, "view"));
  parts.push(plural(s.relations.length, "relation"));
  const hidden = s.tables.length - visible;
  if (relationsOnly && hidden > 0) parts.push(`${hidden} hidden`);
  return parts.join(" · ");
}

/** A row's tooltip: `customer_id · int(10) unsigned · indexed · NOT NULL`. */
export function rowHelp(c: DesignerColumn, external?: string): string {
  const parts = [`${c.name} · ${c.type}`];
  if (c.key === "PRI") parts.push("primary key");
  else if (c.key === "UNI") parts.push("unique");
  else if (c.key === "MUL") parts.push("indexed");
  parts.push(c.nullable ? "NULL allowed" : "NOT NULL");
  if (external) parts.push(`refers to ${external}`);
  return parts.join(" · ");
}

/** A plain-language hint for MariaDB's usual refusals. */
export function explain(error: string): string | null {
  const e = error.toLowerCase();
  if (e.includes("errno: 150") || e.includes("incorrectly formed"))
    return "The two columns usually differ in type, signedness or collation, or the referenced column has no index.";
  if (e.includes("1452") || e.includes("child row"))
    return "Some rows have a value that does not exist in the referenced table. Fix or remove those rows first, or allow NULL and clear them.";
  if (e.includes("1215") || e.includes("cannot add foreign key"))
    return "Check that both columns have the same type and that the referenced column is indexed.";
  if (e.includes("1826") || e.includes("duplicate")) return "A constraint with this name already exists in the database. Choose another name.";
  if (e.includes("1830") || e.includes("set null")) return "SET NULL needs a column that allows NULL.";
  return null;
}

const ident = (name: string) => "`" + name.replaceAll("`", "``") + "`";

/** The SQL the delete confirmation shows: the text the backend runs (relation.rs `drop_sql`). */
export const dropSql = (db: string, table: string, name: string) => `ALTER TABLE ${ident(db)}.${ident(table)} DROP FOREIGN KEY ${ident(name)};`;

const KEYWORDS = new Set(["ALTER", "TABLE", "ADD", "CONSTRAINT", "FOREIGN", "KEY", "REFERENCES", "ON", "DELETE", "UPDATE",
  "RESTRICT", "CASCADE", "SET", "NULL", "NO", "ACTION", "DROP"]);

/** The relation SQL in coloured pieces: keywords, `names` and the rest. */
export function sqlTokens(sql: string): { text: string; kind: "keyword" | "name" | "plain" }[] {
  return [...sql.matchAll(/`(?:[^`]|``)*`|[A-Za-z_]+|[^`A-Za-z_]+/g)].map(([text]) => ({
    text,
    kind: text.startsWith("`") ? "name" : KEYWORDS.has(text) ? "keyword" : "plain",
  }));
}

const isKey = (c: DesignerColumn) => c.key === "PRI" || c.key === "UNI";

/** A drag from one column onto another: the dragged column refers to the target, except that dragging from a
 *  key onto a plain column (or from an indexed one onto an unindexed one) reads the other way round. */
export function linkRelation(table: string, from: DesignerColumn, refTable: string, to: DesignerColumn): DesignerNewRelation {
  const swap = (isKey(from) && !isKey(to)) || (!isKey(to) && from.key !== "" && to.key === "");
  return swap
    ? { table: refTable, column: to.name, refTable: table, refColumn: from.name }
    : { table, column: from.name, refTable, refColumn: to.name };
}

/** The tables and relations kept bright while a relation or a table is hovered (or dragged); null: nothing dims. */
export function emphasis(s: DesignerSchema, relation: DesignerRelation | null, table: string | null) {
  if (relation) return { tables: new Set([relation.table, relation.refTable]), relations: new Set([relationId(relation)]) };
  if (table === null) return null;
  const tables = new Set([table]);
  const relations = new Set<string>();
  for (const r of s.relations) {
    if (r.table !== table && r.refTable !== table) continue;
    relations.add(relationId(r));
    tables.add(r.table);
    tables.add(r.refTable);
  }
  return { tables, relations };
}

// ---- Cards ----

export interface CardLayout {
  width: number;
  height: number;
  /** Indexes into the table's columns, in order. */
  rows: number[];
  /** Columns not shown (0 when the card shows all of them). */
  hidden: number;
}

/** Text widths: one character of the column-name and type fonts (monospaced), and a title in its font. */
export interface Measure {
  nameChar: number;
  chipChar: number;
  title: (text: string) => number;
}

/** Columns that take part in relations inside the database, per table (both ends). */
export function relationColumns(s: DesignerSchema): Map<string, Set<string>> {
  const out = new Map<string, Set<string>>();
  const add = (t: string, cols: string[]) => {
    const set = out.get(t) ?? new Set<string>();
    cols.forEach((c) => set.add(c));
    out.set(t, set);
  };
  for (const r of s.relations) {
    add(r.table, r.columns);
    add(r.refTable, r.refColumns);
  }
  return out;
}

/** A card's size and the columns it shows: all of them, or above 24 the first 16 plus the key columns. */
export function cardLayout(t: DesignerTable, linked: Set<string> | undefined, m: Measure): CardLayout {
  const all = t.columns;
  const rows = all.length <= M.maxRows
    ? all.map((_, i) => i)
    : all.flatMap((c, i) => (i < M.firstRows || c.key === "PRI" || linked?.has(c.name) ? [i] : []));
  const hidden = all.length - rows.length;
  const name = rows.length ? Math.max(...rows.map((i) => [...all[i].name].length)) : 4;
  const chip = rows.length ? Math.max(...rows.map((i) => [...compactType(all[i].type)].length)) : 3;
  const rowWidth = M.padding * 2 + M.iconSlot + name * m.nameChar + 14 + chip * m.chipChar + 12;
  const headerWidth = M.padding * 2 + 20 + 8 + m.title(t.name) + (t.isView ? 60 : 0) + 14;
  const width = Math.min(M.maxWidth, Math.max(M.minWidth, Math.ceil(Math.max(rowWidth, headerWidth))));
  const height = M.headerHeight + Math.max(1, rows.length + (hidden > 0 ? 1 : 0)) * M.rowHeight + M.bottomPadding;
  return { width, height, rows, hidden };
}

/** Vertical centre of a column's row from the top of its card; hidden columns use the "+ N more" row. */
export function rowCenter(l: CardLayout, t: DesignerTable, column: string): number {
  const index = t.columns.findIndex((c) => c.name === column);
  let row = index < 0 ? -1 : l.rows.indexOf(index);
  if (row < 0) row = l.hidden > 0 ? l.rows.length : 0;
  return M.headerHeight + (row + 0.5) * M.rowHeight;
}

/** The column whose row is at `y` (card coordinates), if any. */
export function columnAt(l: CardLayout, t: DesignerTable, y: number): string | null {
  const row = Math.floor((y - M.headerHeight) / M.rowHeight);
  if (y < M.headerHeight || row < 0 || row >= l.rows.length) return null;
  return t.columns[l.rows[row]].name;
}

// ---- Relation lines ----

/** One drawn line of a relation (a composite key draws one per column pair): a cubic Bézier. */
export interface Curve {
  id: string;
  /** Which column pair of the relation (0 for single-column keys). */
  pair: number;
  start: Point;
  c1: Point;
  c2: Point;
  end: Point;
  /** +1 when the line leaves the FK card to the right, −1 to the left. */
  startSide: number;
  /** +1 when the line enters the referenced card from the right (the arrow points left), −1 from the left. */
  endSide: number;
}

/** The lines from the FK rows of `child` (frame `a`) to the referenced rows of `parent` (frame `b`). */
export function relationCurves(r: DesignerRelation, child: DesignerTable, a: Rect, la: CardLayout,
  parent: DesignerTable, b: Rect, lb: CardLayout): Curve[] {
  const id = relationId(r);
  const out: Curve[] = [];
  const aRight = a.x + a.w;
  const bRight = b.x + b.w;
  for (let i = 0; i < Math.min(r.columns.length, r.refColumns.length); i++) {
    const y1 = a.y + rowCenter(la, child, r.columns[i]);
    const y2 = b.y + rowCenter(lb, parent, r.refColumns[i]);
    const spread = i * 6;
    if (r.table === r.refTable) {
      // A loop on the right side of the card.
      const reach = 46 + spread + Math.min(40, Math.abs(y2 - y1) * 0.25);
      const ye = y1 === y2 ? y2 + M.rowHeight * 0.35 : y2;
      out.push({ id, pair: i, start: { x: aRight, y: y1 }, c1: { x: aRight + reach, y: y1 }, c2: { x: aRight + reach, y: ye },
        end: { x: aRight + M.arrowLength, y: ye }, startSide: 1, endSide: 1 });
    } else if (b.x >= aRight + 24) {
      // Parent to the right: leave right, enter left.
      const k = Math.max(40, (b.x - aRight) * 0.5);
      const s = { x: aRight, y: y1 };
      const e = { x: b.x - M.arrowLength, y: y2 };
      out.push({ id, pair: i, start: s, c1: { x: s.x + k, y: s.y }, c2: { x: e.x - k, y: e.y }, end: e, startSide: 1, endSide: -1 });
    } else if (bRight <= a.x - 24) {
      // Parent to the left: leave left, enter right.
      const k = Math.max(40, (a.x - bRight) * 0.5);
      const s = { x: a.x, y: y1 };
      const e = { x: bRight + M.arrowLength, y: y2 };
      out.push({ id, pair: i, start: s, c1: { x: s.x - k, y: s.y }, c2: { x: e.x + k, y: e.y }, end: e, startSide: -1, endSide: 1 });
    } else {
      // Above or below each other: a C-shaped curve on the right.
      const x = Math.max(aRight, bRight) + 50 + spread + Math.min(60, Math.abs(y2 - y1) * 0.08);
      const s = { x: aRight, y: y1 };
      const e = { x: bRight + M.arrowLength, y: y2 };
      out.push({ id, pair: i, start: s, c1: { x, y: s.y }, c2: { x, y: e.y }, end: e, startSide: 1, endSide: 1 });
    }
  }
  return out;
}

export function curvePoint(c: Curve, t: number): Point {
  const u = 1 - t;
  const a = u * u * u, b = 3 * u * u * t, d = 3 * u * t * t, e = t * t * t;
  return { x: a * c.start.x + b * c.c1.x + d * c.c2.x + e * c.end.x, y: a * c.start.y + b * c.c1.y + d * c.c2.y + e * c.end.y };
}

/** The box around the curve's control points (the curve stays inside it). */
export function curveBounds(c: Curve): Rect {
  const xs = [c.start.x, c.c1.x, c.c2.x, c.end.x];
  const ys = [c.start.y, c.c1.y, c.c2.y, c.end.y];
  const x = Math.min(...xs), y = Math.min(...ys);
  return { x, y, w: Math.max(...xs) - x, h: Math.max(...ys) - y };
}

export function segmentDistance(p: Point, a: Point, b: Point): number {
  const dx = b.x - a.x, dy = b.y - a.y;
  const len = dx * dx + dy * dy;
  const t = len > 0 ? Math.max(0, Math.min(1, ((p.x - a.x) * dx + (p.y - a.y) * dy) / len)) : 0;
  return Math.hypot(a.x + t * dx - p.x, a.y + t * dy - p.y);
}

/** Distance from `p` to the curve, measured on 24 straight pieces. */
export function curveDistance(c: Curve, p: Point): number {
  let best = Infinity;
  let previous = c.start;
  for (let i = 1; i <= 24; i++) {
    const next = curvePoint(c, i / 24);
    best = Math.min(best, segmentDistance(p, previous, next));
    previous = next;
  }
  return best;
}

const n2 = (v: number) => Math.round(v * 100) / 100;

/** SVG path of the line. */
export const curvePath = (c: Curve) =>
  `M${n2(c.start.x)} ${n2(c.start.y)}C${n2(c.c1.x)} ${n2(c.c1.y)} ${n2(c.c2.x)} ${n2(c.c2.y)} ${n2(c.end.x)} ${n2(c.end.y)}`;

/** The arrowhead into the referenced row: the tip on the card's edge, the base where the line ends. Half as wide
 *  as 4.2 points, but at least 2.5 px on screen (zoomed far out), and 15 % more while the line is hovered. */
export function arrowPath(c: Curve, zoom = 1, hovered = false): string {
  const half = Math.max(4.2, 2.5 / zoom) * (hovered ? 1.15 : 1);
  const tip = c.end.x - c.endSide * M.arrowLength;
  return `M${n2(tip)} ${n2(c.end.y)}L${n2(c.end.x)} ${n2(c.end.y - half)}L${n2(c.end.x)} ${n2(c.end.y + half)}Z`;
}

/** The dashed line while a relation is dragged: from the source handle to the pointer or the target row's edge. */
export function linkPath(from: Point, to: Point): string {
  const k = Math.max(30, Math.abs(to.x - from.x) * 0.45);
  const back = to.x < from.x ? k : -k;
  return `M${n2(from.x)} ${n2(from.y)}C${n2(from.x + k)} ${n2(from.y)} ${n2(to.x + back)} ${n2(to.y)} ${n2(to.x)} ${n2(to.y)}`;
}

// ---- Automatic layout ----

export const LAYOUT = { layerGap: 110, rowGap: 34, componentGap: 120, margin: 40 } as const;
const L = LAYOUT;
const DEFAULT_SIZE: Size = { w: 200, h: 100 };

/** Places the cards: related tables in layers (referenced tables to the left of the tables that point to them),
 *  each group of connected tables as a block, and the tables without relations in columns to the right. */
export function autoLayout(s: DesignerSchema, sizes: Map<string, Size>): Map<string, Point> {
  const size = (n: string) => sizes.get(n) ?? DEFAULT_SIZE;
  const parents = new Map<string, Set<string>>(); // child → referenced tables
  const neighbours = new Map<string, Set<string>>();
  const link = (map: Map<string, Set<string>>, a: string, b: string) => map.set(a, (map.get(a) ?? new Set()).add(b));
  for (const r of s.relations) {
    if (r.table === r.refTable) continue;
    link(parents, r.table, r.refTable);
    link(neighbours, r.table, r.refTable);
    link(neighbours, r.refTable, r.table);
  }

  // Connected groups, found in name order; the biggest first.
  const seen = new Set<string>();
  const groups: string[][] = [];
  const loners: string[] = [];
  for (const { name } of s.tables) {
    if (seen.has(name)) continue;
    seen.add(name);
    if (!neighbours.has(name)) {
      loners.push(name);
      continue;
    }
    const group: string[] = [];
    const stack = [name];
    while (stack.length) {
      const t = stack.pop()!;
      group.push(t);
      for (const x of [...(neighbours.get(t) ?? [])].sort()) {
        if (seen.has(x)) continue;
        seen.add(x);
        stack.push(x);
      }
    }
    groups.push(group.sort());
  }
  groups.sort((a, b) => b.length - a.length);

  // Groups side by side, wrapping into rows when they get too wide.
  const result = new Map<string, Point>();
  const blocks = groups.map((g) => layoutGroup(g, parents, neighbours, size));
  const area = blocks.reduce((sum, b) => sum + b.size.w * b.size.h, 0);
  const rowLimit = Math.max(1400, ...blocks.map((b) => b.size.w), Math.sqrt(area) * 1.6);
  let x: number = L.margin, y: number = L.margin, rowHeight = 0, maxX: number = L.margin;
  for (const b of blocks) {
    if (x > L.margin && x + b.size.w > rowLimit) {
      x = L.margin;
      y += rowHeight + L.componentGap;
      rowHeight = 0;
    }
    for (const [name, p] of b.places) result.set(name, { x: x + p.x, y: y + p.y });
    x += b.size.w + L.componentGap;
    maxX = Math.max(maxX, x - L.componentGap);
    rowHeight = Math.max(rowHeight, b.size.h);
  }
  const connectedHeight = blocks.length ? y + rowHeight - L.margin : 0;

  // Tables without relations: columns of cards to the right (or a grid of their own when nothing is related).
  if (loners.length) {
    const startX = blocks.length ? maxX + L.componentGap : L.margin;
    const looseArea = loners.reduce((sum, n) => sum + (size(n).w + 40) * (size(n).h + L.rowGap), 0);
    const columnLimit = Math.max(blocks.length ? connectedHeight : 0, Math.sqrt(looseArea * 0.6), 420);
    let cx = startX, cy: number = L.margin, columnWidth = 0;
    for (const n of loners) {
      const sz = size(n);
      if (cy > L.margin && cy + sz.h > L.margin + columnLimit) {
        cx += columnWidth + 40;
        cy = L.margin;
        columnWidth = 0;
      }
      result.set(n, { x: cx, y: cy });
      cy += sz.h + L.rowGap;
      columnWidth = Math.max(columnWidth, sz.w);
    }
  }
  return result;
}

/** One connected group in layers: tables that reference nothing on the left, every other table one layer to the
 *  right of the furthest table it references; the order inside a layer follows the neighbours (barycentre sweeps). */
function layoutGroup(group: string[], parents: Map<string, Set<string>>, neighbours: Map<string, Set<string>>,
  size: (n: string) => Size): { size: Size; places: Map<string, Point> } {
  const members = new Set(group);
  const rank = new Map<string, number>();
  const visiting = new Set<string>();
  const depth = (t: string): number => {
    const known = rank.get(t);
    if (known !== undefined) return known;
    visiting.add(t);
    let d = 0;
    for (const p of [...(parents.get(t) ?? [])].sort()) {
      if (members.has(p) && !visiting.has(p)) d = Math.max(d, depth(p) + 1);
    }
    visiting.delete(t);
    rank.set(t, d);
    return d;
  };
  group.forEach(depth);

  const layerCount = Math.max(...rank.values()) + 1;
  const layers: string[][] = Array.from({ length: layerCount }, () => []);
  for (const t of group) layers[rank.get(t)!].push(t);

  // Barycentre ordering, a few sweeps in both directions.
  const positions = () => {
    const p = new Map<string, number>();
    for (const layer of layers) layer.forEach((t, i) => p.set(t, (i + 0.5) / layer.length));
    return p;
  };
  for (let sweep = 0; sweep < 6; sweep++) {
    const order = [...layers.keys()];
    if (sweep % 2) order.reverse();
    for (const li of order) {
      const p = positions();
      const bary = new Map<string, number>();
      for (const t of layers[li]) {
        const ns = [...(neighbours.get(t) ?? [])].filter((n) => members.has(n) && rank.get(n) !== li);
        bary.set(t, ns.length ? ns.reduce((sum, n) => sum + (p.get(n) ?? 0.5), 0) / ns.length : p.get(t) ?? 0.5);
      }
      layers[li].sort((a, b) => bary.get(a)! - bary.get(b)! || (a < b ? -1 : a > b ? 1 : 0));
    }
  }

  // Tall layers wrap into several columns.
  const columns: string[][] = [];
  for (const layer of layers) {
    let column: string[] = [];
    let h = 0;
    for (const t of layer) {
      if (column.length && h + size(t).h > 1500) {
        columns.push(column);
        column = [];
        h = 0;
      }
      column.push(t);
      h += size(t).h + L.rowGap;
    }
    if (column.length) columns.push(column);
  }
  const heights = columns.map((c) => c.reduce((sum, t) => sum + size(t).h, 0) + Math.max(0, c.length - 1) * L.rowGap);
  const tallest = Math.max(0, ...heights);
  const places = new Map<string, Point>();
  let x = 0;
  columns.forEach((column, ci) => {
    let y = (tallest - heights[ci]) / 2;
    for (const t of column) {
      places.set(t, { x, y });
      y += size(t).h + L.rowGap;
    }
    x += Math.max(...column.map((t) => size(t).w)) + L.layerGap;
  });
  return { size: { w: x - L.layerGap, h: tallest }, places };
}

/** The positions to show: the known ones (dragged or saved) where there are any, and the automatic layout for
 *  new tables, placed beside the known ones. */
export function placed(s: DesignerSchema, sizes: Map<string, Size>, current: Map<string, Point>): Map<string, Point> {
  const auto = autoLayout(s, sizes);
  const known = new Map<string, Point>();
  for (const t of s.tables) {
    const p = current.get(t.name);
    if (p) known.set(t.name, { x: p.x, y: p.y });
  }
  if (!known.size) return auto;
  const missing = s.tables.map((t) => t.name).filter((n) => !known.has(n));
  if (!missing.length) return known;
  const right = Math.max(...[...known].map(([n, p]) => p.x + (sizes.get(n) ?? DEFAULT_SIZE).w));
  const top = Math.min(...[...known.values()].map((p) => p.y));
  const minX = Math.min(...missing.map((n) => auto.get(n)?.x ?? 0));
  const minY = Math.min(...missing.map((n) => auto.get(n)?.y ?? 0));
  for (const n of missing) {
    const p = auto.get(n) ?? { x: 0, y: 0 };
    known.set(n, { x: right + L.componentGap + p.x - minX, y: top + p.y - minY });
  }
  return known;
}

// ---- Hit tests (world coordinates) ----

/** The card under `p`; the last drawn is on top, and the handles stick out by their radius. */
export function tableAt(p: Point, order: string[], frame: (name: string) => Rect | undefined): string | null {
  for (let i = order.length - 1; i >= 0; i--) {
    const f = frame(order[i]);
    if (f && p.x >= f.x - M.handleRadius && p.x < f.x + f.w + M.handleRadius && p.y >= f.y && p.y < f.y + f.h) return order[i];
  }
  return null;
}

/** The relation whose line is nearest to `p`, within `tolerance`. */
export function relationAt(p: Point, curves: Curve[], visible: (id: string) => boolean, tolerance: number): string | null {
  let best: string | null = null;
  let bestDistance = Infinity;
  for (const c of curves) {
    const b = curveBounds(c);
    if (p.x < b.x - tolerance || p.x > b.x + b.w + tolerance || p.y < b.y - tolerance || p.y > b.y + b.h + tolerance) continue;
    if (!visible(c.id)) continue;
    const d = curveDistance(c, p);
    if (d <= tolerance && d < bestDistance) {
      best = c.id;
      bestDistance = d;
    }
  }
  return best;
}

export function unionRects(rects: Rect[]): Rect | null {
  if (!rects.length) return null;
  const x = Math.min(...rects.map((r) => r.x));
  const y = Math.min(...rects.map((r) => r.y));
  return { x, y, w: Math.max(...rects.map((r) => r.x + r.w)) - x, h: Math.max(...rects.map((r) => r.y + r.h)) - y };
}

// ---- Zoom and scroll ----

export const ZOOM = { min: 0.05, max: 2.5 } as const;

/** Where the world's origin sits on the canvas, and the scale. */
export interface View { zoom: number; pan: Point }

export const toWorld = (v: View, p: Point): Point => ({ x: (p.x - v.pan.x) / v.zoom, y: (p.y - v.pan.y) / v.zoom });

/** Keeps at least a 120 px strip of the diagram on screen, so it can't be scrolled away for good. */
export function clampPan(p: Point, zoom: number, size: Size, content: Rect | null): Point {
  if (!content || size.w <= 0) return p;
  const keep = 120;
  const minX = keep - (content.x + content.w) * zoom, maxX = size.w - keep - content.x * zoom;
  const minY = keep - (content.y + content.h) * zoom, maxY = size.h - keep - content.y * zoom;
  return {
    x: minX <= maxX ? Math.min(maxX, Math.max(minX, p.x)) : p.x,
    y: minY <= maxY ? Math.min(maxY, Math.max(minY, p.y)) : p.y,
  };
}

/** Zooms keeping the world point under `anchor` (canvas coordinates) in place; the canvas centre when null. */
export function zoomView(v: View, value: number, anchor: Point | null, size: Size, content: Rect | null): View {
  const zoom = Math.min(ZOOM.max, Math.max(ZOOM.min, value));
  const a = anchor ?? { x: size.w / 2, y: size.h / 2 };
  const w = toWorld(v, a);
  return { zoom, pan: clampPan({ x: a.x - w.x * zoom, y: a.y - w.y * zoom }, zoom, size, content) };
}

/** The pan that brings `r` fully on screen with a 24 px margin, moving as little as possible (its top left first
 *  when it does not fit); null when it is on screen already. */
export function revealPan(v: View, r: Rect, size: Size): Point | null {
  const m = 24;
  const axis = (pan: number, start: number, length: number, room: number) => {
    const a = start * v.zoom + pan, b = a + length * v.zoom;
    if (a >= 0 && b <= room) return pan;
    return a < 0 || b - a > room - 2 * m ? pan + m - a : pan + room - m - b;
  };
  const x = axis(v.pan.x, r.x, r.w, size.w), y = axis(v.pan.y, r.y, r.h, size.h);
  return x === v.pan.x && y === v.pan.y ? null : { x, y };
}

/** Shows all of `rect` with a 28 px margin, at most at 100 %; null while the canvas has no real size. */
export function fitView(rect: Rect | null, size: Size): View | null {
  if (!rect || size.w <= 40 || size.h <= 40) return null;
  const m = 28;
  const zoom = Math.min(1, Math.max(ZOOM.min, Math.min((size.w - 2 * m) / Math.max(1, rect.w), (size.h - 2 * m) / Math.max(1, rect.h))));
  return { zoom, pan: { x: (size.w - rect.w * zoom) / 2 - rect.x * zoom, y: (size.h - rect.h * zoom) / 2 - rect.y * zoom } };
}

// ---- Loading ----

/** Reads per database with a shared cache (the macOS pitfall: the diagram vanished on a database switch and was
 *  read twice). On a switch the cached value shows at once; the fresh read replaces it only when it differs, and
 *  an answer that arrives after another database (or a newer read) was asked for is dropped. */
export class Loader<T> {
  cache: Map<string, T>;
  #read: (key: string) => Promise<T>;
  #same: (a: T, b: T) => boolean;
  #current: string | null = null;
  #ticket = 0;

  constructor(read: (key: string) => Promise<T>, same: (a: T, b: T) => boolean, cache = new Map<string, T>()) {
    this.#read = read;
    this.#same = same;
    this.cache = cache;
  }

  async load(key: string, show: (value: T) => void, fail: (error: unknown) => void): Promise<void> {
    if (key !== this.#current) {
      this.#current = key;
      const cached = this.cache.get(key);
      if (cached !== undefined) show(cached);
    }
    const ticket = ++this.#ticket;
    let fresh: T;
    try {
      fresh = await this.#read(key);
    } catch (e) {
      if (ticket === this.#ticket) fail(e);
      return;
    }
    if (ticket !== this.#ticket) return;
    const old = this.cache.get(key);
    this.cache.set(key, fresh);
    if (old === undefined || !this.#same(old, fresh)) show(fresh);
  }

  /** A value newer than any read under way (the schema an add or drop returned): it goes in the cache, and a
   *  read of the same key that started before it is dropped, so it can't bring the old value back. */
  put(key: string, value: T) {
    this.cache.set(key, value);
    if (key === this.#current) this.#ticket++;
  }
}
