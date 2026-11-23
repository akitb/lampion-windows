<script lang="ts">
  // One table or view on the diagram (port of DesignerCard): a header, then its columns with key icons and
  // compact types. Its size comes from the card layout, so the lines never measure the DOM. The canvas does
  // all pointer handling; the card only shows state.
  import { Eye, KeyRound, Link, Table } from "@lucide/svelte";
  import IconTile from "$lib/components/IconTile.svelte";
  import type { DesignerTable } from "$lib/types";
  import { compactType, METRICS, rowHelp, typeFamily, type CardLayout } from "./diagram";
  import type { CardState } from "./store.svelte";

  let {
    table,
    layout,
    x,
    y,
    state,
    foreign,
    external,
    lit,
    dropTarget,
    dragSource,
    hovered,
    dragging,
  }: {
    table: DesignerTable;
    layout: CardLayout;
    x: number;
    y: number;
    state: CardState;
    /** Columns with a foreign key (link icon). */
    foreign: Set<string> | undefined;
    /** Keys into other databases: column → `db.table.column`. */
    external: Map<string, string> | undefined;
    /** Rows of the hovered relation. */
    lit: Set<string> | null;
    /** The row a relation drag would connect to. */
    dropTarget: string | null;
    /** The row a relation drag started from. */
    dragSource: string | null;
    /** The pointer is over the card: handles (tables only) and row tooltips. */
    hovered: boolean;
    dragging: boolean;
  } = $props();

  const kind = $derived(table.isView ? "View" : "Table");
  const handles = $derived((hovered && !table.isView) || dragSource !== null);
</script>

<div
  class="card {state}"
  class:view={table.isView}
  class:dragging
  data-table={table.name}
  tabindex="0"
  role="button"
  aria-label="{kind} {table.name}, {table.columns.length} column{table.columns.length === 1 ? '' : 's'}"
  style="transform: translate({x}px, {y}px); width: {layout.width}px; height: {layout.height}px"
>
  <div class="head">
    <IconTile icon={table.isView ? Eye : Table} tint={table.isView ? "var(--db-purple)" : "var(--db-blue)"}
      top={table.isView ? "var(--dz-purple-top)" : "var(--dz-blue-top)"} size={20} glyph={0.55} />
    <span class="title">{table.name}</span>
    {#if table.isView}<span class="tag">VIEW</span>{/if}
  </div>
  {#each layout.rows as i (i)}
    {@const c = table.columns[i]}
    <div class="row" class:lit={lit?.has(c.name) || dragSource === c.name} class:drop={dropTarget === c.name}
      title={hovered ? rowHelp(c, external?.get(c.name)) : undefined}>
      <span class="icons">
        {#if c.key === "PRI"}<span class="filled pk"><KeyRound size={11} strokeWidth={2.4} /></span>{/if}
        {#if foreign?.has(c.name)}<span class="fk"><Link size={10} strokeWidth={2.6} /></span>{/if}
      </span>
      <span class="name" class:primary={c.key === "PRI"}>{c.name}</span>
      <span class="dz-chip fam-{typeFamily(c.type)}">{compactType(c.type)}</span>
    </div>
  {/each}
  {#if layout.hidden > 0}
    <div class="note more">+ {layout.hidden} more column{layout.hidden === 1 ? "" : "s"}</div>
  {:else if table.columns.length === 0}
    <div class="note">No columns</div>
  {/if}
  {#if handles}
    {#each layout.rows as i, row (i)}
      <span class="handle" class:on={dragSource === table.columns[i].name}
        style="top: {METRICS.headerHeight + (row + 0.5) * METRICS.rowHeight - METRICS.handleRadius}px"></span>
    {/each}
  {/if}
</div>

<style>
  .card {
    --tint: var(--db-blue);
    position: absolute;
    left: 0;
    top: 0;
    display: flex;
    flex-direction: column;
    border-radius: 10px;
    background: var(--db-content);
    box-shadow: 0 1.5px 6px rgba(0, 0, 0, 0.07);
    outline: none;
    transition: opacity 0.12s ease-out, box-shadow 0.12s ease-out;
  }
  .card.view { --tint: var(--db-purple); }
  /* The border on top of the header's tint, as a stroke inside the card. */
  .card::after {
    content: "";
    position: absolute;
    inset: 0;
    border: 1px solid var(--dz-edge);
    border-radius: inherit;
    pointer-events: none;
  }
  .card.highlighted { box-shadow: 0 1.5px 16px rgba(0, 0, 0, 0.16); }
  .card.highlighted::after { border: 1.5px solid color-mix(in srgb, var(--tint) 85%, transparent); }
  .card.dimmed { opacity: 0.35; }
  .card.dragging { z-index: 1; }
  .card:focus-visible::after { border: 2px solid var(--accent); }

  .head {
    display: flex;
    align-items: center;
    gap: 8px;
    flex: none;
    height: 36px;
    padding: 0 10px;
    border-bottom: 1px solid var(--dz-rule);
    border-radius: 10px 10px 0 0;
    background: color-mix(in srgb, var(--tint) 7%, transparent);
  }
  .highlighted .head { background: color-mix(in srgb, var(--tint) 13%, transparent); }
  .title {
    flex: 1;
    min-width: 0;
    overflow: hidden;
    font-size: 12.5px;
    font-weight: 600;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .tag {
    flex: none;
    padding: 1px 5px;
    border-radius: 9px;
    background: color-mix(in srgb, var(--db-purple) 15%, transparent);
    color: var(--db-purple);
    font-size: 9.5px;
    font-weight: 600;
    line-height: 13px;
  }

  .row {
    display: flex;
    align-items: center;
    flex: none;
    height: 22px;
    padding: 0 10px;
  }
  .row.lit { background: color-mix(in srgb, var(--db-purple) 14%, transparent); }
  .row.drop { background: color-mix(in srgb, var(--accent) 22%, transparent); }
  .icons { display: flex; align-items: center; gap: 2px; flex: none; width: 30px; }
  .icons span { display: inline-flex; }
  .pk { --cut: currentColor; color: var(--db-yellow); }
  .fk { color: var(--db-purple); }
  .name {
    flex: 0 1 auto;
    min-width: 0;
    margin-right: 10px;
    overflow: hidden;
    font-family: var(--mono);
    font-size: 11.5px;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .name.primary { font-weight: 600; }
  .row :global(.dz-chip) { margin-left: auto; }
  .note {
    flex: none;
    height: 22px;
    padding-left: 10px;
    color: var(--text-3);
    font-size: 10.5px;
    line-height: 22px;
  }
  .note.more { padding-left: 40px; }

  .handle {
    position: absolute;
    z-index: 1;
    right: -5px;
    width: 10px;
    height: 10px;
    border-radius: 50%;
    background: var(--db-content);
    box-shadow: inset 0 0 0 1.5px var(--accent);
    pointer-events: none;
  }
  .handle.on { background: var(--accent); }

  @media (prefers-reduced-motion: reduce) {
    .card { transition: none; }
  }
</style>
