<script lang="ts">
  // Filled glyphs like the macOS app's SF Symbols (tablecells.fill, cylinder.split.1x2.fill, eye.fill),
  // drawn from Lucide's table, database and eye paths: filled with the colour, lines cut in `--cut`.
  let { name, size = 16 }: { name: "table" | "database" | "view"; size?: number } = $props();
</script>

{#if name === "table"}
  <svg class="glyph" width={size} height={size * 0.8} viewBox="1 3.5 22 17.6" aria-hidden="true">
    <rect x="2" y="4.5" width="20" height="15.6" rx="2.6" />
    <path d="M12 4.5v15.6M2 9.7h20M2 14.9h20" />
  </svg>
{:else if name === "database"}
  <svg class="glyph" width={size * 0.82} height={size} viewBox="2 1 20 24.4" aria-hidden="true">
    <path d="M3 5V19A9 3 0 0 0 21 19V5" />
    <ellipse cx="12" cy="5" rx="9" ry="3" />
    <path d="M3 12A9 3 0 0 0 21 12" fill="none" />
  </svg>
{:else}
  <svg class="glyph" width={size} height={size} viewBox="0 0 24 24" aria-hidden="true">
    <path d="M2.062 12.348a1 1 0 0 1 0-.696 10.75 10.75 0 0 1 19.876 0 1 1 0 0 1 0 .696 10.75 10.75 0 0 1-19.876 0" />
    <circle cx="12" cy="12" r="3" />
  </svg>
{/if}

<style>
  .glyph {
    flex: none;
    fill: currentColor;
    stroke: var(--cut, var(--db-content));
    stroke-width: 1.9;
    stroke-linecap: round;
    stroke-linejoin: round;
  }
</style>
