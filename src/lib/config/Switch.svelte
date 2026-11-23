<script lang="ts">
  // The macOS switch (36 × 16 track, pill knob) as a checkbox with role="switch".
  // Bound (`bind:checked`) for form fields; with `onchange` it only reports the wish and shows
  // `checked` until the caller changes it (actions that can fail, like root without a password).
  let {
    checked = $bindable(false),
    disabled = false,
    id,
    label,
    onchange,
  }: { checked?: boolean; disabled?: boolean; id?: string; label?: string; onchange?: (on: boolean) => void } = $props();

  function changed(e: Event & { currentTarget: HTMLInputElement }) {
    if (!onchange) return;
    const on = e.currentTarget.checked;
    e.currentTarget.checked = checked;
    onchange(on);
  }
</script>

{#if onchange}
  <input type="checkbox" role="switch" class="switch" {id} {checked} {disabled} aria-label={label} onchange={changed} />
{:else}
  <input type="checkbox" role="switch" class="switch" {id} bind:checked {disabled} aria-label={label} />
{/if}

<style>
  .switch {
    appearance: none;
    position: relative;
    flex: none;
    width: 36px;
    height: 16px;
    margin: 0;
    border-radius: 8px;
    background: color-mix(in srgb, var(--text) 11%, transparent);
    transition: background-color var(--fade);
  }
  .switch::before {
    content: "";
    position: absolute;
    top: 1.5px;
    left: 1.5px;
    width: 21px;
    height: 13px;
    border-radius: 6.5px;
    background: #fff;
    box-shadow: 0 0.5px 1.5px rgba(0, 0, 0, 0.3);
    transition: translate var(--spring);
  }
  .switch:checked { background: var(--accent); }
  .switch:checked::before { translate: 12px 0; }
  .switch:disabled { opacity: 0.45; }
  .switch:focus-visible { outline: 2px solid var(--accent); outline-offset: 2px; }
</style>
