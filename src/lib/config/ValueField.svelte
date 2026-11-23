<script lang="ts">
  // A short right-aligned field (sizes, ports, counts), outlined in red while its value is invalid.
  // Screen readers hear the problem with the field.
  let {
    value = $bindable(""),
    width = 90,
    placeholder = "",
    problem,
    id,
    label,
    mono = true,
    type = "text",
  }: { value?: string; width?: number; placeholder?: string; problem?: string; id?: string; label?: string; mono?: boolean; type?: "text" | "password" } = $props();
  const errId = $props.id();
</script>

<input
  {id}
  {type}
  class="field"
  class:mono
  class:bad={!!problem}
  style:width="{width}px"
  bind:value
  {placeholder}
  aria-label={label}
  aria-invalid={problem ? "true" : "false"}
  aria-describedby={problem ? errId : undefined}
  spellcheck="false"
  autocomplete="off"
/>
{#if problem}<span id={errId} class="cfg-sr-only">{problem}</span>{/if}

<style>
  .field {
    flex: none;
    height: 26px;
    min-height: 0;
    padding: 0 7px;
    border: 1px solid var(--border);
    border-radius: 6px;
    background: var(--input-bg);
    color: var(--text);
    font-size: 13px;
    text-align: right;
  }
  .mono { font-family: var(--mono); }
  .field::placeholder { color: var(--text-3); }
  .bad { box-shadow: inset 0 0 0 1.5px var(--red); }
</style>
