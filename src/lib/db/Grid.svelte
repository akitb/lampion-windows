<script lang="ts">
  // A fast, calm data grid for table data and query results (port of GridView.swift): only the rows in
  // view are drawn. With `info`, headers get a second line with the SQL type, a key for primary-key
  // columns, and numbers align right. With `table`, the menu offers "Copy Row as INSERT".
  import { ChevronDown, ChevronUp, KeyRound } from "@lucide/svelte";
  import { untrack } from "svelte";
  import { dbapi, message } from "./dbapi.svelte";
  import { showError } from "./dbstate.svelte";
  import { contextMenu, keyboardMenu, menu, type MenuItem } from "./menu.svelte";
  import type { ColumnInfo } from "./types";
  import { copy, flatten, isSimple, measure } from "./util";
  import { badges, KEY_HELP } from "./KeyBadge.svelte";

  type Props = {
    columns: string[];
    rows: (string | null)[][];
    info?: ColumnInfo[];
    /** Changes when `rows` is replaced (a reload keeps the scroll position). */
    version?: number;
    /** Changes when a different slice is shown (table, page, sort): scrolls back to the top. */
    scrollKey?: string;
    editable?: boolean;
    sortColumn?: string | null;
    ascending?: boolean;
    selected?: number[];
    onedit?: (row: number, column: number, value: string | null) => void;
    onsort?: (column: string) => void;
    table?: string | null;
    label?: string;
    /** [row, column] of values cut to 64 KB (`Grid.cut`): never edited in place. */
    cut?: [number, number][];
    /** The whole value of a cut cell (Copy Cell). */
    loadCell?: (row: number, column: number) => Promise<string | null>;
  };
  let {
    columns,
    rows,
    cut = [],
    loadCell,
    info = [],
    version = 0,
    scrollKey = "",
    editable = false,
    sortColumn = null,
    ascending = true,
    selected = $bindable([]),
    onedit,
    onsort,
    table = null,
    label = "Rows",
  }: Props = $props();

  const ROW = 24;
  let scroller: HTMLDivElement | undefined = $state();
  let scrollTop = $state(0);
  let viewH = $state(600);
  let widths = $state<number[]>([]);
  let editing = $state<{ row: number; col: number; value: string } | null>(null);
  /** The keyboard's column: ← → move it, Enter / F2 edit it in the selected row. */
  let cursor = $state(0);
  /** This grid's menus (`menu.owner`): a reload closes them. */
  const owner = `grid-${Math.random()}`;
  let anchor = -1;
  let builtKey = "";
  let seenScroll = "";

  const byName = $derived(new Map(info.map((c) => [c.name, c])));
  const colInfo = $derived(columns.map((c) => byName.get(c)));
  const two = $derived(colInfo.some(Boolean));
  const headH = $derived(two ? 38 : 28);
  const lineAt = $derived(two ? 33 : 28);
  const right = $derived(colInfo.map((c) => !!c?.numeric));
  const total = $derived(widths.reduce((a, b) => a + b, 0));
  const first = $derived(Math.max(0, Math.floor((scrollTop - headH) / ROW) - 6));
  const last = $derived(Math.min(rows.length, Math.ceil((scrollTop + viewH) / ROW) + 6));
  const visible = $derived(Array.from({ length: Math.max(0, last - first) }, (_, i) => first + i));
  const sel = $derived(new Set(selected));
  const cutCells = $derived(new Set(cut.map(([r, c]) => `${r}:${c}`)));
  const isCut = (r: number, c: number) => cutCells.has(`${r}:${c}`);

  function fonts() {
    const cs = getComputedStyle(scroller ?? document.body);
    const sans = cs.fontFamily;
    const mono = cs.getPropertyValue("--mono") || "monospace";
    return { title: `600 11.5px ${sans}`, type: `10px ${mono}`, cell: `12px ${sans}`, nul: `italic 11px ${sans}` };
  }

  /** Header and the first 60 values decide a column's width (at most 380 px). */
  function width(i: number, f: ReturnType<typeof fonts>): number {
    const c = colInfo[i];
    let w = measure(columns[i], f.title) + (c?.key === "PRI" ? 14 : 0) + 14;
    if (c) w = Math.max(w, measure(c.type, f.type));
    for (const row of rows.slice(0, 60)) {
      const v = row[i];
      // Cells use tabular digits, which the canvas doesn't: measure every digit as "0".
      w = Math.max(w, v === null || v === undefined ? measure("NULL", f.nul) : measure(flatten(v).slice(0, 80).replace(/\d/g, "0"), f.cell));
      if (w >= 380) break;
    }
    return Math.min(380, Math.max(56, Math.ceil(w + 18)));
  }

  // New columns (another table or result): measure again and go back to the top left.
  $effect(() => {
    const key = columns.join("\u0001") + "\u0002" + colInfo.map((c) => (c ? `${c.type}\u0001${c.key}` : "")).join("\u0002");
    if (key === builtKey) return;
    builtKey = key;
    const f = fonts();
    widths = columns.map((_, i) => width(i, f));
    cursor = 0;
    toTop();
  });

  $effect(() => {
    if (scrollKey !== seenScroll) {
      seenScroll = scrollKey;
      toTop();
    }
  });

  // A reload ends an edit in progress and closes this grid's menu: its rows may be others now.
  $effect(() => {
    void version;
    editing = null;
    untrack(() => {
      if (menu.open && menu.owner === owner) menu.open = false;
    });
  });

  function toTop() {
    if (!scroller) return;
    scroller.scrollTop = 0;
    scroller.scrollLeft = 0;
    scrollTop = 0;
  }

  function scrollIntoView(r: number) {
    if (!scroller) return;
    const top = headH + r * ROW;
    if (top < scroller.scrollTop + headH) scroller.scrollTop = top - headH;
    else if (top + ROW > scroller.scrollTop + viewH) scroller.scrollTop = top + ROW - viewH;
  }

  function columnIntoView(c: number) {
    if (!scroller) return;
    const left = widths.slice(0, c).reduce((a, b) => a + b, 0);
    const w = widths[c] ?? 0;
    if (left < scroller.scrollLeft) scroller.scrollLeft = left;
    else if (left + w > scroller.scrollLeft + scroller.clientWidth) scroller.scrollLeft = left + w - scroller.clientWidth;
  }

  // ---- Selection ----

  function press(e: PointerEvent, r: number) {
    if (e.button !== 0 || e.target instanceof HTMLInputElement) return;
    if (editing) commit();
    // Focus the grid itself: focusing the row would scroll the grid back to its left edge.
    scroller?.focus({ preventScroll: true });
    const c = cellAt(e);
    if (c >= 0) cursor = c;
    if (e.ctrlKey || e.metaKey) {
      selected = sel.has(r) ? selected.filter((x) => x !== r) : [...selected, r].sort((a, b) => a - b);
      anchor = r;
    } else if (e.shiftKey && anchor >= 0) {
      const [a, b] = anchor < r ? [anchor, r] : [r, anchor];
      selected = Array.from({ length: b - a + 1 }, (_, i) => a + i);
    } else {
      selected = [r];
      anchor = r;
    }
  }

  /** The selected row the keyboard acts on (the last one clicked or moved to). */
  function current(): number {
    return selected.includes(anchor) ? anchor : (selected[0] ?? -1);
  }

  function keys(e: KeyboardEvent) {
    // Keys on a header button (Enter sorts) or the edit field are theirs.
    if (editing || e.target !== scroller) return;
    const mod = e.ctrlKey || e.metaKey;
    const r = current();
    if ((e.key === "Enter" || e.key === "F2") && !mod && r >= 0) {
      if (beginAt(r, cursor)) e.preventDefault();
      return;
    }
    if ((e.key === "F10" && e.shiftKey) || e.key === "ContextMenu") {
      e.preventDefault();
      if (r < 0 || !scroller) return;
      scrollIntoView(r);
      const box = scroller.getBoundingClientRect();
      const x = Math.min(box.right - 20, box.left + widths.slice(0, cursor).reduce((a, b) => a + b, 0) - scroller.scrollLeft + 12);
      keyboardMenu(Math.max(box.left + 4, x), box.top + headH + (r + 1) * ROW - scroller.scrollTop, menuItems(r, cursor), scroller, owner);
      return;
    }
    if ((e.key === "ArrowLeft" || e.key === "ArrowRight") && !mod && columns.length) {
      cursor = Math.max(0, Math.min(columns.length - 1, cursor + (e.key === "ArrowRight" ? 1 : -1)));
      columnIntoView(cursor);
      e.preventDefault();
      return;
    }
    if (mod && e.key.toLowerCase() === "c") {
      if (selected.length) copy(tsv(selected));
      e.preventDefault();
    } else if (mod && e.key.toLowerCase() === "a") {
      selected = rows.map((_, i) => i);
      e.preventDefault();
    } else if ((e.key === "ArrowDown" || e.key === "ArrowUp") && rows.length) {
      const from = selected.length ? (e.key === "ArrowDown" ? selected[selected.length - 1] : selected[0]) : -1;
      const to = Math.max(0, Math.min(rows.length - 1, from + (e.key === "ArrowDown" ? 1 : -1)));
      if (e.shiftKey && anchor >= 0) {
        const [a, b] = anchor < to ? [anchor, to] : [to, anchor];
        selected = Array.from({ length: b - a + 1 }, (_, i) => a + i);
      } else {
        selected = [to];
        anchor = to;
      }
      scrollIntoView(to);
      e.preventDefault();
    }
  }

  // ---- Editing ----

  function cellAt(e: Event): number {
    const el = (e.target as HTMLElement).closest<HTMLElement>("[data-c]");
    return el ? Number(el.dataset.c) : -1;
  }

  /** Starts editing a cell in place; false when it can't be (long, multi-line, cut or read-only). */
  function beginAt(r: number, c: number): boolean {
    const v = rows[r]?.[c];
    if (!editable || c < 0 || v === undefined || (v !== null && !isSimple(v)) || isCut(r, c)) return false;
    selected = [r];
    anchor = r;
    cursor = c;
    editing = { row: r, col: c, value: v ?? "" };
    return true;
  }

  function commit() {
    const ed = editing;
    if (!ed) return;
    editing = null;
    const old = rows[ed.row]?.[ed.col];
    if (old === undefined || (old === null && ed.value === "") || old === ed.value) return;
    onedit?.(ed.row, ed.col, ed.value);
    scroller?.focus({ preventScroll: true });
  }

  function editKeys(e: KeyboardEvent) {
    if (e.key === "Enter") {
      e.preventDefault();
      commit();
    } else if (e.key === "Escape") {
      e.preventDefault();
      editing = null;
      scroller?.focus({ preventScroll: true });
    }
  }

  function focusInput(el: HTMLInputElement) {
    el.focus();
    el.select();
  }

  // ---- Copy and the menu ----

  function tsv(list: number[]): string {
    return list.filter((r) => r < rows.length).map((r) => rows[r].map((v) => v ?? "NULL").join("\t")).join("\n");
  }

  async function copyInsert(list: number[]) {
    if (!table) return;
    const text = await dbapi.insertText(
      table,
      columns,
      colInfo.map((c) => !!c?.numeric),
      colInfo.map((c) => !!c && (c.binary || c.family === "spatial")),
      list.map((r) => rows[r]),
    );
    await copy(text);
  }

  async function copyCell(r: number, c: number) {
    try {
      // A cut value is copied whole.
      const v = isCut(r, c) && loadCell ? await loadCell(r, c) : rows[r][c];
      await copy(v ?? "NULL");
    } catch (e) {
      showError(message(e));
    }
  }

  /** The row menu. Its actions belong to the rows as they are now: after a reload they do nothing. */
  function menuItems(r: number, c: number): MenuItem[] {
    const targets = sel.has(r) ? [...selected] : [r];
    const n = targets.length;
    const at = version;
    const now = (f: () => void) => () => {
      if (version === at) f();
    };
    const someCut = cut.some(([row]) => targets.includes(row));
    const items: MenuItem[] = [];
    if (c >= 0) items.push({ label: "Copy Cell", action: now(() => copyCell(r, c)) });
    items.push({ label: n > 1 ? `Copy ${n} Rows` : "Copy Row", action: now(() => copy(tsv(targets))) });
    // A cut value would go into the INSERT cut.
    if (table) items.push({ label: n > 1 ? `Copy ${n} Rows as INSERT` : "Copy Row as INSERT", disabled: someCut, action: now(() => copyInsert(targets)) });
    if (c >= 0) items.push({ label: "Copy Column Name", action: () => copy(columns[c]) });
    if (editable && c >= 0) {
      items.push({ separator: true });
      // Not for NOT NULL columns: a relaxed sql_mode would store 0 or '' instead of failing.
      items.push({ label: "Set to NULL", disabled: rows[r][c] === null || colInfo[c]?.nullable === false, action: now(() => onedit?.(r, c, null)) });
    }
    return items;
  }

  function menuFor(e: MouseEvent, r: number) {
    const c = cellAt(e);
    if (c >= 0) cursor = c;
    contextMenu(e, menuItems(r, c), owner);
  }

  // ---- Column widths ----

  let dragged = false;

  function resize(e: PointerEvent, i: number) {
    e.preventDefault();
    e.stopPropagation();
    const handle = e.currentTarget as HTMLElement;
    handle.setPointerCapture(e.pointerId);
    const startX = e.clientX;
    const startW = widths[i];
    dragged = false;
    const move = (m: PointerEvent) => {
      dragged = true;
      widths[i] = Math.max(40, Math.round(startW + m.clientX - startX));
    };
    const up = () => {
      handle.removeEventListener("pointermove", move);
      handle.removeEventListener("pointerup", up);
      setTimeout(() => (dragged = false));
    };
    handle.addEventListener("pointermove", move);
    handle.addEventListener("pointerup", up);
  }

  function headerTip(i: number): string {
    const c = colInfo[i];
    if (!c) return columns[i];
    return [columns[i], c.type, ...badges(c).map((b) => KEY_HELP[b]), c.comment].filter(Boolean).join("\n");
  }
</script>

<div
  class="grid"
  bind:this={scroller}
  bind:clientHeight={viewH}
  onscroll={() => (scrollTop = scroller?.scrollTop ?? 0)}
  onkeydown={keys}
  tabindex="0"
  role="grid"
  aria-label={label}
  aria-rowcount={rows.length}
  aria-multiselectable="true"
>
  <div class="sizer" style="width:{total}px;height:{headH + rows.length * ROW}px">
    <div class="head" style="height:{headH}px;width:{total}px" role="row">
      {#each columns as name, i (i)}
        {@const c = colInfo[i]}
        <div class="hcell" style="width:{widths[i]}px" role="columnheader" aria-sort={sortColumn === name ? (ascending ? "ascending" : "descending") : undefined}>
          <button
            type="button"
            class="hbtn"
            class:right={right[i]}
            class:two
            tabindex={onsort ? 0 : -1}
            title={onsort ? `${headerTip(i)}\nClick or press Enter to sort` : headerTip(i)}
            onclick={() => { if (!dragged) onsort?.(name); }}
          >
            <span class="title">
              {#if c?.key === "PRI"}<span class="key filled"><KeyRound size={9} strokeWidth={2.4} /></span>{/if}
              <span class="name">{name}</span>
              {#if sortColumn === name}
                <span class="sort">{#if ascending}<ChevronUp size={10} strokeWidth={3} />{:else}<ChevronDown size={10} strokeWidth={3} />{/if}</span>
              {/if}
            </span>
            {#if two}<span class="type">{c?.type ?? ""}</span>{/if}
          </button>
          <span class="sep" class:two aria-hidden="true"></span>
          <!-- svelte-ignore a11y_no_static_element_interactions -->
          <span class="resize" onpointerdown={(e) => resize(e, i)}></span>
        </div>
      {/each}
      <div class="hline" style="top:{lineAt - 1}px"></div>
    </div>
    {#each visible as r (r)}
      <div
        class="row"
        class:alt={r % 2 === 1}
        class:sel={sel.has(r)}
        style="top:{headH + r * ROW}px;width:{total}px"
        role="row"
        tabindex="-1"
        aria-rowindex={r + 1}
        aria-selected={sel.has(r)}
        onpointerdown={(e) => press(e, r)}
        onmousedown={(e) => { if (!(e.target instanceof HTMLInputElement)) e.preventDefault(); }}
        ondblclick={(e) => beginAt(r, cellAt(e))}
        oncontextmenu={(e) => menuFor(e, r)}
      >
        {#each columns as _, c (c)}
          {@const v = rows[r]?.[c]}
          {@const short = isCut(r, c)}
          <div class="cell" class:right={right[c]} class:cur={c === cursor && sel.has(r)} class:cut={short} style="width:{widths[c]}px" data-c={c} role="gridcell"
               title={short ? `Only the first 64 KB of this value are shown.${loadCell ? " Copy Cell and the row details load all of it." : ""}` : v && (v.length > 60 || !isSimple(v)) ? v.slice(0, 1500) : undefined}>
            {#if editing && editing.row === r && editing.col === c}
              <input class="edit" class:right={right[c]} bind:value={editing.value} onkeydown={editKeys} onblur={commit} use:focusInput aria-label={columns[c]} />
            {:else if v === null || v === undefined}
              <span class="nullv">NULL</span>
            {:else}
              {flatten(v)}
            {/if}
          </div>
        {/each}
      </div>
    {/each}
  </div>
</div>

<style>
  .grid {
    position: relative;
    height: 100%;
    overflow: auto;
    background: var(--db-content);
    outline: none;
  }
  .grid:focus-visible { box-shadow: inset 0 0 0 2px color-mix(in srgb, var(--accent) 70%, transparent); }
  .sizer { position: relative; min-width: 100%; }
  .head {
    position: sticky;
    top: 0;
    z-index: 2;
    display: flex;
    min-width: 100%;
    background: var(--db-content);
  }
  .hline { position: absolute; left: 0; right: 0; height: 1px; background: var(--db-sep); }
  .hcell { position: relative; flex: none; }
  .hbtn {
    display: flex;
    flex-direction: column;
    justify-content: center;
    width: 100%;
    height: 28px;
    min-height: 0;
    padding: 0 8px;
    border: none;
    border-radius: 0;
    background: none;
    box-shadow: none;
    color: var(--text);
    text-align: left;
    overflow: hidden;
  }
  .hbtn:hover:not(:disabled) { background: none; }
  .hbtn.two { height: 33px; justify-content: flex-start; padding-top: 4px; }
  .hbtn.right { align-items: flex-end; text-align: right; }
  .hbtn:focus-visible { outline-offset: -2px; }
  .title { display: flex; align-items: center; gap: 4px; max-width: 100%; font-size: 11.5px; font-weight: 600; line-height: 14px; }
  .name { overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .key { display: inline-flex; color: var(--db-yellow); --cut: currentColor; flex: none; }
  .sort { display: inline-flex; color: var(--text-2); flex: none; margin-left: auto; }
  .right .sort { order: -2; margin-left: 0; margin-right: auto; }
  .right .key { order: -1; }
  .type {
    max-width: 100%;
    margin-top: 1px;
    overflow: hidden;
    color: var(--text-2);
    font-family: var(--mono);
    font-size: 10px;
    line-height: 13px;
    white-space: nowrap;
    text-overflow: ellipsis;
  }
  .sep { position: absolute; right: 0; top: 7px; height: 14px; width: 1px; background: var(--db-sep); }
  .sep.two { top: 8px; height: 18px; }
  .resize { position: absolute; right: -4px; top: 0; bottom: 0; width: 8px; cursor: col-resize; z-index: 1; }

  .row { position: absolute; left: 0; display: flex; height: 24px; min-width: 100%; background: var(--db-content); }
  .row.alt { background: var(--db-alt); }
  .row.sel { background: var(--db-sel); }
  .grid:focus-within .row.sel { background: var(--accent); color: var(--accent-text); }
  .grid:focus-within .row.sel .nullv { color: color-mix(in srgb, var(--accent-text) 60%, transparent); }
  .cell {
    flex: none;
    padding: 0 8px;
    overflow: hidden;
    font-size: 12px;
    font-variant-numeric: tabular-nums;
    line-height: 24px;
    white-space: nowrap;
    text-overflow: ellipsis;
    user-select: none;
  }
  .cell.right { text-align: right; }
  .cell.cut { font-style: italic; }
  .grid:focus-visible .row.sel .cell.cur { box-shadow: inset 0 0 0 1.5px var(--accent-text); }
  .cell:first-child:not(.right) { padding-left: 14px; }
  .edit {
    width: calc(100% + 8px);
    height: 20px;
    min-height: 0;
    margin: 2px -4px;
    padding: 0 3px;
    border: none;
    border-radius: 3px;
    background: var(--input-bg);
    color: var(--text);
    font-size: 12px;
    outline: 2px solid var(--accent);
  }
  .edit.right { text-align: right; }
</style>
