<script lang="ts">
  // The Tables tab of a database: every table and view with its size, sortable; double-click opens one
  // (port of DBOverview.swift).
  import { ChevronDown, ChevronUp, Database, Table } from "@lucide/svelte";
  import Glyph from "./Glyph.svelte";
  import { db, server } from "./dbstate.svelte";
  import Empty from "./Empty.svelte";
  import { contextMenu, type MenuItem } from "./menu.svelte";
  import { askDropTables, exportItem, importInto, newTable, select, tableMenu } from "./model.svelte";
  import type { TableInfo } from "./types";
  import { count } from "./util";

  type Key = "name" | "rows" | "bytes" | "engine" | "collation";
  let sortKey = $state<Key>("name");
  let asc = $state(true);
  let picked = $state<string[]>([]);
  let anchor = -1;
  let list: HTMLDivElement | undefined = $state();

  const d = $derived(db.database);
  const all = $derived(d ? server.tables[d] : undefined);
  const collator = new Intl.Collator(undefined, { numeric: true, sensitivity: "base" });
  const sorted = $derived.by(() => {
    const l = [...(all ?? [])];
    const by: Record<Key, (a: TableInfo, b: TableInfo) => number> = {
      name: (a, b) => collator.compare(a.name, b.name),
      rows: (a, b) => (a.rows ?? -1) - (b.rows ?? -1),
      bytes: (a, b) => a.bytes - b.bytes,
      engine: (a, b) => collator.compare(a.engine, b.engine),
      collation: (a, b) => collator.compare(a.collation, b.collation),
    };
    l.sort(by[sortKey]);
    if (!asc) l.reverse();
    return l;
  });
  const largest = $derived(Math.max(1, ...(all ?? []).map((t) => t.bytes)));

  // Another database: nothing picked.
  $effect(() => {
    void d;
    picked = [];
  });

  function sortBy(k: Key) {
    if (sortKey === k) asc = !asc;
    else {
      sortKey = k;
      asc = true;
    }
  }

  function press(e: PointerEvent, i: number) {
    if (e.button !== 0) return;
    list?.focus({ preventScroll: true });
    const name = sorted[i].name;
    if (e.ctrlKey || e.metaKey) picked = picked.includes(name) ? picked.filter((p) => p !== name) : [...picked, name];
    else if (e.shiftKey && anchor >= 0) {
      const [a, b] = anchor < i ? [anchor, i] : [i, anchor];
      picked = sorted.slice(a, b + 1).map((t) => t.name);
      return;
    } else picked = [name];
    anchor = i;
  }

  function open(t: TableInfo) {
    if (d) select(d, t.name);
  }

  function keys(e: KeyboardEvent) {
    if (!sorted.length) return;
    const i = sorted.findIndex((t) => t.name === picked[picked.length - 1]);
    if (e.key === "ArrowDown" || e.key === "ArrowUp") {
      const n = Math.max(0, Math.min(sorted.length - 1, i + (e.key === "ArrowDown" ? 1 : -1)));
      picked = [sorted[n].name];
      anchor = n;
      list?.querySelectorAll(".row")[n]?.scrollIntoView({ block: "nearest" });
      e.preventDefault();
    } else if (e.key === "Enter" && i >= 0) {
      open(sorted[i]);
    } else if (e.key === "ContextMenu" && i >= 0) {
      const r = list?.querySelectorAll(".row")[i]?.getBoundingClientRect();
      if (r) showMenu(new MouseEvent("contextmenu", { clientX: r.left + 40, clientY: r.bottom }), sorted[i]);
      e.preventDefault();
    }
  }

  function showMenu(e: MouseEvent, t: TableInfo | null) {
    if (!d) return;
    const names = t && !picked.includes(t.name) ? [t.name] : picked;
    if (t && !picked.includes(t.name)) picked = [t.name];
    const items = (all ?? []).filter((x) => names.includes(x.name));
    let menu: MenuItem[];
    if (items.length === 1) menu = tableMenu(d, items[0], true);
    else if (items.length > 1) menu = [{ label: `Delete ${items.length} Items…`, action: () => askDropTables(d, items.map((x) => ({ name: x.name, view: x.view }))) }];
    else
      menu = [
        { label: "New Table…", action: () => newTable(d) },
        { label: "Export Database…", action: () => exportItem(d) },
        { label: "Import SQL…", action: () => importInto(d) },
      ];
    contextMenu(e, menu);
  }

  const columns: { key: Key; title: string }[] = [
    { key: "name", title: "Name" },
    { key: "rows", title: "Rows" },
    { key: "bytes", title: "Size" },
    { key: "engine", title: "Engine" },
    { key: "collation", title: "Collation" },
  ];
</script>

{#if !d}
  <Empty icon={Database} title="No Database Selected" description="Choose a database in the sidebar." />
{:else if !all}
  <div class="loading"><span class="spinner" aria-label="Loading"></span></div>
{:else if !all.length}
  <Empty icon={Table} title="No Tables" description="“{d}” has no tables yet. Create one, or import an SQL file.">
    <button type="button" class="pushbtn prominent" onclick={() => newTable(d)}>New Table…</button>
    <button type="button" class="pushbtn" onclick={() => importInto(d)}>Import SQL…</button>
  </Empty>
{:else}
  <!-- svelte-ignore a11y_no_noninteractive_tabindex -->
  <div class="tables" bind:this={list} tabindex="0" role="grid" aria-label="Tables in {d}" aria-multiselectable="true"
       onkeydown={keys} oncontextmenu={(e) => showMenu(e, null)}>
    <div class="head" role="row">
      {#each columns as c (c.key)}
        <div class="hcell" role="columnheader" aria-sort={sortKey === c.key ? (asc ? "ascending" : "descending") : undefined}>
          <button type="button" tabindex="-1" onclick={() => sortBy(c.key)}>
            <span>{c.title}</span>
            {#if sortKey === c.key}{#if asc}<ChevronUp size={10} strokeWidth={2.6} />{:else}<ChevronDown size={10} strokeWidth={2.6} />{/if}{/if}
          </button>
        </div>
      {/each}
    </div>
    {#each sorted as t, i (t.name)}
      <div
        class="row"
        class:sel={picked.includes(t.name)}
        role="row"
        tabindex="-1"
        aria-selected={picked.includes(t.name)}
        onpointerdown={(e) => press(e, i)}
        onmousedown={(e) => e.preventDefault()}
        ondblclick={() => open(t)}
        oncontextmenu={(e) => showMenu(e, t)}
      >
        <div class="cell name" role="gridcell">
          <span class="icon" class:view={t.view}><Glyph name={t.view ? "view" : "table"} size={16} /></span>
          <span class="text">{t.name}</span>
        </div>
        <div class="cell num" role="gridcell">{#if t.rows !== null}{count(t.rows)}{:else}<span class="dash">—</span>{/if}</div>
        <div class="cell size" role="gridcell">
          <span class="bytes">{t.bytes > 0 ? t.size : "—"}</span>
          <span class="bar" aria-hidden="true"><span style="width:{t.bytes > 0 ? Math.max(3, 44 * Math.min(1, t.bytes / largest)) : 0}px"></span></span>
        </div>
        <div class="cell dim" class:faint={t.view} role="gridcell">{t.view ? "View" : t.engine}</div>
        <div class="cell dim" role="gridcell" title={t.collation}>{t.collation}</div>
      </div>
    {/each}
  </div>
{/if}

<style>
  .loading { display: grid; place-items: center; height: 100%; }
  .spinner { width: 16px; height: 16px; border: 2px solid var(--border-strong); border-top-color: var(--text-2); border-radius: 50%; animation: spin 0.8s linear infinite; }
  @keyframes spin { to { transform: rotate(360deg); } }
  .tables {
    --cols: minmax(140px, 28fr) minmax(60px, 15fr) minmax(110px, 19fr) minmax(60px, 15fr) minmax(100px, 23fr);
    height: 100%;
    overflow: auto;
    padding: 0 8px 8px;
    outline: none;
  }
  .tables:focus-visible { box-shadow: inset 0 0 0 2px color-mix(in srgb, var(--accent) 70%, transparent); }
  .head { position: sticky; top: 0; z-index: 1; display: grid; grid-template-columns: var(--cols); height: 28px; background: var(--db-content); border-bottom: 1px solid var(--db-sep); margin-bottom: 4px; }
  .hcell { position: relative; min-width: 0; }
  .hcell::before { content: ""; position: absolute; left: 0; top: 7px; height: 14px; width: 1px; background: var(--db-sep); }
  .hcell:first-child::before { left: 1px; }
  .hcell button {
    display: flex;
    align-items: center;
    gap: 6px;
    width: 100%;
    height: 27px;
    min-height: 0;
    padding: 0 10px;
    border: none;
    border-radius: 0;
    background: none;
    box-shadow: none;
    color: var(--text);
    font-size: 11.5px;
    font-weight: 600;
  }
  .hcell button span { flex: 1; text-align: left; }
  .hcell button :global(svg) { color: var(--text-2); }
  .row {
    position: relative;
    display: grid;
    grid-template-columns: var(--cols);
    height: 24px;
    align-items: center;
    border-radius: 5px;
  }
  .row::after { content: ""; position: absolute; left: 34px; right: 8px; bottom: -1px; height: 1px; background: var(--db-row-sep); }
  .row.sel { background: var(--db-sel); }
  .tables:focus .row.sel { background: var(--accent); color: var(--accent-text); }
  .tables:focus .row.sel .dim, .tables:focus .row.sel .bytes, .tables:focus .row.sel .dash { color: inherit; }
  .row.sel::after, .row:has(+ .row.sel)::after { opacity: 0; }
  .cell { min-width: 0; padding: 0 10px; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; font-size: 13px; }
  .name { display: flex; align-items: center; gap: 8px; padding-left: 8px; }
  .icon { display: inline-flex; flex: none; color: var(--db-blue); --cut: var(--db-content); }
  .icon.view { color: var(--db-purple); }
  .row.sel .icon { --cut: var(--db-sel); }
  .tables:focus .row.sel .icon { --cut: var(--accent); color: var(--accent-text); }
  .text { overflow: hidden; text-overflow: ellipsis; }
  .num { text-align: right; font-variant-numeric: tabular-nums; }
  .dash, .faint { color: var(--text-3); }
  .size { display: flex; align-items: center; gap: 8px; }
  .bytes { min-width: 52px; text-align: right; color: var(--text-2); font-variant-numeric: tabular-nums; }
  .bar { position: relative; width: 44px; height: 4px; border-radius: 2px; background: color-mix(in srgb, var(--text) 8%, transparent); flex: none; overflow: hidden; }
  .bar span { position: absolute; left: 0; top: 0; bottom: 0; border-radius: 2px; background: color-mix(in srgb, var(--db-tint) 75%, transparent); }
  .dim { color: var(--text-2); }
</style>
