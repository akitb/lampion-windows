<script lang="ts">
  // An empty or error state in the middle of a view (macOS ContentUnavailableView).
  import type { Component, Snippet } from "svelte";
  let {
    icon: Icon,
    title,
    description = "",
    mono = false,
    filled = false,
    children,
  }: { icon: Component<any>; title: string; description?: string; mono?: boolean; filled?: boolean; children?: Snippet } = $props();
</script>

<div class="empty" role="status">
  <span class="icon" class:filled><Icon size={36} strokeWidth={filled ? 2 : 1.5} /></span>
  <h2>{title}</h2>
  {#if description}<p class:mono>{description}</p>{/if}
  {#if children}<div class="actions">{@render children()}</div>{/if}
</div>

<style>
  .empty {
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    height: 100%;
    padding: 24px;
    color: var(--text-3);
    text-align: center;
  }
  .icon { display: inline-flex; }
  .icon.filled :global(svg) { fill: currentColor; stroke: var(--db-content); }
  h2 { margin: 16px 0 0; font-size: 15px; font-weight: 700; color: var(--text-2); }
  p {
    max-width: 480px;
    margin: 6px 0 0;
    font-size: 13px;
    line-height: 20px;
    color: var(--text-2);
    white-space: pre-line;
    user-select: text;
  }
  p.mono { font-family: var(--mono); font-size: 11px; line-height: 14px; overflow-wrap: anywhere; }
  .actions { display: flex; gap: 10px; margin-top: 18px; }
</style>
