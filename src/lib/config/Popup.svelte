<script lang="ts">
  // The macOS pop-up button of a form row: the chosen text and a round ⌃⌄ badge. A native <select>
  // underneath, so the keyboard and screen readers work as usual.
  import { ChevronsUpDown } from "@lucide/svelte";
  type Option = { value: string; label: string };
  let {
    value = $bindable(""),
    options,
    disabled = false,
    id,
    label,
    onchange,
  }: { value?: string; options: Option[]; disabled?: boolean; id?: string; label?: string; onchange?: (v: string) => void } = $props();
</script>

<span class="popup" class:disabled>
  <select {id} bind:value {disabled} aria-label={label} onchange={() => onchange?.(value)}>
    {#each options as o (o.value)}
      <option value={o.value}>{o.label}</option>
    {/each}
  </select>
  <span class="badge" aria-hidden="true"><ChevronsUpDown size={10} strokeWidth={2.4} /></span>
</span>

<style>
  .popup { position: relative; display: inline-flex; align-items: center; min-width: 0; }
  select {
    appearance: none;
    field-sizing: content;
    max-width: 100%;
    height: 23px;
    min-height: 0;
    padding: 0 36px 0 4px;
    border: none;
    background: transparent;
    box-shadow: none;
    color: var(--text);
    font-size: 13px;
    text-overflow: ellipsis;
  }
  select:focus-visible { outline: 2px solid var(--accent); outline-offset: 0; border-radius: 6px; }
  option { background: var(--surface-3); color: var(--text); }
  .badge {
    position: absolute;
    right: 5px;
    display: grid;
    place-items: center;
    width: 17px;
    height: 17px;
    border-radius: 50%;
    background: color-mix(in srgb, var(--text) 8%, transparent);
    pointer-events: none;
  }
  .disabled { opacity: 0.45; }
</style>
