<script lang="ts">
  // The New Row sheet: one field per column with its type, keys and what an empty field becomes
  // ("automatic", the default, NULL); required fields are marked. Port of InsertRowSheet (Sheets.swift).
  import { ChevronsUpDown, Info, SquarePlus, TriangleAlert } from "@lucide/svelte";
  import IconTile from "$lib/components/IconTile.svelte";
  import { db } from "./dbstate.svelte";
  import KeyBadge, { badges } from "./KeyBadge.svelte";
  import { data, insertRow, ui } from "./model.svelte";
  import TypeChip from "./TypeChip.svelte";
  import type { ColumnInfo } from "./types";

  let dialog: HTMLDialogElement | undefined = $state();
  let values = $state<Record<string, string>>({});
  let busy = $state(false);
  const titleId = $props.id();

  const missing = $derived(data.info.filter((c) => c.required && !(values[c.name] ?? "")).length);

  $effect(() => {
    if (dialog && !dialog.open) dialog.showModal();
  });

  function prompt(c: ColumnInfo): string {
    if (c.autoIncrement) return "automatic";
    if (c.default !== null) return c.default === "" ? "empty text" : c.default;
    if (c.nullable) return "NULL";
    return c.type.toLowerCase();
  }

  async function submit(e: SubmitEvent) {
    e.preventDefault();
    busy = true;
    const filled = Object.fromEntries(Object.entries(values).filter(([, v]) => v !== ""));
    const ok = await insertRow(filled);
    busy = false;
    if (ok) ui.insertRow = false;
  }
</script>

<dialog bind:this={dialog} class="sheet" aria-labelledby={titleId} onclose={() => (ui.insertRow = false)}>
  <form onsubmit={submit}>
    <header>
      <span class="tile"><IconTile icon={SquarePlus} tint="var(--green)" size={38} glyph={0.55} /></span>
      <div>
        <h1 id={titleId}>New Row</h1>
        <p>{[db.table, db.database ? `in ${db.database}` : ""].filter(Boolean).join(" ")}</p>
      </div>
    </header>

    <div class="form">
      <div class="group">
        {#each data.info as c, i (c.name)}
          <div class="row">
            <div class="label">
              <div class="name">
                <label for="f-{i}">{c.name}</label>
                {#if c.required}<span class="required" title="NOT NULL without a default value: fill this in">Required</span>{/if}
              </div>
              <div class="chips">
                <TypeChip type={c.type} family={c.family} />
                {#each badges(c).filter((b) => !(c.required && b === "notNull")) as kind (kind)}<KeyBadge {kind} />{/each}
              </div>
            </div>
            <div class="field">
              <input id="f-{i}" type="text" bind:value={values[c.name]} placeholder={prompt(c)} spellcheck="false" autocomplete="off" />
              {#if c.enumValues.length}
                <span class="pick" title="Choose one of the allowed values">
                  <ChevronsUpDown size={12} strokeWidth={2.2} />
                  <select aria-label="Allowed values of {c.name}" value="" onchange={(e) => { values[c.name] = e.currentTarget.value; e.currentTarget.value = ""; }}>
                    <option value="" disabled>Choose…</option>
                    {#each c.enumValues as v (v)}<option value={v}>{v}</option>{/each}
                  </select>
                </span>
              {/if}
            </div>
          </div>
        {/each}
      </div>
    </div>

    <footer>
      {#if missing > 0}
        <span class="note warn"><TriangleAlert size={14} strokeWidth={2.2} /> {missing} required field{missing === 1 ? " is" : "s are"} still empty.</span>
      {:else}
        <span class="note"><Info size={14} strokeWidth={2} /> Empty fields get their default value.</span>
      {/if}
      <span class="spacer"></span>
      <button type="button" class="pushbtn large" onclick={() => dialog?.close()}>Cancel</button>
      <button type="submit" class="pushbtn large prominent" disabled={busy}>Insert</button>
    </footer>
  </form>
</dialog>

<style>
  .sheet {
    width: 580px;
    max-width: calc(100vw - 40px);
    max-height: calc(100vh - 40px);
    padding: 0;
    border: 1px solid var(--border-strong);
    border-radius: 18px;
    background: var(--db-sheet);
    box-shadow: 0 24px 60px rgba(0, 0, 0, 0.45);
    color: var(--text);
    overflow: hidden;
  }
  .sheet::backdrop { background: rgba(0, 0, 0, 0.25); }
  form { display: flex; flex-direction: column; max-height: calc(100vh - 42px); }
  header { display: flex; align-items: center; gap: 12px; padding: 18px 20px 2px; }
  .tile { display: inline-flex; color: #fff; }
  h1 { margin: 0; font-size: 15px; font-weight: 600; }
  header p { margin: 2px 0 0; font-size: 12px; color: var(--text-2); }
  .form { overflow: auto; max-height: 540px; padding: 14px 20px 8px; }
  .group { border-radius: 10px; background: var(--db-group); padding: 0 14px; }
  .row { display: flex; align-items: flex-start; justify-content: space-between; gap: 12px; padding: 10px 0; }
  .row + .row { border-top: 1px solid var(--db-row-sep); }
  .label { display: flex; flex-direction: column; gap: 4px; min-width: 0; }
  .name { display: flex; align-items: baseline; gap: 5px; min-width: 0; }
  .name label { font-size: 13px; font-weight: 500; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .required { font-size: 10px; font-weight: 600; color: var(--red); }
  .chips { display: flex; flex-wrap: wrap; gap: 4px; }
  .field { position: relative; display: flex; align-items: center; width: 250px; flex: none; }
  .field input {
    width: 100%;
    height: 24px;
    min-height: 0;
    padding: 0 8px;
    border: 1px solid color-mix(in srgb, var(--text) 10%, transparent);
    border-radius: 7px;
    background: var(--db-field);
    font-family: var(--mono);
    font-size: 12px;
  }
  .field input::placeholder { color: var(--text-3); }
  .field input:focus { outline: 3px solid color-mix(in srgb, var(--accent) 55%, transparent); outline-offset: 0; }
  .pick { position: relative; display: inline-flex; align-items: center; justify-content: center; width: 22px; height: 22px; margin-left: 4px; color: var(--text-2); flex: none; }
  .pick select { position: absolute; inset: 0; opacity: 0; width: 100%; height: 100%; min-height: 0; padding: 0; cursor: default; }
  .pick:focus-within { outline: 2px solid var(--accent); border-radius: 5px; }
  footer { display: flex; align-items: center; gap: 10px; padding: 14px 20px; border-top: 1px solid var(--db-sep); }
  .note { display: inline-flex; align-items: center; gap: 6px; font-size: 12px; color: var(--text-2); }
  .note :global(svg) { color: var(--text-2); flex: none; }
  .note.warn :global(svg) { color: var(--orange); }
  .spacer { flex: 1; }
</style>
