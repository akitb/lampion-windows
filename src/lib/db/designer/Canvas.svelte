<script lang="ts">
  // The pannable, zoomable diagram (port of DesignerCanvas). Lines and cards live in one scaled "world" layer,
  // so panning and zooming change only its transform; the dots, the relation being dragged and the tooltip are
  // drawn in canvas coordinates. All pointer handling (hover, drag, double-click, context menu) is here, with
  // hit tests against the store's geometry, so the cards need no handlers of their own.
  import { untrack } from "svelte";
  import { fade } from "svelte/transition";
  import { openTable } from "../dbstate.svelte";
  import { contextMenu, type MenuItem } from "../menu.svelte";
  import { copy } from "../util";
  import Card from "./Card.svelte";
  import { arrowPath, curvePath, linkPath, rowCenter, summary, toWorld, type Point } from "./diagram";
  import type { DesignerStore } from "./store.svelte";

  let { store, database }: { store: DesignerStore; database: string } = $props();

  type Mode =
    | { kind: "pan"; start: Point; from: Point }
    | { kind: "move"; table: string; start: Point; from: Point }
    | { kind: "link" };

  let el: HTMLDivElement | undefined = $state();
  let origin = { x: 0, y: 0 };
  let press: Point | null = null;
  let mode = $state.raw<Mode | null>(null);
  /** While panning or zooming, the world is its own layer (no repaint per frame); then it is redrawn crisp. */
  let moving = $state(false);
  let settle: ReturnType<typeof setTimeout> | undefined;
  let tipWidth = $state(0);
  let tipHeight = $state(0);

  const v = $derived(store.view);
  const screen = (p: Point): Point => ({ x: p.x * v.zoom + v.pan.x, y: p.y * v.zoom + v.pan.y });

  // Background dots every 24 points of the world (fewer when zoomed far out). Set as plain (not inherited)
  // properties: a custom property here would restyle every card on each scroll step.
  const dots = $derived.by(() => {
    let step = 24 * v.zoom;
    while (step < 12) step *= 2;
    const offset = (a: number) => (((a % step) + step) % step) - step / 2;
    return { step, x: offset(v.pan.x), y: offset(v.pan.y) };
  });

  /** From the source handle to the pointer, or to the nearer edge of the target row. */
  const linkLine = $derived.by(() => {
    const l = store.link;
    if (!l) return null;
    const row = (table: string, column: string) => {
      const f = store.frame(table), t = store.tables.get(table), lay = store.layouts.get(table);
      return f && t && lay ? { f, y: f.y + rowCenter(lay, t, column) } : null;
    };
    const a = row(l.table, l.column);
    if (!a) return null;
    const from = screen({ x: a.f.x + a.f.w, y: a.y });
    let to = screen(l.point);
    const b = l.target && row(l.target.table, l.target.column);
    if (b) {
      const left = screen({ x: b.f.x, y: b.y }), right = screen({ x: b.f.x + b.f.w, y: b.y });
      to = Math.abs(left.x - from.x) < Math.abs(right.x - from.x) ? left : right;
    }
    return { d: linkPath(from, to), to };
  });

  const tip = $derived(!mode && !store.link && store.hoveredRelation !== null ? store.relations.get(store.hoveredRelation) ?? null : null);

  const at = (e: MouseEvent): Point => ({ x: e.clientX - origin.x, y: e.clientY - origin.y });

  function measureOrigin() {
    const r = el?.getBoundingClientRect();
    if (r) origin = { x: r.left, y: r.top };
  }

  function settleSoon() {
    moving = true;
    clearTimeout(settle);
    settle = setTimeout(() => (moving = false), 150);
  }

  function fitIfNeeded() {
    if (!store.needsFit || !store.schema || store.size.w <= 40) return;
    store.fit();
    store.needsFit = false;
  }

  $effect(() => {
    const node = el;
    if (!node) return;
    const resized = new ResizeObserver(() => {
      store.size = { w: node.clientWidth, h: node.clientHeight };
      measureOrigin();
      if (store.followsFit && !store.needsFit) store.fit();
      fitIfNeeded();
    });
    resized.observe(node);
    // Not passive: Ctrl+wheel must not zoom the page, and the wheel must not scroll it.
    node.addEventListener("wheel", wheel, { passive: false });
    return () => {
      resized.disconnect();
      node.removeEventListener("wheel", wheel);
      clearTimeout(settle);
    };
  });

  $effect(() => {
    if (store.needsFit && store.schema) untrack(fitIfNeeded);
  });

  function open(table: string, tab: "structure" | "data") {
    openTable(database, table, tab, store.tables.get(table)?.isView ?? false);
  }

  // ---- Pointer ----

  function down(e: PointerEvent) {
    if (e.button !== 0 || !el) return;
    measureOrigin();
    press = at(e);
    el.setPointerCapture(e.pointerId);
  }

  /** What a drag does depends on where it starts: a row's handle links, a card moves, empty space pans. */
  function begin(start: Point) {
    const p = toWorld(v, start);
    const name = store.tableAt(p);
    const f = name === null ? undefined : store.frame(name);
    if (name === null || !f) {
      mode = { kind: "pan", start, from: v.pan };
      moving = true;
      return;
    }
    const reach = Math.max(7, 7 / v.zoom);
    const c = store.canLink(name) && Math.abs(p.x - (f.x + f.w)) <= reach ? store.columnAt(p) : null;
    if (c && c.table === name) {
      store.clearHover();
      store.beginLink(name, c.column, p);
      mode = { kind: "link" };
      return;
    }
    store.draggingTable = name;
    mode = { kind: "move", table: name, start, from: { x: f.x, y: f.y } };
  }

  function pointerMove(e: PointerEvent) {
    const s = at(e);
    if (press && !mode && Math.hypot(s.x - press.x, s.y - press.y) >= 2) begin(press);
    const m = mode;
    if (m?.kind === "pan") store.setPan({ x: m.from.x + s.x - m.start.x, y: m.from.y + s.y - m.start.y });
    else if (m?.kind === "move") store.move(m.table, { x: m.from.x + (s.x - m.start.x) / v.zoom, y: m.from.y + (s.y - m.start.y) / v.zoom });
    else if (m?.kind === "link") store.updateLink(toWorld(v, s));
    else store.hover(toWorld(v, s), s);
  }

  function finish(e: PointerEvent, cancelled: boolean) {
    const m = mode;
    press = null;
    mode = null;
    if (m?.kind === "move") {
      store.draggingTable = null;
      store.savePositions();
    } else if (m?.kind === "link") {
      if (cancelled) store.link = null;
      else {
        store.updateLink(toWorld(v, at(e)));
        store.endLink();
      }
    } else if (m?.kind === "pan") settleSoon();
    // The confirmation sheet covers the canvas: nothing stays hovered behind it.
    if (store.pending) store.clearHover();
    else if (!cancelled) store.hover(toWorld(v, at(e)), at(e));
  }

  function leave() {
    if (!mode) store.clearHover();
  }

  function doubleClick(e: MouseEvent) {
    const t = store.tableAt(toWorld(v, at(e)));
    if (t !== null) open(t, "structure");
  }

  // ---- Wheel: scroll, or zoom around the pointer with Ctrl (and touchpad pinch) ----

  function wheel(e: WheelEvent) {
    e.preventDefault();
    measureOrigin();
    const s = at(e);
    const unit = e.deltaMode === 1 ? 16 : e.deltaMode === 2 ? 400 : 1;
    let dx = e.deltaX * unit, dy = e.deltaY * unit;
    if (e.ctrlKey || e.metaKey) store.zoomTo(store.view.zoom * Math.exp(-dy / 400), s);
    else {
      if (e.shiftKey && dx === 0) [dx, dy] = [dy, 0];
      store.scrollBy(-dx, -dy);
    }
    settleSoon();
    if (!mode) store.hover(toWorld(store.view, s), s);
  }

  // ---- Menus and keys ----

  const tableItems = (t: string): MenuItem[] => [
    { label: "Open Structure", action: () => open(t, "structure") },
    { label: "Open Data", action: () => open(t, "data") },
    { separator: true },
    { label: "Copy Name", action: () => void copy(t) },
  ];

  function menu(e: MouseEvent) {
    const card = (e.target as HTMLElement).closest<HTMLElement>("[data-table]");
    if (e.button !== 2 && card?.dataset.table) {
      // From the keyboard (Menu key, Shift+F10) on a focused card.
      contextMenu(e, tableItems(card.dataset.table));
      return;
    }
    measureOrigin();
    store.hover(toWorld(v, at(e)), at(e));
    const r = store.hoveredRelation === null ? undefined : store.relations.get(store.hoveredRelation);
    const t = store.hoveredTable;
    contextMenu(e, r ? [
      { label: summary(r), disabled: true, action: () => {} },
      { separator: true },
      { label: "Delete Relation…", action: () => store.confirmDelete(r) },
      { label: `Open “${r.table}” Structure`, action: () => open(r.table, "structure") },
    ] : t !== null ? tableItems(t) : [
      { label: "Fit to Window", action: () => store.fit() },
      { label: "Auto Arrange", action: () => store.autoArrange() },
      { label: "Relations Only", checked: store.relationsOnly, action: () => store.setRelationsOnly(!store.relationsOnly) },
    ]);
  }

  function key(e: KeyboardEvent) {
    const t = (e.target as HTMLElement).closest<HTMLElement>("[data-table]")?.dataset.table;
    if (t && (e.key === "Enter" || e.key === " ")) {
      e.preventDefault();
      open(t, "structure");
    }
  }
</script>

<!-- Pointer work on the drawing; for the keyboard, the cards are buttons. -->
<!-- svelte-ignore a11y_no_noninteractive_element_interactions -->
<div
  bind:this={el}
  class="canvas"
  class:panning={mode?.kind === "pan"}
  role="region"
  aria-label="Diagram of {database}"
  style="background-size: {dots.step}px {dots.step}px; background-position: {dots.x}px {dots.y}px"
  onpointerdown={down}
  onpointermove={pointerMove}
  onpointerup={(e) => finish(e, false)}
  onpointercancel={(e) => finish(e, true)}
  onpointerenter={measureOrigin}
  onpointerleave={leave}
  ondblclick={doubleClick}
  oncontextmenu={menu}
  onkeydown={key}
>
  <div class="world" class:moving style="transform: translate({v.pan.x}px, {v.pan.y}px) scale({v.zoom})">
    <svg class="lines" aria-hidden="true" style="--z: {v.zoom}">
      {#each store.curves as c (c.id + "\u0002" + c.pair)}
        {@const bright = store.emphasis?.relations.has(c.id) ?? false}
        {@const hot = store.hoveredRelation === c.id}
        <g class="rel" class:bright={store.emphasis && bright} class:dim={store.emphasis && !bright}>
          <path class="line" d={curvePath(c)} />
          <path d={arrowPath(c, hot ? 4.83 : 4.2)} />
          <circle cx={c.start.x} cy={c.start.y} />
        </g>
      {/each}
    </svg>
    {#each store.visibleTables as t (t.name)}
      {@const p = store.positions.get(t.name)}
      {@const l = store.layouts.get(t.name)}
      {#if p && l}
        <Card
          table={t}
          layout={l}
          x={p.x}
          y={p.y}
          state={store.cardState(t.name)}
          foreign={store.foreign.get(t.name)}
          external={store.external.get(t.name)}
          lit={store.litColumns(t.name)}
          dropTarget={store.link?.target?.table === t.name ? store.link.target.column : null}
          dragSource={store.link?.table === t.name ? store.link.column : null}
          hovered={store.hoveredTable === t.name && !store.link}
          dragging={store.draggingTable === t.name}
        />
      {/if}
    {/each}
  </div>

  {#if linkLine}
    <svg class="link" aria-hidden="true">
      <path d={linkLine.d} />
      <circle cx={linkLine.to.x} cy={linkLine.to.y} r="4" />
    </svg>
  {/if}

  {#if tip}
    {@const a = store.hoverAnchor}
    <div
      class="tip"
      role="tooltip"
      bind:offsetWidth={tipWidth}
      bind:offsetHeight={tipHeight}
      style="left: {Math.min(a.x + 14, Math.max(8, store.size.w - tipWidth - 8))}px; top: {a.y + 18 + tipHeight > store.size.h ? a.y - tipHeight - 10 : a.y + 18}px"
      transition:fade={{ duration: 120 }}
    >
      {summary(tip)}
    </div>
  {/if}
</div>

<style>
  .canvas {
    position: relative;
    flex: 1;
    min-height: 0;
    overflow: hidden;
    background-color: var(--db-content);
    background-image: radial-gradient(circle, var(--dz-dot) 0.9px, transparent 1.4px);
    outline: none;
    touch-action: none;
  }
  .canvas.panning { cursor: grabbing; }
  .world {
    position: absolute;
    left: 0;
    top: 0;
    transform-origin: 0 0;
  }
  .world.moving { will-change: transform; }

  .lines {
    position: absolute;
    left: 0;
    top: 0;
    width: 1px;
    height: 1px;
    overflow: visible;
    pointer-events: none;
  }
  .rel {
    --alpha: 62%;
    --w: max(1.4px, calc(0.8px / var(--z)));
    color: color-mix(in srgb, var(--db-purple) var(--alpha), transparent);
    fill: currentColor;
  }
  .rel.bright { --alpha: 95%; --w: max(2.38px, calc(1.36px / var(--z))); }
  .rel.dim { --alpha: 10%; }
  .line {
    fill: none;
    stroke: currentColor;
    stroke-width: var(--w);
    stroke-linecap: round;
  }
  .rel circle { r: max(2.6px, calc(1.6px / var(--z))); }

  .link {
    position: absolute;
    inset: 0;
    width: 100%;
    height: 100%;
    overflow: visible;
    pointer-events: none;
  }
  .link path {
    fill: none;
    stroke: var(--accent);
    stroke-width: 2;
    stroke-linecap: round;
    stroke-dasharray: 6 4;
  }
  .link circle { fill: var(--accent); }

  .tip {
    position: absolute;
    z-index: 2;
    padding: 5px 9px;
    border: 1px solid var(--dz-tip-edge);
    border-radius: 7px;
    background: var(--dz-material);
    box-shadow: 0 2px 12px rgba(0, 0, 0, 0.12);
    font-family: var(--mono);
    font-size: 11.5px;
    font-weight: 500;
    white-space: nowrap;
    pointer-events: none;
  }
</style>
