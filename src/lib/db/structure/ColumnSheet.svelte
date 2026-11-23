<script lang="ts">
  // Add a column, or change one (name, comment, type and length, Unsigned, NULL, default, auto increment,
  // ON UPDATE CURRENT_TIMESTAMP, position). The footer shows the column's SQL as it will be sent.
  import { slide } from "svelte/transition";
  import { Columns3 } from "@lucide/svelte";
  import { reducedMotion } from "../util";
  import { schemaChanged, showError } from "../dbstate.svelte";
  import { message } from "../dbapi.svelte";
  import { structure } from "./api";
  import { family, isNumeric, isUnsigned, joinType, typeParts, withUnsigned, type Family } from "./family";
  import SheetFrame from "./SheetFrame.svelte";
  import TypeField from "./TypeField.svelte";
  import ChoiceButton from "./ChoiceButton.svelte";
  import Switch from "$lib/config/Switch.svelte";
  import Popup from "$lib/config/Popup.svelte";
  import type { ColumnDraft, StructureColumn, TableStructure } from "$lib/types";

  let { database, table, data, editing, onclose }: { database: string; table: string; data: TableStructure; editing: StructureColumn | null; onclose: () => void } = $props();

  /** A draft that, saved unchanged, keeps the column as it is. */
  function draftOf(c: StructureColumn | null): ColumnDraft {
    if (!c) return { name: "", type: "VARCHAR(255)", nullable: true, defaultText: "", autoIncrement: false, primaryKey: false, onUpdateTimestamp: false, comment: "" };
    const extra = c.extra.toLowerCase();
    return {
      name: c.name,
      type: c.type,
      nullable: c.nullable,
      // Exactly as the server keeps it: the text 'NULL' stays text, '007' stays quoted.
      defaultText: c.defaultText,
      autoIncrement: extra.includes("auto_increment"),
      primaryKey: c.key === "PRI",
      onUpdateTimestamp: extra.includes("on update"),
      comment: c.comment,
      // What the sheet doesn't show, kept as it is.
      collation: c.collation,
      invisible: c.invisible,
    };
  }

  // svelte-ignore state_referenced_locally
  let draft = $state<ColumnDraft>(draftOf(editing));
  let position = $state("unchanged");
  let busy = $state(false);
  let preview = $state<string | null>(null);
  let frame = $state<SheetFrame | undefined>();
  /** Rows that come and go slide (a short spring on macOS); none with reduced motion. */
  const motion = { duration: reducedMotion() ? 0 : 200 };

  const fam = $derived(family(draft.type));
  const parts = $derived(typeParts(draft.type));
  const unsigned = $derived(isUnsigned(parts));
  /** A computed column's expression isn't in the sheet: saving would turn it into a plain column. */
  const generated = $derived(!!editing?.generated);
  const canConfirm = $derived(!generated && !!draft.name.trim() && !!draft.type.trim());

  const hints: Partial<Record<Family, string>> = {
    text: "VARCHAR(n) and CHAR(n) hold up to n characters. TEXT needs no length.",
    decimal: "DECIMAL(p,s): p digits in all, s of them after the point.",
    enumSet: "The allowed values, quoted and separated by commas: 'small','large'.",
    integer: "The number in brackets is only a display width; it doesn't limit the value.",
    boolean: "The number in brackets is only a display width; it doesn't limit the value.",
    temporal: "DATETIME(3) and TIMESTAMP(6) keep fractions of a second.",
  };
  const lengthHint = $derived(hints[fam] ?? "Leave the length empty when the type doesn't need one.");

  const defaults = $derived([
    {
      items: [
        { value: "none", label: "No default" },
        { value: "NULL", label: "NULL" },
        { value: "''", label: "Empty text ('')" },
        ...(fam === "temporal" ? [{ value: "CURRENT_TIMESTAMP", label: "CURRENT_TIMESTAMP" }] : []),
        ...(isNumeric(fam) || fam === "boolean" ? [{ value: "0", label: "0" }] : []),
      ],
    },
  ]);

  const positions = $derived([
    { value: "unchanged", label: editing ? "Keep position" : "At the end" },
    { value: "first", label: "First" },
    ...data.columns.filter((c) => c.name !== editing?.name).map((c) => ({ value: "after:" + c.name, label: `After ${c.name}` })),
  ]);

  // The SQL preview comes from the same code that builds the statement.
  $effect(() => {
    const d = $state.snapshot(draft);
    let current = true;
    structure.columnSql(d).then((s) => current && (preview = s), () => current && (preview = null));
    return () => (current = false);
  });

  async function save() {
    busy = true;
    const where = position === "unchanged" ? { kind: "unchanged" as const } : position === "first" ? { kind: "first" as const } : { kind: "after" as const, column: position.slice(6) };
    try {
      await structure.saveColumn(database, table, $state.snapshot(draft), editing?.name ?? null, where);
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
  title={editing ? "Edit Column" : "Add Column"}
  subtitle={editing ? `${editing.name} in ${table}` : `To ${table}`}
  icon={Columns3}
  tint="var(--st-indigo)"
  top="var(--st-indigo-top)"
  confirm={editing ? "Save" : "Add Column"}
  {canConfirm}
  {busy}
  onconfirm={save}
  {onclose}
  width={540}
  note={generated ? "This column is computed (GENERATED ALWAYS AS …), which this sheet can't edit. Change it with ALTER TABLE … MODIFY in the Query tab." : canConfirm ? preview : null}
  noteIsCode={!generated}
  maxFormHeight={640}
>
  <div class="section">
    <div class="group">
      <div class="row field"><label for="col-name">Name</label><input id="col-name" class="text" bind:value={draft.name} placeholder="column_name" spellcheck="false" autocomplete="off" data-autofocus /></div>
      <div class="row field"><label for="col-comment">Comment</label><input id="col-comment" class="text prose" bind:value={draft.comment} placeholder="optional" autocomplete="off" /></div>
    </div>
  </div>

  <div class="section">
    <h3>Type &amp; length</h3>
    <div class="group">
      <div class="row field"><span class="label">Type</span><TypeField bind:type={draft.type} /></div>
      <div class="row field">
        <label for="col-length">Length / values</label>
        <input id="col-length" class="text" value={parts.args} oninput={(e) => (draft.type = joinType({ ...parts, args: e.currentTarget.value }))} placeholder="none" spellcheck="false" autocomplete="off" />
      </div>
      {#if fam === "integer" || unsigned}
        <div class="row" title="Only zero and positive numbers, with twice the upper range" transition:slide={motion}>
          <label for="col-unsigned">Unsigned</label>
          <Switch id="col-unsigned" checked={unsigned} onchange={(on) => (draft.type = joinType(withUnsigned(parts, on)))} />
        </div>
      {/if}
    </div>
    <p class="hint">{lengthHint}</p>
  </div>

  <div class="section">
    <h3>Null &amp; default</h3>
    <div class="group">
      <div class="row"><label for="col-null">Allow NULL</label><Switch id="col-null" bind:checked={draft.nullable} /></div>
      <div class="row field">
        <label for="col-default">Default</label>
        <span class="with-menu">
          <input id="col-default" class="text" bind:value={draft.defaultText} placeholder="none" disabled={draft.autoIncrement} spellcheck="false" autocomplete="off" />
          <ChoiceButton label="Common defaults" groups={defaults} disabled={draft.autoIncrement} onpick={(v) => (draft.defaultText = v === "none" ? "" : v)} />
        </span>
      </div>
      <div class="row"><label for="col-auto">Auto increment</label><Switch id="col-auto" bind:checked={draft.autoIncrement} /></div>
      {#if fam === "temporal" || draft.onUpdateTimestamp}
        <div class="row" title="ON UPDATE CURRENT_TIMESTAMP: set to the current time whenever the row changes" transition:slide={motion}>
          <label for="col-onupdate">Update on every change</label>
          <Switch id="col-onupdate" bind:checked={draft.onUpdateTimestamp} />
        </div>
      {/if}
    </div>
    <p class="hint">Empty means no default. NULL, numbers (except for text types), (expressions) and CURRENT_TIMESTAMP are used as written; other text is quoted.</p>
  </div>

  <div class="section">
    <div class="group">
      <div class="row"><span class="label" id="col-position">Position</span><Popup bind:value={position} options={positions} label="Position" /></div>
    </div>
  </div>
</SheetFrame>

<style>
  .with-menu { display: inline-flex; flex: none; align-items: center; gap: 4px; width: 260px; }
  .with-menu .text { flex: 1; width: auto; min-width: 0; }
</style>
