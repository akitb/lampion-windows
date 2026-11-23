<script lang="ts">
  // The Data tab: a WHERE filter bar, the rows of the selected table, paging, and the row inspector.
  import { ArrowDown, ArrowUp, ChevronLeft, ChevronRight, CircleX, Inbox, ListFilter, PanelRight, Plus, RotateCw, Trash2, TriangleAlert, X } from "@lucide/svelte";
  import { fade } from "svelte/transition";
  import { db } from "./dbstate.svelte";
  import Empty from "./Empty.svelte";
  import Grid from "./Grid.svelte";
  import { applyFilter, askDeleteSelected, clearSort, data, loadData, PAGE_SIZE, sortBy, turnPage, ui, updateRow } from "./model.svelte";
  import RowInspector from "./RowInspector.svelte";
  import { count, reducedMotion } from "./util";

  const readOnly = $derived(db.isView);
  const n = $derived(data.page.rows.length);
  const range = $derived.by(() => {
    if (!n) return data.offset > 0 ? "No more rows" : "0 rows";
    const r = `${count(data.offset + 1)}–${count(data.offset + n)}`;
    if (data.total === null) return `${r} rows`;
    return `${r} of ${count(data.total)} ${data.total === 1 ? "row" : "rows"}`;
  });
  const scrollKey = $derived(`${db.database}\u0001${db.table}\u0001${data.offset}\u0001${data.orderBy}\u0001${data.ascending}`);
  const filtered = $derived(data.filter.trim() !== "");

  function clearFilter() {
    data.filter = "";
    applyFilter();
  }

  /** The inspector slides in from the right edge (only fades with reduced motion). */
  function slide(_: Element) {
    const still = reducedMotion();
    return { duration: still ? 120 : 260, css: (t: number) => (still ? `opacity:${t}` : `width:${t * 301}px;opacity:${Math.min(1, t * 1.5)}`) };
  }
</script>

<div class="data">
  <div class="main">
    <div class="dbbar top">
      <label class="filter" title="Filter rows with a WHERE condition. Press Enter to apply.">
        <span class="where">WHERE</span>
        <input
          type="text"
          bind:value={data.filter}
          placeholder="e.g. id > 10 AND name LIKE 'a%'"
          spellcheck="false"
          autocomplete="off"
          aria-label="WHERE filter"
          onkeydown={(e) => { if (e.key === "Enter") applyFilter(); }}
        />
        {#if data.filter}
          <button type="button" class="clear" onclick={clearFilter} title="Clear the filter" aria-label="Clear the filter"><CircleX size={14} strokeWidth={2} /></button>
        {/if}
      </label>
      <button type="button" class="iconbtn" onclick={() => loadData()} title="Reload rows" aria-label="Reload rows"><RotateCw size={17} strokeWidth={1.7} /></button>
      <span class="divider"></span>
      <button type="button" class="iconbtn" disabled={readOnly} onclick={() => (ui.insertRow = true)}
              title={readOnly ? "Views can't be edited" : "Add a new row"} aria-label="New row"><Plus size={18} strokeWidth={1.7} /></button>
      <button type="button" class="iconbtn" disabled={!data.selected.length || readOnly} onclick={askDeleteSelected}
              title={data.selected.length > 1 ? `Delete the ${data.selected.length} selected rows` : "Delete the selected row"} aria-label="Delete selected rows"><Trash2 size={17} strokeWidth={1.7} /></button>
      <span class="divider"></span>
      <button type="button" class="iconbtn" class:on={ui.showInspector} aria-pressed={ui.showInspector} onclick={() => (ui.showInspector = !ui.showInspector)}
              title={ui.showInspector ? "Hide row details" : "Show row details"} aria-label="Row details"><PanelRight size={18} strokeWidth={1.7} /></button>
    </div>

    <div class="content">
      {#if data.error}
        <Empty icon={TriangleAlert} title="Couldn't Load Rows" description={data.error} mono filled>
          {#if filtered}<button type="button" class="pushbtn" onclick={clearFilter}>Clear Filter</button>{/if}
          <button type="button" class="pushbtn" onclick={() => loadData()}>Try Again</button>
        </Empty>
      {:else}
        <div class="gridwrap" class:shown={data.page.columns.length > 0}>
          <Grid
            columns={data.page.columns}
            rows={data.page.rows}
            info={data.info}
            version={data.version}
            {scrollKey}
            editable={!readOnly}
            sortColumn={data.orderBy}
            ascending={data.ascending}
            bind:selected={data.selected}
            onedit={(r, c, v) => updateRow(r, [{ column: c, value: v }])}
            onsort={sortBy}
            table={db.table}
            label={db.table ?? "Rows"}
          />
        </div>
        {#if n === 0 && data.page.columns.length}
          <div class="nothing" transition:fade={{ duration: 200 }}>
            {#if filtered}<ListFilter size={26} strokeWidth={1.6} />{:else}<Inbox size={26} strokeWidth={1.6} />{/if}
            <span>{filtered ? "No rows match the filter" : "No rows yet"}</span>
          </div>
        {/if}
      {/if}
    </div>

    <div class="dbbar bottom">
      <span class="info" title="Double-click a cell to edit it. Right-click a cell to copy it or set it to NULL.">{range}</span>
      {#if data.selected.length}
        <span class="dot">·</span>
        <span class="info">{count(data.selected.length)} selected</span>
      {/if}
      {#if data.orderBy}
        <button type="button" class="sortchip" onclick={clearSort}
                title="Sorted by {data.orderBy}, {data.ascending ? 'ascending' : 'descending'}. Click to remove the sort.">
          {#if data.ascending}<ArrowUp size={10} strokeWidth={3} />{:else}<ArrowDown size={10} strokeWidth={3} />{/if}
          <span>{data.orderBy}</span>
          <X size={9} strokeWidth={3} />
        </button>
      {/if}
      <span class="spacer"></span>
      <button type="button" class="iconbtn" disabled={data.offset === 0} onclick={() => turnPage(-1)} title="Previous {PAGE_SIZE} rows" aria-label="Previous page"><ChevronLeft size={16} strokeWidth={2} /></button>
      <button type="button" class="iconbtn" disabled={data.offset + n >= (data.total ?? 0)} onclick={() => turnPage(1)} title="Next {PAGE_SIZE} rows" aria-label="Next page"><ChevronRight size={16} strokeWidth={2} /></button>
    </div>
  </div>

  {#if ui.showInspector}
    <aside class="inspector" transition:slide aria-label="Row details">
      <RowInspector />
    </aside>
  {/if}
</div>

<style>
  .data { display: flex; height: 100%; overflow: hidden; }
  .main { display: flex; flex-direction: column; flex: 1; min-width: 320px; }
  .filter {
    display: flex;
    align-items: center;
    gap: 6px;
    flex: 1;
    min-width: 0;
    height: 26px;
    padding: 0 7px 0 5px;
    border: 1px solid color-mix(in srgb, var(--text) 12%, transparent);
    border-radius: 7px;
    background: var(--db-content);
  }
  .filter:focus-within { border-color: color-mix(in srgb, var(--accent) 80%, transparent); box-shadow: 0 0 0 0.5px color-mix(in srgb, var(--accent) 80%, transparent); }
  .where {
    flex: none;
    padding: 1px 5px;
    border-radius: 4px;
    background: color-mix(in srgb, var(--db-tint) 14%, transparent);
    color: var(--db-tint);
    font-family: var(--mono);
    font-size: 9.5px;
    font-weight: 700;
    line-height: 14px;
  }
  .filter input {
    flex: 1;
    min-width: 0;
    min-height: 0;
    height: 22px;
    padding: 0;
    border: none;
    background: none;
    font-family: var(--mono);
    font-size: 12px;
    outline: none;
  }
  .filter input::placeholder { color: var(--text-3); }
  .clear { display: inline-flex; padding: 0; min-height: 0; border: none; background: none; box-shadow: none; color: var(--text-3); }
  .clear:hover:not(:disabled) { background: none; color: var(--text-2); }
  .content { position: relative; flex: 1; min-height: 0; }
  .gridwrap { height: 100%; opacity: 0; transition: opacity var(--fade); }
  .gridwrap.shown { opacity: 1; }
  .nothing {
    position: absolute;
    inset: 0;
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    gap: 6px;
    padding-top: 40px;
    color: var(--text-3);
    pointer-events: none;
  }
  .nothing span { font-size: 12px; color: var(--text-2); }
  .info { font-size: 12px; color: var(--text-2); font-variant-numeric: tabular-nums; white-space: nowrap; }
  .dot { color: var(--text-3); }
  .spacer { flex: 1; }
  .sortchip {
    display: inline-flex;
    align-items: center;
    gap: 4px;
    height: 20px;
    min-height: 0;
    padding: 0 7px;
    border: none;
    border-radius: 10px;
    background: color-mix(in srgb, var(--accent) 12%, transparent);
    box-shadow: none;
    color: var(--accent);
  }
  .sortchip span { font-family: var(--mono); font-size: 11px; font-weight: 500; max-width: 200px; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .sortchip :global(svg:last-child) { color: var(--text-2); }
  .inspector { flex: none; width: 301px; border-left: 1px solid var(--db-sep); overflow: hidden; }
</style>
