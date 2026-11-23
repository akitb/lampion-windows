<script lang="ts">
  // The 56px strip above the content: what is selected and its key facts (port of DBHeader.swift).
  import { CaseSensitive, Engine, Eye, FileDown, FileUp, HardDrive, ListOrdered, Plus } from "@lucide/svelte";
  import Glyph from "./Glyph.svelte";
  import { db } from "./dbstate.svelte";
  import { exportItem, importInto, newTable, tableInfo, tablesOf, ui } from "./model.svelte";
  import { count } from "./util";

  const info = $derived(tableInfo(db.database, db.table));
  const summary = $derived.by(() => {
    const list = tablesOf(db.database);
    if (!db.database || !list) return "Loading…";
    const views = list.filter((t) => t.view).length;
    const tables = list.length - views;
    const parts = [tables === 1 ? "1 table" : `${tables} tables`];
    if (views) parts.push(views === 1 ? "1 view" : `${views} views`);
    const extra = ui.dbInfo[db.database];
    if (extra) {
      parts.push(extra.size);
      if (extra.collation) parts.push(extra.collation);
    }
    return parts.join(" · ");
  });
</script>

<header class="header">
  {#if db.database && db.table}
    {@const tint = db.isView ? "var(--db-purple)" : "var(--db-blue)"}
    <span class="tile" style="--cut:{tint};background:{tint}"><Glyph name={db.isView ? "view" : "table"} size={19} /></span>
    <div class="titles">
      <h2 title={db.table}>{db.table}</h2>
      <span>{db.database}</span>
    </div>
    <span class="spacer"></span>
    {#if info?.view}
      <span class="metachip" title="A saved query that behaves like a read-only table"><Eye size={12} strokeWidth={1.8} /> View</span>
    {:else if info}
      <div class="chips">
        {#if info.engine}<span class="metachip" title="Storage engine"><Engine size={13} strokeWidth={1.6} /> {info.engine}</span>{/if}
        {#if info.rows !== null}<span class="metachip" title="Approximate number of rows"><ListOrdered size={12} strokeWidth={1.8} /> ~{count(info.rows)} rows</span>{/if}
        <span class="metachip" title="Data and index size on disk"><HardDrive size={12} strokeWidth={1.8} /> {info.size}</span>
        {#if info.collation}<span class="metachip collation" title="Collation"><CaseSensitive size={14} strokeWidth={1.6} /> {info.collation}</span>{/if}
      </div>
    {/if}
  {:else if db.database}
    <span class="tile" style="--cut:var(--db-tint);background:var(--db-tint)"><Glyph name="database" size={19} /></span>
    <div class="titles">
      <h2 title={db.database}>{db.database}</h2>
      <span>{summary}</span>
    </div>
    <span class="spacer"></span>
    {#if db.tab === "tables"}
      {@const d = db.database}
      <div class="buttons">
        <button type="button" class="pushbtn" onclick={() => newTable(d)} title="Create a table in {d}"><Plus size={14} strokeWidth={1.8} /> New Table…</button>
        <button type="button" class="pushbtn" onclick={() => exportItem(d)} title="Save the whole database to an SQL file"><FileUp size={14} strokeWidth={1.8} /> Export Database…</button>
        <button type="button" class="pushbtn" onclick={() => importInto(d)} title="Run an SQL file in {d}"><FileDown size={14} strokeWidth={1.8} /> Import SQL…</button>
      </div>
    {/if}
  {/if}
</header>

<style>
  .header {
    display: flex;
    align-items: center;
    gap: 12px;
    height: 56px;
    flex: none;
    padding: 0 16px;
    border-bottom: 1px solid var(--db-sep);
    background: var(--db-content);
    container-type: inline-size;
  }
  .tile { display: inline-flex; align-items: center; justify-content: center; width: 32px; height: 32px; border-radius: 8px; color: #fff; flex: none; box-shadow: inset 0 1.5px 0 rgba(255, 255, 255, 0.18); }
  .titles { display: flex; flex-direction: column; gap: 1px; min-width: 0; }
  h2 { margin: 0; font-size: 15px; font-weight: 600; line-height: 19px; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; user-select: text; }
  .titles span { font-size: 12px; line-height: 15px; color: var(--text-2); overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .spacer { flex: 1; min-width: 12px; }
  .chips { display: flex; align-items: center; gap: 14px; flex: none; }
  .buttons { display: flex; gap: 10px; flex: none; }
  @container (max-width: 760px) { .collation { display: none; } }
  @container (max-width: 520px) { .chips { display: none; } }
</style>
