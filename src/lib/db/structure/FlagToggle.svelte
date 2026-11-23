<script lang="ts">
  // A small capsule that turns a column flag on and off (PK, AUTO, NULL), styled like a key badge.
  let { title, color, on, help, disabled = false, ontoggle }: { title: string; color: string; on: boolean; help: string; disabled?: boolean; ontoggle: (on: boolean) => void } = $props();
</script>

<button type="button" class="flag" class:on style:--c={color} aria-pressed={on} aria-label={help} title={help} {disabled} onclick={() => ontoggle(!on)}>{title}</button>

<style>
  .flag {
    flex: none;
    min-height: 0;
    height: auto;
    padding: 2.5px 6px;
    border: 1px solid color-mix(in srgb, var(--text) 15%, transparent);
    border-radius: 999px;
    background: transparent;
    box-shadow: none;
    color: var(--text-2);
    font-size: 9.5px;
    font-weight: 600;
    line-height: 12px;
    white-space: nowrap;
  }
  .flag.on { border-color: transparent; background: color-mix(in srgb, var(--c) 18%, transparent); color: var(--c); }
  .flag:hover:not(:disabled) { background: color-mix(in srgb, var(--text) 6%, transparent); }
  .flag.on:hover:not(:disabled) { background: color-mix(in srgb, var(--c) 24%, transparent); }
</style>
