<script lang="ts">
  // The main window: sidebar navigation, toolbar, notice banners and the current page.
  import { fade } from "svelte/transition";
  import { Gauge, ScrollText, Feather, Code, Database, Settings2, Pencil, Folder, SquareTerminal, Globe } from "@lucide/svelte";
  import { app } from "$lib/state.svelte";
  import { core } from "$lib/api";
  import type { Pane, ServerId } from "$lib/types";
  import IconTile from "$lib/components/IconTile.svelte";
  import StatusDot from "$lib/components/StatusDot.svelte";
  import OverviewPage from "$lib/pages/OverviewPage.svelte";
  import LogsPage from "$lib/pages/LogsPage.svelte";
  import ApacheSettings from "$lib/pages/ApacheSettings.svelte";
  import PhpSettings from "$lib/pages/PhpSettings.svelte";
  import MariadbSettings from "$lib/pages/MariadbSettings.svelte";
  import GeneralSettings from "$lib/pages/GeneralSettings.svelte";
  import SetupBanner from "$lib/banners/SetupBanner.svelte";
  import UpdateBanner from "$lib/banners/UpdateBanner.svelte";

  type Row = { pane: Pane; title: string; icon: any; tint: string; key: string; server?: ServerId };
  const main: Row[] = [
    { pane: "overview", title: "Overview", icon: Gauge, tint: "var(--tint-overview)", key: "1" },
    { pane: "logs", title: "Logs", icon: ScrollText, tint: "var(--tint-logs)", key: "2" },
  ];
  const configuration: Row[] = [
    { pane: "apache", title: "Apache", icon: Feather, tint: "var(--tint-apache)", key: "3", server: "apache" },
    { pane: "php", title: "PHP", icon: Code, tint: "var(--tint-php)", key: "4" },
    { pane: "mariadb", title: "MariaDB", icon: Database, tint: "var(--tint-mariadb)", key: "5", server: "mariadb" },
    { pane: "general", title: "General", icon: Settings2, tint: "var(--tint-general)", key: "6" },
  ];
  const all = [...main, ...configuration];
  const pages: Record<Pane, any> = {
    overview: OverviewPage, logs: LogsPage, apache: ApacheSettings,
    php: PhpSettings, mariadb: MariadbSettings, general: GeneralSettings,
  };
  const Page = $derived(pages[app.pane]);
  const stateTitle = { running: "Running", stopped: "Stopped", busy: "Working…", notInstalled: "Not installed" };

  import { startServerStatus } from "$lib/servers.svelte";
  import { startLogBadge } from "$lib/logs.svelte";

  let webFolder = $state("");
  $effect(() => {
    core.appInfo().then((i) => (webFolder = i.lampionRoot + "\\www"));
    startServerStatus();
    startLogBadge();
  });

  function onKey(e: KeyboardEvent) {
    if (!e.ctrlKey || e.altKey || e.shiftKey) return;
    const row = all.find((r) => r.key === e.key);
    if (row) { app.pane = row.pane; e.preventDefault(); }
    else if (e.key === ",") { app.pane = "general"; e.preventDefault(); }
    else if (e.key.toLowerCase() === "d") { core.openDatabaseWindow(); e.preventDefault(); }
  }

  const apache = $derived(app.servers.apache);
</script>

<svelte:window onkeydown={onKey} />

<div class="window">
  <nav class="sidebar" aria-label="Sections">
    {#each [{ label: "", rows: main }, { label: "Configuration", rows: configuration }] as section}
      {#if section.label}<div class="section">{section.label}</div>{/if}
      {#each section.rows as r}
        <button class="row" class:selected={app.pane === r.pane} onclick={() => (app.pane = r.pane)}
                title="{r.title} (Ctrl+{r.key})" aria-current={app.pane === r.pane ? "page" : undefined}>
          <IconTile icon={r.icon} tint={r.tint} />
          <span class="title">{r.title}</span>
          {#if app.unsaved.includes(r.pane)}
            <span class="pencil" title="Unsaved changes" aria-label="Unsaved changes"><Pencil size={10} strokeWidth={3} color="#fff" /></span>
          {/if}
          {#if r.server}
            <span title="{r.title}: {stateTitle[app.servers[r.server].state]}"><StatusDot state={app.servers[r.server].state} /></span>
          {/if}
          {#if r.pane === "logs" && app.logErrors > 0}<span class="badge">{app.logErrors}</span>{/if}
        </button>
      {/each}
    {/each}
    <div class="footer">
      {#each [["apache", "Apache"], ["mariadb", "MariaDB"]] as [id, name]}
        {@const s = app.servers[id as ServerId]}
        <div class="srv">
          <StatusDot state={s.state} />
          <span>{name}</span>
          <span class="port">{s.state === "running" ? ":" + (s.ports[0] ?? s.port) : stateTitle[s.state]}</span>
        </div>
      {/each}
    </div>
  </nav>

  <main class="detail">
    <header class="toolbar">
      <h1>{all.find((r) => r.pane === app.pane)?.title}</h1>
      <div class="tools">
        <button class="plain" onclick={() => core.openDatabaseWindow()} title="Open the built-in database manager (Ctrl+D)"><Database size={16} /> Database Manager</button>
        <button class="plain" onclick={() => core.openPath(webFolder)} title="Open the web folder in Explorer ({webFolder})"><Folder size={16} /> Web Folder</button>
        <button class="plain" onclick={() => core.openTerminal(webFolder)} title="Open a terminal in the web folder"><SquareTerminal size={16} /> Terminal</button>
        <button class="plain" disabled={apache.state !== "running"} onclick={() => core.openUrl(`http://localhost:${apache.ports[0] ?? apache.port}/`)}
                title="Open http://localhost:{apache.ports[0] ?? apache.port}/ in the browser"><Globe size={16} /> Open Website</button>
      </div>
    </header>
    <UpdateBanner />
    <SetupBanner />
    <div class="content">
      {#key app.pane}
        <div class="page" in:fade={{ duration: 180 }}><Page /></div>
      {/key}
    </div>
  </main>
</div>

<style>
  .window { display: flex; height: 100vh; }
  .sidebar { width: 210px; flex: none; background: var(--bg-sidebar); border-right: 1px solid var(--border); display: flex; flex-direction: column; padding: 10px 8px; gap: 1px; }
  .section { font-size: 11px; font-weight: 600; color: var(--text-3); padding: 14px 8px 4px; }
  .row { display: flex; align-items: center; gap: 8px; border: none; background: none; box-shadow: none; padding: 5px 8px; border-radius: 6px; text-align: left; width: 100%; min-height: 30px; }
  .row:hover { background: var(--hover); }
  .row.selected { background: var(--selected); }
  .title { flex: 1; }
  .pencil { display: inline-flex; width: 15px; height: 15px; border-radius: 50%; background: var(--orange); align-items: center; justify-content: center; }
  .badge { background: var(--red); color: #fff; font-size: 11px; font-weight: 600; border-radius: 9px; padding: 0 6px; min-width: 18px; text-align: center; }
  .footer { margin-top: auto; border-top: 1px solid var(--border); padding: 10px 6px 2px; display: flex; flex-direction: column; gap: 7px; }
  .srv { display: flex; align-items: center; gap: 8px; font-size: 12px; }
  .srv .port { margin-left: auto; color: var(--text-2); font-variant-numeric: tabular-nums; }
  .detail { flex: 1; display: flex; flex-direction: column; min-width: 0; }
  .toolbar { display: flex; align-items: center; gap: 12px; height: 48px; padding: 0 12px 0 20px; border-bottom: 1px solid var(--border); flex: none; }
  .toolbar h1 { font-size: 15px; font-weight: 600; margin: 0; }
  .tools { margin-left: auto; display: flex; gap: 2px; }
  .tools button { display: inline-flex; align-items: center; gap: 6px; color: var(--text-2); }
  .content { flex: 1; position: relative; min-height: 0; }
  .page { position: absolute; inset: 0; overflow: auto; }
</style>
