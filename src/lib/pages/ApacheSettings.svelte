<script lang="ts">
  // Apache settings: edits httpd.conf. Apache checks every change on a copy before it is saved.
  import { onMount } from "svelte";
  import { Server } from "@lucide/svelte";
  import { call, core } from "$lib/api";
  import Page from "$lib/config/Page.svelte";
  import Header from "$lib/config/Header.svelte";
  import Section from "$lib/config/Section.svelte";
  import Row from "$lib/config/Row.svelte";
  import Switch from "$lib/config/Switch.svelte";
  import ValueField from "$lib/config/ValueField.svelte";
  import Popup from "$lib/config/Popup.svelte";
  import { apacheForm as form, showLog } from "$lib/config/forms.svelte";
  import type { ConfigFileId } from "$lib/types";

  onMount(() => void form.load());

  const d = $derived(form.draft);
  const info = $derived(form.info);
  const port = $derived(d?.port.trim() ?? "");
  const levels = $derived.by(() => {
    const list = info?.logLevels ?? [];
    const all = d && !list.includes(d.logLevel) ? [...list, d.logLevel] : list;
    return all.map((l) => ({ value: l, label: l.charAt(0).toUpperCase() + l.slice(1) }));
  });

  async function chooseFolder() {
    if (!d) return;
    const path = await core.pickPath({ kind: "folder", title: "Choose the web folder", startDir: d.documentRoot });
    if (path) d.documentRoot = path;
  }

  async function edit(id: ConfigFileId) {
    try {
      await call("config_edit_file", { id });
    } catch (e) {
      form.error = String(e);
    }
  }
</script>

<Page {form}>
  <Header
    icon={Server}
    tint="var(--tint-apache)"
    top="var(--tint-apache-top)"
    title="Apache"
    detail="The web server. Changes are checked with Apache before they are saved."
    file={info?.file}
    fileId="apache"
    hasBackup={info?.hasBackup}
    dirty={form.dirty}
    onrestore={() => form.restore()}
    onerror={(m) => (form.error = m)}
  />

  {#if form.loadError && !d}
    <Section>
      <Row title="httpd.conf could not be read" detail={form.loadError}>
        <button type="button" class="cfg-btn" onclick={() => form.load(true)}>Try Again</button>
      </Row>
    </Section>
  {:else if d && info}
    <fieldset disabled={form.saving}>
      <Section title="Server">
        <Row title="Port" detail="The address of your sites is http://localhost:port. Use 1024 or higher." for="apache-port">
          <ValueField id="apache-port" bind:value={d.port} placeholder="8080" problem={form.problem("port")} />
          <a class="link" href="http://localhost:{port}/" onclick={(e) => { e.preventDefault(); core.openUrl(`http://localhost:${port}/`); }}>localhost:{port}</a>
        </Row>
        <Row title="Web folder" detail="DocumentRoot: the files Apache serves at http://localhost:{port}/.">
          <span class="cfg-path" class:bad={!!form.problem("documentRoot")} title={d.documentRoot}>{d.documentRoot}</span>
          <button type="button" class="cfg-btn" onclick={chooseFolder}>Choose…</button>
        </Row>
        {#if form.warning("documentRoot")}
          <Row>
            {#snippet detail()}<span class="warn">{form.warning("documentRoot")}</span>{/snippet}
          </Row>
        {/if}
        <Row title="Administrator email" detail="Shown on Apache's error pages." for="apache-admin">
          <ValueField id="apache-admin" bind:value={d.serverAdmin} placeholder="you@example.com" width={220} mono={false} problem={form.problem("serverAdmin")} />
        </Row>
      </Section>

      <Section title="Web folder behaviour">
        <Row title=".htaccess files" detail="Lets folders change settings with .htaccess (AllowOverride All). Most PHP frameworks need this." for="apache-htaccess">
          <Switch id="apache-htaccess" bind:checked={d.allowOverride} />
        </Row>
        <Row title="List folder contents" detail="Show a file list for folders without an index.php or index.html." for="apache-listing">
          <Switch id="apache-listing" bind:checked={d.directoryListing} />
        </Row>
      </Section>

      <Section title="Modules" footer="PHP and phpMyAdmin are always on. For anything else, edit httpd.conf directly.">
        {#each info.features as f (f.id)}
          <Row title={f.title} detail={f.detail} for="apache-mod-{f.id}">
            <Switch id="apache-mod-{f.id}" bind:checked={d.modules[f.id]} />
          </Row>
        {/each}
      </Section>

      <Section title="Logging">
        <Row title="Error log detail" detail="Warn is a good default. Debug writes a lot and slows Apache down." for="apache-loglevel">
          <Popup id="apache-loglevel" bind:value={d.logLevel} options={levels} />
        </Row>
        <Row title="Log files">
          <button type="button" class="cfg-btn" onclick={() => showLog("apacheError")}>Errors</button>
          <button type="button" class="cfg-btn" onclick={() => showLog("apacheAccess")}>Requests</button>
        </Row>
      </Section>

      <Section title="More configuration">
        <Row title="Virtual hosts" detail="httpd-vhosts.conf. Include it from httpd.conf to use it.">
          <button type="button" class="cfg-btn" onclick={() => edit("vhosts")}>Edit</button>
        </Row>
        <Row title="Full configuration" detail="httpd.conf, for settings not shown here. Reopen this page afterwards.">
          <button type="button" class="cfg-btn" onclick={() => edit("apache")}>Edit</button>
        </Row>
      </Section>
    </fieldset>
  {/if}
</Page>

<style>
  fieldset { margin: 0; padding: 0; border: none; min-width: 0; }
  .link { color: color-mix(in srgb, var(--accent) 80%, white); font-size: 13px; text-decoration: none; white-space: nowrap; }
  .link:hover { text-decoration: underline; }
  .link:focus-visible { outline: 2px solid var(--accent); outline-offset: 1px; border-radius: 3px; }
  .bad { color: var(--red); }
  .warn { color: var(--orange); }
</style>
