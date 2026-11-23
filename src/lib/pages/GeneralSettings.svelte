<script lang="ts">
  // How Lampion itself behaves. Everything here applies straight away (no save bar).
  import { onMount } from "svelte";
  import { Settings as Gear } from "@lucide/svelte";
  import { revealItemInDir } from "@tauri-apps/plugin-opener";
  import { call, core, on } from "$lib/api";
  import Page from "$lib/config/Page.svelte";
  import Header from "$lib/config/Header.svelte";
  import Section from "$lib/config/Section.svelte";
  import Row from "$lib/config/Row.svelte";
  import Switch from "$lib/config/Switch.svelte";
  import Dialog from "$lib/config/Dialog.svelte";
  import { config, loadLocations } from "$lib/config/forms.svelte";
  import type { ConfigBackup, LampionFolders, Settings } from "$lib/types";

  let settings: Settings | null = $state(null);
  let version = $state("");
  let atLogin: boolean | null = $state(null);
  let loginError: string | null = $state(null);
  let folders: LampionFolders | null = $state(null);
  let backups: ConfigBackup[] = $state([]);
  let checking = $state(false);
  let updateError: string | null = $state(null);
  let error: string | null = $state(null);

  let moveTo: string | null = $state(null);
  let moving = $state(false);

  async function refresh() {
    [folders, backups] = await Promise.all([
      call<LampionFolders>("config_folders"),
      call<ConfigBackup[]>("config_backups").catch(() => []),
    ]);
    void loadLocations();
  }

  onMount(() => {
    core.getSettings().then((s) => (settings = s));
    core.appInfo().then((i) => (version = i.version));
    call<boolean>("config_open_at_login").then((v) => (atLogin = v));
    call<boolean>("servers_moving").then((v) => (moving = moving || v)).catch(() => {});
    void refresh();
    const off = on<Settings>("settings://changed", (s) => (settings = s));
    return () => void off.then((f) => f());
  });

  async function set(patch: Partial<Settings>) {
    try {
      settings = await core.setSettings(patch);
    } catch (e) {
      error = String(e);
    }
  }

  async function setAtLogin(enabled: boolean) {
    try {
      await call("config_set_open_at_login", { enabled });
      loginError = null;
    } catch (e) {
      loginError = String(e);
    }
    atLogin = await call<boolean>("config_open_at_login");
  }

  let servicesBusy = $state(false);
  async function setServices(enabled: boolean) {
    servicesBusy = true;
    try {
      await call("servers_use_services", { enabled });
      settings = await core.getSettings();
    } catch (e) {
      error = String(e);
    } finally {
      servicesBusy = false;
    }
  }

  async function checkNow() {
    checking = true;
    try {
      await call("update_check", { userInitiated: true });
      updateError = null;
    } catch (e) {
      updateError = String(e);
    } finally {
      checking = false;
    }
  }

  async function chooseMove() {
    const path = await core.pickPath({ kind: "folder", title: "Choose the new place of the Lampion folder", startDir: folders?.root });
    if (path) moveTo = path;
  }

  async function move() {
    if (!moveTo) return;
    const newRoot = moveTo;
    moving = true;
    moveTo = null;
    try {
      await call("servers_move_root", { target: newRoot });
    } catch (e) {
      error = String(e);
    } finally {
      moving = false;
      void refresh();
    }
  }

  async function show(path: string, file = false) {
    try {
      await (file ? revealItemInDir(path) : core.openPath(path));
    } catch (e) {
      error = String(e);
    }
  }

  const l = $derived(config.locations);
</script>

<Page>
  <Header icon={Gear} tint="var(--tint-general)" top="var(--tint-general-top)" title="General" detail="How Lampion itself behaves. These settings apply straight away." />

  <Section title="Startup">
    <Row title="Open Lampion at login" detail={loginError ?? "Lampion opens when you log in to Windows."} for="gen-login">
      <Switch id="gen-login" checked={atLogin ?? false} disabled={atLogin === null} onchange={setAtLogin} />
    </Row>
    <Row title="Start servers when Lampion opens" detail="Starts Apache and MariaDB if they are not running yet." for="gen-start">
      <Switch id="gen-start" checked={settings?.startServersOnOpen ?? false} disabled={!settings} onchange={(v) => set({ startServersOnOpen: v })} />
    </Row>
    <Row title="Stop servers when Lampion quits" detail="Off: the servers keep running in the background after you quit." for="gen-stop">
      <Switch id="gen-stop" checked={settings?.stopServersOnQuit ?? false} disabled={!settings} onchange={(v) => set({ stopServersOnQuit: v })} />
    </Row>
    <Row
      title="Run the servers as Windows services"
      detail="Apache and MariaDB then start with Windows, even before you log in. Changing this asks for administrator rights once."
      for="gen-services"
    >
      <Switch id="gen-services" checked={settings?.useServices ?? false} disabled={!settings || servicesBusy} onchange={setServices} />
    </Row>
  </Section>

  <Section title="Updates">
    <Row title="Check for updates automatically" detail="Looks for a new release on GitHub each time Lampion opens." for="gen-updates">
      <Switch id="gen-updates" checked={settings?.checkForUpdates ?? false} disabled={!settings} onchange={(v) => set({ checkForUpdates: v })} />
    </Row>
    <Row title="Version {version}" detail={updateError ?? "Updates come from Lampion's releases on GitHub."}>
      <button type="button" class="cfg-btn" disabled={checking} onclick={checkNow}>{checking ? "Checking…" : "Check Now"}</button>
    </Row>
  </Section>

  <Section title="Locations">
    {#if folders}
      <Row
        title="Lampion folder"
        detail={moving ? "Moving… progress shows in Logs › Lampion Activity." : `Sites, databases, logs and backups, all in one place.\n${folders.root}`}
      >
        <button type="button" class="cfg-btn" onclick={() => show(folders!.root)}>Show</button>
        <button type="button" class="cfg-btn" disabled={moving} onclick={chooseMove}>{moving ? "Moving…" : "Move…"}</button>
      </Row>
      {#if l}
        <Row title="Web folder" detail={l.documentRoot}>
          <button type="button" class="cfg-btn" onclick={() => show(l.documentRoot)}>Show</button>
        </Row>
        <Row title="Database files" detail={l.mariadbDatadir}>
          <button type="button" class="cfg-btn" onclick={() => show(l.mariadbDatadir)}>Show</button>
        </Row>
      {/if}
      <Row title="Logs" detail={folders.logs}>
        <button type="button" class="cfg-btn" onclick={() => show(folders!.logs)}>Show</button>
      </Row>
      <Row title="Servers" detail={`Apache, PHP, MariaDB and phpMyAdmin.\n${folders.bin}`}>
        <button type="button" class="cfg-btn" onclick={() => show(folders!.bin)}>Show</button>
      </Row>
      <Row title="Lampion's log" detail={folders.appLog}>
        <button type="button" class="cfg-btn" onclick={() => show(folders!.appLog, true)}>Show</button>
      </Row>
    {/if}
  </Section>

  <Section title="Backups" footer="Before Lampion first changes a file, it saves the original next to it. Each settings page can put it back from its file menu.">
    {#each backups as b (b.id)}
      <Row title={b.name} detail={b.backup}>
        <button type="button" class="cfg-btn" onclick={() => show(b.backup, true)}>Show</button>
      </Row>
    {:else}
      <Row detail="Lampion has not changed any configuration file yet." />
    {/each}
  </Section>
</Page>

<Dialog
  bind:open={() => moveTo !== null, (v) => { if (!v) moveTo = null; }}
  title="Move the Lampion folder?"
  message={`Lampion stops Apache and MariaDB, copies the sites, databases, logs and backups to ${moveTo ?? ""}, updates the settings and starts the servers again. It checks that every database is still there. The old folder stays where it is until you delete it.`}
  cancel="Cancel"
  confirm="Move"
  onconfirm={move}
/>

<Dialog bind:open={() => error !== null, (v) => { if (!v) error = null; }} title="Something went wrong" message={error ?? ""} />
