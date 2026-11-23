<script lang="ts">
  // The SQL type as a small tinted capsule in its family's colour, e.g. `varchar(100)`.
  import { family, familyColor } from "./family";
  let { type }: { type: string } = $props();
</script>

<span class="chip" style:--c={familyColor[family(type)]} title={type}>{type}</span>

<style>
  .chip {
    display: inline-block;
    max-width: 100%;
    overflow: hidden;
    padding: 2px 6px;
    border-radius: 4px;
    background: color-mix(in srgb, var(--c) 13%, transparent);
    color: var(--c);
    font: 500 10.5px/14px var(--mono);
    text-overflow: ellipsis;
    white-space: nowrap;
    vertical-align: middle;
  }
</style>
