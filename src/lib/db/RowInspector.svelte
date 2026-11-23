<script lang="ts">
  // The panel beside the grid: every field of the selected row (port of RowInspector.swift).
  import { Layers, MousePointerClick } from "@lucide/svelte";
  import Empty from "./Empty.svelte";
  import { data } from "./model.svelte";
  import RowEditor from "./RowEditor.svelte";

  const row = $derived(data.selected.length === 1 ? data.selected[0] : -1);
  const values = $derived(row >= 0 ? data.page.rows[row] : undefined);
</script>

<div class="inspector">
  {#if data.selected.length > 1}
    <Empty icon={Layers} title="{data.selected.length} Rows Selected" description="Select a single row to see and edit its fields." />
  {:else if values}
    {#key `${data.version}-${row}`}
      <RowEditor {row} original={values} />
    {/key}
  {:else}
    <Empty icon={MousePointerClick} title="No Row Selected" description="Select a row to see all of its fields here." />
  {/if}
</div>

<style>
  .inspector { width: 300px; height: 100%; background: var(--bg); }
</style>
