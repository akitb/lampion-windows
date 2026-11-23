<script lang="ts">
  // MariaDB settings: Lampion's settings in my.ini (empty field = MariaDB's default, shown in grey),
  // how programs connect (root without a password, ready-made settings) and phpMyAdmin's sign-in.
  import { onMount } from "svelte";
  import { Database, Copy } from "@lucide/svelte";
  import { writeText } from "@tauri-apps/plugin-clipboard-manager";
  import { call } from "$lib/api";
  import { app } from "$lib/state.svelte";
  import Page from "$lib/config/Page.svelte";
  import Header from "$lib/config/Header.svelte";
  import Section from "$lib/config/Section.svelte";
  import Row from "$lib/config/Row.svelte";
  import Switch from "$lib/config/Switch.svelte";
  import ValueField from "$lib/config/ValueField.svelte";
  import Popup from "$lib/config/Popup.svelte";
  import Menu from "$lib/config/Menu.svelte";
  import Dialog from "$lib/config/Dialog.svelte";
  import { mariadbForm as form, showLog } from "$lib/config/forms.svelte";
  import type { ConfigFileId, SqlMode, Snippet } from "$lib/types";

  onMount(() => {
    void form.load();
    void loadPma();
  });

  const d = $derived(form.draft);
  const info = $derived(form.info);
  const def = $derived(info?.defaults);
  const server = $derived(app.servers.mariadb);
  /** The port programs reach MariaDB on now (the configured one while it is stopped). */
  const port = $derived(server.ports[0] ?? server.port);

  const charsets = $derived.by(() => {
    const list = (info?.charsets ?? []).map((c) => ({ value: c.charset, label: c.charset }));
    if (d?.characterSet && !list.some((c) => c.value === d.characterSet)) list.push({ value: d.characterSet, label: d.characterSet });
    return [{ value: "", label: `Server default (${def?.characterSet ?? "utf8mb4"})` }, ...list];
  });
  const collations = $derived.by(() => {
    const list = (info?.charsets.find((c) => c.charset === d?.characterSet)?.collations ?? []).map((c) => ({ value: c, label: c }));
    if (d?.collation && !list.some((c) => c.value === d.collation)) list.push({ value: d.collation, label: d.collation });
    return [{ value: "", label: "Default for the character set" }, ...list];
  });
  const modes = $derived.by(() => {
    const list: { value: SqlMode; label: string }[] = [
      { value: "default", label: "Server default" },
      { value: "strict", label: "Strict" },
      { value: "relaxed", label: "Relaxed (like old MySQL)" },
    ];
    if (form.saved?.sqlMode === "custom" || d?.sqlMode === "custom") {
      list.push({ value: "custom", label: `Custom: ${d?.customSqlMode || "none"}` });
    }
    return list;
  });

  /** A new character set keeps the collation only when it belongs to it. */
  function charsetChanged(cs: string) {
    if (!d || !info) return;
    const set = info.charsets.find((c) => c.charset === cs);
    if (!cs) d.collation = "";
    else if (set && !set.collations.includes(d.collation)) d.collation = set.collations[0];
  }

  /** Turning the slow query log on starts with Lampion's threshold, 2 seconds. */
  function setSlowLog(on: boolean) {
    if (!d) return;
    d.slowQueryLog = on;
    if (on && !d.longQueryTime.trim()) d.longQueryTime = "2";
  }

  // ---- Connecting from apps ----

  let rootOpen: boolean | null = $state(null);
  let rootError: string | null = $state(null);
  let rootBusy = $state(false);
  /** Asks for root's new password (turning it off) or its current one (turning it on). */
  let ask: "new" | "current" | null = $state(null);
  let pw1 = $state("");
  let pw2 = $state("");

  async function loadRoot() {
    rootOpen = await call<boolean | null>("config_root_state").catch(() => null);
  }

  // Again whenever MariaDB starts or stops.
  $effect(() => {
    void server.state;
    void loadRoot();
  });

  async function setRoot(on: boolean, password?: string) {
    rootBusy = true;
    try {
      await call("config_set_root_no_password", { enabled: on, password: password ?? null });
      rootError = null;
      ask = null;
    } catch (e) {
      const msg = String(e);
      if (on && !password && /ERROR (1045|1698)/.test(msg)) {
        ask = "current";
      } else {
        rootError = msg;
        ask = null;
      }
    } finally {
      rootBusy = false;
      pw1 = pw2 = "";
      await loadRoot();
    }
  }

  function rootToggled(on: boolean) {
    if (on) void setRoot(true);
    else ask = "new";
  }

  let database = $state("");
  let copied: string | null = $state(null);
  let copiedTimer: ReturnType<typeof setTimeout> | undefined;
  let snippets: Snippet[] = $state([]);

  async function copy(text: string, label: string) {
    try {
      await writeText(text);
      copied = label;
      clearTimeout(copiedTimer);
      copiedTimer = setTimeout(() => (copied = null), 3000);
    } catch (e) {
      form.error = String(e);
    }
  }

  // The menu's texts follow the database name and the port.
  $effect(() => {
    const db = database;
    call<Snippet[]>("config_snippets", { database: db, port }).then((s) => (snippets = s)).catch(() => {});
  });

  // ---- phpMyAdmin ----

  let pma: boolean | null = $state(null);
  let pmaError: string | null = $state(null);

  async function loadPma() {
    pma = await call<boolean | null>("config_phpmyadmin_state").catch(() => null);
  }

  async function setPma(on: boolean) {
    try {
      await call("config_set_phpmyadmin_auto_login", { enabled: on });
      pmaError = null;
    } catch (e) {
      pmaError = String(e);
    }
    await loadPma();
  }

  async function edit(id: ConfigFileId) {
    try {
      await call("config_edit_file", { id });
    } catch (e) {
      form.error = String(e);
    }
  }
</script>

{#snippet copyable(value: string, label: string)}
  <span class="value">{value}</span>
  <button type="button" class="icon" onclick={() => copy(value, label)} title="Copy" aria-label="Copy {label}"><Copy size={14} strokeWidth={1.7} /></button>
{/snippet}

<Page {form}>
  <Header
    icon={Database}
    tint="var(--tint-mariadb)"
    top="var(--tint-mariadb-top)"
    title={server.version ? `MariaDB ${server.version}` : "MariaDB"}
    detail="The database server. Empty fields use MariaDB's default, shown in grey."
    file={info?.file}
    fileId="mariadb"
    hasBackup={info?.hasBackup}
    dirty={form.dirty}
    onrestore={() => form.restore()}
    onerror={(m) => (form.error = m)}
  />

  {#if form.loadError && !d}
    <Section>
      <Row title="my.ini could not be read" detail={form.loadError}>
        <button type="button" class="cfg-btn" onclick={() => form.load(true)}>Try Again</button>
      </Row>
    </Section>
  {:else if d && info && def}
    <fieldset disabled={form.saving}>
      <Section title="Connection">
        <Row title="Port" detail="Programs connect to 127.0.0.1 on this port. phpMyAdmin follows it when you save." for="mdb-port">
          <ValueField id="mdb-port" bind:value={d.port} placeholder={def.port} problem={form.problem("port")} />
        </Row>
        <Row title="Only this computer can connect" detail="bind-address = 127.0.0.1. Turn off to reach the database from other devices on your network." for="mdb-local">
          <Switch id="mdb-local" bind:checked={d.localOnly} />
        </Row>
        {#if form.warning("localOnly") && form.saved?.localOnly}
          <Row>
            {#snippet detail()}<span class="warn">{form.warning("localOnly")}</span>{/snippet}
          </Row>
        {/if}
      </Section>

      <Section title="Capacity">
        <Row title="Maximum connections" detail="max_connections. How many clients may be connected at once." for="mdb-conns">
          <ValueField id="mdb-conns" bind:value={d.maxConnections} placeholder={def.maxConnections} problem={form.problem("maxConnections")} />
        </Row>
        <Row title="Maximum packet size" detail="max_allowed_packet. Raise it to import large dumps or store big BLOBs." for="mdb-packet">
          <ValueField id="mdb-packet" bind:value={d.maxAllowedPacket} placeholder={def.maxAllowedPacket} problem={form.problem("maxAllowedPacket")} />
        </Row>
        <Row title="InnoDB buffer pool" detail="innodb_buffer_pool_size. Memory for cached table data. 256M–1G suits most projects." for="mdb-pool">
          <ValueField id="mdb-pool" bind:value={d.bufferPoolSize} placeholder={def.bufferPoolSize} problem={form.problem("bufferPoolSize")} />
        </Row>
      </Section>

      <Section title="Text and SQL">
        <Row title="Character set" detail="character-set-server, for new databases and tables." for="mdb-charset">
          <Popup id="mdb-charset" bind:value={d.characterSet} options={charsets} onchange={charsetChanged} />
        </Row>
        <Row title="Collation" detail="How text is sorted and compared. Pick a _hungarian_ collation for correct á, ő, ű order." for="mdb-collation">
          <Popup id="mdb-collation" bind:value={d.collation} options={collations} disabled={!d.characterSet} />
        </Row>
        <Row title="SQL mode" detail="Strict rejects invalid values; relaxed quietly adjusts them, as old MySQL did." for="mdb-sqlmode">
          <Popup id="mdb-sqlmode" bind:value={d.sqlMode} options={modes} />
        </Row>
      </Section>

      <Section title="Query logs">
        {#snippet footer()}
          <div class="logbuttons">
            <button type="button" class="cfg-btn small" onclick={() => showLog("mariadb")}>Server Log</button>
            <button type="button" class="cfg-btn small" onclick={() => showLog("mariadbSlow")}>Slow Queries</button>
            <button type="button" class="cfg-btn small" onclick={() => showLog("mariadbGeneral")}>All Queries</button>
          </div>
        {/snippet}
        <Row title="Log slow queries" detail="Records queries that take longer than the threshold below." for="mdb-slow">
          <Switch id="mdb-slow" bind:checked={() => d.slowQueryLog, setSlowLog} />
        </Row>
        {#if d.slowQueryLog}
          <Row title="Slow query threshold" for="mdb-threshold">
            <ValueField id="mdb-threshold" bind:value={d.longQueryTime} placeholder="2" width={70} problem={form.problem("longQueryTime")} />
            <span class="unit">seconds</span>
          </Row>
        {/if}
        <Row title="Log every query" detail="Useful for seeing exactly what an app sends. The file grows quickly, so turn it off when done." for="mdb-general">
          <Switch id="mdb-general" bind:checked={d.generalLog} />
        </Row>
      </Section>
    </fieldset>
    <!-- Not part of the form: these apply straight away. -->
    <Section title="Connecting from apps">
      <Row
        title="root without a password"
        detail={rootError ?? (rootOpen === null
          ? "Start MariaDB to see or change this."
          : "Like XAMPP: programs log in as root with an empty password. Only from this computer, never from other computers.")}
        for="mdb-root"
      >
        <Switch id="mdb-root" checked={rootOpen ?? false} disabled={rootOpen === null || rootBusy} onchange={rootToggled} />
      </Row>
      <Row title="Host">{@render copyable("127.0.0.1", "Host")}</Row>
      <Row title="Port">{@render copyable(String(port), "Port")}</Row>
      <Row title="User">{@render copyable("root", "User")}</Row>
      <Row title="Password"><span class="value plain">{rootOpen ? "(empty)" : "—"}</span></Row>
      <Row title="For your app" detail={copied ? `Copied: ${copied}` : "Ready-made settings for PHP, Laravel, Prisma, Node.js, Python and Java."} for="mdb-dbname">
        <ValueField id="mdb-dbname" bind:value={database} placeholder="database name" width={160} mono={false} />
        <Menu label="Copy Settings">
          {#snippet children(close)}
            {#each snippets as s (s.id)}
              <button type="button" onclick={() => { close(); copy(s.text, s.title); }}>{s.title}</button>
            {/each}
          {/snippet}
        </Menu>
      </Row>
    </Section>

    <Section title="phpMyAdmin">
      <Row
        title="Sign in automatically"
        detail={pmaError ?? (pma === null
          ? "phpMyAdmin is not installed."
          : "Opens phpMyAdmin as root without a login page. Only this computer can open phpMyAdmin. Applies straight away.")}
        for="mdb-pma"
      >
        <Switch id="mdb-pma" checked={pma ?? false} disabled={pma === null} onchange={setPma} />
      </Row>
    </Section>

    <Section title="More configuration">
      <Row title="my.ini" detail="The whole option file, for settings not shown here. Reopen this page afterwards.">
        <button type="button" class="cfg-btn" onclick={() => edit("mariadb")}>Edit</button>
      </Row>
      <Row title="phpMyAdmin" detail="config.inc.php">
        <button type="button" class="cfg-btn" onclick={() => edit("phpmyadmin")}>Edit</button>
      </Row>
    </Section>
  {/if}
</Page>

<Dialog
  bind:open={() => ask !== null, (v) => { if (!v) { ask = null; pw1 = pw2 = ""; } }}
  title={ask === "current" ? "Enter root's current password" : "Turn off root without a password?"}
  message={ask === "current"
    ? "MariaDB asks for root's password. Lampion uses it once to remove the password."
    : "Programs then need root's password to connect. Choose the password:"}
  cancel="Cancel"
  confirm={ask === "current" ? "Continue" : "Set Password"}
  busy={rootBusy}
  disabled={!pw1 || (ask === "new" && pw1 !== pw2)}
  onconfirm={() => setRoot(ask === "current", pw1)}
>
  <div class="passwords">
    <ValueField type="password" bind:value={pw1} width={226} mono={false} placeholder="Password" label="Password" />
    {#if ask === "new"}
      <ValueField
        type="password"
        bind:value={pw2}
        width={226}
        mono={false}
        placeholder="Again"
        label="Password again"
        problem={pw2 && pw1 !== pw2 ? "The passwords are not the same." : undefined}
      />
    {/if}
  </div>
</Dialog>

<style>
  fieldset { margin: 0; padding: 0; border: none; min-width: 0; }
  .unit { width: 52px; color: var(--text-2); font-size: 13px; }
  .warn { color: var(--orange); }
  .value { font-family: var(--mono); font-size: 13px; color: var(--text-2); user-select: text; }
  .plain { font-family: var(--font); }
  .icon { display: inline-grid; place-items: center; width: 20px; height: 20px; min-height: 0; padding: 0; border: none; background: none; box-shadow: none; color: var(--text-2); border-radius: 5px; }
  .icon:hover:not(:disabled) { background: var(--hover); color: var(--text); }
  .logbuttons { display: flex; gap: 9px; margin-top: 4px; }
  .passwords { display: flex; flex-direction: column; gap: 8px; margin: 0 6px 14px; }
  .passwords :global(input) { text-align: left; }
</style>
