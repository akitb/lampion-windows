// The Designer tab's state (port of the macOS DesignerStore and DesignerViewport): what is drawn and where,
// what the pointer is over, and the zoom. One store per mounted view; the schemas are cached per database for
// as long as the window is open, so switching back (or leaving the tab and returning) shows a diagram at once.
import { call } from "$lib/api";
import type { DesignerLoad, DesignerNewRelation, DesignerPreview, DesignerRelation, DesignerSchema } from "$lib/types";
import { SvelteMap } from "svelte/reactivity";
import { activity, message } from "../dbapi.svelte";
import { askConfirm, db, schemaChanged, showError } from "../dbstate.svelte";
import { measure } from "../util";
import {
  autoLayout, cardLayout, clampPan, columnAt, curveBounds, dropSql, emphasis as emphasize, fitView, linkRelation, Loader,
  placed, relationAt, relationColumns, relationCurves, relationId, summary, tableAt, unionRects, zoomView,
  type CardLayout, type Curve, type Measure, type Point, type Rect, type Size, type View,
} from "./diagram";

async function track<T>(p: Promise<T>): Promise<T> {
  activity.busy++;
  try {
    return await p;
  } finally {
    activity.busy--;
  }
}

export const designerApi = {
  schema: (database: string) => track(call<DesignerLoad>("db_designer_schema", { database })),
  /** `name` null: use (and return) the suggested name. */
  preview: (database: string, relation: DesignerNewRelation, onDelete: string, onUpdate: string, name: string | null) =>
    call<DesignerPreview>("db_designer_preview", { database, relation, onDelete, onUpdate, name }),
  /** `created` false: MariaDB took the statement but kept no foreign key (MyISAM); the schema is fresh all the same. */
  addRelation: (database: string, relation: DesignerNewRelation, onDelete: string, onUpdate: string, name: string) =>
    track(call<{ schema: DesignerSchema; created: boolean }>("db_designer_add_relation", { database, relation, onDelete, onUpdate, name })),
  dropRelation: (database: string, table: string, name: string) =>
    track(call<DesignerSchema>("db_designer_drop_relation", { database, table, name })),
  /** An empty map forgets the positions. */
  savePositions: (database: string, positions: Record<string, [number, number]>) =>
    call("db_designer_save_positions", { database, positions }),
};

const cache = new Map<string, DesignerLoad>();
const sameSchema = (a: DesignerLoad, b: DesignerLoad) => JSON.stringify(a.schema) === JSON.stringify(b.schema);

const RELATIONS_ONLY = "db.designer.relationsOnly";
function storedRelationsOnly(): boolean {
  try {
    return localStorage.getItem(RELATIONS_ONLY) === "1";
  } catch {
    return false;
  }
}

/** Text widths for the card sizes, in the fonts the cards use. */
function measurer(): Measure {
  const css = getComputedStyle(document.documentElement);
  const mono = css.getPropertyValue("--mono").trim() || "monospace";
  const font = css.getPropertyValue("--font").trim() || "sans-serif";
  return {
    nameChar: measure("0", `11.5px ${mono}`),
    chipChar: measure("0", `500 10px ${mono}`),
    title: (text) => measure(text, `600 12.5px ${font}`),
  };
}

/** A column-to-column drag in progress; `point` is the pointer in world coordinates. */
export interface LinkDrag {
  table: string;
  column: string;
  point: Point;
  /** The column under the pointer that would become the other end. */
  target: { table: string; column: string } | null;
}

export type CardState = "normal" | "dimmed" | "highlighted";

export class DesignerStore {
  database: string | null = null;
  schema = $state.raw<DesignerSchema | null>(null);
  loadError = $state<string | null>(null);
  layouts = $state.raw(new Map<string, CardLayout>());
  positions = new SvelteMap<string, Point>();
  hoveredTable = $state<string | null>(null);
  hoveredRelation = $state<string | null>(null);
  /** Where the pointer entered the hovered relation (canvas coordinates): the tooltip sits there. */
  hoverAnchor = $state.raw<Point>({ x: 0, y: 0 });
  link = $state.raw<LinkDrag | null>(null);
  draggingTable = $state<string | null>(null);
  /** The relation waiting in the confirmation sheet. */
  pending = $state.raw<DesignerNewRelation | null>(null);
  relationsOnly = $state(storedRelationsOnly());
  /** A database's first diagram arrived: fit it once the canvas has a size. */
  needsFit = $state(false);
  view = $state.raw<View>({ zoom: 1, pan: { x: 0, y: 0 } });
  /** The canvas size, kept up to date by the canvas. */
  size: Size = { w: 0, h: 0 };
  /** True after Fit until the user pans or zooms: resizing the window fits again. */
  followsFit = false;
  /** The schema version this view caused itself, which needs no reload. */
  ownVersion = -1;
  #loader = new Loader<DesignerLoad>((d) => designerApi.schema(d), sameSchema, cache);
  #reloadTimer: ReturnType<typeof setTimeout> | undefined;

  tables = $derived(new Map((this.schema?.tables ?? []).map((t) => [t.name, t])));
  relations = $derived(new Map((this.schema?.relations ?? []).map((r) => [relationId(r), r])));

  /** All tables, or with "Relations only" those that have or receive a foreign key. */
  visibleTables = $derived.by(() => {
    const s = this.schema;
    if (!s || !this.relationsOnly) return s?.tables ?? [];
    const linked = new Set(s.relations.flatMap((r) => [r.table, r.refTable]));
    return s.tables.filter((t) => linked.has(t.name));
  });
  visibleNames = $derived(this.visibleTables.map((t) => t.name));

  curves = $derived.by(() => {
    const out: Curve[] = [];
    for (const r of this.schema?.relations ?? []) {
      const child = this.tables.get(r.table), parent = this.tables.get(r.refTable);
      const la = this.layouts.get(r.table), lb = this.layouts.get(r.refTable);
      const a = this.frame(r.table), b = this.frame(r.refTable);
      if (child && parent && la && lb && a && b) out.push(...relationCurves(r, child, a, la, parent, b, lb));
    }
    return out;
  });

  /** The area the visible cards and the lines cover. */
  contentBounds = $derived(unionRects([
    ...this.visibleNames.flatMap((n) => this.frame(n) ?? []),
    ...this.curves.map(curveBounds),
  ]));

  /** What stays bright while something is hovered; null: nothing is dimmed. */
  emphasis = $derived(this.link || !this.schema ? null
    : emphasize(this.schema, this.relations.get(this.hoveredRelation ?? "") ?? null, this.hoveredTable ?? this.draggingTable));

  /** Foreign key columns per table (lines and keys into other databases) for the link icon. */
  foreign = $derived.by(() => {
    const m = new Map<string, Set<string>>();
    for (const r of [...(this.schema?.relations ?? []), ...(this.schema?.external ?? [])]) {
      const set = m.get(r.table) ?? new Set<string>();
      r.columns.forEach((c) => set.add(c));
      m.set(r.table, set);
    }
    return m;
  });

  /** Keys into other databases: table → column → `db.table.column`. */
  external = $derived.by(() => {
    const m = new Map<string, Map<string, string>>();
    for (const r of this.schema?.external ?? []) {
      const cols = m.get(r.table) ?? new Map<string, string>();
      r.columns.forEach((c, i) => cols.set(c, `${r.refSchema}.${r.refTable}.${r.refColumns[i] ?? ""}`));
      m.set(r.table, cols);
    }
    return m;
  });

  // ---- Loading ----

  load(database: string) {
    if (database !== this.database) {
      this.database = database;
      this.schema = null;
      this.loadError = null;
      this.positions.clear();
      this.layouts = new Map();
      this.clearHover();
      this.link = null;
    }
    this.#loader.load(
      database,
      (v) => {
        if (this.database !== database) return;
        this.loadError = null;
        this.#apply(v);
      },
      (e) => {
        if (this.database === database) this.loadError = message(e);
      },
    );
  }

  reload() {
    if (this.database) this.load(this.database);
  }

  /** Several requests in a row (Refresh reloads every table list) make one read. */
  requestReload() {
    clearTimeout(this.#reloadTimer);
    this.#reloadTimer = setTimeout(() => this.reload(), 80);
  }

  destroy() {
    clearTimeout(this.#reloadTimer);
  }

  /** A schema that came back from adding or dropping a relation. */
  applySchema(s: DesignerSchema) {
    if (s.database !== this.database) return;
    cache.set(s.database, { schema: s, positions: cache.get(s.database)?.positions ?? {} });
    this.#apply({ schema: s, positions: {} });
  }

  /** Tells the window the schema changed (sidebar, header), without reloading this view again. */
  changed() {
    schemaChanged();
    this.ownVersion = db.schemaVersion;
  }

  #apply(load: DesignerLoad) {
    const s = load.schema;
    const first = this.schema === null;
    const linked = relationColumns(s);
    const m = measurer();
    const layouts = new Map(s.tables.map((t) => [t.name, cardLayout(t, linked.get(t.name), m)]));
    const current = this.positions.size
      ? new Map(this.positions)
      : new Map(Object.entries(load.positions).map(([n, [x, y]]) => [n, { x, y }]));
    this.layouts = layouts;
    this.schema = s;
    this.#setPositions(placed(s, this.#sizes(), current));
    if (this.hoveredTable !== null && !layouts.has(this.hoveredTable)) this.hoveredTable = null;
    if (this.hoveredRelation !== null && !this.relations.has(this.hoveredRelation)) this.hoveredRelation = null;
    if (first) this.needsFit = true;
  }

  // ---- Positions ----

  #sizes(): Map<string, Size> {
    return new Map([...this.layouts].map(([n, l]) => [n, { w: l.width, h: l.height }]));
  }

  #setPositions(next: Map<string, Point>) {
    this.positions.clear();
    for (const [n, p] of next) this.positions.set(n, p);
  }

  /** Keeps the positions for this window session and in the database's positions file. */
  #remember(saved: Record<string, [number, number]>) {
    const d = this.database;
    if (!d) return;
    const c = cache.get(d);
    if (c) c.positions = saved;
    // A failed save only loses the arrangement for the next start; the diagram keeps it now.
    designerApi.savePositions(d, saved).catch(() => {});
  }

  frame(name: string): Rect | undefined {
    const p = this.positions.get(name), l = this.layouts.get(name);
    return p && l ? { x: p.x, y: p.y, w: l.width, h: l.height } : undefined;
  }

  move(table: string, p: Point) {
    this.positions.set(table, { x: Math.max(0, p.x), y: Math.max(0, p.y) });
  }

  savePositions() {
    this.#remember(Object.fromEntries([...this.positions].map(([n, p]) => [n, [Math.round(p.x), Math.round(p.y)]])));
  }

  /** Forgets the dragged positions and lays the diagram out again. */
  autoArrange() {
    if (!this.schema) return;
    this.#setPositions(autoLayout(this.schema, this.#sizes()));
    this.#remember({});
    this.fit();
  }

  setRelationsOnly(on: boolean) {
    this.relationsOnly = on;
    try {
      localStorage.setItem(RELATIONS_ONLY, on ? "1" : "0");
    } catch {
      // Storage unavailable: the choice lasts until the window closes.
    }
    this.fit();
  }

  // ---- Emphasis ----

  cardState(name: string): CardState {
    const e = this.emphasis;
    if (!e) return "normal";
    if (!e.tables.has(name)) return "dimmed";
    return this.hoveredTable === name || this.draggingTable === name || this.hoveredRelation !== null ? "highlighted" : "normal";
  }

  /** Columns of `table` in the hovered relation (highlighted rows). */
  litColumns(table: string): Set<string> | null {
    const r = this.link ? undefined : this.relations.get(this.hoveredRelation ?? "");
    if (!r || (r.table !== table && r.refTable !== table)) return null;
    return new Set([...(r.table === table ? r.columns : []), ...(r.refTable === table ? r.refColumns : [])]);
  }

  // ---- Hit testing (world coordinates) ----

  tableAt(p: Point): string | null {
    return tableAt(p, this.visibleNames, (n) => this.frame(n));
  }

  columnAt(p: Point): { table: string; column: string } | null {
    const name = this.tableAt(p);
    const f = name === null ? undefined : this.frame(name);
    const t = name === null ? undefined : this.tables.get(name);
    const l = name === null ? undefined : this.layouts.get(name);
    if (name === null || !f || !t || !l) return null;
    const column = columnAt(l, t, p.y - f.y);
    return column === null ? null : { table: name, column };
  }

  /** Updates what the pointer is over; writes only what changed. */
  hover(world: Point, screen: Point) {
    const t = this.tableAt(world);
    const visible = new Set(this.visibleNames);
    const r = t !== null ? null : relationAt(world, this.curves, (id) => {
      const rel = this.relations.get(id);
      return !!rel && visible.has(rel.table) && visible.has(rel.refTable);
    }, 6 / Math.max(this.view.zoom, 0.2));
    if (r !== this.hoveredRelation) {
      this.hoverAnchor = screen;
      this.hoveredRelation = r;
    }
    if (t !== this.hoveredTable) this.hoveredTable = t;
  }

  clearHover() {
    if (this.hoveredTable !== null) this.hoveredTable = null;
    if (this.hoveredRelation !== null) this.hoveredRelation = null;
  }

  // ---- Relations ----

  canLink(table: string): boolean {
    return this.tables.get(table)?.isView === false;
  }

  beginLink(table: string, column: string, p: Point) {
    this.link = { table, column, point: p, target: null };
  }

  updateLink(p: Point) {
    const l = this.link;
    if (!l) return;
    const c = this.columnAt(p);
    const target = c && this.canLink(c.table) && !(c.table === l.table && c.column === l.column) ? c : null;
    this.link = { ...l, point: p, target };
  }

  /** Ends the drag; over a column of another table, the confirmation sheet opens. */
  endLink() {
    const l = this.link;
    this.link = null;
    if (!l?.target) return;
    const from = this.tables.get(l.table)?.columns.find((c) => c.name === l.column);
    const to = this.tables.get(l.target.table)?.columns.find((c) => c.name === l.target!.column);
    if (from && to) this.pending = linkRelation(l.table, from, l.target.table, to);
  }

  /** Asks, then drops the foreign key; the diagram follows. */
  confirmDelete(r: DesignerRelation) {
    const database = this.database;
    if (!database) return;
    askConfirm({
      title: `Delete the relation “${r.name}”?`,
      message: `${summary(r)}\n\nThe columns and their data stay; only the foreign key is removed.\n\n${dropSql(database, r.table, r.name)}`,
      button: "Delete Relation",
      action: () => {
        designerApi
          .dropRelation(database, r.table, r.name)
          .then((s) => {
            this.applySchema(s);
            this.changed();
          })
          .catch((e) => {
            showError(message(e));
            if (this.database === database) this.reload();
          });
      },
    });
  }

  // ---- Zoom and scroll ----

  zoomTo(value: number, anchor: Point | null = null) {
    this.followsFit = false;
    this.view = zoomView(this.view, value, anchor, this.size, this.contentBounds);
  }

  scrollBy(dx: number, dy: number) {
    this.setPan({ x: this.view.pan.x + dx, y: this.view.pan.y + dy });
  }

  setPan(p: Point) {
    this.followsFit = false;
    this.view = { zoom: this.view.zoom, pan: clampPan(p, this.view.zoom, this.size, this.contentBounds) };
  }

  /** Shows the whole diagram, at most at 100 %. */
  fit() {
    const v = fitView(this.contentBounds, this.size);
    if (!v) return;
    this.view = v;
    this.followsFit = true;
  }
}
