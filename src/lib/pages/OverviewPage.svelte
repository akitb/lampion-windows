<script lang="ts">
  // The first page: "Local Server" with a one-line summary and Start All / Stop All, a card for
  // Apache and MariaDB side by side, and PHP below them.
  import { fade } from "svelte/transition";
  import { Play, Square } from "@lucide/svelte";
  import { app } from "$lib/state.svelte";
  import { servers, setup, startSetupState } from "$lib/servers.svelte";
  import ServerCard from "$lib/servers/ServerCard.svelte";
  import PhpCard from "$lib/servers/PhpCard.svelte";

  startSetupState();

  const list = $derived([app.servers.apache, app.servers.mariadb]);
  const running = $derived(list.filter((s) => s.state === "running").length);
  const busy = $derived(list.some((s) => s.state === "busy"));
  const ready = $derived(setup.state?.phase === "ready");
  const site = $derived(app.servers.apache.ports[0] ?? app.servers.apache.port);
  const summary = $derived(
    busy
      ? "Working…"
      : running === list.length
        ? `Everything is running at localhost:${site}`
        : running === 0
          ? "All servers are stopped"
          : `${running} of ${list.length} servers running`,
  );
</script>

<div class="overview">
  <header>
    <div class="titles">
      <h1>Local Server</h1>
      <p>{summary}</p>
    </div>
    {#if running < list.length}
      <button class="big primary" disabled={busy || !ready} onclick={servers.startAll} transition:fade={{ duration: 180 }}>
        <Play size={14} fill="currentColor" strokeWidth={0} /> Start All
      </button>
    {/if}
    {#if running > 0}
      <button class="big" disabled={busy} onclick={servers.stopAll} transition:fade={{ duration: 180 }}>
        <Square size={14} fill="currentColor" strokeWidth={0} /> Stop All
      </button>
    {/if}
  </header>

  <div class="servers">
    <ServerCard id="apache" />
    <ServerCard id="mariadb" />
  </div>
  <PhpCard />
</div>

<style>
  .overview { max-width: 1100px; margin: 0 auto; padding: 24px; }
  header { display: flex; align-items: center; gap: 8px; }
  .titles { flex: 1; min-width: 0; }
  h1 { margin: 0; font-family: "Segoe UI Variable Display", var(--font); font-size: 26px; line-height: 31px; font-weight: 700; color: var(--text); }
  header p { margin: 4px 0 0; font-size: 15px; line-height: 18px; color: var(--text-2); }
  .big {
    display: inline-flex; align-items: center; gap: 9px; flex: none;
    height: 28px; min-height: 0; padding: 0 18px; border: none; border-radius: 14px; box-shadow: none;
    background: var(--control); color: var(--text); font-size: 13px;
  }
  .big:hover:not(:disabled) { background: color-mix(in srgb, var(--control) 86%, var(--text)); }
  .big.primary { background: var(--accent); color: var(--accent-text); }
  .big.primary:hover:not(:disabled) { background: var(--accent); filter: brightness(1.08); }
  .servers { display: grid; grid-template-columns: 1fr 1fr; gap: 14px; margin: 23px 0 14px; }
</style>
