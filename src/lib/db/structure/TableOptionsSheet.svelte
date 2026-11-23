<script lang="ts">
  // Rename or move the table, copy it (with or without its rows), change its engine or collation, and
  // run OPTIMIZE / ANALYZE / CHECK / REPAIR (the server's answer shows in a "Done" alert).
  import type { Component } from "svelte";
  import { SlidersHorizontal, WandSparkles, ChartColumn, ShieldCheck, Wrench } from "@lucide/svelte";
  import { db, openTable, schemaChanged, showError, server } from "../dbstate.svelte";
  import { message } from "../dbapi.svelte";
  import { structure } from "./api";
  import SheetFrame from "./SheetFrame.svelte";
  import Switch from "$lib/config/Switch.svelte";
  import Popup from "$lib/config/Popup.svelte";
  import Dialog from "$lib/config/Dialog.svelte";
  import type { StructureChoices, TableStructure } from "$lib/types";

  let { database, table, data, choices, onclose }: { database: string; table: string; data: TableStructure; choices: StructureChoices; onclose: () => void } = $props();

  // svelte-ignore state_referenced_locally
  let info = $state(data);
  // svelte-ignore state_referenced_locally
  let newName = $state(table);
  // svelte-ignore state_referenced_locally
  let newDatabase = $state(database);
  // svelte-ignore state_referenced_locally
  let copyName = $state(`${table}_copy`);
  let copyData = $state(true);
  // svelte-ignore state_referenced_locally
  let engine = $state(data.engine || "InnoDB");
  // svelte-ignore state_referenced_locally
  let collation = $state(data.collation || choices.defaultCollation);
  let busy = $state(false);
  /** The "Done" alert's message (copy, maintenance). */
  let done = $state("");
  let doneOpen = $state(false);
  let frame = $state<SheetFrame | undefined>();

  const subtitle = $derived(
    [`${table} in ${database}`, info.rows !== null ? `${info.rows} rows` : "", info.bytes > 0 ? info.size : ""].filter(Boolean).join(" · "),
  );
  const renamed = $derived(newName.trim() !== table || newDatabase !== database);
  const storageChanged = $derived(engine !== info.engine || collation !== info.collation);
  const withCurrent = (list: string[], current: string) => (current && !list.includes(current) ? [current, ...list] : list);
  const option = (v: string) => ({ value: v, label: v });
  const databases = $derived(withCurrent(server.databases, database).map(option));
  const engines = $derived(withCurrent(choices.engines, info.engine).map(option));
  const collations = $derived(withCurrent(choices.collations.map((c) => c.name), info.collation).map(option));
  const collationDetail = $derived(choices.collations.find((c) => c.name === collation)?.detail ?? "");

  const operations: { op: string; title: string; icon: Component<any>; help: string }[] = [
    { op: "OPTIMIZE", title: "Optimize", icon: WandSparkles, help: "Reclaims unused space and defragments the table" },
    { op: "ANALYZE", title: "Analyze", icon: ChartColumn, help: "Updates the statistics the server uses to pick indexes" },
    { op: "CHECK", title: "Check", icon: ShieldCheck, help: "Looks for errors in the table" },
    { op: "REPAIR", title: "Repair", icon: Wrench, help: "Fixes a damaged table (MyISAM and Aria)" },
  ];

  async function run(work: () => Promise<void>) {
    busy = true;
    try {
      await work();
    } catch (e) {
      showError(message(e));
    } finally {
      busy = false;
    }
  }

  const rename = () =>
    run(async () => {
      const name = await structure.renameTable(database, table, newDatabase, newName);
      schemaChanged();
      // Like selecting the table in the sidebar: Structure and Query stay, anything else shows the data.
      openTable(newDatabase, name, db.tab === "structure" || db.tab === "query" ? db.tab : "data");
      frame?.close();
    });

  const copyTable = () =>
    run(async () => {
      done = await structure.copyTable(database, table, copyName, copyData);
      doneOpen = true;
      schemaChanged();
    });

  const apply = () =>
    run(async () => {
      await structure.setOptions(database, table, engine === info.engine ? null : engine, collation === info.collation ? null : collation);
      schemaChanged();
      info = await structure.load(database, table);
      engine = info.engine || engine;
      collation = info.collation || collation;
    });

  const maintain = (op: string) =>
    run(async () => {
      done = await structure.maintenance(database, table, op);
      doneOpen = true;
    });
</script>

<SheetFrame
  bind:this={frame}
  title="Table Options"
  {subtitle}
  icon={SlidersHorizontal}
  tint="var(--st-gray)"
  top="var(--st-gray-top)"
  confirm="Done"
  onconfirm={() => frame?.close()}
  {onclose}
  width={560}
  showsCancel={false}
  maxFormHeight={660}
>
  <div class="section">
    <h3>Rename or move</h3>
    <div class="group">
      <div class="row field"><label for="to-name">Name</label><input id="to-name" class="text" bind:value={newName} spellcheck="false" autocomplete="off" data-autofocus /></div>
      <div class="row"><span class="label">Database</span><Popup bind:value={newDatabase} options={databases} label="Database" /></div>
    </div>
    <div class="action">
      {#if renamed && newName.trim()}<span class="caption" title="{newDatabase}.{newName.trim()}">Becomes {newDatabase}.{newName.trim()}</span>{/if}
      <button type="button" class="cfg-btn" disabled={busy || !newName.trim() || !renamed} onclick={rename}>Rename</button>
    </div>
  </div>

  <div class="section">
    <h3>Copy</h3>
    <div class="group">
      <div class="row field"><label for="to-copy">Name of the copy</label><input id="to-copy" class="text" bind:value={copyName} spellcheck="false" autocomplete="off" /></div>
      <div class="row"><label for="to-rows">Copy the rows too</label><Switch id="to-rows" bind:checked={copyData} /></div>
    </div>
    <div class="action">
      <span class="caption">{copyData ? "Structure, indexes and rows." : "Structure and indexes only."}</span>
      <button type="button" class="cfg-btn" disabled={busy || !copyName.trim()} onclick={copyTable}>Copy</button>
    </div>
  </div>

  <div class="section">
    <h3>Storage</h3>
    <div class="group">
      <div class="row"><span class="label">Engine</span><Popup bind:value={engine} options={engines} label="Engine" /></div>
      <div class="row"><span class="label">Collation</span><Popup bind:value={collation} options={collations} label="Collation" /></div>
    </div>
    <div class="action">
      <span class="caption">{storageChanged ? "Rewrites the table; large tables take a while." : collationDetail}</span>
      <button type="button" class="cfg-btn" disabled={busy || !storageChanged} onclick={apply}>Apply</button>
    </div>
  </div>

  <div class="section">
    <h3>Maintenance</h3>
    <div class="group tools">
      {#each operations as o (o.op)}
        <button type="button" class="cfg-btn tool" title={o.help} disabled={busy} onclick={() => maintain(o.op)}><o.icon size={13} aria-hidden="true" />{o.title}</button>
      {/each}
    </div>
  </div>
</SheetFrame>

<Dialog bind:open={doneOpen} title="Done" message={done} />

<style>
  .action { display: flex; align-items: center; gap: 8px; min-height: 23px; margin: 9.5px 10px 0; }
  .caption { flex: 1; min-width: 0; overflow: hidden; color: var(--text-2); font-size: 12px; line-height: 15px; text-overflow: ellipsis; white-space: nowrap; }
  .action .cfg-btn { margin-left: auto; }
  .tools { display: grid; grid-template-columns: repeat(4, 1fr); gap: 8px; padding: 10px; }
  .tool { display: inline-flex; justify-content: center; align-items: center; gap: 7px; height: 24px; min-width: 0; }
</style>
