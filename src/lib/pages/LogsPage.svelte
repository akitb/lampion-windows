<script lang="ts">
  // The Logs page (LogsView.swift): Lampion's own activity and the server logs, followed live. Source
  // chips, a level filter with counts, search, Follow and a "…" menu above the lines; a status bar below.
  // Counting and filtering run only when the lines, the filter or the search change.
  import { tick, type Component } from "svelte";
  import {
    ArrowDownToLine,
    ArrowLeftRight,
    CircleEllipsis,
    CircleX,
    CodeXml,
    Database,
    History,
    Inbox,
    ListFilter,
    OctagonAlert,
    Search,
    SquareMenu,
    Turtle,
  } from "@lucide/svelte";
  import { writeText } from "@tauri-apps/plugin-clipboard-manager";
  import { revealItemInDir } from "@tauri-apps/plugin-opener";
  import { call, on } from "$lib/api";
  import { app } from "$lib/state.svelte";
  import { clearActivity, logs } from "$lib/logs.svelte";
  import { fileRow, fileSize, missingHint, sources, sourceTitles, type Row } from "$lib/logs/rows";
  import LogList from "$lib/logs/LogList.svelte";
  import LogMenu, { type MenuItem } from "$lib/logs/LogMenu.svelte";
  import Dialog from "$lib/config/Dialog.svelte";
  import type { LogSource, LogSourceInfo, LogUpdate } from "$lib/types";

  const icons: Record<LogSource, Component<any>> = {
    activity: History,
    apacheError: OctagonAlert,
    apacheAccess: ArrowLeftRight,
    php: CodeXml,
    mariadb: Database,
    mariadbSlow: Turtle,
    mariadbGeneral: SquareMenu,
  };
  /** The SF Symbols with a filled variant, which the selected chip shows. */
  const filled = new Set<LogSource>(["apacheError", "mariadb", "mariadbSlow", "mariadbGeneral"]);
  const MAX_LINES = 5000;

  type Filter = "all" | "warnings" | "errors";
  let filter = $state<Filter>("all");
  let search = $state("");
  let follow = $state(true);

  const source = $derived(logs.source);
  const isFile = $derived(source !== "activity");

  // The followed server log.
  let fileLines = $state.raw<Row[]>([]);
  let exists = $state(true);
  let size = $state(0);
  let path = $state("");
  let loaded = $state(false);
  let failure = $state<string | null>(null);
  let seq = 0;

  function apply(u: LogUpdate) {
    if (u.source !== logs.source || u.seq <= seq) return;
    seq = u.seq;
    const added = u.lines.map(fileRow);
    if (u.reset) fileLines = added;
    else if (added.length) {
      const next = fileLines.concat(added);
      fileLines = next.length > MAX_LINES ? next.slice(-MAX_LINES) : next;
    }
    exists = u.exists;
    size = u.size;
    path = u.path;
    loaded = true;
  }

  // Follows the chosen server log while the page shows it; updates that came before the first answer wait.
  $effect(() => {
    const s = logs.source;
    fileLines = [];
    loaded = false;
    failure = null;
    seq = 0;
    if (s === "activity") return;
    let gone = false;
    let waiting: LogUpdate[] | null = [];
    const off = on<LogUpdate>("logs://update", (u) => (waiting ? waiting.push(u) : apply(u)));
    call<LogUpdate>("logs_follow", { source: s }).then(
      (state) => {
        if (gone) return;
        apply(state);
        const early = waiting ?? [];
        waiting = null;
        early.forEach(apply);
      },
      (e) => {
        if (!gone) failure = String(e);
      },
    );
    return () => {
      gone = true;
      void off.then((f) => f());
      void call("logs_unfollow", { source: s });
    };
  });

  // Where each log is and whether it exists (greys the chips of missing files).
  let infos = $state<LogSourceInfo[]>([]);
  $effect(() => {
    void logs.source;
    void exists;
    call<LogSourceInfo[]>("logs_sources").then((i) => (infos = i), () => {});
  });
  const info = (s: LogSource) => infos.find((i) => i.source === s);

  const all = $derived(isFile ? fileLines : logs.activity);
  const counts = $derived.by(() => {
    let warnings = 0;
    let errors = 0;
    for (const r of all) {
      if (r.level === "error") errors++;
      else if (r.level === "warn") warnings++;
    }
    return { warnings, errors };
  });
  const shown = $derived.by(() => {
    if (filter === "all" && !search) return all;
    const q = search.toLowerCase();
    return all.filter((r) => (filter === "all" || r.level === "error" || (filter === "warnings" && r.level === "warn")) && (!q || r.lower.includes(q)));
  });

  // The chosen chip stays in view in a narrow window.
  const chips: Partial<Record<LogSource, HTMLButtonElement>> = {};
  $effect(() => {
    const reduce = matchMedia("(prefers-reduced-motion: reduce)").matches;
    chips[logs.source]?.scrollIntoView({ block: "nearest", inline: "center", behavior: reduce ? "instant" : "smooth" });
  });

  const levels: { id: Filter; label: () => string }[] = [
    { id: "all", label: () => "All" },
    { id: "warnings", label: () => `Warnings ${counts.warnings + counts.errors}` },
    { id: "errors", label: () => `Errors ${counts.errors}` },
  ];

  function levelKey(e: KeyboardEvent) {
    const i = levels.findIndex((l) => l.id === filter);
    const n = e.key === "ArrowRight" || e.key === "ArrowDown" ? i + 1 : e.key === "ArrowLeft" || e.key === "ArrowUp" ? i - 1 : i;
    if (n === i) return;
    e.preventDefault();
    filter = levels[(n + levels.length) % levels.length].id;
    const group = (e.currentTarget as HTMLElement).parentElement;
    void tick().then(() => group?.querySelector<HTMLElement>('[aria-checked="true"]')?.focus());
  }

  // ---- Menus and actions ----

  let problem = $state<string | null>(null);
  let clearProblem = $state<string | null>(null);
  let confirmClear = $state(false);
  let moreButton: HTMLButtonElement | undefined = $state();
  let more: LogMenu | undefined = $state();
  let rowMenu: LogMenu | undefined = $state();
  let rowItems = $state<MenuItem[]>([]);

  async function attempt(f: () => Promise<unknown>) {
    try {
      await f();
    } catch (e) {
      problem = String(e);
    }
  }

  const copy = (lines: Row[]) => attempt(() => writeText(lines.map((r) => r.raw).join("\r\n")));
  const reveal = (file: string) => attempt(() => revealItemInDir(file));

  const moreItems: MenuItem[] = $derived([
    { label: "Copy Shown Lines", run: () => void copy(shown) },
    isFile
      ? { label: "Show in Explorer", run: () => void reveal(path), disabled: !exists }
      : { label: "Show Lampion.log in Explorer", run: () => void reveal(info("activity")?.path ?? ""), disabled: !info("activity")?.exists },
    ...(isFile ? [{ label: "Reload", run: () => void attempt(async () => apply(await call<LogUpdate>("logs_reload", { source }))) }] : []),
    null,
    { label: isFile ? "Clear Log File…" : "Clear List", run: askClear, disabled: all.length === 0 },
  ]);

  function askClear() {
    if (isFile) confirmClear = true;
    else void attempt(clearActivity);
  }

  async function clearFile() {
    confirmClear = false;
    try {
      const u = await call<LogUpdate | null>("logs_clear", { source });
      if (u) apply(u);
    } catch (e) {
      clearProblem = String(e);
    }
  }

  function openRowMenu(row: Row, e: MouseEvent) {
    rowItems = [
      { label: "Copy Line", run: () => void copy([row]) },
      { label: `Search for “${row.tag ?? ""}”`, run: () => (search = row.tag ?? ""), disabled: !row.tag },
    ];
    void rowMenu?.show({ x: e.clientX, y: e.clientY });
  }

  /** The folder part of the path is cut first, so the file name stays readable. */
  const pathParts = $derived.by(() => {
    const cut = Math.max(path.lastIndexOf("\\"), path.lastIndexOf("/")) + 1;
    return { head: path.slice(0, cut), tail: path.slice(cut) };
  });

  const Icon = $derived(icons[source]);
</script>

<div class="logs">
  <div class="sources" role="toolbar" aria-label="Log">
    {#each sources as s (s)}
      {@const ChipIcon = icons[s]}
      <button
        type="button"
        class="chip"
        class:selected={source === s}
        class:missing={info(s)?.exists === false}
        aria-pressed={source === s}
        title={s === "activity" ? "Everything Lampion does: starting, stopping, setup and errors." : info(s)?.path}
        bind:this={chips[s]}
        onclick={() => (logs.source = s)}
      >
        <ChipIcon size={14} strokeWidth={1.6} class={source === s && filled.has(s) ? "filled" : undefined} />
        {sourceTitles[s]}
      </button>
      {#if s === "activity" || s === "php"}<span class="separator" aria-hidden="true"></span>{/if}
    {/each}
  </div>
  <hr />
  <div class="controls">
    <div class="levels" role="radiogroup" aria-label="Show">
      {#each levels as l (l.id)}
        <button
          type="button"
          role="radio"
          class:on={filter === l.id}
          aria-checked={filter === l.id}
          tabindex={filter === l.id ? 0 : -1}
          onclick={() => (filter = l.id)}
          onkeydown={levelKey}>{l.label()}</button
        >
      {/each}
    </div>
    <label class="search">
      <Search size={13} strokeWidth={2} aria-hidden="true" />
      <input
        type="text"
        placeholder="Search"
        aria-label="Search the lines"
        spellcheck="false"
        bind:value={search}
        onkeydown={(e) => {
          if (e.key === "Escape" && search) {
            search = "";
            e.stopPropagation();
          }
        }}
      />
      {#if search}
        <button type="button" class="clear" title="Clear the search" aria-label="Clear the search" onclick={() => (search = "")}>
          <CircleX size={14} strokeWidth={2} />
        </button>
      {/if}
    </label>
    <span class="grow"></span>
    <button type="button" class="follow" class:on={follow} aria-pressed={follow} title="Keep the newest lines in view" onclick={() => (follow = !follow)}>
      <ArrowDownToLine size={14} strokeWidth={1.8} aria-hidden="true" />
      Follow
    </button>
    <button
      type="button"
      class="more"
      title="More"
      aria-label="More"
      aria-haspopup="menu"
      bind:this={moreButton}
      onclick={(e) => moreButton && more?.show(moreButton, e.detail === 0)}
    >
      <CircleEllipsis size={16} strokeWidth={1.5} />
    </button>
  </div>
  <hr />
  {#if isFile && failure}
    <div class="empty">
      <Icon size={40} strokeWidth={1.3} />
      <h2>{sourceTitles[source]} could not be read</h2>
      <p>{failure}</p>
    </div>
  {:else if isFile && !loaded}
    <div class="empty"></div>
  {:else if isFile && !exists}
    <div class="empty">
      <Icon size={40} strokeWidth={1.3} />
      <h2>{sourceTitles[source]}: no log file yet</h2>
      <p>{missingHint(source)}</p>
      {#if source === "php"}
        <button type="button" onclick={() => (app.pane = "php")}>PHP Settings</button>
      {:else if source === "mariadbSlow" || source === "mariadbGeneral"}
        <button type="button" onclick={() => (app.pane = "mariadb")}>MariaDB Settings</button>
      {/if}
    </div>
  {:else if shown.length === 0}
    <div class="empty">
      {#if all.length === 0}<Inbox size={40} strokeWidth={1.3} />{:else if search}<Search size={40} strokeWidth={1.3} />{:else}<ListFilter size={40} strokeWidth={1.3} />{/if}
      <h2>{all.length === 0 ? "The log is empty" : "No matching lines"}</h2>
      <p>{all.length === 0 ? "New lines appear here as soon as they are written." : "Try another search or show all levels."}</p>
    </div>
  {:else}
    <LogList rows={shown} highlight={search} {follow} onrowmenu={openRowMenu} />
  {/if}
  <hr />
  <footer class="status">
    <span>{shown.length === all.length ? `${all.length} ${all.length === 1 ? "line" : "lines"}` : `${shown.length} of ${all.length} lines`}</span>
    {#if counts.errors > 0}
      <span class="count error" title="Errors">
        <svg width="11" height="11" viewBox="0 0 16 16" aria-hidden="true"
          ><path fill="currentColor" d="M5.1.5h5.8l4.6 4.6v5.8l-4.6 4.6H5.1L.5 10.9V5.1z" /><path
            d="M5.6 5.6l4.8 4.8m0-4.8l-4.8 4.8"
            class="cut"
            stroke-width="1.7"
            stroke-linecap="round"
          /></svg
        >
        {counts.errors}
      </span>
    {/if}
    {#if counts.warnings > 0}
      <span class="count warning" title="Warnings">
        <svg width="12" height="11" viewBox="0 0 16 15" aria-hidden="true"
          ><path fill="currentColor" d="M8 .8c.5 0 .9.3 1.2.7l6.4 11.2c.5.9-.1 2-1.2 2H1.6c-1.1 0-1.7-1.1-1.2-2L6.8 1.5c.3-.4.7-.7 1.2-.7z" /><path
            d="M8 5v4.2"
            class="cut"
            stroke-width="1.7"
            stroke-linecap="round"
          /><circle class="cut-dot" cx="8" cy="11.9" r="1" /></svg
        >
        {counts.warnings}
      </span>
    {/if}
    <span class="grow"></span>
    {#if isFile && path}
      <span class="path" title={path}><span class="head">{pathParts.head}</span><span class="tail">{pathParts.tail}</span></span>
      {#if exists}<span>{fileSize(size)}</span>{/if}
    {/if}
    <span class="live" title={isFile ? "The file is checked every second" : "New events appear as they happen"}><span class="dot"></span>Live</span>
  </footer>
</div>

<LogMenu bind:this={more} items={moreItems} label="More" />
<LogMenu bind:this={rowMenu} items={rowItems} label="Line" />

<Dialog
  bind:open={confirmClear}
  title="Clear {sourceTitles[source]}?"
  message="This empties {pathParts.tail} on disk. It cannot be undone."
  cancel="Cancel"
  confirm="Clear"
  destructive
  onconfirm={clearFile}
/>
<Dialog bind:open={() => clearProblem !== null, (v) => { if (!v) clearProblem = null; }} title="The log could not be cleared" message={clearProblem ?? ""} />
<Dialog bind:open={() => problem !== null, (v) => { if (!v) problem = null; }} title="Something went wrong" message={problem ?? ""} />

<style>
  /* Sizes are the macOS screenshots' measurements halved (they are 2x). Colours only the Logs page uses:
     the macOS system colours of tags and marks (dark values measured from the screenshots where shown). */
  .logs {
    --log-line: color-mix(in srgb, var(--text) 12%, transparent);
    --log-orange: var(--orange);
    --log-blue: var(--accent);
    --log-teal: #30b0c7;
    --log-indigo: #5856d6;
    --log-brown: #a2845e;
    --log-cyan: #32ade6;
    --log-mint: #00c7be;
    --log-purple: #af52de;
    --log-pink: #ff2d55;
    --log-yellow: #ffcc00;
    display: flex;
    flex-direction: column;
    height: 100%;
  }
  @media (prefers-color-scheme: dark) {
    .logs {
      --log-orange: #f09748;
      --log-teal: #40c8e0;
      --log-indigo: #5e5ce6;
      --log-brown: #ac8e68;
      --log-cyan: #6dd0f9;
      --log-mint: #63d7c3;
      --log-purple: #bf5af2;
      --log-pink: #ff375f;
      --log-yellow: #ffd60a;
    }
  }
  hr { flex: none; height: 1px; margin: 0; border: none; background: var(--log-line); }
  .grow { flex: 1; }

  .sources {
    flex: none;
    display: flex;
    align-items: center;
    gap: 6px;
    padding: 11px 16px;
    overflow-x: auto;
    scrollbar-width: none;
    /* Narrow windows cannot show every chip: the edges fade as a hint. */
    mask-image: linear-gradient(to right, transparent 0, #000 2%, #000 97%, transparent 100%);
  }
  .chip {
    flex: none;
    display: inline-flex;
    align-items: center;
    gap: 7px;
    height: 24px;
    min-height: 0;
    padding: 0 10px;
    border: none;
    border-radius: 12px;
    background: color-mix(in srgb, var(--text) 5%, transparent);
    box-shadow: none;
    color: var(--text);
    font-size: 12px;
    white-space: nowrap;
    transition: background-color var(--fade), color var(--fade);
  }
  .chip:hover:not(.selected) { background: color-mix(in srgb, var(--text) 9%, transparent); }
  .chip.missing { color: var(--text-2); }
  .chip.selected { background: color-mix(in srgb, var(--accent) 16%, transparent); color: var(--accent); font-weight: 600; }
  /* A filled glyph: the shape in the text colour, its inner lines in the chip's colour. */
  .chip.selected :global(svg.filled) { fill: currentColor; stroke: color-mix(in srgb, var(--accent) 16%, var(--bg)); }
  .separator { flex: none; width: 1px; height: 16px; margin: 0 2px; background: var(--log-line); }

  .controls { flex: none; display: flex; align-items: center; gap: 10px; height: 40px; padding: 0 16px; }
  .levels {
    flex: none;
    display: inline-grid;
    grid-auto-flow: column;
    grid-auto-columns: 1fr;
    height: 24px;
    border-radius: 6px;
    background: color-mix(in srgb, var(--text) 9%, transparent);
  }
  .levels button {
    position: relative;
    height: 24px;
    min-height: 0;
    padding: 0 13px;
    border: none;
    border-radius: 6px;
    background: none;
    box-shadow: none;
    color: var(--text);
    font-size: 13px;
    white-space: nowrap;
  }
  .levels button:hover:not(.on) { background: none; }
  .levels button + button::before {
    content: "";
    position: absolute;
    top: 5px;
    bottom: 5px;
    left: 0;
    width: 1px;
    background: color-mix(in srgb, var(--text) 30%, transparent);
  }
  .levels button.on::before, .levels button.on + button::before { display: none; }
  .levels button.on { background: var(--accent); color: var(--accent-text); }

  .search {
    flex: 0 1 280px;
    min-width: 120px;
    display: flex;
    align-items: center;
    gap: 5px;
    height: 24px;
    padding: 0 6px 0 8px;
    border-radius: 6px;
    background: color-mix(in srgb, var(--text) 6%, transparent);
    color: var(--text-2);
  }
  .search:has(input:focus-visible) { outline: 2px solid var(--accent); outline-offset: 1px; }
  .search input {
    flex: 1;
    min-width: 0;
    min-height: 0;
    height: 22px;
    padding: 0;
    border: none;
    background: none;
    box-shadow: none;
    color: var(--text);
    font-size: 13px;
    outline: none;
  }
  .search input::placeholder { color: var(--text-2); }
  .search .clear {
    display: grid;
    place-items: center;
    width: 16px;
    height: 16px;
    min-height: 0;
    padding: 0;
    border: none;
    background: none;
    box-shadow: none;
    color: var(--text-2);
  }
  /* xmark.circle.fill: a filled circle, its cross cut out. */
  .search .clear :global(svg) { fill: currentColor; stroke: var(--bg); }

  .follow {
    flex: none;
    display: inline-flex;
    align-items: center;
    gap: 8px;
    height: 24px;
    min-height: 0;
    padding: 0 12px;
    border: none;
    border-radius: 6px;
    background: var(--control);
    box-shadow: none;
    color: var(--text);
    font-size: 13px;
  }
  .follow.on { background: var(--accent); color: var(--accent-text); }
  .follow.on:hover:not(:disabled) { background: var(--accent); filter: brightness(1.08); }
  .more {
    flex: none;
    display: grid;
    place-items: center;
    width: 21px;
    height: 24px;
    min-height: 0;
    padding: 0;
    border: none;
    border-radius: 5px;
    background: none;
    box-shadow: none;
    color: var(--text);
  }
  .more:hover:not(:disabled) { background: var(--hover); }

  .empty {
    flex: 1;
    min-height: 0;
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    gap: 6px;
    padding: 24px;
    color: var(--text-2);
    text-align: center;
  }
  .empty h2 { margin: 6px 0 0; color: var(--text); font-size: 17px; font-weight: 600; }
  .empty p { max-width: 420px; margin: 0; font-size: 13px; line-height: 17px; user-select: text; }
  .empty button { margin-top: 8px; height: 24px; min-height: 0; padding: 0 12px; border: none; border-radius: 5px; background: var(--control); box-shadow: none; }

  .status {
    flex: none;
    display: flex;
    align-items: center;
    gap: 14px;
    height: 25px;
    padding: 0 16px;
    background: var(--bg);
    color: var(--text-2);
    font-size: 11px;
    white-space: nowrap;
  }
  .count { display: inline-flex; align-items: center; gap: 5px; font-variant-numeric: tabular-nums; }
  .count.error { color: var(--red); }
  .count.warning { color: var(--orange); }
  .count .cut { fill: none; stroke: var(--bg); }
  .count .cut-dot { fill: var(--bg); }
  .path { display: flex; min-width: 0; user-select: text; }
  .path .head { min-width: 0; overflow: hidden; text-overflow: ellipsis; }
  .path .tail { flex: none; }
  .live { display: inline-flex; align-items: center; gap: 4px; }
  .dot { width: 6px; height: 6px; border-radius: 50%; background: var(--green); }
</style>
