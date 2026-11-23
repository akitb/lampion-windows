<script lang="ts">
  // Apache or MariaDB on the Overview: state, PORT / PID / VERSION, Start or Stop + Restart,
  // Open Site / phpMyAdmin and the "…" menu (settings, log, the configuration file).
  import { Database, Globe, Play, RotateCw, Square } from "@lucide/svelte";
  import IconTile from "$lib/components/IconTile.svelte";
  import Fact from "./Fact.svelte";
  import MoreMenu from "./MoreMenu.svelte";
  import StatusPill from "./StatusPill.svelte";
  import { app } from "$lib/state.svelte";
  import { core } from "$lib/api";
  import { actionErrors, servers } from "$lib/servers.svelte";
  import type { ServerId } from "$lib/types";

  let { id }: { id: ServerId } = $props();
  const apache = $derived(id === "apache");
  const title = $derived(apache ? "Apache" : "MariaDB");
  const s = $derived(app.servers[id]);
  const web = $derived(app.servers.apache);
  /** Running, or busy while it still runs (stopping, restarting). */
  const up = $derived(s.state === "running" || (s.state === "busy" && s.pids.length > 0));
  const port = $derived(s.state === "running" ? s.ports.join(", ") || "–" : String(s.port));

  const edit = (file: "apache" | "mariadb" | "phpmyadmin") => () =>
    void servers.editConfig(file).catch((e) => (actionErrors[id] = String(e)));

  const items = $derived(
    apache
      ? [
          { label: "Settings…", run: () => (app.pane = "apache") },
          { label: "Error Log", run: () => (app.pane = "logs") },
          null,
          { label: "Edit httpd.conf", run: edit("apache") },
        ]
      : [
          { label: "Settings…", run: () => (app.pane = "mariadb") },
          { label: "Server Log", run: () => (app.pane = "logs") },
          null,
          { label: "Edit my.ini", run: edit("mariadb") },
          { label: "Edit phpMyAdmin config", run: edit("phpmyadmin") },
        ],
  );

  function openAdmin() {
    const p = web.ports[0] ?? web.port;
    core.openUrl(`http://localhost:${p}${apache ? "/" : "/phpmyadmin/"}`).catch((e) => (actionErrors[id] = String(e)));
  }
</script>

<section class="card server" aria-label={title}>
  <div class="head">
    <span class="tile"><IconTile icon={apache ? Globe : Database} tint="var(--tint-{id})" top="var(--tint-{id}-top)" size={40} /></span>
    <div class="names">
      <h2>{title}</h2>
      <p>{apache ? "Web server" : "Database server"}</p>
    </div>
    <StatusPill state={s.state} />
  </div>

  <div class="facts">
    <Fact label="Port" value={port} />
    <Fact label="PID" value={s.pids[0]?.toString() ?? "–"} />
    <Fact label="Version" value={s.version ?? "–"} />
  </div>

  <div class="actions">
    {#if up}
      <button class="control wide" disabled={s.state === "busy"} onclick={() => servers.stop(id)}>
        <Square size={14} fill="currentColor" strokeWidth={0} /> Stop
      </button>
      <button class="control restart" disabled={s.state === "busy"} onclick={() => servers.restart(id)} title="Restart {title}" aria-label="Restart {title}">
        <RotateCw size={15} strokeWidth={2} />
      </button>
    {:else}
      <button class="control wide start" style="--tint: var(--tint-{id})" disabled={s.state !== "stopped"} onclick={() => servers.start(id)}>
        <Play size={14} fill="currentColor" strokeWidth={0} /> Start
      </button>
    {/if}
    <span class="spacer"></span>
    <button class="control" disabled={web.state !== "running"} onclick={openAdmin}
            title={apache ? `Open http://localhost:${web.ports[0] ?? web.port}/` : "Open phpMyAdmin in the browser"}>
      {apache ? "Open Site" : "phpMyAdmin"}
    </button>
    <MoreMenu {items} label="More {title} actions" />
  </div>

  {#if actionErrors[id]}<p class="error" role="alert">{actionErrors[id]}</p>{/if}
</section>

<style>
  /* 15px + the 1px border: macOS draws the outline inside the 16px padding. */
  .server { padding: 15px; display: flex; flex-direction: column; min-width: 0; }
  .head { display: flex; align-items: flex-start; gap: 12px; }
  /* The glyph is smaller and lighter than IconTile's default, as on the macOS cards. */
  .tile { display: flex; flex: none; }
  .tile :global(svg) { width: 20px; height: 20px; stroke-width: 1.8; }
  .names { flex: 1; min-width: 0; display: flex; flex-direction: column; gap: 2px; }
  h2 { margin: 0; font-size: 13px; line-height: 16px; font-weight: 600; color: var(--text); }
  .names p { margin: 0; font-size: 10px; line-height: 13px; color: var(--text-2); }
  .facts { display: flex; gap: 23px; margin-top: 16px; }
  .actions { display: flex; align-items: center; gap: 8px; margin-top: 16px; }
  .spacer { flex: 1; }
  .control {
    display: inline-flex; align-items: center; justify-content: center; gap: 9px;
    height: 24px; min-height: 0; padding: 0 12px; border: none; border-radius: 5px; box-shadow: none;
    background: var(--control); color: var(--text); font-size: 13px; white-space: nowrap;
  }
  .control:hover:not(:disabled) { background: color-mix(in srgb, var(--control) 86%, var(--text)); }
  .wide { min-width: 80px; }
  .restart { width: 38px; padding: 0; }
  .start { background: var(--tint); color: #fff; }
  .start:hover:not(:disabled) { background: var(--tint); filter: brightness(1.08); }
  .error { margin: 10px 0 0; font-size: 11px; line-height: 14px; color: var(--red); user-select: text; }
</style>
