<script lang="ts">
  // The Query tab: Run, target database, history and clear; the editor; and the results, which can be
  // folded away (remembered). Port of QueryView.swift.
  import { ChevronDown, CircleCheck, Copy, Database, Eraser, History, OctagonX, Play, SquarePlay, TriangleAlert } from "@lucide/svelte";
  import { fade } from "svelte/transition";
  import { activity } from "./dbapi.svelte";
  import { db, server } from "./dbstate.svelte";
  import Empty from "./Empty.svelte";
  import Grid from "./Grid.svelte";
  import { dropdown, type MenuItem } from "./menu.svelte";
  import { clearHistory, isSystem, loadHistory, query, runQuery, ui } from "./model.svelte";
  import SqlEditor from "./SqlEditor.svelte";
  import { copy, count, rows as rowsText, summary } from "./util";

  let box: HTMLDivElement | undefined = $state();
  let busy = $state(false);

  const target = $derived(query.database ?? db.database);
  const shown = $derived(query.sets[query.shown]);

  $effect(() => {
    loadHistory();
  });

  // The spinner shows only when a statement runs longer than a blink.
  $effect(() => {
    if (activity.busy > 0) {
      const t = setTimeout(() => (busy = true), 250);
      return () => clearTimeout(t);
    }
    busy = false;
  });

  function databaseMenu(e: MouseEvent) {
    const item = (name: string): MenuItem => ({ label: name, checked: query.database === name, action: () => (query.database = name) });
    const user = server.databases.filter((d) => !isSystem(d));
    const system = server.databases.filter(isSystem);
    const items: MenuItem[] = [{ label: `Selected in sidebar (${db.database ?? "none"})`, checked: query.database === null, action: () => (query.database = null) }];
    if (user.length) items.push({ separator: true }, ...user.map(item));
    if (system.length) items.push({ separator: true }, ...system.map(item));
    dropdown(e.currentTarget as HTMLElement, items, { keyboard: e.detail === 0 });
  }

  function historyMenu(e: MouseEvent) {
    const items: MenuItem[] = query.history.length
      ? query.history.map((q) => ({ label: summary(q), action: () => (query.text = q) }))
      : [{ label: "No queries yet", disabled: true, action: () => {} }];
    items.push({ separator: true }, { label: "Clear History", disabled: !query.history.length, action: clearHistory });
    dropdown(e.currentTarget as HTMLElement, items, { keyboard: e.detail === 0 });
  }

  /** Dragging the line between the editor and the results. */
  function resize(e: PointerEvent) {
    if (!box || !ui.resultsOpen) return;
    e.preventDefault();
    const el = e.currentTarget as HTMLElement;
    el.setPointerCapture(e.pointerId);
    const rect = box.getBoundingClientRect();
    const move = (m: PointerEvent) => {
      const editor = Math.min(rect.height - 150 - 31, Math.max(110, m.clientY - rect.top));
      query.split = editor / rect.height;
    };
    const up = () => {
      el.removeEventListener("pointermove", move);
      el.removeEventListener("pointerup", up);
    };
    el.addEventListener("pointermove", move);
    el.addEventListener("pointerup", up);
  }
</script>

<div class="query">
  <div class="dbbar top">
    <button type="button" class="run" disabled={!query.text.trim()} onclick={runQuery} title="Run the SQL in the editor (Ctrl+R)">
      <Play size={12} strokeWidth={0} fill="currentColor" /> Run
    </button>
    <button type="button" class="menubtn" onclick={databaseMenu} aria-haspopup="menu"
            title={query.database === null ? "The query runs in the database selected in the sidebar. Choose one to always use it." : `The query runs in ${query.database}`}>
      <Database size={14} strokeWidth={1.8} />
      <span class="label">{target ?? "No database"}</span>
      <ChevronDown size={12} strokeWidth={2.4} />
    </button>
    <button type="button" class="menubtn" onclick={historyMenu} aria-haspopup="menu" title="Recently run queries">
      <History size={14} strokeWidth={1.8} />
      <span class="label">History</span>
      <ChevronDown size={12} strokeWidth={2.4} />
    </button>
    <button type="button" class="iconbtn" disabled={!query.text} onclick={() => (query.text = "")} title="Clear Editor" aria-label="Clear Editor"><Eraser size={15} strokeWidth={1.7} /></button>
    <span class="spacer"></span>
    <span class="spinner" class:on={busy} aria-hidden={!busy} title="Working…"></span>
    {#key query.version}
      {#if query.message || query.error}
        <span class="status" in:fade={{ duration: 200 }} title="Result of the last run">
          {#if query.error}
            <span class="bad solid"><OctagonX size={16} strokeWidth={2.2} /></span>
            Failed{query.message ? ` · ${query.message}` : ""}
          {:else}
            <span class="good solid"><CircleCheck size={16} strokeWidth={2.2} /></span>
            {query.message}
          {/if}
        </span>
      {/if}
    {/key}
  </div>

  <div class="body" bind:this={box}>
    <div class="editor" style={ui.resultsOpen ? `flex:0 0 ${query.split * 100}%` : "flex:1"}>
      <SqlEditor />
    </div>
    <!-- svelte-ignore a11y_no_static_element_interactions -->
    <div class="splitter" class:active={ui.resultsOpen} onpointerdown={resize}></div>
    <button type="button" class="resultsbar" onclick={() => (ui.resultsOpen = !ui.resultsOpen)} aria-expanded={ui.resultsOpen}
            title={ui.resultsOpen ? "Fold the results away" : "Show the results"}>
      <span class="chev" class:closed={!ui.resultsOpen}><ChevronDown size={11} strokeWidth={2.6} /></span>
      <strong>Result</strong>
      {#if query.error}
        <span class="err"><span class="solid"><TriangleAlert size={12} strokeWidth={2.2} /></span> Error</span>
      {:else if shown}
        <span class="sum">{query.sets.length > 1 ? `${query.sets.length} results · ${rowsText(shown.rows.length)}` : `${rowsText(shown.rows.length)} · ${shown.columns.length === 1 ? "1 column" : `${count(shown.columns.length)} columns`}`}</span>
      {:else if query.version > 0 && query.message}
        <span class="sum">{query.message}</span>
      {:else}
        <span class="sum faint">Nothing run yet</span>
      {/if}
      <span class="spacer"></span>
      <span class="hide">{ui.resultsOpen ? "Hide" : "Show"}</span>
    </button>
    {#if ui.resultsOpen}
      <div class="pane">
        {#key query.version}
          <div class="fill" in:fade={{ duration: 200 }}>
            {#if query.error}
              <div class="errorcard" role="alert">
                <span class="bad solid card"><TriangleAlert size={20} strokeWidth={2} /></span>
                <div class="text">
                  <strong>The query failed</strong>
                  <p>{query.error}</p>
                  <small>When the server names the location, it is marked in the editor.</small>
                </div>
                <button type="button" class="iconbtn" onclick={() => copy(query.error ?? "")} title="Copy the error message" aria-label="Copy the error message"><Copy size={15} strokeWidth={1.7} /></button>
              </div>
            {:else if shown}
              {#if query.sets.length > 1}
                <div class="dbbar top chips" role="tablist" aria-label="Results">
                  {#each query.sets as set, i (i)}
                    <button type="button" class="chip" class:sel={i === query.shown} role="tab" aria-selected={i === query.shown} onclick={() => (query.shown = i)}>
                      <span class="n">Result {i + 1}</span><span class="dot">·</span><span class="r">{rowsText(set.rows.length)}</span>
                    </button>
                  {/each}
                </div>
              {/if}
              <div class="grid">
                <Grid columns={shown.columns} rows={shown.rows} version={query.version * 100 + query.shown} scrollKey={`${query.version}-${query.shown}`} label="Result" />
              </div>
            {:else if query.version > 0 && query.message}
              <Empty icon={CircleCheck} title="Query Finished" description={query.message} />
            {:else}
              <Empty icon={SquarePlay} title="Run a query" description={"Type SQL above and press Ctrl+R.\nPress Tab to complete table and column names."} />
            {/if}
          </div>
        {/key}
      </div>
    {/if}
  </div>
</div>

<style>
  .query { display: flex; flex-direction: column; height: 100%; }
  .run {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    height: 23px;
    min-height: 0;
    padding: 0 14px 0 12px;
    border: none;
    border-radius: 6px;
    background: var(--accent);
    box-shadow: none;
    color: var(--accent-text);
    font-size: 13px;
  }
  .run:hover:not(:disabled) { background: var(--accent); filter: brightness(1.08); }
  .menubtn {
    display: inline-flex;
    align-items: center;
    gap: 7px;
    height: 23px;
    min-height: 0;
    max-width: 260px;
    padding: 0 7px 0 9px;
    border: none;
    border-radius: 6px;
    background: var(--db-menu-btn);
    box-shadow: none;
    color: var(--text);
    font-size: 13px;
  }
  .menubtn:hover:not(:disabled) { background: color-mix(in srgb, var(--text) 5%, var(--db-menu-btn)); }
  .menubtn .label { overflow: hidden; text-overflow: ellipsis; white-space: nowrap; margin-right: 16px; }
  .menubtn :global(svg) { flex: none; }
  .spacer { flex: 1; }
  .spinner { width: 14px; height: 14px; border: 2px solid var(--border-strong); border-top-color: var(--text-2); border-radius: 50%; opacity: 0; transition: opacity 0.15s; flex: none; }
  .spinner.on { opacity: 1; animation: spin 0.8s linear infinite; }
  @keyframes spin { to { transform: rotate(360deg); } }
  .status { display: inline-flex; align-items: center; gap: 5px; font-size: 13px; color: var(--text-2); font-variant-numeric: tabular-nums; white-space: nowrap; overflow: hidden; text-overflow: ellipsis; user-select: text; }
  .good { display: inline-flex; color: var(--green); }
  .solid { display: inline-flex; --cut: var(--db-bar); }
  .solid :global(svg) { fill: currentColor; stroke: var(--cut); }
  .solid.card { --cut: color-mix(in srgb, var(--red) 8%, var(--db-content)); }
  .bad { display: inline-flex; color: var(--red); }
  .body { display: flex; flex-direction: column; flex: 1; min-height: 0; }
  .editor { min-height: 110px; overflow: hidden; }
  .splitter { flex: none; height: 2px; background: var(--db-sep); }
  .splitter.active { cursor: row-resize; }
  .resultsbar {
    display: flex;
    align-items: center;
    gap: 8px;
    height: 30px;
    flex: none;
    min-height: 0;
    padding: 0 12px;
    border: none;
    border-radius: 0;
    background: var(--db-bar);
    box-shadow: none;
    color: var(--text);
    text-align: left;
  }
  .resultsbar:hover:not(:disabled) { background: var(--db-bar); }
  .resultsbar strong { font-size: 13px; font-weight: 600; }
  .chev { display: inline-flex; width: 12px; color: var(--text-2); transition: transform var(--fade); }
  .chev.closed { transform: rotate(-90deg); }
  .sum { font-size: 11px; color: var(--text-2); font-variant-numeric: tabular-nums; white-space: nowrap; overflow: hidden; text-overflow: ellipsis; }
  .sum.faint { color: var(--text-3); }
  .err { display: inline-flex; align-items: center; gap: 4px; font-size: 11px; color: var(--red); }
  .hide { font-size: 11px; color: var(--text-2); }
  .pane { position: relative; flex: 1; min-height: 150px; background: var(--db-content); }
  .fill { position: absolute; inset: 0; display: flex; flex-direction: column; }
  .grid { flex: 1; min-height: 0; }
  .chips { gap: 6px; overflow-x: auto; }
  .chip {
    display: inline-flex;
    align-items: center;
    gap: 4px;
    height: 24px;
    min-height: 0;
    padding: 0 10px;
    border: 1px solid color-mix(in srgb, var(--text) 8%, transparent);
    border-radius: 12px;
    background: color-mix(in srgb, var(--text) 5%, transparent);
    box-shadow: none;
    color: var(--text);
    font-size: 13px;
    white-space: nowrap;
  }
  .chip .n { font-weight: 500; }
  .chip .dot { color: var(--text-3); }
  .chip .r { color: var(--text-2); font-variant-numeric: tabular-nums; }
  .chip.sel { border-color: color-mix(in srgb, var(--accent) 35%, transparent); background: color-mix(in srgb, var(--accent) 16%, transparent); color: var(--accent); }
  .chip.sel .r { color: var(--accent); }
  .errorcard {
    display: flex;
    align-items: flex-start;
    gap: 12px;
    margin: 16px;
    padding: 14px;
    border: 1px solid color-mix(in srgb, var(--red) 25%, transparent);
    border-radius: 10px;
    background: color-mix(in srgb, var(--red) 8%, transparent);
  }
  .errorcard .text { flex: 1; min-width: 0; display: flex; flex-direction: column; gap: 6px; }
  .errorcard strong { font-size: 13px; font-weight: 600; }
  .errorcard p { margin: 0; font-family: var(--mono); font-size: 12px; line-height: 15px; color: var(--text); white-space: pre-wrap; overflow-wrap: anywhere; user-select: text; }
  .errorcard small { font-size: 11px; color: var(--text-2); }
</style>
