<script lang="ts">
  // New Table: name, engine, collation and all the columns at once (name, type, default, PK / AUTO / NULL).
  import { slide } from "svelte/transition";
  import { Table, Plus, Minus, CircleAlert } from "@lucide/svelte";
  import { reducedMotion } from "../util";
  import { openTable, schemaChanged, showError, server } from "../dbstate.svelte";
  import { message } from "../dbapi.svelte";
  import { structure } from "./api";
  import SheetFrame from "./SheetFrame.svelte";
  import TypeField from "./TypeField.svelte";
  import FlagToggle from "./FlagToggle.svelte";
  import Popup from "$lib/config/Popup.svelte";
  import type { ColumnDraft, StructureChoices } from "$lib/types";

  let { database, choices, onclose }: { database: string; choices: StructureChoices; onclose: () => void } = $props();

  const blank = (over: Partial<ColumnDraft> = {}): ColumnDraft => ({
    name: "",
    type: "VARCHAR(255)",
    nullable: true,
    defaultText: "",
    autoIncrement: false,
    primaryKey: false,
    onUpdateTimestamp: false,
    comment: "",
    ...over,
  });
  let next = 0;
  let rows = $state([
    { id: next++, c: blank({ name: "id", type: "INT UNSIGNED", nullable: false, autoIncrement: true, primaryKey: true }) },
    { id: next++, c: blank() },
  ]);
  let name = $state("");
  let engine = $state("InnoDB");
  // svelte-ignore state_referenced_locally
  let collation = $state(choices.defaultCollation);
  let problem = $state<string | null>(null);
  let busy = $state(false);
  let frame = $state<SheetFrame | undefined>();
  const motion = { duration: reducedMotion() ? 0 : 200 };

  const usable = $derived(rows.map((r) => r.c).filter((c) => c.name.trim() && c.type.trim()));
  const duplicate = $derived.by(() => {
    const names = usable.map((c) => c.name.trim().toLowerCase());
    return names.find((n, i) => names.indexOf(n) !== i) ?? null;
  });
  const option = (v: string) => ({ value: v, label: v });

  $effect(() => {
    const n = name;
    const existing = server.tables[database]?.map((t) => t.name) ?? [];
    let current = true;
    structure.checkName("Table", n, existing).then((r) => current && (problem = r.problem), () => {});
    return () => (current = false);
  });

  function flag(c: ColumnDraft, key: "primaryKey" | "autoIncrement" | "nullable", on: boolean) {
    c[key] = on;
    // A key column can't be NULL.
    if (key === "primaryKey" && on) c.nullable = false;
  }

  async function create() {
    busy = true;
    try {
      const created = await structure.createTable(database, name, $state.snapshot(usable), engine, collation);
      schemaChanged();
      openTable(database, created, "structure");
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
  title="New Table"
  subtitle="In {database}"
  icon={Table}
  tint="var(--st-blue)"
  top="var(--st-blue-top)"
  confirm="Create Table"
  canConfirm={!!name.trim() && !problem && usable.length > 0 && !duplicate && !!collation}
  {busy}
  onconfirm={create}
  {onclose}
  width={700}
  note="Rows without a name are ignored. Add indexes and foreign keys on the Structure tab afterwards."
  warning={duplicate ? `Two columns are named “${duplicate}”.` : null}
  maxFormHeight={580}
>
  <div class="section">
    <div class="group">
      <div class="row field">
        <label for="nt-name">Name</label>
        <input id="nt-name" class="text" bind:value={name} placeholder="table_name" spellcheck="false" autocomplete="off" aria-invalid={!!problem} data-autofocus />
      </div>
      <div class="row"><span class="label">Engine</span><Popup bind:value={engine} options={choices.engines.map(option)} label="Engine" /></div>
      <div class="row"><span class="label">Collation</span><Popup bind:value={collation} options={choices.collations.map((c) => option(c.name))} label="Collation" /></div>
    </div>
    {#if problem}<p class="hint problem" role="alert"><CircleAlert size={12} aria-hidden="true" />{problem}</p>{/if}
  </div>

  <div class="section">
    <h3>Columns</h3>
    <div class="group">
      <div class="row designer heading" aria-hidden="true"><span>Name</span><span class="typehead">Type</span><span>Default</span><span>Flags</span></div>
      {#each rows as r (r.id)}
        <div class="row field designer" role="group" aria-label="Column {r.c.name || 'without a name'}" transition:slide={motion}>
          <input class="text" bind:value={r.c.name} placeholder="name" aria-label="Name" spellcheck="false" autocomplete="off" />
          <TypeField bind:type={r.c.type} compact />
          <input class="text" bind:value={r.c.defaultText} placeholder="none" aria-label="Default" disabled={r.c.autoIncrement} spellcheck="false" autocomplete="off" />
          <span class="flags">
            <FlagToggle title="PK" color="var(--st-yellow)" on={r.c.primaryKey} help="Primary key: identifies each row" ontoggle={(on) => flag(r.c, "primaryKey", on)} />
            <FlagToggle title="AUTO" color="var(--st-teal)" on={r.c.autoIncrement} help="Auto increment: numbered automatically" ontoggle={(on) => flag(r.c, "autoIncrement", on)} />
            <FlagToggle title="NULL" color="var(--st-gray)" on={r.c.nullable} help="Allow NULL: the value may be missing" disabled={r.c.primaryKey} ontoggle={(on) => flag(r.c, "nullable", on)} />
          </span>
          <button type="button" class="round remove" title="Remove this column" aria-label="Remove this column" disabled={rows.length < 2} onclick={() => (rows = rows.filter((x) => x.id !== r.id))}>
            <Minus size={10} strokeWidth={3.4} aria-hidden="true" />
          </button>
        </div>
      {/each}
      <div class="row">
        <button type="button" class="add" onclick={() => rows.push({ id: next++, c: blank() })}>
          <span class="round" aria-hidden="true"><Plus size={10} strokeWidth={3.4} /></span>Add Column
        </button>
        <span class="count">{usable.length} column{usable.length === 1 ? "" : "s"}</span>
      </div>
    </div>
  </div>
</SheetFrame>

<style>
  .designer { display: grid; grid-template-columns: 140px 190px 96px 1fr 16px; justify-content: stretch; gap: 8px; }
  /* Measured: 43 px rows, 25 px fields with 9 px above and below. */
  .designer.field { min-height: 43px; padding-top: 9px; padding-bottom: 9px; }
  .designer .text { width: 100%; height: 25px; }
  .heading { min-height: 36px; color: var(--text-2); font-size: 10px; font-weight: 500; }
  .typehead { padding-left: 24px; }
  .flags { display: flex; gap: 4px; }
  .round {
    display: grid;
    flex: none;
    place-items: center;
    width: 16px;
    height: 16px;
    min-height: 0;
    padding: 0;
    border: none;
    border-radius: 50%;
    background: color-mix(in srgb, var(--text-2) 75%, transparent);
    box-shadow: none;
    color: var(--bg);
  }
  .remove { background: color-mix(in srgb, var(--red) 80%, transparent); }
  .remove:hover:not(:disabled) { background: var(--red); }
  .remove:disabled { background: color-mix(in srgb, var(--text-2) 75%, transparent); opacity: 0.45; }
  .add {
    display: inline-flex;
    align-items: center;
    gap: 7px;
    min-height: 0;
    padding: 0;
    border: none;
    background: none;
    box-shadow: none;
    font-size: 13px;
  }
  .add:hover:not(:disabled) { background: none; }
  .count { color: var(--text-2); font-size: 12px; }
</style>
