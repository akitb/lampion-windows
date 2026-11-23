<script lang="ts">
  // A titled group of rows, like a macOS grouped form section. Sizes: the macOS screenshots halved.
  import type { Snippet } from "svelte";
  let { title, actions, footer, children }: { title?: string; actions?: Snippet; footer?: string | Snippet; children: Snippet } = $props();
</script>

<section class="section">
  {#if title || actions}
    <div class="head">
      {#if title}<h2>{title}</h2>{/if}
      {#if actions}<div class="actions">{@render actions()}</div>{/if}
    </div>
  {/if}
  <div class="group">{@render children()}</div>
  {#if footer}
    <div class="footer">{#if typeof footer === "string"}{footer}{:else}{@render footer()}{/if}</div>
  {/if}
</section>

<style>
  .section { margin-top: 28px; }
  .head { display: flex; align-items: center; gap: 9px; min-height: 16px; margin: 0 10px 11px; }
  h2 { flex: 1; margin: 0; font-size: 13px; line-height: 16px; font-weight: 700; }
  .actions { display: flex; gap: 9px; }
  .group { background: var(--surface-3); border-radius: 10px; }
  .footer { margin: 7px 10px 0; font-size: 10px; line-height: 13px; color: var(--text-2); user-select: text; }
</style>
