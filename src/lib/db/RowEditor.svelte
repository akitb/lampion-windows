<script lang="ts">
  // One row as a form: name, type, key badges, NULL switch and the value of every field; Save writes
  // only the changed fields in one UPDATE. Binary columns and views are read-only; a value the grid got
  // cut (64 KB) is read-only until its whole value is loaded, so a cut value is never written back.
  import { Lock } from "@lucide/svelte";
  import { message } from "./dbapi.svelte";
  import { db, showError } from "./dbstate.svelte";
  import KeyBadge, { badges } from "./KeyBadge.svelte";
  import { data, updateRow, wholeCell } from "./model.svelte";
  import TypeChip from "./TypeChip.svelte";

  let { row, original }: { row: number; original: (string | null)[] } = $props();

  const initial = () => [...original];
  /** The values Save and Revert compare with: as loaded, with whole values in place of cut ones. */
  let base = $state(initial());
  let draft = $state(initial());
  /** Columns of this row the grid got cut whose whole value isn't loaded yet. */
  let cutCols = $state((data.page.cut ?? []).filter(([r]) => r === row).map(([, c]) => c));
  let loading = $state(-1);
  const readOnly = $derived(!db.table || db.isView);
  const changed = $derived(draft.map((v, i) => v !== base[i]));
  const nChanged = $derived(changed.filter(Boolean).length);
  const keyText = $derived.by(() => {
    const pk = data.info.filter((c) => c.key === "PRI").map((c) => c.name);
    if (!pk.length) return `${db.table} · no primary key`;
    return pk.map((name) => `${name} = ${original[data.page.columns.indexOf(name)] ?? "NULL"}`).join(", ");
  });

  function isLong(i: number, type: string): boolean {
    const v = original[i] ?? "";
    return /text|^json/i.test(type) || v.includes("\n") || v.length > 60;
  }

  // A failed save keeps the rows, so this editor and its draft stay.
  function save() {
    updateRow(row, changed.flatMap((c, i) => (c ? [{ column: i, value: draft[i] }] : [])));
  }

  async function loadWhole(i: number) {
    loading = i;
    try {
      const v = await wholeCell(row, i);
      base[i] = v;
      draft[i] = v;
      cutCols = cutCols.filter((c) => c !== i);
    } catch (e) {
      showError(message(e));
    } finally {
      loading = -1;
    }
  }
</script>

<div class="editor">
  <div class="dbbar top head">
    <div class="who">
      <strong>Row {data.offset + row + 1}</strong>
      <span class="keys" title={keyText}>{keyText}</span>
    </div>
    {#if readOnly}
      <span class="ro" title={db.isView ? "Views can't be edited here." : "Read-only"}><Lock size={11} strokeWidth={2.2} /> Read-only</span>
    {/if}
  </div>

  <div class="fields">
    {#each data.page.columns as name, i (i)}
      {@const info = data.info.find((c) => c.name === name)}
      {@const cut = cutCols.includes(i)}
      {@const editable = !readOnly && !info?.binary && !cut}
      <div class="field">
        <div class="line">
          <span class="name" title={info?.comment ? `${name}\n${info.comment}` : name}>{name}</span>
          {#if changed[i]}<span class="dotmark" title="Changed"></span>{/if}
          <span class="spacer"></span>
          {#each info ? badges(info).filter((b) => b !== "notNull") : [] as kind (kind)}<KeyBadge {kind} />{/each}
        </div>
        <div class="line">
          {#if info}<TypeChip type={info.type} family={info.family} />{/if}
          <span class="spacer"></span>
          {#if editable && (info?.nullable ?? true)}
            <label class="nullbox" title="Store NULL (no value) in this field">
              <input type="checkbox" checked={draft[i] === null} onchange={(e) => (draft[i] = e.currentTarget.checked ? null : (original[i] ?? ""))} />
              NULL
            </label>
          {/if}
        </div>
        {#if draft[i] === null}
          <button type="button" class="box nullfield" class:changed={changed[i]} disabled={!editable}
                  title={editable ? "NULL. Click to enter a value." : "NULL"} onclick={() => (draft[i] = "")}>
            <span class="nullv">NULL</span>
          </button>
        {:else if !editable}
          <div class="box rovalue" class:changed={changed[i]} title={cut ? "Only the first 64 KB are shown." : info?.binary ? "Binary data can't be edited here." : "Read-only"}>{(draft[i] ?? "").slice(0, 4000)}</div>
          {#if cut}
            <button type="button" class="small whole" disabled={loading >= 0} onclick={() => loadWhole(i)}
                    title="Load the whole value{readOnly || info?.binary ? '' : ' to edit it'}">{loading === i ? "Loading…" : "Load Whole Value"}</button>
          {/if}
        {:else if isLong(i, info?.type ?? "")}
          <textarea class="box long" class:changed={changed[i]} bind:value={draft[i]} spellcheck="false" aria-label={name}></textarea>
        {:else}
          <input class="box" class:changed={changed[i]} class:right={info?.numeric} bind:value={draft[i]} placeholder="Empty" spellcheck="false" aria-label={name} />
        {/if}
      </div>
    {/each}
  </div>

  {#if !readOnly}
    <div class="dbbar bottom foot">
      <span class="count">{nChanged === 0 ? "No changes" : nChanged === 1 ? "1 field changed" : `${nChanged} fields changed`}</span>
      <span class="spacer"></span>
      <button type="button" class="small" disabled={!nChanged} onclick={() => (draft = [...base])} title="Discard your changes to this row">Revert</button>
      <button type="button" class="small prominent" disabled={!nChanged} onclick={save} title="Write the changed fields to the database">Save</button>
    </div>
  {/if}
</div>

<style>
  .editor { display: flex; flex-direction: column; height: 100%; }
  .head { justify-content: space-between; }
  .who { display: flex; flex-direction: column; min-width: 0; gap: 1px; }
  .who strong { font-size: 13px; font-weight: 600; line-height: 16px; }
  .keys { font-family: var(--mono); font-size: 11px; line-height: 13px; color: var(--text-2); overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .ro { display: inline-flex; align-items: center; gap: 4px; font-size: 11px; color: var(--text-2); white-space: nowrap; }
  .fields { flex: 1; overflow: auto; padding: 14px; display: flex; flex-direction: column; gap: 16px; }
  .field { display: flex; flex-direction: column; gap: 6px; }
  .line { display: flex; align-items: center; gap: 5px; min-width: 0; }
  .spacer { flex: 1; }
  .name { font-size: 12px; font-weight: 600; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .dotmark { width: 6px; height: 6px; border-radius: 50%; background: var(--accent); flex: none; }
  .nullbox { display: inline-flex; align-items: center; gap: 5px; font-size: 11px; color: var(--text); }
  .nullbox input { margin: 0; width: 13px; height: 13px; accent-color: var(--accent); }
  .box {
    width: 100%;
    min-height: 24px;
    padding: 0 8px;
    border: 1px solid color-mix(in srgb, var(--text) 10%, transparent);
    border-radius: 6px;
    background: var(--db-content);
    box-shadow: none;
    color: var(--text);
    font-family: var(--mono);
    font-size: 12px;
    text-align: left;
  }
  .box.changed { border-color: color-mix(in srgb, var(--accent) 70%, transparent); }
  input.box { height: 24px; }
  input.box.right { text-align: right; }
  input.box::placeholder { color: var(--text-3); }
  .nullfield { display: flex; align-items: center; height: 24px; }
  .nullfield:hover:not(:disabled) { background: var(--db-content); }
  .nullfield:disabled { opacity: 1; }
  .long { height: 96px; padding: 4px 8px; resize: vertical; line-height: 16px; }
  .rovalue { padding: 4px 8px; color: var(--text-2); line-height: 16px; white-space: pre-wrap; overflow-wrap: anywhere; max-height: 210px; overflow: auto; user-select: text; }
  .foot .count { font-size: 11px; color: var(--text-2); }
  .small {
    height: 20px;
    min-height: 0;
    padding: 0 9px;
    border: none;
    border-radius: 5px;
    background: var(--control);
    box-shadow: none;
    font-size: 12px;
  }
  .small.whole { align-self: flex-start; }
  .small.prominent { background: var(--accent); color: var(--accent-text); }
  .small.prominent:hover:not(:disabled) { background: var(--accent); filter: brightness(1.08); }
</style>
