<script lang="ts">
  // The main window: sidebar navigation, toolbar, notice banners and the current page.
  import { fade } from "svelte/transition";
  import { LayoutGrid, FileSearch, Server, CodeXml, Database, Settings, Pencil, Folder, SquareTerminal, Globe } from "@lucide/svelte";
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

  type Row = { pane: Pane; title: string; icon: any; tint: string; top: string; key: string; server?: ServerId };
  const main: Row[] = [
    { pane: "overview", title: "Overview", icon: LayoutGrid, tint: "var(--tint-overview)", top: "var(--tint-overview-top)", key: "1" },
    { pane: "logs", title: "Logs", icon: FileSearch, tint: "var(--tint-logs)", top: "var(--tint-logs-top)", key: "2" },
  ];
  const configuration: Row[] = [
    { pane: "apache", title: "Apache", icon: Server, tint: "var(--tint-apache)", top: "var(--tint-apache-top)", key: "3", server: "apache" },
    { pane: "php", title: "PHP", icon: CodeXml, tint: "var(--tint-php)", top: "var(--tint-php-top)", key: "4" },
    { pane: "mariadb", title: "MariaDB", icon: Database, tint: "var(--tint-mariadb)", top: "var(--tint-mariadb-top)", key: "5", server: "mariadb" },
    { pane: "general", title: "General", icon: Settings, tint: "var(--tint-general)", top: "var(--tint-general-top)", key: "6" },
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
  import { config, loadLocations } from "$lib/config/forms.svelte";

  // The web folder is httpd.conf's DocumentRoot (kept current by `config://locations`); until that is
  // read, Lampion's default one.
  let lampionRoot = $state("");
  const webFolder = $derived(config.locations?.documentRoot || (lampionRoot && lampionRoot + "\\www"));
  $effect(() => {
    core.appInfo().then((i) => (lampionRoot = i.lampionRoot));
    void loadLocations();
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
          <IconTile icon={r.icon} tint={r.tint} top={r.top} />
          <span class="title">{r.title}</span>
          {#if app.unsaved.includes(r.pane)}
            <span class="pencil" title="Unsaved changes" aria-label="Unsaved changes"><Pencil size={8} strokeWidth={3} color="#3a2a10" /></span>
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
      <div class="tools" role="toolbar" aria-label="Shortcuts">
        <button onclick={() => core.openDatabaseWindow()} title="Open the built-in database manager (Ctrl+D)" aria-label="Database Manager"><Database size={18} strokeWidth={1.6} /></button>
        <button disabled={!webFolder} onclick={() => core.openPath(webFolder)} title="Open the web folder in Explorer ({webFolder})" aria-label="Web Folder"><Folder size={18} strokeWidth={1.6} /></button>
        <button disabled={!webFolder} onclick={() => core.openTerminal(webFolder)} title="Open a terminal in the web folder" aria-label="Terminal"><SquareTerminal size={18} strokeWidth={1.6} /></button>
        <button disabled={apache.state !== "running"} onclick={() => core.openUrl(`http://localhost:${apache.ports[0] ?? apache.port}/`)}
                title="Open http://localhost:{apache.ports[0] ?? apache.port}/ in the browser" aria-label="Open Website"><Globe size={18} strokeWidth={1.6} /></button>
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
  /* Sizes are the macOS screenshots' measurements halved (they are 2x). */
  .window { display: flex; height: 100vh; }
  .sidebar { width: 190px; flex: none; background: var(--bg-sidebar); display: flex; flex-direction: column; padding: 8px 10px 10px; gap: 0; }
  .section { font-size: 11px; font-weight: 500; color: var(--text-3); padding: 12px 4px 5px; }
  .row { display: flex; align-items: center; gap: 8px; border: none; background: none; box-shadow: none; padding: 0 8px 0 6px; border-radius: 7px; text-align: left; width: 100%; height: 31px; font-size: 13px; }
  .row:hover:not(.selected) { background: var(--hover); }
  .row.selected { background: var(--selected); color: var(--selected-text); font-weight: 500; }
  .title { flex: 1; }
  .pencil { display: inline-flex; width: 13px; height: 13px; border-radius: 50%; background: var(--unsaved); align-items: center; justify-content: center; margin-right: 4px; }
  .badge { background: var(--red); color: #fff; font-size: 11px; font-weight: 600; border-radius: 9px; padding: 0 6px; min-width: 18px; text-align: center; }
  .footer { margin-top: auto; border-top: 1px solid var(--border-strong); padding: 9px 0 0; display: flex; flex-direction: column; gap: 6px; }
  .srv { display: flex; align-items: center; gap: 8px; font-size: 11px; padding-left: 2px; }
  .srv .port { margin-left: auto; color: var(--text-2); font-variant-numeric: tabular-nums; }
  .detail { flex: 1; display: flex; flex-direction: column; min-width: 0; background: var(--bg); }
  .toolbar { display: flex; align-items: center; gap: 12px; height: 52px; padding: 0 8px 0 20px; flex: none; }
  .toolbar h1 { font-size: 15px; font-weight: 600; margin: 0; }
  .tools { margin-left: auto; display: flex; align-items: center; gap: 2px; background: var(--toolbar-pill); border-radius: 18px; padding: 3px 6px; height: 36px; }
  .tools button { display: inline-flex; align-items: center; justify-content: center; width: 34px; height: 30px; padding: 0; border: none; background: none; box-shadow: none; border-radius: 15px; color: var(--text); }
  .tools button:hover:not(:disabled) { background: var(--hover); }
  .content { flex: 1; position: relative; min-height: 0; }
  .page { position: absolute; inset: 0; overflow: auto; }
</style>
