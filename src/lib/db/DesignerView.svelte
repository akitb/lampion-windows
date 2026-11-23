<script lang="ts">
  // The Designer tab (port of DesignerView): the database's tables as cards and its foreign keys as lines, on one
  // pannable, zoomable diagram. Drag the handle beside a column onto another table's column to add a relation,
  // right-click a line to delete one, double-click a table to open its structure.
  import { Check, LayoutPanelLeft, Link, Maximize2, Pointer, TriangleAlert, Waypoints, ZoomIn, ZoomOut } from "@lucide/svelte";
  import { onDestroy, untrack } from "svelte";
  import { fade } from "svelte/transition";
  import { db, server } from "./dbstate.svelte";
  import Empty from "./Empty.svelte";
  import { dropdown } from "./menu.svelte";
  import { reducedMotion } from "./util";
  import Canvas from "./designer/Canvas.svelte";
  import { countText, ZOOM } from "./designer/diagram";
  import RelationSheet from "./designer/RelationSheet.svelte";
  import { DesignerStore } from "./designer/store.svelte";

  let { database }: { database: string } = $props();

  const store = new DesignerStore();
  const fadeTime = reducedMotion() ? 120 : 200;
  let spinner = $state(false);

  // A database loads when shown; a diagram read before shows at once and refreshes in the background.
  $effect(() => {
    const d = database;
    untrack(() => store.load(d));
  });

  // Structure changes elsewhere (sheets, other tabs): read again, unless this view made the change itself.
  let seenVersion = db.schemaVersion;
  $effect(() => {
    const v = db.schemaVersion;
    if (v === seenVersion) return;
    seenVersion = v;
    if (v !== store.ownVersion) untrack(() => store.requestReload());
  });

  // The sidebar's table lists reload on Refresh and after queries: follow only real changes, since selecting a
  // database reloads its list too and that must not restart the read under way.
  const signatures = new Map<string, string>();
  $effect(() => {
    const list = server.tables[database];
    if (!list) return;
    const signature = list.map((t) => `${t.name}\u0001${t.view}\u0001${t.engine}`).join("\u0002");
    const previous = signatures.get(database);
    signatures.set(database, signature);
    if (previous !== undefined && previous !== signature) untrack(() => store.requestReload());
  });

  // A calm empty canvas while a database is read for the first time; a spinner only if that takes a while,
  // counted from each switch (between two databases read for the first time nothing else changes).
  $effect(() => {
    void database;
    spinner = false;
    if (store.schema || store.loadError) return;
    const t = setTimeout(() => (spinner = true), 450);
    return () => clearTimeout(t);
  });

  onDestroy(() => store.destroy());

  const zoomOut = () => store.zoomTo(store.view.zoom / 1.25);
  const zoomIn = () => store.zoomTo(store.view.zoom * 1.25);

  function zoomMenu(e: MouseEvent) {
    dropdown(e.currentTarget as HTMLElement, [
      ...[50, 75, 100, 150, 200].map((p) => ({ label: `${p} %`, action: () => store.zoomTo(p / 100) })),
      { separator: true },
      { label: "Fit to Window", action: () => store.fit() },
    ], { keyboard: e.detail === 0 });
  }

  function keys(e: KeyboardEvent) {
    if (e.defaultPrevented || !(e.ctrlKey || e.metaKey) || e.altKey || !store.schema?.tables.length) return;
    if (document.querySelector("dialog[open]")) return;
    if (e.key === "0" || e.code === "Numpad0") store.fit();
    else if (e.key === "-" || e.code === "NumpadSubtract") zoomOut();
    else if (e.key === "+" || e.key === "=" || e.code === "NumpadAdd") zoomIn();
    else return;
    e.preventDefault();
  }

  /** A one-line hint in the corner of the canvas that follows what the user is doing. */
  const hint = $derived.by(() => {
    const s = store.schema;
    if (!s?.tables.length) return null;
    const l = store.link;
    if (l) {
      const text = l.target ? `${l.table}.${l.column}  ↔  ${l.target.table}.${l.target.column}` : "Release over a column of another table";
      return { text, link: true };
    }
    if (s.tables.every((t) => t.isView)) return null;
    return { text: "Drag the ○ beside a column onto another table’s column to add a relation · Double-click a table to open it", link: false };
  });
</script>

<svelte:window onkeydown={keys} />

<div class="designer">
  {#if store.schema}
    {@const s = store.schema}
    {#key s.database}
      <div class="pane" transition:fade|global={{ duration: fadeTime }}>
        {#if !s.tables.length}
          <Empty icon={Waypoints} title="No Tables"
            description="“{s.database}” has no tables yet. Once it has some, they appear here as boxes, with lines for their foreign keys.">
            <button type="button" class="pushbtn prominent" onclick={() => (db.sheet = { kind: "newTable", database: s.database })}>New Table…</button>
          </Empty>
        {:else}
          <div class="dbbar top bar">
            <div class="zoom">
              <button type="button" class="iconbtn" onclick={zoomOut} disabled={store.view.zoom <= ZOOM.min + 0.001}
                title="Zoom out (Ctrl+−)" aria-label="Zoom out"><ZoomOut size={14} strokeWidth={1.8} /></button>
              <button type="button" class="percent" onclick={zoomMenu} aria-haspopup="menu" title="Zoom level">
                {Math.round(store.view.zoom * 100)} %
              </button>
              <button type="button" class="iconbtn" onclick={zoomIn} disabled={store.view.zoom >= ZOOM.max - 0.001}
                title="Zoom in (Ctrl++)" aria-label="Zoom in"><ZoomIn size={14} strokeWidth={1.8} /></button>
            </div>
            <span class="divider"></span>
            <button type="button" class="dz-small" onclick={() => store.fit()} title="Show the whole diagram (Ctrl+0)">
              <span class="mirror"><Maximize2 size={15} strokeWidth={1.7} /></span> Fit
            </button>
            <button type="button" class="dz-small" onclick={() => store.autoArrange()} title="Forget the positions you dragged and lay the tables out again">
              <span class="mirror"><LayoutPanelLeft size={15} strokeWidth={1.7} /></span> Auto Arrange
            </button>
            <label class="check" title="Hide the tables that have no foreign keys and are not referenced">
              <input type="checkbox" checked={store.relationsOnly} onchange={(e) => store.setRelationsOnly(e.currentTarget.checked)} />
              <span class="box" aria-hidden="true">{#if store.relationsOnly}<Check size={11} strokeWidth={3} />{/if}</span>
              Relations only
            </label>
            <span class="count">{countText(s, store.visibleTables.length, store.relationsOnly)}</span>
          </div>
          <div class="stage">
            <Canvas {store} database={s.database} />
            {#if hint}
              <div class="hint" aria-live="polite">
                {#if hint.link}<Link size={13} strokeWidth={2} />{:else}<Pointer size={13} strokeWidth={1.8} />{/if}
                <span>{hint.text}</span>
              </div>
            {/if}
            {#if store.relationsOnly && !s.relations.length}
              <div class="cover" transition:fade={{ duration: fadeTime }}>
                <Empty icon={Link} title="No Relations"
                  description="None of the tables in “{s.database}” are linked by foreign keys yet. Show all tables, then drag from a column’s handle onto another table’s column to link them.">
                  <button type="button" class="pushbtn" onclick={() => store.setRelationsOnly(false)}>Show All Tables</button>
                </Empty>
              </div>
            {/if}
          </div>
        {/if}
      </div>
    {/key}
  {:else if store.loadError}
    <div class="pane" transition:fade|global={{ duration: fadeTime }}>
      <Empty icon={TriangleAlert} title="Can't Read the Database" description={store.loadError}>
        <button type="button" class="pushbtn" onclick={() => store.reload()}>Try Again</button>
      </Empty>
    </div>
  {:else}
    <div class="pane loading">
      {#if spinner}<span class="spinner" role="status" aria-label="Loading" transition:fade={{ duration: 200 }}></span>{/if}
    </div>
  {/if}

  {#if store.pending}<RelationSheet {store} {database} initial={store.pending} />{/if}
</div>

<style>
  /* Local tokens: dark values measured from the macOS screenshots (03, 34), light ones the macOS equivalents. */
  .designer {
    --dz-dot: rgba(0, 0, 0, 0.09);
    --dz-edge: color-mix(in srgb, var(--text) 13%, transparent);
    --dz-rule: color-mix(in srgb, var(--text) 8%, transparent);
    --dz-blue-top: color-mix(in srgb, var(--db-blue) 78%, white);
    --dz-purple-top: color-mix(in srgb, var(--db-purple) 78%, white);
    --dz-material: rgba(250, 250, 251, 0.94);
    --dz-tip-edge: color-mix(in srgb, var(--text) 10%, transparent);
    --dz-button-edge: rgba(0, 0, 0, 0.12);
    --dz-check: #ffffff;
    position: relative;
    display: flex;
    flex: 1;
    flex-direction: column;
    min-height: 0;
    background: var(--db-content);
  }
  @media (prefers-color-scheme: dark) {
    .designer {
      --dz-dot: rgba(255, 255, 255, 0.1);
      --dz-blue-top: #54a8ff;
      --dz-purple-top: #e266ff;
      --dz-material: rgba(36, 36, 37, 0.94);
      --dz-button-edge: transparent;
      --dz-check: #414143;
    }
  }

  .pane { position: absolute; inset: 0; display: flex; flex-direction: column; }
  .loading { align-items: center; justify-content: center; }
  .spinner {
    width: 16px;
    height: 16px;
    border: 2px solid var(--border-strong);
    border-top-color: var(--text-2);
    border-radius: 50%;
    animation: spin 0.8s linear infinite;
  }
  @keyframes spin { to { transform: rotate(360deg); } }

  .bar { gap: 8px; }
  .bar .divider { margin-left: 8px; background: color-mix(in srgb, var(--text) 18%, transparent); }
  .zoom { display: flex; align-items: center; gap: 2px; }
  .zoom .iconbtn { width: 22px; min-width: 0; height: 22px; padding: 0; }
  .percent {
    width: 35px;
    height: 22px;
    min-height: 0;
    padding: 0;
    border: none;
    border-radius: 5px;
    background: none;
    box-shadow: none;
    color: var(--text);
    font-size: 12px;
    font-variant-numeric: tabular-nums;
    white-space: nowrap;
  }
  .percent:hover { background: var(--hover); }
  .mirror { display: inline-flex; transform: scaleX(-1); }
  .check { position: relative; display: inline-flex; align-items: center; gap: 5px; font-size: 11px; white-space: nowrap; }
  .check input { position: absolute; width: 14px; height: 14px; min-height: 0; margin: 0; opacity: 0; }
  .box {
    display: inline-grid;
    place-items: center;
    width: 14px;
    height: 14px;
    flex: none;
    border-radius: 4px;
    background: var(--dz-check);
    box-shadow: inset 0 0 0 1px var(--dz-button-edge);
    color: #fff;
  }
  .check input:checked + .box { background: var(--accent); box-shadow: none; }
  .check input:focus-visible + .box { outline: 2px solid var(--accent); outline-offset: 1px; }
  .count { margin-left: auto; min-width: 0; overflow: hidden; font-size: 12px; color: var(--text-2); text-overflow: ellipsis; white-space: nowrap; }

  .stage { position: relative; display: flex; flex: 1; min-height: 0; }
  .hint {
    position: absolute;
    left: 12px;
    bottom: 12px;
    display: inline-flex;
    align-items: center;
    gap: 6px;
    max-width: calc(100% - 24px);
    padding: 6px 10px;
    border-radius: 14px;
    background: var(--dz-material);
    box-shadow: inset 0 0 0 1px var(--dz-tip-edge);
    color: var(--text-2);
    font-size: 12px;
    line-height: 15px;
    pointer-events: none;
  }
  .hint :global(svg) { flex: none; }
  .hint span { overflow: hidden; text-overflow: ellipsis; white-space: pre; }
  .cover { position: absolute; inset: 0; background: color-mix(in srgb, var(--db-content) 90%, transparent); }

  /* Shared by the canvas and the relation sheet. */
  .designer :global(.dz-small) {
    display: inline-flex;
    align-items: center;
    gap: 8px;
    flex: none;
    height: 20px;
    min-height: 0;
    padding: 0 11px;
    border: none;
    border-radius: 5px;
    background: var(--db-menu-btn);
    box-shadow: inset 0 0 0 1px var(--dz-button-edge);
    color: var(--text);
    font-size: 11px;
    white-space: nowrap;
  }
  .designer :global(.dz-small:hover:not(:disabled)) { background: color-mix(in srgb, var(--text) 6%, var(--db-menu-btn)); }
  .designer :global(.dz-chip) {
    flex: none;
    padding: 1.5px 5px;
    border-radius: 4px;
    background: color-mix(in srgb, var(--fam) 12%, transparent);
    color: var(--fam);
    font-family: var(--mono);
    font-size: 10px;
    font-weight: 500;
    line-height: 12.5px;
    white-space: nowrap;
  }

  @media (prefers-reduced-motion: reduce) {
    .spinner { animation-duration: 2s; }
  }
</style>
