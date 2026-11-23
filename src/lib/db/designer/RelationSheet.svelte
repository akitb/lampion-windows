<script lang="ts">
  // Confirms a relation drawn on the diagram (port of DesignerRelationSheet): both ends, the rules and the exact
  // SQL, then runs it. MariaDB's error (type mismatch, missing index, rows without a match…) shows here, so the
  // user can change something and try again.
  import { ArrowLeftRight, ArrowRight, Info, Link, OctagonX, TriangleAlert } from "@lucide/svelte";
  import { onMount } from "svelte";
  import { fade } from "svelte/transition";
  import IconTile from "$lib/components/IconTile.svelte";
  import Popup from "$lib/config/Popup.svelte";
  import type { DesignerColumn, DesignerNewRelation, DesignerPreview } from "$lib/types";
  import { message } from "../dbapi.svelte";
  import { compactType, explain, sqlTokens, typeFamily } from "./diagram";
  import { designerApi, type DesignerStore } from "./store.svelte";

  let { store, database, initial }: { store: DesignerStore; database: string; initial: DesignerNewRelation } = $props();

  const RULES = ["RESTRICT", "CASCADE", "SET NULL", "NO ACTION"].map((r) => ({ value: r, label: r }));
  const titleId = $props.id();

  let dialog: HTMLDialogElement | undefined = $state();
  let nameField: HTMLInputElement | undefined = $state();
  /** The relation as dragged; Swap overrides it. */
  let relation = $derived(initial);
  let name = $state("");
  let onDelete = $state("RESTRICT");
  let onUpdate = $state("RESTRICT");
  let preview = $state.raw<DesignerPreview | null>(null);
  let error = $state<string | null>(null);
  let running = $state(false);
  let asked = 0;

  const column = (table: string, col: string) => store.tables.get(table)?.columns.find((c) => c.name === col);
  const fk = $derived(column(relation.table, relation.column));
  const referenced = $derived(column(relation.refTable, relation.refColumn));
  const hint = $derived(error ? explain(error) : null);

  /** The SQL and warnings for the current choices; `suggest` also takes a fresh suggested name. */
  async function refresh(suggest: boolean) {
    const n = ++asked;
    try {
      const p = await designerApi.preview(database, relation, onDelete, onUpdate, suggest ? null : name);
      if (n !== asked) return;
      if (suggest) name = p.suggestedName;
      preview = p;
    } catch (e) {
      if (n === asked) error = message(e);
    }
  }

  onMount(() => {
    dialog?.showModal();
    nameField?.focus();
    refresh(true);
  });

  function swap() {
    relation = { table: relation.refTable, column: relation.refColumn, refTable: relation.table, refColumn: relation.column };
    error = null;
    refresh(true);
  }

  function rulesChanged() {
    error = null;
    refresh(false);
  }

  async function create(e: SubmitEvent) {
    e.preventDefault();
    if (running) return;
    running = true;
    error = null;
    try {
      const s = await designerApi.addRelation(database, relation, onDelete, onUpdate, name);
      store.applySchema(s);
      store.changed();
      dialog?.close();
    } catch (err) {
      error = message(err);
      running = false;
    }
  }
</script>

{#snippet end(table: string, col: string, info: DesignerColumn | undefined)}
  <span class="end">
    <span class="ref">{table}<span class="dot">.</span><b>{col}</b></span>
    {#if info}<span class="dz-chip fam-{typeFamily(info.type)}">{compactType(info.type)}</span>{/if}
  </span>
{/snippet}

<dialog
  bind:this={dialog}
  class="sheet"
  aria-labelledby={titleId}
  onclose={() => (store.pending = null)}
  oncancel={(e) => running && e.preventDefault()}
>
  <form onsubmit={create}>
    <header>
      <IconTile icon={Link} tint="var(--db-purple)" top="var(--dz-purple-top)" size={38} glyph={0.46} />
      <div class="titles">
        <h1 id={titleId}>New Relation</h1>
        <p>{database}</p>
      </div>
    </header>

    <div class="form">
      <section>
        <div class="group">
          <div class="row ends">
            {@render end(relation.table, relation.column, fk)}
            <span class="arrow"><ArrowRight size={14} strokeWidth={2.6} /></span>
            {@render end(relation.refTable, relation.refColumn, referenced)}
            <button type="button" class="dz-small swap" onclick={swap} disabled={store.tables.get(relation.refTable)?.isView !== false}
              title="Make “{relation.refTable}.{relation.refColumn}” the foreign key instead">
              <ArrowLeftRight size={12} strokeWidth={2.2} /> Swap
            </button>
          </div>
        </div>
        <p class="foot">“{relation.table}.{relation.column}” becomes a foreign key that refers to “{relation.refTable}.{relation.refColumn}”.</p>
      </section>

      {#if error}
        <section transition:fade={{ duration: 150 }}>
          <h2>MariaDB refused the relation</h2>
          <div class="group">
            <div class="row problem">
              <span class="stop"><OctagonX size={15} strokeWidth={2.2} /></span>
              <div>
                <p class="error">{error}</p>
                {#if hint}<p class="explain">{hint}</p>{/if}
              </div>
            </div>
          </div>
        </section>
      {/if}

      <section>
        <h2>When the referenced row changes</h2>
        <div class="group">
          <div class="row">
            <label for="{titleId}-delete">On delete</label>
            <Popup id="{titleId}-delete" bind:value={onDelete} options={RULES} onchange={rulesChanged} />
          </div>
          <div class="row">
            <label for="{titleId}-update">On update</label>
            <Popup id="{titleId}-update" bind:value={onUpdate} options={RULES} onchange={rulesChanged} />
          </div>
          <div class="row">
            <label for="{titleId}-name">Name</label>
            <input id="{titleId}-name" class="field" bind:this={nameField} bind:value={name} placeholder="automatic"
              spellcheck="false" autocomplete="off" oninput={() => refresh(false)} />
          </div>
        </div>
        <p class="foot">RESTRICT refuses the change, CASCADE deletes or updates these rows too, SET NULL clears the column.</p>
      </section>

      <section>
        <h2>SQL</h2>
        <div class="group">
          <pre class="sql">{#each sqlTokens(preview?.sql ?? "") as t, i (i)}<span class="t-{t.kind}">{t.text}</span>{/each}</pre>
        </div>
      </section>
    </div>

    <footer>
      <span class="notes">
        {#if preview?.warnings.length}
          <span class="note warn" transition:fade={{ duration: 150 }}><TriangleAlert size={14} strokeWidth={2.2} /> {preview.warnings[0]}</span>
        {:else}
          <span class="note" transition:fade={{ duration: 150 }}><Info size={14} strokeWidth={2} /> Every value in “{relation.column}” must then exist in “{relation.refTable}.{relation.refColumn}”.</span>
        {/if}
      </span>
      <button type="button" class="pushbtn large" onclick={() => dialog?.close()}>Cancel</button>
      <button type="submit" class="pushbtn large prominent" disabled={running}>{running ? "Creating…" : "Create Relation"}</button>
    </footer>
  </form>
</dialog>

<style>
  .sheet {
    width: 560px;
    max-width: calc(100vw - 40px);
    max-height: calc(100vh - 40px);
    padding: 0;
    border: 1px solid var(--border-strong);
    border-radius: 18px;
    background: var(--db-sheet);
    box-shadow: 0 20px 50px rgba(0, 0, 0, 0.45);
    color: var(--text);
  }
  .sheet::backdrop { background: rgba(0, 0, 0, 0.25); }
  form { display: flex; flex-direction: column; max-height: calc(100vh - 42px); }
  header { display: flex; align-items: center; gap: 12px; padding: 18px 20px 2px; }
  .titles { min-width: 0; }
  h1 { margin: 0; font-size: 15px; font-weight: 600; }
  header p { margin: 2px 0 0; overflow: hidden; font-size: 12px; color: var(--text-2); text-overflow: ellipsis; white-space: nowrap; }

  .form { overflow: auto; max-height: 520px; padding: 20px 20px 8px; }
  section + section { margin-top: 26px; }
  h2 { margin: 0 10px 10px; font-size: 13px; font-weight: 700; line-height: 16px; }
  .group { border-radius: 10px; background: var(--db-group); }
  .row { display: flex; align-items: center; justify-content: space-between; gap: 12px; min-height: 37px; padding: 0 10px; font-size: 13px; }
  .row + .row { border-top: 1px solid var(--db-row-sep); }
  .foot { margin: 10px 10px 0; font-size: 12px; line-height: 15px; color: var(--text-2); user-select: text; }

  .ends { justify-content: flex-start; gap: 10px; min-height: 39px; }
  .end { display: inline-flex; align-items: center; gap: 6px; min-width: 0; }
  .ref { overflow: hidden; font-family: var(--mono); font-size: 12px; text-overflow: ellipsis; white-space: nowrap; }
  .ref b { font-weight: 600; }
  .dot { color: var(--text-2); }
  .arrow { display: inline-flex; flex: none; color: var(--db-purple); }
  .ends .swap { margin-left: auto; padding: 0 9px; gap: 5px; }

  .problem { align-items: flex-start; justify-content: flex-start; gap: 8px; padding: 10px; }
  .stop { display: inline-flex; flex: none; margin-top: 1px; color: var(--red); }
  .error, .explain { margin: 0; font-size: 12px; line-height: 16px; overflow-wrap: anywhere; user-select: text; }
  .explain { margin-top: 6px; color: var(--text-2); }

  .field {
    width: 260px;
    height: 24px;
    min-height: 0;
    padding: 0 8px;
    border: 1px solid color-mix(in srgb, var(--text) 10%, transparent);
    border-radius: 7px;
    background: var(--db-field);
    font-family: var(--mono);
    font-size: 12px;
  }
  .field::placeholder { color: var(--text-3); }
  .field:focus { outline: 3px solid color-mix(in srgb, var(--accent) 55%, transparent); outline-offset: 0; }

  .sql {
    margin: 0;
    padding: 9px 10px;
    overflow-x: auto;
    font-family: var(--mono);
    font-size: 12px;
    line-height: 16px;
    scrollbar-width: none;
    user-select: text;
  }
  .t-keyword { color: var(--db-purple); }
  .t-name { color: var(--db-orange); }

  footer { display: flex; align-items: center; gap: 10px; padding: 14px 20px; border-top: 1px solid var(--db-sep); }
  /* The note and the warning swap with a fade in the same place. */
  .notes { display: grid; flex: 1; min-width: 0; }
  .note { grid-area: 1 / 1; display: inline-flex; align-items: flex-start; gap: 6px; font-size: 12px; line-height: 15px; color: var(--text-2); }
  .note :global(svg) { flex: none; margin-top: 0.5px; color: var(--text-2); }
  .note.warn :global(svg) { color: var(--orange); }
</style>
