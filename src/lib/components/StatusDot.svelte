<script lang="ts">
  import type { ServerState } from "$lib/types";
  let { state }: { state: ServerState } = $props();
</script>

<span class="dot {state}" aria-hidden="true"></span>

<style>
  .dot { display: inline-block; width: 7px; height: 7px; border-radius: 50%; background: var(--text-3); opacity: 0.6; flex: none; }
  .running { background: var(--green); opacity: 1; box-shadow: 0 0 4px color-mix(in srgb, var(--green) 60%, transparent); }
  .busy { background: var(--orange); opacity: 1; }
</style>
