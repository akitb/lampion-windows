<script lang="ts">
  // Coloured dot (or a small spinner) plus text, so a server's state never relies on colour alone.
  import type { ServerState } from "$lib/types";
  let { state }: { state: ServerState } = $props();
  const titles: Record<ServerState, string> = { running: "Running", stopped: "Stopped", busy: "Working…", notInstalled: "Not installed" };
</script>

<span class="pill {state}">
  {#if state === "busy"}<span class="spinner" aria-hidden="true"></span>{:else}<span class="dot" aria-hidden="true"></span>{/if}
  {titles[state]}
</span>

<style>
  .pill {
    --c: var(--text-2);
    display: inline-flex; align-items: center; gap: 6px; height: 21px; padding: 0 9px; border-radius: 11px; flex: none;
    font-size: 10px; font-weight: 500; white-space: nowrap; color: var(--c);
    background: color-mix(in srgb, var(--c) 14%, transparent);
    transition: color var(--spring), background-color var(--spring);
  }
  .running { --c: var(--green); }
  .busy { --c: var(--orange); }
  .notInstalled { --c: var(--red); }
  .stopped { background: color-mix(in srgb, var(--c) 10%, transparent); }
  .dot { width: 7px; height: 7px; border-radius: 50%; background: var(--c); }
  .running .dot { box-shadow: 0 0 3px color-mix(in srgb, var(--green) 60%, transparent); }
  .spinner { width: 9px; height: 9px; border: 1.5px solid currentColor; border-right-color: transparent; border-radius: 50%; animation: spin 0.8s linear infinite; }
  @keyframes spin { to { transform: rotate(360deg); } }
</style>
