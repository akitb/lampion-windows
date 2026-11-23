<script lang="ts">
  // The log lines, drawn only around what is in view, so 5000 lines stay smooth. Long messages wrap, so
  // rows differ in height: each drawn row is measured and remembered, the others are estimated from
  // their length. Follow keeps the end in view (jumps, never animated: that stutters on long lists).
  import { tick } from "svelte";
  import { marks, tagColor, type Row } from "./rows";

  let {
    rows,
    highlight,
    follow,
    onrowmenu,
  }: { rows: Row[]; highlight: string; follow: boolean; onrowmenu: (row: Row, e: MouseEvent) => void } = $props();

  /** Space above the first row and below the last one. */
  const PAD = 6;
  /** A message line. */
  const LINE = 15;
  /** Rows drawn beyond the edges of the view, in pixels. */
  const OVERSCAN = 400;

  let scroller: HTMLDivElement | undefined = $state();
  let top = $state(0);
  let height = $state(0);
  let width = $state(0);
  let probe = $state(0);
  /** Bumped when measured heights change. */
  let version = $state(0);
  const measured = new WeakMap<Row, number>();
  const charWidth = $derived(probe > 0 ? probe / 10 : 7);

  function estimate(r: Row) {
    const used = 29 + (r.time ? 72 : 0) + (r.tag ? 106 : 0);
    const perLine = Math.max(10, Math.floor((width - used) / charWidth));
    const lines = Math.ceil(Math.max(1, r.message.length) / perLine);
    return lines * LINE + 6 + (r.tag && lines === 1 ? 1 : 0);
  }

  /** Top of each row (and the total height at the end). */
  const offsets = $derived.by(() => {
    void version;
    const o = new Float64Array(rows.length + 1);
    for (let i = 0; i < rows.length; i++) o[i + 1] = o[i] + (measured.get(rows[i]) ?? estimate(rows[i]));
    return o;
  });

  /** The first row that ends below `y`. */
  function rowAt(y: number) {
    let lo = 0;
    let hi = rows.length;
    while (lo < hi) {
      const mid = (lo + hi) >> 1;
      if (offsets[mid + 1] > y) hi = mid;
      else lo = mid + 1;
    }
    return lo;
  }

  const first = $derived(rowAt(top - PAD - OVERSCAN));
  const last = $derived(Math.min(rows.length, rowAt(top - PAD + height + OVERSCAN) + 1));
  const drawn = $derived(rows.slice(first, last));

  /** Follow is on and the end is in view: keep it there while rows are measured. */
  let stick = false;

  function toEnd() {
    if (!scroller) return;
    scroller.scrollTop = scroller.scrollHeight;
    top = scroller.scrollTop;
  }

  // New lines, another filter or Follow turned on: jump to the end.
  $effect(() => {
    void rows;
    if (!follow) {
      stick = false;
      return;
    }
    stick = true;
    void tick().then(toEnd);
  });

  function scrolled() {
    if (!scroller) return;
    top = scroller.scrollTop;
    stick = follow && scroller.scrollTop + scroller.clientHeight >= scroller.scrollHeight - 2;
  }

  const owners = new Map<Element, Row>();
  const observer = new ResizeObserver((entries) => {
    let changed = false;
    let shift = 0;
    for (const e of entries) {
      const row = owners.get(e.target);
      if (!row) continue;
      const h = e.borderBoxSize?.[0]?.blockSize ?? (e.target as HTMLElement).offsetHeight;
      const before = measured.get(row) ?? estimate(row);
      measured.set(row, h);
      if (Math.abs(h - before) < 0.5) continue;
      changed = true;
      // A row above the view changed height: scroll by as much, so the lines in view stay put.
      const i = Number((e.target as HTMLElement).dataset.i);
      if (scroller && offsets[i] + PAD < scroller.scrollTop) shift += h - before;
    }
    if (!changed) return;
    version++;
    void tick().then(() => {
      if (stick) toEnd();
      else if (shift && scroller) scroller.scrollTop += shift;
    });
  });

  function context(e: MouseEvent) {
    const el = (e.target as Element).closest<HTMLElement>(".row");
    const row = el ? rows[Number(el.dataset.i)] : undefined;
    if (!row) return;
    e.preventDefault();
    onrowmenu(row, e);
  }

  function measure(node: HTMLElement, row: Row) {
    owners.set(node, row);
    observer.observe(node);
    return {
      destroy() {
        observer.unobserve(node);
        owners.delete(node);
      },
    };
  }
</script>

<!-- svelte-ignore a11y_no_noninteractive_element_interactions (the right-click menu is a mouse shortcut; the "…" menu copies lines from the keyboard) -->
<div
  class="scroller"
  bind:this={scroller}
  bind:clientHeight={height}
  bind:clientWidth={width}
  onscroll={scrolled}
  oncontextmenu={context}
  role="log"
  aria-label="Log lines"
>
  <span class="probe" aria-hidden="true" bind:offsetWidth={probe}>0000000000</span>
  <div class="space" style:height="{offsets[rows.length] + 2 * PAD}px">
    <div class="rows" style:transform="translateY({offsets[first] + PAD}px)">
      {#each drawn as row, i (row)}
        <div class="row {row.level}" class:tagged={row.tag !== null} data-i={first + i} use:measure={row}>
          {#if row.time}<span class="time">{row.time}</span>{/if}
          {#if row.tag}<span class="tagcell"><span class="tag" style:--tag={tagColor(row.tag)} title={row.tag}>{row.tag}</span></span>{/if}
          <span class="msg"
            >{#if highlight}{#each marks(row.message, highlight) as m, j (j)}{#if m.hit}<mark>{m.text}</mark>{:else}{m.text}{/if}{/each}{:else}{row.message}{/if}</span
          >
        </div>
      {/each}
    </div>
  </div>
</div>

<style>
  .scroller { position: relative; flex: 1; min-height: 0; overflow-x: hidden; overflow-y: auto; background: var(--bg); outline: none; }
  .scroller:focus-visible { box-shadow: inset 0 0 0 2px var(--accent); }
  .probe { position: absolute; visibility: hidden; font: 12px var(--mono); white-space: pre; pointer-events: none; }
  .space { position: relative; }
  .rows { position: absolute; top: 0; left: 0; right: 0; }
  /* Sizes from the macOS screenshots (2x, halved): rows 21 px, with a tag 22 px, wrapped lines 15 px. */
  .row {
    position: relative;
    display: flex;
    align-items: baseline;
    gap: 10px;
    padding: 3px 16px 3px 13px;
    font: 12px/15px var(--mono);
    color: var(--text);
    user-select: text;
  }
  .row.tagged { min-height: 22px; }
  .row::before { content: ""; position: absolute; top: 0; bottom: 0; left: 0; width: 3px; }
  .row.error { background: color-mix(in srgb, var(--red) 7%, transparent); color: var(--red); }
  .row.error::before { background: var(--red); }
  .row.warn { background: color-mix(in srgb, var(--log-orange) 6%, transparent); }
  .row.warn::before { background: var(--log-orange); }
  .row.ok { color: var(--green); }
  .row.ok::before { background: color-mix(in srgb, var(--green) 70%, transparent); }
  .time { flex: none; width: 62px; color: var(--text-2); }
  .tagcell { flex: none; display: flex; width: 96px; }
  .tag {
    max-width: 100%;
    padding: 0 6px;
    border-radius: 4px;
    background: color-mix(in srgb, var(--tag) 14%, transparent);
    color: var(--tag);
    font: 600 10.5px/14px var(--mono);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .msg { flex: 1; min-width: 0; white-space: pre-wrap; overflow-wrap: anywhere; tab-size: 28px; }
  mark { border-radius: 2px; background: color-mix(in srgb, var(--log-yellow) 45%, transparent); color: inherit; }
</style>
