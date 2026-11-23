<script lang="ts">
  // A column type field: a tile in the type family's colour, the text, and a menu of common types.
  import { family, familyColor, familyIcon, typeGroups } from "./family";
  import ChoiceButton from "./ChoiceButton.svelte";
  let { type = $bindable(""), compact = false, label = "Type" }: { type?: string; compact?: boolean; label?: string } = $props();
  const f = $derived(family(type));
  const Icon = $derived(familyIcon[f]);
  const groups = typeGroups.map((g) => ({ title: g.title, items: g.types.map((t) => ({ value: t, label: t })) }));
</script>

<span class="typefield" class:compact>
  <span class="tile" style:--c={familyColor[f]} title="Type family"><Icon size={compact ? 10 : 11} strokeWidth={2.6} aria-hidden="true" /></span>
  <input class="text" type="text" bind:value={type} placeholder="VARCHAR(255)" aria-label={label} spellcheck="false" autocomplete="off" />
  <ChoiceButton label="Common types" {groups} onpick={(t) => (type = t)} />
</span>

<style>
  .typefield { display: inline-flex; flex: none; align-items: center; gap: 6px; width: 260px; }
  .typefield.compact { width: 100%; }
  .tile {
    display: grid;
    flex: none;
    place-items: center;
    width: 22px;
    height: 22px;
    border-radius: 5px;
    background: color-mix(in srgb, var(--c) 13%, transparent);
    color: var(--c);
    transition: color var(--fade), background-color var(--fade);
  }
  .compact .tile { width: 18px; height: 18px; }
  .typefield input.text { flex: 1; width: auto; min-width: 0; }
  .compact input.text { height: 25px; }
</style>
