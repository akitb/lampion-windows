<script module lang="ts">
  // PHP's time zone names, fetched once.
  let zones: string[] = $state([]);
</script>

<script lang="ts">
  // PHP settings: edits Lampion's php.ini. PHP runs inside Apache, so Apache restarts to apply changes.
  import { onMount } from "svelte";
  import { CodeXml } from "@lucide/svelte";
  import { call } from "$lib/api";
  import Page from "$lib/config/Page.svelte";
  import Header from "$lib/config/Header.svelte";
  import Section from "$lib/config/Section.svelte";
  import Row from "$lib/config/Row.svelte";
  import Switch from "$lib/config/Switch.svelte";
  import ValueField from "$lib/config/ValueField.svelte";
  import Popup from "$lib/config/Popup.svelte";
  import Menu from "$lib/config/Menu.svelte";
  import { phpForm as form, php, loadPhpModules, showLog } from "$lib/config/forms.svelte";

  const presets = [
    { label: "Everything (development)", value: "E_ALL" },
    { label: "Everything except deprecations", value: "E_ALL & ~E_DEPRECATED" },
    { label: "Production", value: "E_ALL & ~E_DEPRECATED & ~E_STRICT" },
    { label: "Errors only", value: "E_ERROR | E_PARSE | E_CORE_ERROR | E_COMPILE_ERROR" },
  ];

  onMount(() => {
    void form.load();
    if (!php.info) void loadPhpModules();
    if (!zones.length) call<string[]>("config_timezones").then((z) => (zones = z)).catch(() => {});
  });

  const d = $derived(form.draft);
  const info = $derived(form.info);
  const report = $derived(
    d && !presets.some((p) => p.value === d.errorReporting) ? [...presets, { label: `Custom: ${d.errorReporting}`, value: d.errorReporting }] : presets,
  );

  let filter = $state("");
  const shown = $derived.by(() => {
    const f = filter.trim().toLowerCase().replaceAll(" ", "_");
    return f ? zones.filter((z) => z.toLowerCase().includes(f)) : zones;
  });

  function development() {
    if (!d) return;
    Object.assign(d, { displayErrors: true, displayStartupErrors: true, errorReporting: "E_ALL", logErrors: true });
  }

  function production() {
    if (!d) return;
    Object.assign(d, { displayErrors: false, displayStartupErrors: false, errorReporting: "E_ALL & ~E_DEPRECATED & ~E_STRICT", logErrors: true });
  }

  async function save(restart: boolean) {
    await form.save(restart);
    void loadPhpModules();
  }
</script>

<Page {form} onsave={save}>
  <Header
    icon={CodeXml}
    tint="var(--tint-php)"
    top="var(--tint-php-top)"
    title={php.info?.version ? `PHP ${php.info.version}` : "PHP"}
    detail="PHP runs inside Apache, so Apache restarts to apply changes."
    file={info?.file}
    fileId="php"
    hasBackup={info?.hasBackup}
    dirty={form.dirty}
    onrestore={() => form.restore()}
    onerror={(m) => (form.error = m)}
  />

  {#if form.loadError && !d}
    <Section>
      <Row title="php.ini could not be read" detail={form.loadError}>
        <button type="button" class="cfg-btn" onclick={() => form.load(true)}>Try Again</button>
      </Row>
    </Section>
  {:else if d && info}
    <fieldset disabled={form.saving}>
      <Section title="Limits">
        <Row title="Memory limit" detail="The most memory one request may use. -1 means no limit." for="php-memory">
          <ValueField id="php-memory" bind:value={d.memoryLimit} placeholder="128M" problem={form.problem("memoryLimit")} />
        </Row>
        <Row title="Maximum upload size" detail="upload_max_filesize: the largest file a form can upload." for="php-upload">
          <ValueField id="php-upload" bind:value={d.uploadMaxFilesize} placeholder="2M" problem={form.problem("uploadMaxFilesize")} />
        </Row>
        <Row title="Maximum POST size" detail="post_max_size: the whole form, uploads included. Keep it above the upload size." for="php-post">
          <ValueField id="php-post" bind:value={d.postMaxSize} placeholder="8M" problem={form.problem("postMaxSize")} />
        </Row>
        <Row title="Maximum run time" detail="Seconds a script may run before PHP stops it. 0 means no limit." for="php-runtime">
          <ValueField id="php-runtime" bind:value={d.maxExecutionTime} width={80} problem={form.problem("maxExecutionTime")} />
          <span class="unit">seconds</span>
        </Row>
        <Row title="Maximum input time" detail="Seconds PHP waits for a request's data, such as a large upload. -1 uses the run time." for="php-inputtime">
          <ValueField id="php-inputtime" bind:value={d.maxInputTime} width={80} problem={form.problem("maxInputTime")} />
          <span class="unit">seconds</span>
        </Row>
        <Row title="Maximum input variables" detail="How many form fields a request may have." for="php-inputvars">
          <ValueField id="php-inputvars" bind:value={d.maxInputVars} width={80} problem={form.problem("maxInputVars")} />
          <span class="unit">fields</span>
        </Row>
      </Section>

      <Section title="Errors">
        {#snippet actions()}
          <button type="button" class="cfg-btn small" onclick={development} title="Show every error on the page and log it">Development</button>
          <button type="button" class="cfg-btn small" onclick={production} title="Hide errors from the page and only log them, like a live server">Production</button>
        {/snippet}
        <Row title="Show errors on the page" detail="display_errors. Handy while developing, never on a live site." for="php-display">
          <Switch id="php-display" bind:checked={d.displayErrors} />
        </Row>
        <Row title="Show startup errors" detail="display_startup_errors. Problems while PHP itself starts." for="php-startup">
          <Switch id="php-startup" bind:checked={d.displayStartupErrors} />
        </Row>
        <Row title="Report" detail="error_reporting: which kinds of problems PHP reports." for="php-report">
          <Popup id="php-report" bind:value={d.errorReporting} options={report} />
        </Row>
        <Row title="Write errors to a log" detail="log_errors." for="php-log">
          <Switch id="php-log" bind:checked={d.logErrors} />
        </Row>
        <Row
          title="Separate PHP error log"
          detail={d.separateErrorLog ? `PHP errors go to ${info.errorLog}.` : "Off: PHP errors go to Apache's error log."}
          for="php-separate"
        >
          <Switch id="php-separate" bind:checked={d.separateErrorLog} disabled={!d.logErrors} />
        </Row>
      </Section>

      <Section title="Other">
        <Row title="Time zone" detail="date.timezone, used by date() and DateTime.">
          <Menu label="Time zone" role="dialog">
            {#snippet trigger()}<span class="tz" class:bad={!!form.problem("timezone")}>{d.timezone || "Not set"}</span>{/snippet}
            {#snippet children(close)}
              {#if info.systemTimezone}
                <button type="button" onclick={() => { d.timezone = info.systemTimezone ?? ""; close(); }}>This computer ({info.systemTimezone})</button>
              {/if}
              <button type="button" onclick={() => { d.timezone = "UTC"; close(); }}>UTC</button>
              <button type="button" onclick={() => { d.timezone = ""; close(); }}>Not set</button>
              <hr />
              <input class="search" type="search" placeholder="Search" aria-label="Search time zones" bind:value={filter} />
              <div class="zones">
                {#each shown as z (z)}
                  <button type="button" onclick={() => { d.timezone = z; filter = ""; close(); }}>{z.replaceAll("_", " ")}</button>
                {/each}
              </div>
            {/snippet}
          </Menu>
        </Row>
        <Row title="Short open tags" detail="Treat <? as <?php. Only some old projects need this." for="php-short">
          <Switch id="php-short" bind:checked={d.shortOpenTag} />
        </Row>
        <Row title="OPcache" detail="Keeps compiled scripts in memory. Changes to files are still picked up." for="php-opcache">
          <Switch id="php-opcache" bind:checked={d.opcache} />
        </Row>
        <Row title="Session lifetime" detail="session.gc_maxlifetime. 1440 is 24 minutes." for="php-session">
          <ValueField id="php-session" bind:value={d.sessionLifetime} placeholder="1440" width={80} problem={form.problem("sessionLifetime")} />
          <span class="unit">seconds</span>
        </Row>
      </Section>

      <Section
        title="Extensions ({php.info?.extensions.length ?? 0})"
        footer="These are the extensions PHP loads now. Turn more on with an extension= line in php.ini."
      >
        <div class="extensions">
          {#if php.info}
            {php.info.extensions.join("  ·  ")}
          {:else if php.error}
            {php.error}
          {:else}
            Loading…
          {/if}
        </div>
      </Section>

      <Section>
        <Row title="Error log" detail={d.separateErrorLog ? "PHP Errors" : "Apache Errors"}>
          <button type="button" class="cfg-btn" onclick={() => showLog(d.separateErrorLog ? "php" : "apacheError")}>Show</button>
        </Row>
      </Section>
    </fieldset>
  {/if}
</Page>

<style>
  fieldset { margin: 0; padding: 0; border: none; min-width: 0; }
  .unit { width: 52px; color: var(--text-2); font-size: 13px; }
  .tz { max-width: 280px; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .bad { color: var(--red); }
  .extensions { padding: 10px; font-family: var(--mono); font-size: 10px; line-height: 13px; color: var(--text-2); user-select: text; word-spacing: 1px; }
  .search {
    display: block;
    width: calc(100% - 10px);
    height: 24px;
    min-height: 0;
    margin: 2px 5px 5px;
    padding: 0 8px;
    border: 1px solid var(--border);
    border-radius: 6px;
    background: var(--input-bg);
    font-size: 13px;
  }
  .zones { max-height: 260px; overflow: auto; }
</style>
