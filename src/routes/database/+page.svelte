<script lang="ts">
  // The Database Manager window (port of DatabaseWindow.swift): the database tree on the left; the
  // toolbar with the tabs; the selection's header and the chosen tab; the SQL console at the bottom.
  import "$lib/db/db.css";
  import { ChevronDown, FileDown, FileUp, Plus, RotateCw } from "@lucide/svelte";
  import { getCurrentWindow } from "@tauri-apps/api/window";
  import { untrack } from "svelte";
  import { fade } from "svelte/transition";
  import Dialog from "$lib/config/Dialog.svelte";
  import ConnectionError from "$lib/db/ConnectionError.svelte";
  import DataView from "$lib/db/DataView.svelte";
  import { activity } from "$lib/db/dbapi.svelte";
  import { db, dialogs, type DbTab } from "$lib/db/dbstate.svelte";
  import DesignerView from "$lib/db/DesignerView.svelte";
  import Header from "$lib/db/Header.svelte";
  import InsertRowSheet from "$lib/db/InsertRowSheet.svelte";
  import { dropdown } from "$lib/db/menu.svelte";
  import MenuHost from "$lib/db/MenuHost.svelte";
  import { availableTabs, exportItem, importInto, newDatabase, newTable, reloadAll, runQuery, savePref, selectionChanged, ui } from "$lib/db/model.svelte";
  import QueryView from "$lib/db/QueryView.svelte";
  import Sidebar from "$lib/db/Sidebar.svelte";
  import SqlConsole, { listenConsole } from "$lib/db/SqlConsole.svelte";
  import StructureSheets from "$lib/db/StructureSheets.svelte";
  import StructureView from "$lib/db/StructureView.svelte";
  import TablesView from "$lib/db/TablesView.svelte";
  import { reducedMotion } from "$lib/db/util";

  listenConsole();

  const names: Record<DbTab, string> = { tables: "Tables", designer: "Designer", data: "Data", structure: "Structure", query: "Query" };
  const tabs = $derived(availableTabs());
  const title = $derived(!db.database ? "Database Manager" : db.table ? `${db.database} › ${db.table}` : db.database);
  const fadeTime = reducedMotion() ? 120 : 200;
  let pulsing = $state(false);

  // Every selection change (sidebar, Tables tab, part B's openTable) picks the tab and loads.
  $effect(() => {
    void db.database;
    void db.table;
    untrack(selectionChanged);
  });

  // Connects on open, and reloads after every schema change (schemaChanged()).
  $effect(() => {
    void db.schemaVersion;
    untrack(reloadAll);
  });

  $effect(() => savePref("db.showSystemDatabases", ui.showSystem));
  $effect(() => savePref("db.showInspector", ui.showInspector));
  $effect(() => savePref("db.queryResultsOpen", ui.resultsOpen));

  $effect(() => {
    getCurrentWindow().setTitle(title).catch(() => {});
  });

  // The Refresh icon pulses only when work takes longer than a blink.
  $effect(() => {
    if (activity.busy > 0) {
      const t = setTimeout(() => (pulsing = true), 250);
      return () => clearTimeout(t);
    }
    pulsing = false;
  });

  const tabHelp = $derived(
    !db.database
      ? "Run SQL queries"
      : db.table
        ? "Data: browse and edit rows · Structure: columns, indexes and keys · Query: run SQL"
        : "Tables: the tables in this database · Designer: how they relate · Query: run SQL",
  );
  const exportHelp = $derived(
    db.table ? `Save the table “${db.table}” to an SQL file` : db.database ? `Save the database “${db.database}” to an SQL file` : "Select a database or table to export",
  );

  function newMenu(e: MouseEvent) {
    const d = db.database;
    dropdown(e.currentTarget as HTMLElement, [
      { label: "New Database…", action: newDatabase },
      { label: "New Table…", disabled: !d, action: () => d && newTable(d) },
    ], { alignRight: true, keyboard: e.detail === 0 });
  }

  function keys(e: KeyboardEvent) {
    if (e.defaultPrevented) return;
    const mod = e.ctrlKey || e.metaKey;
    const k = e.key.toLowerCase();
    if ((mod && e.shiftKey && k === "r") || e.key === "F5") {
      e.preventDefault();
      reloadAll();
    } else if (mod && (k === "r" || e.key === "Enter")) {
      // Never reload the page; Ctrl+R runs the query on the Query tab.
      e.preventDefault();
      if (db.tab === "query") runQuery();
    }
  }
</script>

<svelte:window onkeydown={keys} />

<div class="win">
  <Sidebar />

  <main class="detail">
    <header class="toolbar">
      <h1 title={title}>{title}</h1>
      <div class="picker" class:single={tabs.length === 1} role="tablist" aria-label="View" title={tabHelp}>
        {#each tabs as t (t)}
          <button type="button" role="tab" class:sel={db.tab === t} aria-selected={db.tab === t} disabled={!!ui.connectionError} onclick={() => (db.tab = t)}>{names[t]}</button>
        {/each}
      </div>
      <div class="actions">
        <button type="button" class="glass round" onclick={reloadAll} title={pulsing ? "Working…" : "Reload the list of databases and tables (Ctrl+Shift+R)"} aria-label="Refresh">
          <span class:pulsing><RotateCw size={18} strokeWidth={1.7} /></span>
        </button>
        <button type="button" class="glass newbtn" disabled={!!ui.connectionError} onclick={newMenu} aria-haspopup="menu"
                title="Create a database, or a table in the selected database" aria-label="New">
          <Plus size={19} strokeWidth={1.6} /><ChevronDown size={11} strokeWidth={2.6} />
        </button>
        <div class="glass pair">
          <button type="button" disabled={!db.database} onclick={() => db.database && exportItem(db.database, db.table)} title={exportHelp} aria-label="Export"><FileUp size={18} strokeWidth={1.6} /></button>
          <button type="button" disabled={!!ui.connectionError} onclick={() => importInto(db.database)}
                  title={db.database ? `Run an SQL file in “${db.database}”` : "Run an SQL file on the server"} aria-label="Import"><FileDown size={18} strokeWidth={1.6} /></button>
        </div>
      </div>
    </header>

    <div class="content">
      {#if ui.connectionError}
        <div class="layer" transition:fade={{ duration: fadeTime }}><ConnectionError /></div>
      {:else}
        <div class="layer" transition:fade={{ duration: fadeTime }}>
          {#if db.database && db.tab !== "query"}<Header />{/if}
          <div class="tabs">
            {#key db.tab}
              <div class="tab" transition:fade={{ duration: fadeTime }}>
                {#if db.tab === "tables"}
                  <TablesView />
                {:else if db.tab === "designer" && db.database}
                  <DesignerView database={db.database} />
                {:else if db.tab === "data"}
                  <DataView />
                {:else if db.tab === "structure" && db.database}
                  <StructureView database={db.database} table={db.table} />
                {:else}
                  <QueryView />
                {/if}
              </div>
            {/key}
          </div>
        </div>
      {/if}
    </div>

    <SqlConsole />
  </main>
</div>

<MenuHost />
<StructureSheets />
{#if ui.insertRow}<InsertRowSheet />{/if}

<Dialog
  bind:open={() => !!dialogs.confirm, (v) => { if (!v) dialogs.confirm = null; }}
  title={dialogs.confirm?.title ?? ""}
  message={dialogs.confirm?.message}
  confirm={dialogs.confirm?.button ?? "OK"}
  cancel="Cancel"
  destructive
  onconfirm={() => {
    const c = dialogs.confirm;
    dialogs.confirm = null;
    c?.action();
  }}
/>
<Dialog bind:open={() => !!dialogs.alert, (v) => { if (!v) dialogs.alert = null; }} title="Error" message={dialogs.alert ?? ""} confirm="OK" />

<style>
  .win { display: flex; height: 100vh; background: var(--db-content); }
  .detail { display: flex; flex-direction: column; flex: 1; min-width: 0; }
  .toolbar {
    position: relative;
    display: flex;
    align-items: center;
    height: 52px;
    flex: none;
    padding: 0 8px 0 21px;
    background: var(--db-toolbar);
    border-bottom: 1px solid color-mix(in srgb, var(--db-sep) 55%, var(--db-toolbar));
  }
  h1 {
    max-width: calc(50% - 150px);
    margin: 0;
    font-size: 15px;
    font-weight: 600;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .picker {
    position: absolute;
    left: 50%;
    transform: translateX(-50%);
    display: flex;
    height: 36px;
    padding: 4px;
    border-radius: 18px;
    background: var(--db-glass);
    box-shadow: inset 0 0 0 1px var(--db-glass-edge);
  }
  .picker button {
    height: 28px;
    min-height: 0;
    padding: 0 13px;
    border: none;
    border-radius: 14px;
    background: none;
    box-shadow: none;
    color: var(--text);
    font-size: 13px;
    white-space: nowrap;
    transition: background-color var(--fade);
  }
  .picker button:hover:not(:disabled):not(.sel) { background: color-mix(in srgb, var(--text) 5%, transparent); }
  .picker button.sel { background: var(--db-glass-sel); }
  .picker.single button { padding: 0 7px; }
  .actions { display: flex; align-items: center; gap: 8px; margin-left: auto; }
  .glass {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    height: 36px;
    min-height: 0;
    padding: 0;
    border: none;
    border-radius: 18px;
    background: var(--db-glass);
    box-shadow: inset 0 0 0 1px var(--db-glass-edge);
    color: var(--text);
  }
  button.glass:hover:not(:disabled) { background: color-mix(in srgb, var(--text) 6%, var(--db-glass)); }
  .round { width: 36px; }
  .round span { display: inline-flex; }
  .newbtn { width: 44px; gap: 1px; padding-left: 3px; }
  .pair { width: 74px; overflow: hidden; }
  .pair button {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    flex: 1;
    height: 36px;
    min-height: 0;
    padding: 0;
    border: none;
    border-radius: 0;
    background: none;
    box-shadow: none;
    color: var(--text);
  }
  .pair button:hover:not(:disabled) { background: color-mix(in srgb, var(--text) 6%, transparent); }
  .content { position: relative; flex: 1; min-height: 0; }
  .layer { position: absolute; inset: 0; display: flex; flex-direction: column; }
  .tabs { position: relative; flex: 1; min-height: 0; }
  .tab { position: absolute; inset: 0; display: flex; flex-direction: column; background: var(--db-content); }
  .tab > :global(*) { flex: 1; min-height: 0; }
</style>
