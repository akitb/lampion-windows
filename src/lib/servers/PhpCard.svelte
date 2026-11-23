<script lang="ts">
  // PHP has no process of its own: the card shows its version, whether Apache runs it ("Linked") and
  // the two limits people change most, with the error log and the PHP settings one click away.
  import { CodeXml, Link, SlidersHorizontal, TextAlignStart, Unlink } from "@lucide/svelte";
  import IconTile from "$lib/components/IconTile.svelte";
  import Fact from "./Fact.svelte";
  import { app } from "$lib/state.svelte";
  import { servers } from "$lib/servers.svelte";
  import type { PhpInfo } from "$lib/types";

  let info = $state<PhpInfo | null>(null);
  const apacheState = $derived(app.servers.apache.state);

  // "Linked" follows Apache, so ask again whenever Apache starts or stops.
  $effect(() => {
    void apacheState;
    servers.phpInfo().then((i) => (info = i)).catch(() => {});
  });
</script>

<section class="card php" aria-label="PHP">
  <span class="tile"><IconTile icon={CodeXml} tint="var(--tint-php)" top="var(--tint-php-top)" size={40} /></span>
  <div class="names">
    <div class="line">
      <h2>PHP</h2>
      <span class="chip" class:off={!info?.linked}>
        {#if info?.linked}<Link size={11} strokeWidth={2.4} aria-hidden="true" /> Linked{:else}<Unlink size={11} strokeWidth={2.4} aria-hidden="true" /> Not linked{/if}
      </span>
    </div>
    <p>Runs inside Apache</p>
  </div>
  <div class="facts">
    <Fact label="Version" value={info?.version ?? "–"} />
    <Fact label="Memory" value={info?.memoryLimit ?? "–"} />
    <Fact label="Upload" value={info?.uploadMaxFilesize ?? "–"} />
  </div>
  <button class="control log" onclick={() => (app.pane = "logs")} title="PHP error log" aria-label="PHP error log">
    <TextAlignStart size={17} strokeWidth={1.8} />
  </button>
  <button class="control" onclick={() => (app.pane = "php")}><SlidersHorizontal size={16} strokeWidth={1.8} /> Settings</button>
</section>

<style>
  .php { padding: 15px; display: flex; align-items: center; gap: 12px; min-width: 0; }
  .tile { display: flex; flex: none; }
  .tile :global(svg) { width: 24px; height: 24px; stroke-width: 1.9; }
  .names { display: flex; flex-direction: column; gap: 2px; flex: none; }
  .line { display: flex; align-items: center; gap: 8px; }
  h2 { margin: 0; font-size: 13px; line-height: 16px; font-weight: 600; color: var(--text); }
  .names p { margin: 0; font-size: 10px; line-height: 13px; color: var(--text-2); }
  .chip {
    --c: var(--tint-php);
    display: inline-flex; align-items: center; gap: 5px; height: 22px; padding: 0 8px; border-radius: 11px;
    font-size: 10px; font-weight: 500; white-space: nowrap; color: var(--c);
    background: color-mix(in srgb, var(--c) 14%, transparent);
  }
  .chip.off { --c: var(--orange); }
  /* Centred in the room between the name and the buttons, like the macOS card. */
  .facts { flex: 1; min-width: 0; display: flex; justify-content: center; gap: 20px; }
  .control {
    display: inline-flex; align-items: center; justify-content: center; gap: 10px; flex: none;
    height: 24px; min-height: 0; padding: 0 13px; border: none; border-radius: 5px; box-shadow: none;
    background: var(--control); color: var(--text); font-size: 13px; white-space: nowrap;
  }
  .control:hover:not(:disabled) { background: color-mix(in srgb, var(--control) 86%, var(--text)); }
  .log { width: 40px; height: 21px; padding: 0; }
</style>
