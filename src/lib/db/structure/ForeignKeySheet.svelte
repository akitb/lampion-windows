<script lang="ts">
  // Link a column of this table to a column of another table in the same database.
  import { untrack } from "svelte";
  import { Link, ArrowRight } from "@lucide/svelte";
  import { schemaChanged, showError, server } from "../dbstate.svelte";
  import { dbapi, message } from "../dbapi.svelte";
  import { structure } from "./api";
  import SheetFrame from "./SheetFrame.svelte";
  import Popup from "$lib/config/Popup.svelte";
  import type { StructureChoices, TableStructure } from "$lib/types";

  let { database, table, data, choices, onclose }: { database: string; table: string; data: TableStructure; choices: StructureChoices; onclose: () => void } = $props();

  let name = $state("");
  let column = $state("");
  let refTable = $state("");
  let refColumn = $state("");
  let onDelete = $state("RESTRICT");
  let onUpdate = $state("RESTRICT");
  let refColumns = $state<string[]>([]);
  let loadedTables = $state<string[] | null>(null);
  let busy = $state(false);
  let frame = $state<SheetFrame | undefined>();

  // The sidebar's list when it has the database, else the database's own list.
  const tables = $derived(server.tables[database]?.filter((t) => !t.view).map((t) => t.name) ?? loadedTables ?? []);
  $effect(() => {
    if (server.tables[database] || loadedTables) return;
    untrack(() => dbapi.tables(database)).then((info) => (loadedTables = info.tables.filter((t) => !t.view).map((t) => t.name)), () => (loadedTables = []));
  });

  // A new table: its columns, the first one chosen.
  $effect(() => {
    const t = refTable;
    refColumns = [];
    refColumn = "";
    if (!t) return;
    let current = true;
    structure.columns(database, t).then(
      (list) => {
        if (!current) return;
        refColumns = list;
        refColumn = list[0] ?? "";
      },
      (e) => current && showError(message(e)),
    );
    return () => (current = false);
  });

  const choose = { value: "", label: "Choose…" };
  const columnOptions = $derived([choose, ...data.columns.map((c) => ({ value: c.name, label: `${c.name}   ${c.type}` }))]);
  const tableOptions = $derived([choose, ...tables.map((t) => ({ value: t, label: t }))]);
  const refOptions = $derived([choose, ...refColumns.map((c) => ({ value: c, label: c }))]);
  const rules = $derived(choices.rules.map((r) => ({ value: r, label: r })));

  async function save() {
    busy = true;
    try {
      await structure.addForeignKey(database, table, { name, column, refTable, refColumn, onDelete, onUpdate });
      schemaChanged();
      frame?.close();
    } catch (e) {
      showError(message(e));
    } finally {
      busy = false;
    }
  }
</script>

<SheetFrame
  bind:this={frame}
  title="Add Foreign Key"
  subtitle="From {table}"
  icon={Link}
  tint="var(--st-purple)"
  top="var(--st-purple-top)"
  confirm="Add Foreign Key"
  canConfirm={!!column && !!refTable && !!refColumn}
  {busy}
  onconfirm={save}
  {onclose}
  width={540}
  note="The columns need the same type, and the referenced one needs an index (a primary key works)."
>
  <div class="section">
    <div class="group">
      <div class="row relation" aria-label="{table}.{column || 'column'} refers to {refTable || 'table'}.{refColumn || 'column'}">
        <span class="end"><span>{table}</span><span class="dot">.</span><b class:empty={!column}>{column || "column"}</b></span>
        <span class="arrow" aria-hidden="true"><ArrowRight size={14} strokeWidth={2.6} /></span>
        <span class="end"><span class:empty={!refTable}>{refTable || "table"}</span><span class="dot">.</span><b class:empty={!refColumn}>{refColumn || "column"}</b></span>
        <span class="grow"></span>
      </div>
    </div>
  </div>

  <div class="section">
    <h3>This table</h3>
    <div class="group">
      <div class="row"><span class="label">Column</span><Popup bind:value={column} options={columnOptions} label="Column of this table" /></div>
    </div>
  </div>

  <div class="section">
    <h3>Refers to</h3>
    <div class="group">
      <div class="row"><span class="label">Table</span><Popup bind:value={refTable} options={tableOptions} label="Referenced table" /></div>
      <div class="row"><span class="label">Column</span><Popup bind:value={refColumn} options={refOptions} disabled={!refTable} label="Referenced column" /></div>
    </div>
  </div>

  <div class="section">
    <h3>When the referenced row changes</h3>
    <div class="group">
      <div class="row"><span class="label">On delete</span><Popup bind:value={onDelete} options={rules} label="On delete" /></div>
      <div class="row"><span class="label">On update</span><Popup bind:value={onUpdate} options={rules} label="On update" /></div>
      <div class="row field"><label for="fk-name">Name</label><input id="fk-name" class="text" bind:value={name} placeholder="automatic" spellcheck="false" autocomplete="off" data-autofocus /></div>
    </div>
    <p class="hint">RESTRICT refuses the change, CASCADE deletes or updates these rows too, SET NULL clears the column.</p>
  </div>
</SheetFrame>

<style>
  .relation { justify-content: flex-start; gap: 10px; font: 12px var(--mono); }
  .end { overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .end b { font-weight: 600; }
  .dot { color: var(--text-2); }
  .empty { color: var(--text-3); }
  .arrow { display: grid; color: var(--st-purple); }
  .grow { flex: 1; }
</style>
