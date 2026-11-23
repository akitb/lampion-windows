<script lang="ts">
  // One row of a group: title with a line of explanation on the left, the control on the right.
  // A control too wide for the row wraps under the text, right-aligned, as on macOS.
  import type { Snippet } from "svelte";
  let { title, detail, for: forId, children }: { title?: string; detail?: string | Snippet; for?: string; children?: Snippet } = $props();
</script>

<div class="row cfg-row">
  <div class="label">
    {#if forId}<label class="title" for={forId}>{title}</label>{:else if title}<span class="title">{title}</span>{/if}
    {#if detail}
      <div class="detail">{#if typeof detail === "string"}{detail}{:else}{@render detail()}{/if}</div>
    {/if}
  </div>
  {#if children}<div class="control">{@render children()}</div>{/if}
</div>

<style>
  .row { position: relative; display: flex; flex-wrap: wrap; align-items: center; column-gap: 16px; row-gap: 6px; min-height: 36px; padding: 10px; }
  .label { flex: 1 1 260px; min-width: 0; }
  .title { display: block; font-size: 13px; line-height: 16px; }
  .detail { margin-top: 2px; font-size: 10px; line-height: 13px; color: var(--text-2); overflow-wrap: anywhere; user-select: text; white-space: pre-line; }
  .control { margin-left: auto; display: flex; align-items: center; gap: 8px; max-width: 100%; min-width: 0; }
</style>
