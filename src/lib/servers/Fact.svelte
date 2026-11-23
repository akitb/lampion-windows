<script lang="ts">
  // A small grey label above a monospaced value (PORT / PID / VERSION on the Overview cards).
  let { label, value }: { label: string; value: string } = $props();
</script>

<div class="fact">
  <span class="label">{label}</span>
  <span class="value" title={value}>{value}</span>
</div>

<style>
  .fact { display: flex; flex-direction: column; gap: 3px; min-width: 0; }
  .label { font-size: 10px; line-height: 11px; font-weight: 600; letter-spacing: 0.7px; text-transform: uppercase; color: var(--text-3); }
  .value {
    font-family: var(--mono); font-size: 12px; line-height: 15px; color: var(--text);
    white-space: nowrap; overflow: hidden; text-overflow: ellipsis; user-select: text; cursor: text;
  }
</style>
