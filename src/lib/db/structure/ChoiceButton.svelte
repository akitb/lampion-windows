<script lang="ts">
  // A borderless ⌃⌄ button that offers ready values for the field beside it (common types, defaults).
  // A native <select> underneath, so the keyboard and screen readers work as usual.
  import { ChevronsUpDown } from "@lucide/svelte";
  type Group = { title?: string; items: { value: string; label: string }[] };
  let { label, groups, onpick, disabled = false }: { label: string; groups: Group[]; onpick: (value: string) => void; disabled?: boolean } = $props();

  function picked(e: Event & { currentTarget: HTMLSelectElement }) {
    const v = e.currentTarget.value;
    e.currentTarget.value = "";
    if (v) onpick(v);
  }
</script>

<span class="choice" class:disabled title={label}>
  <select aria-label={label} {disabled} value="" onchange={picked}>
    <option value="" disabled hidden></option>
    {#each groups as g, i (i)}
      {#if g.title}
        <optgroup label={g.title}>
          {#each g.items as it (it.value)}<option value={it.value}>{it.label}</option>{/each}
        </optgroup>
      {:else}
        {#each g.items as it (it.value)}<option value={it.value}>{it.label}</option>{/each}
      {/if}
    {/each}
  </select>
  <ChevronsUpDown size={13} strokeWidth={2.2} aria-hidden="true" />
</span>

<style>
  .choice { position: relative; display: inline-grid; flex: none; place-items: center; width: 20px; height: 22px; color: var(--text-2); }
  .choice :global(svg) { pointer-events: none; }
  select {
    position: absolute;
    inset: 0;
    width: 100%;
    height: 100%;
    min-height: 0;
    padding: 0;
    border: none;
    border-radius: 5px;
    background: none;
    color: transparent;
    appearance: none;
  }
  select:focus-visible { outline: 2px solid var(--accent); outline-offset: 0; }
  option, optgroup { background: var(--surface-3); color: var(--text); }
  .disabled { opacity: 0.45; }
</style>
