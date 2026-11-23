<script lang="ts">
  // New Database: a checked name and a collation with a line about each (character set utf8mb4).
  import { slide } from "svelte/transition";
  import { Database, CircleAlert } from "@lucide/svelte";
  import { reducedMotion } from "../util";
  import { db, schemaChanged, showError, server } from "../dbstate.svelte";
  import { message } from "../dbapi.svelte";
  import { structure } from "./api";
  import SheetFrame from "./SheetFrame.svelte";
  import Popup from "$lib/config/Popup.svelte";
  import type { NameCheck, StructureChoices } from "$lib/types";

  let { choices, onclose }: { choices: StructureChoices; onclose: () => void } = $props();

  let name = $state("");
  // svelte-ignore state_referenced_locally
  let collation = $state(choices.defaultCollation);
  let check = $state<NameCheck>({ problem: null, plain: true });
  let busy = $state(false);
  let frame = $state<SheetFrame | undefined>();
  const motion = { duration: reducedMotion() ? 0 : 200 };

  const detail = $derived(choices.collations.find((c) => c.name === collation)?.detail ?? null);

  $effect(() => {
    const n = name;
    const existing = [...server.databases];
    let current = true;
    structure.checkName("Database", n, existing).then((r) => current && (check = r), () => {});
    return () => (current = false);
  });

  async function create() {
    busy = true;
    try {
      const created = await structure.createDatabase(name, collation);
      schemaChanged();
      // Select it, like clicking it in the sidebar.
      db.database = created;
      db.table = null;
      db.isView = false;
      if (db.tab !== "query" && db.tab !== "designer") db.tab = "tables";
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
  title="New Database"
  subtitle={server.version ? `On ${server.version}` : "On the local MariaDB server"}
  icon={Database}
  tint="var(--tint-mariadb)"
  top="var(--tint-mariadb-top)"
  confirm="Create"
  canConfirm={!!name.trim() && !check.problem && !!collation}
  {busy}
  onconfirm={create}
  {onclose}
  note="Character set utf8mb4: full Unicode, emoji included."
>
  <div class="section">
    <div class="group">
      <div class="row field">
        <label for="nd-name">Name</label>
        <input id="nd-name" class="text" bind:value={name} placeholder="my_project" spellcheck="false" autocomplete="off" aria-invalid={!!check.problem} data-autofocus />
      </div>
    </div>
    {#if check.problem}
      <p class="hint problem" role="alert" transition:slide={motion}><CircleAlert size={12} aria-hidden="true" />{check.problem}</p>
    {:else if name.trim() && !check.plain}
      <p class="hint" transition:slide={motion}>Works, but letters, digits and underscores are easier to use in SQL and PHP.</p>
    {/if}
  </div>

  <div class="section">
    <h3>Sorting and comparing</h3>
    <div class="group">
      <div class="row"><span class="label">Collation</span><Popup bind:value={collation} options={choices.collations.map((c) => ({ value: c.name, label: c.name }))} label="Collation" /></div>
      {#if detail}<div class="row detail">{detail}</div>{/if}
    </div>
    <p class="hint">For Hungarian text pick utf8mb4_hungarian_ci, so á, ő and ű sort in the right place. Otherwise the default is a good choice.</p>
  </div>
</SheetFrame>

<style>
  .detail { padding-top: 10px; padding-bottom: 10px; color: var(--text-2); font-size: 12px; line-height: 15px; }
</style>
