<script lang="ts" module>
  // Every statement the window sent (the `database://sql` event). Only the console reads this, so a new
  // statement redraws the console and nothing else.
  import { on } from "$lib/api";
  import type { SqlLogEntry } from "./types";

  type Line = { id: number; text: string; error: boolean };

  const log = $state({ lines: [] as Line[] });
  let next = 0;

  function flat(text: string): string {
    const one = text.split(/\r?\n/).map((l) => l.trim()).filter(Boolean).join(" ");
    return one.length > 600 ? one.slice(0, 600) + "…" : one;
  }

  function add(entry: SqlLogEntry) {
    const lines = [...log.lines, { id: next++, text: flat(entry.sql), error: false }];
    if (entry.error) lines.push({ id: next++, text: flat(entry.error), error: true });
    log.lines = lines.length > 300 ? lines.slice(lines.length - 300) : lines;
  }

  let listening = false;

  /** Starts collecting statements (once per window). */
  export function listenConsole() {
    if (listening) return;
    listening = true;
    on<SqlLogEntry>("database://sql", add);
  }
</script>

<script lang="ts">
  // The SQL console at the bottom of the window: one line when collapsed, +180px list when open
  // (port of SQLLogView.swift).
  import { ChevronRight, ChevronUp, Copy, SquareTerminal, Trash2, TriangleAlert } from "@lucide/svelte";
  import { tick } from "svelte";
  import { ui } from "./model.svelte";
  import { copy, count, reducedMotion } from "./util";

  let list: HTMLDivElement | undefined = $state();
  const statements = $derived(log.lines.filter((l) => !l.error).length);
  const last = $derived(log.lines[log.lines.length - 1]);

  // New lines arrive at the bottom: keep the list scrolled there.
  $effect(() => {
    void log.lines.length;
    if (ui.consoleOpen) tick().then(() => list && (list.scrollTop = list.scrollHeight));
  });

  function drawer(_: Element) {
    const still = reducedMotion();
    return { duration: still ? 0 : 280, css: (t: number) => `height:${t * 181}px` };
  }
</script>

<section class="console" aria-label="SQL console">
  <div class="bar">
    <button type="button" class="toggle" onclick={() => (ui.consoleOpen = !ui.consoleOpen)} aria-expanded={ui.consoleOpen}
            title={ui.consoleOpen ? "Hide the SQL console" : "Show every statement sent to the server"}>
      <span class="icon"><SquareTerminal size={16} strokeWidth={1.5} /></span>
      <strong>Console</strong>
      {#if !ui.consoleOpen}
        {#if last}
          <span class="last" class:error={last.error}>
            {#if last.error}<span class="solid"><TriangleAlert size={12} strokeWidth={2.2} /></span>{/if}
            <span>{last.text}</span>
          </span>
        {:else}
          <span class="none">No statements yet</span>
        {/if}
      {/if}
      <span class="spacer"></span>
      <span class="count">{statements === 1 ? "1 statement" : `${count(statements)} statements`}</span>
    </button>
    {#if ui.consoleOpen}
      <button type="button" class="iconbtn" disabled={!log.lines.length} onclick={() => copy(log.lines.map((l) => l.text).join("\n"))}
              title="Copy all statements" aria-label="Copy All"><Copy size={15} strokeWidth={1.6} /></button>
      <button type="button" class="iconbtn" disabled={!log.lines.length} onclick={() => (log.lines = [])}
              title="Clear the console" aria-label="Clear"><Trash2 size={15} strokeWidth={1.6} /></button>
    {/if}
    <button type="button" class="iconbtn chev" class:open={ui.consoleOpen} onclick={() => (ui.consoleOpen = !ui.consoleOpen)}
            title={ui.consoleOpen ? "Collapse" : "Expand"} aria-label={ui.consoleOpen ? "Collapse console" : "Expand console"}>
      <ChevronUp size={12} strokeWidth={2.6} />
    </button>
  </div>
  {#if ui.consoleOpen}
    <div class="drawer" transition:drawer>
      <div class="list" bind:this={list}>
        {#each log.lines as line (line.id)}
          <div class="line" class:error={line.error}>
            <span class="mark">{#if line.error}<span class="solid list"><TriangleAlert size={11} strokeWidth={2.2} /></span>{:else}<ChevronRight size={10} strokeWidth={2.4} />{/if}</span>
            <span class="text">{line.text}</span>
          </div>
        {:else}
          <p class="empty">Statements sent to the server will appear here.</p>
        {/each}
      </div>
    </div>
  {/if}
</section>

<style>
  .console { flex: none; background: var(--db-bar); }
  .bar { display: flex; align-items: center; gap: 4px; height: 28px; padding: 0 8px 0 12px; }
  .toggle {
    display: flex;
    align-items: center;
    gap: 8px;
    flex: 1;
    min-width: 0;
    height: 28px;
    min-height: 0;
    padding: 0;
    border: none;
    background: none;
    box-shadow: none;
    color: var(--text-2);
    text-align: left;
  }
  .toggle:hover:not(:disabled) { background: none; }
  .icon { display: inline-flex; color: var(--text-2); }
  strong { font-size: 12px; font-weight: 500; color: var(--text); }
  .last { display: inline-flex; align-items: center; gap: 4px; min-width: 0; padding-left: 4px; font-family: var(--mono); font-size: 11px; color: var(--text-2); }
  .last span { overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .last.error { color: var(--red); }
  .solid { display: inline-flex; --cut: var(--db-bar); }
  .solid.list { --cut: var(--db-content); }
  .solid :global(svg) { fill: currentColor; stroke: var(--cut); }
  .none { padding-left: 4px; font-size: 11px; color: var(--text-3); }
  .spacer { flex: 1; min-width: 8px; }
  .count { font-size: 11px; color: var(--text-2); font-variant-numeric: tabular-nums; white-space: nowrap; }
  .iconbtn { min-width: 22px; height: 22px; }
  .chev :global(svg) { transition: transform var(--fade); }
  .chev.open :global(svg) { transform: rotate(180deg); }
  .drawer { overflow: hidden; height: 181px; }
  .list { height: 180px; margin-top: 1px; overflow-y: auto; padding: 8px 12px; background: var(--db-content); box-shadow: 0 -1px 0 var(--db-sep); font-family: var(--mono); font-size: 11px; user-select: text; }
  .line { display: flex; align-items: flex-start; gap: 6px; padding: 1.5px 0; color: var(--text-2); }
  .line .mark { display: inline-flex; justify-content: center; align-items: center; width: 12px; height: 14px; flex: none; color: var(--text-3); }
  .line.error, .line.error .mark { color: var(--red); }
  .text { flex: 1; min-width: 0; overflow-wrap: anywhere; display: -webkit-box; -webkit-line-clamp: 4; line-clamp: 4; -webkit-box-orient: vertical; overflow: hidden; }
  .empty { margin: 60px 0 0; text-align: center; font-family: var(--font); font-size: 12px; color: var(--text-3); }
</style>
