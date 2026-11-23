// Unit tests of the Designer's pure functions. Run: node --test src/lib/db/designer/
// (Node strips the types of diagram.ts itself; .mjs keeps this file out of svelte-check, which has no Node types.)
import { test } from "node:test";
import assert from "node:assert/strict";
import {
  arrowPath, autoLayout, cardLayout, clampPan, columnAt, compactType, countText, curveDistance, curvePoint, dropSql,
  emphasis, explain, fitView, linkPath, linkRelation, Loader, METRICS, placed, relationAt, relationColumns, relationCurves,
  relationId, rowCenter, rowHelp, sqlTokens, summary, tableAt, toWorld, typeFamily, zoomView, ZOOM,
} from "./diagram.ts";

const col = (name, type = "int(10) unsigned", key = "", nullable = false) => ({ name, type, key, nullable });
const table = (name, columns, isView = false, engine = "InnoDB") => ({ name, isView, engine, columns });
const rel = (name, t, columns, refTable, refColumns, onDelete = "RESTRICT", onUpdate = "RESTRICT") =>
  ({ name, table: t, columns, refSchema: "shop", refTable, refColumns, onDelete, onUpdate });
const measure = { nameChar: 7, chipChar: 6, title: (s) => s.length * 7.5 };

const shop = {
  database: "shop",
  tables: [
    table("categories", [col("id", "int(10) unsigned", "PRI"), col("parent_id", "int(10) unsigned", "MUL", true)]),
    table("customers", [col("id", "int(10) unsigned", "PRI"), col("email", "varchar(160)", "UNI")]),
    table("order_items", [col("order_id", "int(10) unsigned", "PRI"), col("product_id", "int(10) unsigned", "PRI")]),
    table("order_totals", [col("order_id")], true, ""),
    table("orders", [col("id", "int(10) unsigned", "PRI"), col("customer_id", "int(10) unsigned", "MUL")]),
    table("products", [col("id", "int(10) unsigned", "PRI"), col("category_id", "int(10) unsigned", "MUL")]),
    table("settings", [col("name", "varchar(60)", "PRI"), col("value", "varchar(255)")]),
  ],
  relations: [
    rel("fk_parent", "categories", ["parent_id"], "categories", ["id"], "SET NULL"),
    rel("fk_item_order", "order_items", ["order_id"], "orders", ["id"], "CASCADE"),
    rel("fk_item_product", "order_items", ["product_id"], "products", ["id"]),
    rel("fk_order_customer", "orders", ["customer_id"], "customers", ["id"]),
    rel("fk_product_category", "products", ["category_id"], "categories", ["id"]),
  ],
  external: [],
};
const layouts = new Map(shop.tables.map((t) => [t.name, cardLayout(t, relationColumns(shop).get(t.name), measure)]));
const sizes = new Map([...layouts].map(([n, l]) => [n, { w: l.width, h: l.height }]));
const byName = (n) => shop.tables.find((t) => t.name === n);

test("compact types drop integer display widths and enum values", () => {
  assert.equal(compactType("int(10) unsigned"), "int unsigned");
  assert.equal(compactType("BIGINT(20) UNSIGNED ZEROFILL"), "bigint unsigned");
  assert.equal(compactType("tinyint(1)"), "tinyint(1)");
  assert.equal(compactType("varchar(255)"), "varchar(255)");
  assert.equal(compactType("decimal(10,2)"), "decimal(10,2)");
  assert.equal(compactType("enum('new','paid','sent')"), "enum");
  assert.equal(compactType("set('a','b')"), "set");
  assert.equal(compactType("timestamp(6)"), "timestamp(6)");
  assert.equal(compactType("varchar(255) character set x"), "varchar(255) characte…");
});

test("type families pick the chip colour", () => {
  assert.equal(typeFamily("int(10) unsigned"), "integer");
  assert.equal(typeFamily("tinyint(1)"), "boolean");
  assert.equal(typeFamily("decimal(10,2)"), "decimal");
  assert.equal(typeFamily("datetime"), "temporal");
  assert.equal(typeFamily("enum('a')"), "enumSet");
  assert.equal(typeFamily("varchar(20)"), "text");
  assert.equal(typeFamily("longblob"), "binary");
  assert.equal(typeFamily("json"), "json");
  assert.equal(typeFamily("point"), "spatial");
  assert.equal(typeFamily("inet6"), "text");
  assert.equal(typeFamily("vector(3)"), "other");
});

test("summaries, counts, row help and error hints read like the macOS app", () => {
  assert.equal(summary(shop.relations[1]), "order_items.order_id → orders.id · ON DELETE CASCADE");
  assert.equal(summary(rel("fk", "a", ["x", "y"], "b", ["x", "y"], "RESTRICT", "CASCADE")),
    "a.(x, y) → b.(x, y) · ON DELETE RESTRICT · ON UPDATE CASCADE");
  assert.equal(summary(rel("fk", "a", ["x"], "b", ["y"], "NO ACTION", "NO ACTION")), "a.x → b.y · ON DELETE NO ACTION");
  assert.equal(countText(shop, 7, false), "6 tables · 1 view · 5 relations");
  assert.equal(countText(shop, 5, true), "6 tables · 1 view · 5 relations · 2 hidden");
  assert.equal(countText({ ...shop, tables: [shop.tables[0]], relations: [shop.relations[0]] }, 1, true), "1 table · 1 relation");
  assert.equal(rowHelp(col("email", "varchar(160)", "UNI")), "email · varchar(160) · unique · NOT NULL");
  assert.equal(rowHelp(col("user_id", "int", "MUL", true), "auth.users.id"), "user_id · int · indexed · NULL allowed · refers to auth.users.id");
  assert.match(explain("ERROR 1005 (HY000): Can't create table (errno: 150 \"Foreign key constraint is incorrectly formed\")"), /differ in type/);
  assert.match(explain("ERROR 1452 (23000): Cannot add or update a child row"), /does not exist in the referenced table/);
  assert.match(explain("ERROR 1826 (HY000): Duplicate foreign key constraint name 'fk'"), /already exists/);
  assert.equal(explain("ERROR 2002: Can't connect"), null);
});

test("the delete SQL matches the backend's and the relation SQL is coloured", () => {
  assert.equal(dropSql("shop", "or`ders", "fk_1"), "ALTER TABLE `shop`.`or``ders` DROP FOREIGN KEY `fk_1`;");
  const tokens = sqlTokens("ALTER TABLE `shop`.`a``b`\n  ADD CONSTRAINT `fk` FOREIGN KEY (`x`)\n  ON DELETE SET NULL ON UPDATE NO ACTION;");
  assert.equal(tokens.map((t) => t.text).join(""), "ALTER TABLE `shop`.`a``b`\n  ADD CONSTRAINT `fk` FOREIGN KEY (`x`)\n  ON DELETE SET NULL ON UPDATE NO ACTION;");
  assert.deepEqual(tokens.filter((t) => t.kind === "name").map((t) => t.text), ["`shop`", "`a``b`", "`fk`", "`x`"]);
  assert.deepEqual(tokens.filter((t) => t.kind === "keyword").map((t) => t.text),
    ["ALTER", "TABLE", "ADD", "CONSTRAINT", "FOREIGN", "KEY", "ON", "DELETE", "SET", "NULL", "ON", "UPDATE", "NO", "ACTION"]);
});

test("a drag from a key onto a plain column reads the other way round", () => {
  const id = col("id", "int", "PRI"), fk = col("customer_id", "int", "MUL"), plain = col("note", "text");
  assert.deepEqual(linkRelation("orders", fk, "customers", id), { table: "orders", column: "customer_id", refTable: "customers", refColumn: "id" });
  assert.deepEqual(linkRelation("customers", id, "orders", fk), { table: "orders", column: "customer_id", refTable: "customers", refColumn: "id" });
  assert.deepEqual(linkRelation("orders", fk, "notes", plain), { table: "notes", column: "note", refTable: "orders", refColumn: "customer_id" });
  assert.deepEqual(linkRelation("notes", plain, "customers", id), { table: "notes", column: "note", refTable: "customers", refColumn: "id" });
});

test("hovering a table keeps it and its neighbours bright", () => {
  const e = emphasis(shop, null, "orders");
  assert.deepEqual([...e.tables].sort(), ["customers", "order_items", "orders"]);
  assert.equal(e.relations.size, 2);
  const r = emphasis(shop, shop.relations[3], "orders");
  assert.deepEqual([...r.tables].sort(), ["customers", "orders"]);
  assert.deepEqual([...r.relations], [relationId(shop.relations[3])]);
  assert.equal(emphasis(shop, null, null), null);
});

test("cards show every column up to 24, then the first 16 plus keys and a +N more row", () => {
  const small = layouts.get("orders");
  assert.deepEqual(small.rows, [0, 1]);
  assert.equal(small.hidden, 0);
  assert.equal(small.height, METRICS.headerHeight + 2 * METRICS.rowHeight + METRICS.bottomPadding);
  assert.equal(small.width, 20 + 30 + 11 * 7 + 14 + 12 * 6 + 12, "the longest name and type decide the width");
  assert.equal(cardLayout(table("t", [col("id", "int")]), undefined, measure).width, METRICS.minWidth, "narrow cards get the minimum");

  const many = table("wide", [...Array.from({ length: 30 }, (_, i) => col(`c${i}`, "text")), col("id", "int", "PRI")]);
  const l = cardLayout(many, new Set(["c20", "c29"]), measure);
  assert.deepEqual(l.rows, [...Array.from({ length: 16 }, (_, i) => i), 20, 29, 30]);
  assert.equal(l.hidden, 31 - 19);
  assert.equal(l.height, METRICS.headerHeight + 20 * METRICS.rowHeight + METRICS.bottomPadding, "19 rows and the +N more row");
  assert.equal(rowCenter(l, many, "c29"), METRICS.headerHeight + 17.5 * METRICS.rowHeight);
  assert.equal(rowCenter(l, many, "c25"), METRICS.headerHeight + 19.5 * METRICS.rowHeight, "hidden columns point at +N more");

  const huge = table("x".repeat(80), [col("y".repeat(80), "varchar(255)")]);
  assert.equal(cardLayout(huge, undefined, measure).width, METRICS.maxWidth);
  const view = cardLayout(table("v", [], true), undefined, measure);
  assert.equal(view.height, METRICS.headerHeight + METRICS.rowHeight + METRICS.bottomPadding, "an empty card keeps one row");
});

test("rows are found by height", () => {
  const t = byName("customers"), l = layouts.get("customers");
  assert.equal(columnAt(l, t, 10), null, "the header");
  assert.equal(columnAt(l, t, METRICS.headerHeight + 1), "id");
  assert.equal(columnAt(l, t, METRICS.headerHeight + METRICS.rowHeight + 1), "email");
  assert.equal(columnAt(l, t, METRICS.headerHeight + 2 * METRICS.rowHeight + 1), null, "below the last row");
});

const frame = (name, x, y) => ({ x, y, w: layouts.get(name).width, h: layouts.get(name).height });
function curvesOf(r, fa, fb) {
  return relationCurves(r, byName(r.table), fa, layouts.get(r.table), byName(r.refTable), fb, layouts.get(r.refTable));
}

test("a line leaves the FK row and ends with an arrow on the referenced side", () => {
  const r = shop.relations[3]; // orders.customer_id → customers.id
  // Referenced card to the right.
  const W = layouts.get("orders").width, C = layouts.get("customers").width;
  const [right] = curvesOf(r, frame("orders", 0, 0), frame("customers", 400, 100));
  assert.deepEqual(right.start, { x: W, y: METRICS.headerHeight + 1.5 * METRICS.rowHeight });
  assert.deepEqual(right.end, { x: 400 - METRICS.arrowLength, y: 100 + METRICS.headerHeight + 0.5 * METRICS.rowHeight });
  assert.equal(right.endSide, -1);
  assert.equal(arrowPath(right), `M400 ${right.end.y}L392 ${right.end.y - 4.2}L392 ${right.end.y + 4.2}Z`, "the tip touches the card");
  // Referenced card to the left.
  const [left] = curvesOf(r, frame("orders", 500, 0), frame("customers", 0, 0));
  assert.equal(left.start.x, 500);
  assert.equal(left.end.x, C + METRICS.arrowLength);
  assert.equal(left.endSide, 1);
  assert.ok(arrowPath(left).startsWith(`M${C} `));
  // Stacked: a C-shaped curve on the right of both.
  const [stacked] = curvesOf(r, frame("orders", 0, 300), frame("customers", 10, 0));
  assert.ok(stacked.c1.x > Math.max(W, 10 + C) + 50 && stacked.c1.x === stacked.c2.x);
  assert.equal(stacked.end.x, 10 + C + METRICS.arrowLength);
});

test("a self-reference is a loop on the right edge", () => {
  const r = shop.relations[0]; // categories.parent_id → categories.id
  const edge = 100 + layouts.get("categories").width;
  const [loop] = curvesOf(r, frame("categories", 100, 50), frame("categories", 100, 50));
  assert.equal(loop.start.x, edge);
  assert.equal(loop.end.x, edge + METRICS.arrowLength);
  assert.ok(loop.c1.x > edge + 46 && loop.c1.x === loop.c2.x);
  assert.equal(loop.start.y, 50 + METRICS.headerHeight + 1.5 * METRICS.rowHeight);
  assert.equal(loop.end.y, 50 + METRICS.headerHeight + 0.5 * METRICS.rowHeight);
  const same = rel("fk_same", "categories", ["id"], "categories", ["id"]);
  const [flat] = curvesOf(same, frame("categories", 0, 0), frame("categories", 0, 0));
  assert.ok(flat.end.y > flat.start.y, "a loop onto its own row still opens up");
});

test("the dragged line bends out of the handle and into the target", () => {
  assert.equal(linkPath({ x: 100, y: 50 }, { x: 300, y: 80 }), "M100 50C190 50 210 80 300 80");
  assert.equal(linkPath({ x: 100, y: 50 }, { x: 90, y: 80 }), "M100 50C130 50 120 80 90 80", "back to the left: a short loop");
});

test("composite keys draw one line per column pair, spread apart", () => {
  const two = { ...shop, tables: [table("a", [col("x"), col("y")]), table("b", [col("x", "int", "PRI"), col("y", "int", "PRI")])] };
  const r = rel("fk_ab", "a", ["x", "y"], "b", ["x", "y"]);
  const l = (n) => cardLayout(two.tables.find((t) => t.name === n), new Set(["x", "y"]), measure);
  const cs = relationCurves(r, two.tables[0], { x: 0, y: 0, w: 170, h: 100 }, l("a"), two.tables[1], { x: 20, y: 200, w: 170, h: 100 }, l("b"));
  assert.equal(cs.length, 2);
  assert.deepEqual(cs.map((c) => c.pair), [0, 1]);
  assert.ok(cs[1].c1.x - cs[0].c1.x >= 6);
});

test("pointer hit tests: cards by frame and lines by distance", () => {
  const frames = { orders: frame("orders", 0, 0), customers: frame("customers", 100, 20) };
  const f = (n) => frames[n];
  assert.equal(tableAt({ x: 150, y: 40 }, ["orders", "customers"], f), "customers", "the last drawn is on top");
  assert.equal(tableAt({ x: 50, y: 40 }, ["orders", "customers"], f), "orders");
  assert.equal(tableAt({ x: 274, y: 40 }, ["orders", "customers"], f), "customers", "the handle beside the edge counts");
  assert.equal(tableAt({ x: 500, y: 500 }, ["orders", "customers"], f), null);

  const [c] = curvesOf(shop.relations[3], frame("orders", 0, 0), frame("customers", 400, 100));
  const mid = curvePoint(c, 0.5);
  assert.ok(curveDistance(c, mid) < 0.5);
  assert.equal(relationAt({ x: mid.x, y: mid.y + 4 }, [c], () => true, 6), c.id);
  assert.equal(relationAt({ x: mid.x, y: mid.y + 9 }, [c], () => true, 6), null);
  assert.equal(relationAt(mid, [c], () => false, 6), null, "lines of hidden tables don't count");
  const [other] = curvesOf(shop.relations[1], frame("order_items", 0, 10), frame("orders", 400, 110));
  assert.equal(relationAt(curvePoint(other, 0.5), [c, other], () => true, 30), other.id, "the nearest line wins");
});

function overlaps(places) {
  const list = [...places].map(([n, p]) => ({ n, x: p.x, y: p.y, w: sizes.get(n).w, h: sizes.get(n).h }));
  for (const a of list) for (const b of list) {
    if (a.n < b.n && a.x < b.x + b.w && b.x < a.x + a.w && a.y < b.y + b.h && b.y < a.y + a.h) return `${a.n} / ${b.n}`;
  }
  return null;
}

test("auto layout: referenced tables left, connected groups first, loners to the right, no overlaps", () => {
  const p = autoLayout(shop, sizes);
  assert.equal(p.size, shop.tables.length);
  assert.ok(p.get("customers").x < p.get("orders").x && p.get("orders").x < p.get("order_items").x);
  assert.ok(p.get("categories").x < p.get("products").x && p.get("products").x < p.get("order_items").x);
  const groupRight = Math.max(...["categories", "customers", "order_items", "orders", "products"].map((n) => p.get(n).x + sizes.get(n).w));
  assert.ok(p.get("order_totals").x > groupRight && p.get("settings").x > groupRight);
  assert.equal(overlaps(p), null);
  assert.deepEqual([...autoLayout(shop, sizes)], [...p], "the same schema always gets the same layout");
});

test("auto layout survives cycles and many tables without overlaps", () => {
  const n = 100;
  const tables = Array.from({ length: n }, (_, i) => table(`t${String(i).padStart(3, "0")}`, [col("id", "int", "PRI"), col("ref", "int", "MUL")]));
  const relations = [];
  for (let i = 0; i < 60; i++) relations.push(rel(`fk${i}`, tables[i].name, ["ref"], tables[(i * 7 + 3) % 60].name, ["id"]));
  relations.push(rel("cycle_a", "t000", ["ref"], "t001", ["id"]), rel("cycle_b", "t001", ["ref"], "t000", ["id"]));
  const big = { database: "big", tables, relations, external: [] };
  const sz = new Map(tables.map((t) => [t.name, { w: 180, h: 80 }]));
  const p = autoLayout(big, sz);
  assert.equal(p.size, n);
  const list = [...p].map(([name, q]) => ({ name, ...q }));
  for (const a of list) for (const b of list) {
    if (a.name < b.name) assert.ok(!(a.x < b.x + 180 && b.x < a.x + 180 && a.y < b.y + 80 && b.y < a.y + 80), `${a.name} / ${b.name}`);
  }
});

test("known positions stay; new tables go beside them; dropped tables go away", () => {
  const current = new Map([["orders", { x: 500, y: 300 }], ["customers", { x: 40, y: 60 }], ["gone", { x: 0, y: 0 }]]);
  const p = placed(shop, sizes, current);
  assert.deepEqual(p.get("orders"), { x: 500, y: 300 });
  assert.deepEqual(p.get("customers"), { x: 40, y: 60 });
  assert.ok(!p.has("gone"));
  const right = 500 + sizes.get("orders").w;
  for (const t of ["categories", "order_items", "order_totals", "products", "settings"]) assert.ok(p.get(t).x >= right + 120, t);
  assert.deepEqual(placed(shop, sizes, new Map()), autoLayout(shop, sizes));
});

test("zoom keeps the point under the pointer, fit shows everything at most at 100 %", () => {
  const size = { w: 800, h: 600 };
  const content = { x: 0, y: 0, w: 2000, h: 1500 };
  const v = { zoom: 1, pan: { x: -100, y: -50 } };
  const anchor = { x: 300, y: 200 };
  const z = zoomView(v, 1.5, anchor, size, content);
  const before = toWorld(v, anchor), after = toWorld(z, anchor);
  assert.ok(Math.abs(before.x - after.x) < 1e-9 && Math.abs(before.y - after.y) < 1e-9);
  assert.equal(zoomView(v, 99, null, size, content).zoom, ZOOM.max);
  assert.equal(zoomView(v, 0, null, size, content).zoom, ZOOM.min);

  const fit = fitView(content, size);
  assert.ok(fit.zoom <= 1 && fit.zoom > 0.3);
  assert.ok(fit.pan.x >= 27.9 && fit.pan.y >= 27.9, "a 28 px margin");
  assert.equal(fitView({ x: 0, y: 0, w: 100, h: 50 }, size).zoom, 1, "small diagrams are not blown up");
  assert.equal(fitView(content, { w: 30, h: 600 }), null);

  const kept = clampPan({ x: -5000, y: 5000 }, 1, size, content);
  assert.deepEqual(kept, { x: 120 - 2000, y: 600 - 120 }, "a strip of the diagram stays on screen");
});

test("the loader shows the cache at once, reads once, and drops stale answers", async () => {
  const reads = [];
  const read = (db) => new Promise((resolve, reject) => reads.push({ db, resolve, reject }));
  const cache = new Map();
  const shown = [];
  const failed = [];
  const loader = new Loader(read, (a, b) => a.v === b.v, cache);
  const show = (x) => shown.push(x.db + ":" + x.v);
  const fail = (e) => failed.push(e);

  const first = loader.load("a", show, fail);
  assert.equal(reads.length, 1);
  reads[0].resolve({ db: "a", v: 1 });
  await first;
  assert.deepEqual(shown, ["a:1"]);

  // Switch to b (slow), then back to a: a's cache shows at once; b's late answer is dropped.
  const toB = loader.load("b", show, fail);
  const backToA = loader.load("a", show, fail);
  assert.deepEqual(shown, ["a:1", "a:1"]);
  reads[1].resolve({ db: "b", v: 1 });
  reads[2].resolve({ db: "a", v: 1 });
  await Promise.all([toB, backToA]);
  assert.deepEqual(shown, ["a:1", "a:1"], "an unchanged schema is not shown again, b's answer is stale");
  assert.ok(!cache.has("b"));

  // A reload of the same database with a change shows it once.
  const again = loader.load("a", show, fail);
  reads[3].resolve({ db: "a", v: 2 });
  await again;
  assert.deepEqual(shown, ["a:1", "a:1", "a:2"]);

  // Errors count only for the current read.
  const broken = loader.load("c", show, fail);
  const current = loader.load("c", show, fail);
  reads[4].reject("old error");
  reads[5].reject("ERROR 2002");
  await Promise.all([broken, current]);
  assert.deepEqual(failed, ["ERROR 2002"]);
});

test("a schema put after an add or drop wins over the read under way", async () => {
  const reads = [];
  const read = (db) => new Promise((resolve) => reads.push(resolve));
  const cache = new Map([["a", { db: "a", v: 1 }]]);
  const shown = [];
  const loader = new Loader(read, (x, y) => x.v === y.v, cache);
  const show = (x) => shown.push(x.db + ":" + x.v);

  const background = loader.load("a", show, () => {});
  assert.deepEqual(shown, ["a:1"]);
  loader.put("a", { db: "a", v: 2 });
  reads[0]({ db: "a", v: 1 }); // read before the change: stale
  await background;
  assert.deepEqual(shown, ["a:1"], "the old schema does not come back");
  assert.equal(cache.get("a").v, 2);

  // Another database is shown now: its cache is updated, and the current read still counts.
  const other = loader.load("b", show, () => {});
  loader.put("a", { db: "a", v: 3 });
  reads[1]({ db: "b", v: 1 });
  await other;
  assert.deepEqual(shown, ["a:1", "b:1"]);
  assert.equal(cache.get("a").v, 3);
});
