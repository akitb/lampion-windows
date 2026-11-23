<script lang="ts">
  // The database tree: a filter, "Databases" and a collapsible "System" section, tables loaded when a
  // database opens, and a footer with the connection and new-database button (port of DBSidebar.swift).
  import { ChevronRight, CircleEllipsis, Plus, Search } from "@lucide/svelte";
  import Glyph from "./Glyph.svelte";
  import { db, server } from "./dbstate.svelte";
  import { contextMenu, dropdown } from "./menu.svelte";
  import { databaseMenu, isSystem, newDatabase, reloadAll, select, setExpanded, tableMenu, ui } from "./model.svelte";
  import type { TableInfo } from "./types";
  import { middle } from "./util";

  let search = $state("");
  let list: HTMLDivElement | undefined = $state();
  let family = $state("sans-serif");

  const q = $derived(search.trim().toLowerCase());
  const matches = (name: string) => !q || name.toLowerCase().includes(q);
  const shown = $derived(server.databases.filter((d) => matches(d) || (server.tables[d] ?? []).some((t) => matches(t.name))));
  const user = $derived(shown.filter((d) => !isSystem(d)));
  const system = $derived(shown.filter(isSystem));
  const systemOpen = $derived(ui.showSystem || !!q);

  function tablesShown(d: string): TableInfo[] {
    const all = server.tables[d] ?? [];
    return matches(d) ? all : all.filter((t) => matches(t.name));
  }

  /** Every row in view, top to bottom: for the arrow keys. */
  const rows = $derived.by(() => {
    const out: { d: string; t: TableInfo | null }[] = [];
    for (const d of [...user, ...(systemOpen ? system : [])]) {
      out.push({ d, t: null });
      if (ui.expanded.includes(d)) for (const t of tablesShown(d)) out.push({ d, t });
    }
    return out;
  });

  /** The row Tab reaches: the selection, else (hidden in a closed database, the filter or System) its
   * database's row, else the first row, so the tree always keeps one tab stop. */
  const tabStop = $derived(rows.find((r) => isSelected(r.d, r.t?.name ?? null)) ?? rows.find((r) => !r.t && r.d === db.database) ?? rows[0]);
  const isStop = (d: string, t: string | null) => tabStop?.d === d && (tabStop.t?.name ?? null) === t;

  $effect(() => {
    if (list) family = getComputedStyle(list).fontFamily;
  });

  const connected = $derived(!ui.connectionError && !!server.version);
  const isSelected = (d: string, t: string | null) => db.database === d && db.table === t;
  /** The name shortened in the middle to fit (selected rows are bold, so wider). */
  const fit = (name: string, width: number, bold: boolean) => middle(name, width, `${bold ? 600 : 400} 13px ${family}`);

  function keys(e: KeyboardEvent) {
    const i = tabStop ? rows.indexOf(tabStop) : -1;
    const cur = rows[i];
    let next: (typeof rows)[number] | undefined;
    if (e.key === "ArrowDown") next = rows[Math.min(rows.length - 1, i + 1)];
    else if (e.key === "ArrowUp") next = rows[Math.max(0, i - 1)];
    else if (e.key === "ArrowRight" && cur && !cur.t) setExpanded(cur.d, true);
    else if (e.key === "ArrowLeft" && cur) {
      if (cur.t) next = { d: cur.d, t: null };
      else setExpanded(cur.d, false);
    } else if ((e.key === "ContextMenu" || (e.shiftKey && e.key === "F10")) && cur) {
      const el = list?.querySelector<HTMLElement>("[role=treeitem][tabindex='0']");
      const r = el?.getBoundingClientRect();
      if (r) {
        const ev = new MouseEvent("contextmenu", { clientX: r.left + 24, clientY: r.bottom });
        contextMenu(ev, cur.t ? tableMenu(cur.d, cur.t) : databaseMenu(cur.d));
      }
    } else return;
    e.preventDefault();
    if (next) {
      select(next.d, next.t?.name ?? null);
      queueMicrotask(() => list?.querySelector<HTMLElement>("[aria-selected='true']")?.focus());
    }
  }

  function sidebarMenu(e: MouseEvent) {
    dropdown(e.currentTarget as HTMLElement, [
      { label: "Show System Databases", checked: ui.showSystem, action: () => (ui.showSystem = !ui.showSystem) },
      { separator: true },
      { label: "Refresh", action: reloadAll },
    ], { keyboard: e.detail === 0 });
  }
</script>

{#snippet databaseRow(d: string)}
  {@const open = ui.expanded.includes(d)}
  {@const loaded = server.tables[d]}
  {@const sel = isSelected(d, null)}
  <div
    class="row db"
    class:sel
    role="treeitem"
    tabindex={isStop(d, null) ? 0 : -1}
    aria-selected={sel}
    aria-expanded={open}
    onclick={() => select(d)}
    ondblclick={() => setExpanded(d, !open)}
    onkeydown={keys}
    oncontextmenu={(e) => contextMenu(e, databaseMenu(d))}
  >
    <button type="button" class="disclosure" class:open tabindex="-1" aria-label={open ? `Close ${d}` : `Open ${d}`}
            onclick={(e) => { e.stopPropagation(); setExpanded(d, !open); }}>
      <ChevronRight size={11} strokeWidth={2.6} />
    </button>
    <span class="icon dbicon"><Glyph name="database" size={18} /></span>
    <span class="name" title={d}>{fit(d, loaded ? 106 : 125, sel)}</span>
    {#if loaded}<span class="count" title="{loaded.length} tables and views">{loaded.length}</span>{/if}
  </div>
  {#if open}
    <div role="group">
      {#each tablesShown(d) as t (t.name)}
        {@const tsel = isSelected(d, t.name)}
        <div
          class="row table"
          class:sel={tsel}
          role="treeitem"
          tabindex={isStop(d, t.name) ? 0 : -1}
          aria-selected={tsel}
          onclick={() => select(d, t.name)}
          onkeydown={keys}
          oncontextmenu={(e) => contextMenu(e, tableMenu(d, t))}
        >
          <span class="icon" class:view={t.view} class:tableicon={!t.view}><Glyph name={t.view ? "view" : "table"} size={19} /></span>
          <span class="name" title={t.name}>{fit(t.name, t.rowsShort ? 98 : 117, tsel)}</span>
          {#if t.rowsShort}<span class="count" title="About {t.rows?.toLocaleString()} rows">{t.rowsShort}</span>{/if}
        </div>
      {/each}
    </div>
  {/if}
{/snippet}

<nav class="sidebar" aria-label="Databases">
  <label class="search">
    <Search size={14} strokeWidth={2} />
    <input type="search" bind:value={search} placeholder="Filter" aria-label="Filter databases and tables" spellcheck="false" />
  </label>

  <div class="list" bind:this={list} role="tree" aria-label="Databases and tables">
    <div class="section">Databases</div>
    {#each user as d (d)}{@render databaseRow(d)}{/each}
    {#if system.length}
      <button type="button" class="section toggle" aria-expanded={systemOpen} onclick={() => (ui.showSystem = !ui.showSystem)}
              title={systemOpen ? "Hide MariaDB's own databases" : "Show MariaDB's own databases"}>
        System
        <span class="chev" class:open={systemOpen}><ChevronRight size={10} strokeWidth={2.6} /></span>
      </button>
      {#if systemOpen}
        {#each system as d (d)}{@render databaseRow(d)}{/each}
      {/if}
    {/if}
    {#if q && !user.length && !system.length}
      <div class="none">
        <Search size={26} strokeWidth={1.6} />
        <strong>No Results for “{search.trim()}”</strong>
        <span>Check the spelling or try a new search.</span>
      </div>
    {/if}
  </div>

  <footer>
    <span class="dot" class:ok={connected} class:bad={!!ui.connectionError}></span>
    <span class="version" title={ui.connectionError ?? "Connected to the local database server"}>{connected ? server.version : "Not connected"}</span>
    <span class="spacer"></span>
    <button type="button" class="foot" onclick={sidebarMenu} title="Sidebar options" aria-label="Sidebar options" aria-haspopup="menu"><CircleEllipsis size={15} strokeWidth={1.6} /></button>
    <button type="button" class="foot" onclick={newDatabase} title="New Database…" aria-label="New Database"><Plus size={16} strokeWidth={1.6} /></button>
  </footer>
</nav>

<style>
  .sidebar { display: flex; flex-direction: column; width: 200px; height: 100%; background: var(--bg-sidebar); border-right: 1px solid var(--db-sep); }
  .search {
    display: flex;
    align-items: center;
    gap: 5px;
    height: 28px;
    margin: 12px 10px 0;
    padding: 0 9px;
    border-radius: 14px;
    background: var(--db-search);
    color: var(--text-2);
    flex: none;
  }
  .search input {
    flex: 1;
    min-width: 0;
    min-height: 0;
    height: 24px;
    padding: 0;
    border: none;
    background: none;
    font-size: 13px;
    outline: none;
  }
  .search input::placeholder { color: var(--text-2); }
  .search input::-webkit-search-cancel-button { filter: grayscale(1); }
  .search:focus-within { box-shadow: 0 0 0 2px color-mix(in srgb, var(--accent) 70%, transparent); }
  .list { flex: 1; overflow-y: auto; padding: 0 10px 10px; }
  .section {
    display: flex;
    align-items: center;
    gap: 4px;
    height: 20px;
    margin-top: 9px;
    padding: 0 4px;
    font-size: 11px;
    font-weight: 600;
    color: var(--text-3);
  }
  .section.toggle {
    width: 100%;
    min-height: 0;
    margin-top: 13px;
    border: none;
    background: none;
    box-shadow: none;
    text-align: left;
  }
  .section.toggle:hover:not(:disabled) { background: none; color: var(--text-2); }
  .chev { display: inline-flex; opacity: 0; transition: transform var(--fade), opacity var(--fade); }
  .chev.open { transform: rotate(90deg); }
  .section.toggle:hover .chev, .section.toggle:focus-visible .chev { opacity: 1; }
  .row {
    --cut: var(--bg-sidebar);
    display: flex;
    align-items: center;
    height: 32px;
    padding: 0 6px 0 0;
    border-radius: 8px;
    font-size: 13px;
    white-space: nowrap;
    cursor: default;
  }
  .row:focus-visible { outline: 2px solid var(--accent); outline-offset: -2px; }
  .row.sel { --cut: var(--db-sidebar-sel); background: var(--db-sidebar-sel); font-weight: 600; }
  .disclosure {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    width: 16px;
    height: 24px;
    min-height: 0;
    padding: 0;
    border: none;
    background: none;
    box-shadow: none;
    color: var(--text-3);
  }
  .disclosure:hover:not(:disabled) { background: none; color: var(--text-2); }
  .row.sel .disclosure { color: var(--text-2); }
  .disclosure :global(svg) { transition: transform var(--fade); }
  .disclosure.open :global(svg) { transform: rotate(90deg); }
  .icon { display: inline-flex; align-items: center; justify-content: center; flex: none; }
  .dbicon { width: 14px; margin: 0 10px 0 5px; color: var(--db-tint); }
  .table .icon { width: 19px; margin: 0 8px 0 30px; }
  .tableicon { color: var(--db-table); }
  .view { color: var(--db-view); }
  .name { flex: 1; min-width: 0; overflow: hidden; }
  .count { margin-left: 4px; font-size: 11px; font-weight: 400; color: var(--text-3); font-variant-numeric: tabular-nums; }
  .row.sel .count { color: var(--text-2); }
  .none { display: flex; flex-direction: column; align-items: center; gap: 6px; padding: 40px 8px; color: var(--text-3); text-align: center; }
  .none strong { font-size: 13px; color: var(--text-2); }
  .none span { font-size: 11px; color: var(--text-2); }
  footer {
    display: flex;
    align-items: center;
    gap: 6px;
    height: 34px;
    flex: none;
    padding: 0 9px 0 12px;
    border-top: 1px solid var(--db-sep);
    background: color-mix(in srgb, var(--bg-sidebar) 60%, var(--bg));
  }
  .dot { width: 7px; height: 7px; border-radius: 50%; background: color-mix(in srgb, var(--text-3) 60%, transparent); flex: none; }
  .dot.ok { background: var(--green); box-shadow: 0 0 3px color-mix(in srgb, var(--green) 60%, transparent); }
  .dot.bad { background: var(--red); }
  .version { font-size: 11px; color: var(--text-2); overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .spacer { flex: 1; }
  .foot { display: inline-flex; align-items: center; justify-content: center; width: 22px; height: 22px; min-height: 0; padding: 0; border: none; background: none; box-shadow: none; color: var(--text-2); }
  .foot:hover:not(:disabled) { background: var(--hover); color: var(--text); }
</style>
