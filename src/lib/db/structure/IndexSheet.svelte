<script lang="ts">
  // Add an index or the primary key: kind, name, and the columns in the order they are picked.
  import { Zap, Circle, CircleCheck } from "@lucide/svelte";
  import { schemaChanged, showError } from "../dbstate.svelte";
  import { message } from "../dbapi.svelte";
  import { structure } from "./api";
  import SheetFrame from "./SheetFrame.svelte";
  import TypeChip from "./TypeChip.svelte";
  import KeyBadge, { columnBadges } from "./KeyBadge.svelte";
  import Popup from "$lib/config/Popup.svelte";
  import type { IndexKind, TableStructure } from "$lib/types";

  let { database, table, data, onclose }: { database: string; table: string; data: TableStructure; onclose: () => void } = $props();

  /** An IndexKind (a string for the pop-up's binding). */
  let kind = $state<string>("index");
  let name = $state("");
  let chosen = $state<string[]>([]);
  let busy = $state(false);
  let frame = $state<SheetFrame | undefined>();

  const kinds = [
    { value: "index", label: "Index" },
    { value: "unique", label: "Unique" },
    { value: "primary", label: "Primary Key" },
    { value: "fulltext", label: "Fulltext" },
  ];
  const kindHint: Record<IndexKind, string> = {
    index: "Makes searching and sorting by these columns faster.",
    unique: "Faster searching, and no two rows may have the same values.",
    primary: "Identifies each row. A table has at most one, and its columns can't be NULL.",
    fulltext: "For word searches in text columns with MATCH … AGAINST.",
  };

  function toggle(column: string) {
    chosen = chosen.includes(column) ? chosen.filter((c) => c !== column) : [...chosen, column];
  }

  async function save() {
    busy = true;
    try {
      await structure.addIndex(database, table, kind as IndexKind, kind === "primary" ? "" : name, chosen);
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
  title="Add Index"
  subtitle="To {table}"
  icon={Zap}
  tint="var(--st-orange)"
  top="var(--st-orange-top)"
  confirm="Add Index"
  canConfirm={chosen.length > 0}
  {busy}
  onconfirm={save}
  {onclose}
  width={500}
  note={chosen.length ? null : "Choose at least one column."}
  maxFormHeight={560}
>
  <div class="section">
    <div class="group">
      <div class="row"><span class="label">Kind</span><Popup bind:value={kind} options={kinds} label="Kind" /></div>
      <div class="row field">
        <label for="ix-name">Name</label>
        <input id="ix-name" class="text" bind:value={name} placeholder={kind === "primary" ? "PRIMARY" : "automatic"} disabled={kind === "primary"} spellcheck="false" autocomplete="off" data-autofocus />
      </div>
    </div>
    <p class="hint">{kindHint[kind as IndexKind]}</p>
  </div>

  <div class="section">
    <h3>Columns</h3>
    <div class="group" role="group" aria-label="Columns of the index">
      {#each data.columns as c (c.name)}
        {@const order = chosen.indexOf(c.name) + 1}
        <button type="button" class="row pick" aria-pressed={order > 0} onclick={() => toggle(c.name)}>
          <span class="check" class:on={order > 0} aria-hidden="true">
            {#if order > 0}<CircleCheck size={16} strokeWidth={2.2} />{:else}<Circle size={16} strokeWidth={1.6} />{/if}
          </span>
          <span class="cname">{c.name}</span>
          <TypeChip type={c.type} />
          {#each columnBadges(c).filter((k) => k !== "notNull") as k (k)}<KeyBadge kind={k} />{/each}
          <span class="grow"></span>
          {#if order > 0}<span class="order" title="Position in the index">{order}</span>{/if}
        </button>
      {/each}
    </div>
    <p class="hint">The order you pick them in is the order of the index. Put the column you filter on most first.</p>
  </div>
</SheetFrame>

<style>
  .pick {
    justify-content: flex-start;
    gap: 8px;
    width: 100%;
    min-height: 38px;
    border: none;
    border-radius: 0;
    background: none;
    box-shadow: none;
    color: var(--text);
    text-align: left;
  }
  .pick:first-child { border-radius: 12px 12px 0 0; }
  .pick:last-child { border-radius: 0 0 12px 12px; }
  .pick:only-child { border-radius: 12px; }
  .pick:hover:not(:disabled) { background: none; }
  .pick:focus-visible { outline-offset: -2px; }
  .check { display: grid; flex: none; color: var(--text-2); }
  .check.on { color: var(--accent); }
  .cname { overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .grow { flex: 1; }
  .order {
    display: grid;
    flex: none;
    place-items: center;
    min-width: 18px;
    height: 18px;
    padding: 0 4px;
    border-radius: 9px;
    background: var(--accent);
    color: #fff;
    font-size: 10px;
    font-weight: 600;
    font-variant-numeric: tabular-nums;
  }
</style>
