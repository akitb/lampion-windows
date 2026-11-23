<script lang="ts">
  // The SQL type as a small tinted capsule, e.g. `varchar(100)`, coloured by its family.
  import type { TypeFamily } from "./types";
  let { type, family }: { type: string; family: TypeFamily } = $props();
</script>

<span class="chip fam-{family}" title={type}>{type}</span>

<style>
  .chip {
    display: inline-block;
    max-width: 100%;
    overflow: hidden;
    padding: 1px 6px;
    border-radius: 4px;
    background: color-mix(in srgb, var(--fam) 13%, transparent);
    color: var(--fam);
    font-family: var(--mono);
    font-size: 10.5px;
    font-weight: 500;
    line-height: 15px;
    white-space: nowrap;
    text-overflow: ellipsis;
  }
</style>
