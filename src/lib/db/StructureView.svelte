<script lang="ts">
  // The Structure tab: one scrolling page (at most 1000 px wide) with cards for the columns, indexes,
  // foreign keys and the CREATE statement. Hover or focus shows a row's edit/delete buttons, double-click
  // edits a column, right-click opens its menu; the sheets are in StructureSheets.svelte. Views are read-only.
  import { db, openTable, schemaChanged, showError, askConfirm } from "./dbstate.svelte";
  import { message } from "./dbapi.svelte";
  import { copy } from "./util";
  import { structure } from "./structure/api";
  import { shown } from "./structure/store.svelte";
  import { highlight, syntaxWords, type Segment } from "./structure/highlight";
  import IconTile from "$lib/components/IconTile.svelte";
  import TypeChip from "./structure/TypeChip.svelte";
  import KeyBadge, { columnBadges, type KeyKind } from "./structure/KeyBadge.svelte";
  import ContextMenu, { menuPoint, type MenuItem } from "./structure/ContextMenu.svelte";
  import { Engine, ListOrdered, HardDrive, Lock, SlidersHorizontal, Columns3, Zap, Link, ScrollText, Plus, Pencil, Trash2, KeyRound, ArrowRight, Copy, ChevronDown, ChevronUp } from "@lucide/svelte";
  import type { SheetKind } from "./dbstate.svelte";
  import type { StructureColumn, StructureForeignKey, StructureIndex, TableStructure } from "$lib/types";
  import "./structure/structure.css";
  import "$lib/config/config.css";

  let { database, table = null }: { database: string; table?: string | null } = $props();

  let data = $state<TableStructure | null>(null);
  /** Which table `data` is about ("database.table"). */
  let loadedFor = $state("");
  let loadError = $state<string | null>(null);
  let showCreate = $state(false);
  let segments = $state<Segment[] | null>(null);
  let menu = $state<{ items: MenuItem[]; x: number; y: number } | null>(null);

  // Until the new table's structure is there, trust the selection (no edit buttons flashing on a view).
  const view = $derived(data && loadedFor === `${database}.${table}` ? data.view : db.isView);
  const canEdit = $derived(!view);
  const columns = $derived(data?.columns ?? []);
  const fkColumns = $derived(new Set(data?.foreignKeys.flatMap((f) => f.columns) ?? []));

  // Load on a new table and after every schema change; the old content stays until the new one is there.
  $effect(() => {
    const d = database;
    const t = table;
    void db.schemaVersion;
    if (!t) return;
    let current = true;
    structure.load(d, t).then(
      (s) => {
        if (!current) return;
        data = s;
        loadedFor = `${d}.${t}`;
        loadError = null;
        shown.database = d;
        shown.table = t;
        shown.data = s;
      },
      (e) => {
        if (!current) return;
        data = null;
        loadError = message(e);
      },
    );
    return () => (current = false);
  });

  // Colour the statement only while it is shown.
  $effect(() => {
    const sql = data?.createSql ?? "";
    if (!showCreate) return;
    let current = true;
    syntaxWords().then((w) => current && (segments = highlight(sql, w)));
    return () => (current = false);
  });

  /** Comment, extra attributes other than auto increment, and collation: the line under the name. */
  function detail(c: StructureColumn): string | null {
    const extra = c.extra.replace(/auto_increment/i, "").trim();
    const parts = [c.comment, extra, c.collation].filter(Boolean);
    return parts.length ? parts.join("  ·  ") : null;
  }

  /** The column's badges, FK after the index markers when a foreign key uses it. */
  function badges(c: StructureColumn): KeyKind[] {
    const list = columnBadges(c);
    if (fkColumns.has(c.name)) {
      const at = list.findIndex((k) => k !== "primary" && k !== "unique" && k !== "index");
      list.splice(at < 0 ? list.length : at, 0, "foreign");
    }
    return list;
  }

  const autoIncrement = (c: StructureColumn) => c.extra.toLowerCase().includes("auto_increment");

  function openSheet(kind: SheetKind, column?: string) {
    if (table && canEdit) db.sheet = { kind, database, table, column };
  }

  async function change(work: () => Promise<unknown>) {
    try {
      await work();
      schemaChanged();
    } catch (e) {
      showError(message(e));
    }
  }

  function deleteColumn(name: string) {
    if (!table || !canEdit) return;
    const t = table;
    askConfirm({
      title: `Delete the column “${name}”?`,
      message: "All data in the column will be permanently deleted.",
      button: "Delete",
      action: () => change(() => structure.dropColumn(database, t, name)),
    });
  }

  function deleteIndex(name: string) {
    if (!table || !canEdit) return;
    const t = table;
    askConfirm({
      title: name === "PRIMARY" ? "Delete the primary key?" : `Delete the index “${name}”?`,
      message: "The data stays; only the index is removed.",
      button: "Delete",
      action: () => change(() => structure.dropIndex(database, t, name)),
    });
  }

  function deleteForeignKey(name: string) {
    if (!table || !canEdit) return;
    const t = table;
    askConfirm({
      title: `Delete the foreign key “${name}”?`,
      message: "The data stays; only the relationship is removed.",
      button: "Delete",
      action: () => change(() => structure.dropForeignKey(database, t, name)),
    });
  }

  function openReference(fk: StructureForeignKey) {
    openTable(fk.refSchema || database, fk.refTable, "structure");
  }

  const target = (fk: StructureForeignKey) => (fk.refSchema && fk.refSchema !== database ? fk.refSchema + "." : "") + fk.refTable;
  const refColumns = (fk: StructureForeignKey) => (fk.refColumns.length === 1 ? fk.refColumns[0] : `(${fk.refColumns.join(", ")})`);

  function columnMenu(e: MouseEvent, c: StructureColumn) {
    e.preventDefault();
    const items: MenuItem[] = canEdit
      ? [{ label: "Edit Column…", action: () => openSheet("editColumn", c.name) }, { label: "Delete Column…", destructive: true, action: () => deleteColumn(c.name) }, "separator"]
      : [];
    menu = { items: [...items, { label: "Copy Name", action: () => copy(c.name) }], ...menuPoint(e) };
  }

  function indexMenu(e: MouseEvent, ix: StructureIndex) {
    e.preventDefault();
    const label = ix.name === "PRIMARY" ? "Delete Primary Key…" : "Delete Index…";
    menu = { items: [{ label, destructive: true, action: () => deleteIndex(ix.name) }, "separator", { label: "Copy Name", action: () => copy(ix.name) }], ...menuPoint(e) };
  }

  function foreignKeyMenu(e: MouseEvent, fk: StructureForeignKey) {
    e.preventDefault();
    menu = {
      items: [
        { label: `Open “${fk.refTable}”`, action: () => openReference(fk) },
        "separator",
        { label: "Delete Foreign Key…", destructive: true, action: () => deleteForeignKey(fk.name) },
        "separator",
        { label: "Copy Name", action: () => copy(fk.name) },
      ],
      ...menuPoint(e),
    };
  }
</script>

<div class="st page">
  <div class="content">
    <div class="pagehead">
      {#if data && view}
        <p class="readonly"><Lock size={12} aria-hidden="true" />Views are read-only here. To change one, run ALTER VIEW in the Query tab.</p>
      {:else if data}
        {#if data.engine}<span class="meta"><Engine size={15} aria-hidden="true" />{data.engine}</span>{/if}
        {#if data.collation}<span class="meta"><span class="abc" aria-hidden="true">Abc</span>{data.collation}</span>{/if}
        {#if data.rows !== null}<span class="meta"><ListOrdered size={12} aria-hidden="true" />≈ {data.rows} rows</span>{/if}
        <span class="meta"><HardDrive size={14} aria-hidden="true" />{data.size}</span>
      {/if}
      <span class="spacer"></span>
      {#if canEdit}
        <button type="button" class="cfg-btn" title="Engine and collation of the table" onclick={() => openSheet("tableOptions")}>
          <SlidersHorizontal size={15} aria-hidden="true" />Table Options…
        </button>
      {/if}
    </div>

    <section class="card" aria-labelledby="st-columns">
      <header class="card-head">
        <IconTile icon={Columns3} tint="var(--st-blue)" top="var(--st-blue-top)" size={24} glyph={0.46} />
        <h2 id="st-columns">Columns</h2>
        <span class="count">{columns.length}</span>
        <span class="spacer"></span>
        {#if canEdit}
          <button type="button" class="cfg-btn small" onclick={() => openSheet("addColumn")}><Plus size={14} aria-hidden="true" />Add Column</button>
        {/if}
      </header>
      {#if !columns.length}
        <p class="empty">{loadError ?? (data ? "No columns to show." : "")}</p>
      {:else}
        <div class="grid cols heading" class:ro={!canEdit} aria-hidden="true">
          <span class="num">#</span><span>Name</span><span>Type</span><span>Keys</span><span>Default</span>
        </div>
        {#each columns as c, i (c.name)}
          {@const line = detail(c)}
          <!-- Double-click and the context menu are shortcuts; the row's buttons do the same from the keyboard. -->
          <!-- svelte-ignore a11y_no_noninteractive_element_interactions -->
          <div class="grid cols row" class:ro={!canEdit} role="group" aria-label="Column {c.name}, {c.type}" ondblclick={() => openSheet("editColumn", c.name)} oncontextmenu={(e) => columnMenu(e, c)}>
            <span class="num">{i + 1}</span>
            <span class="namecell">
              <span class="name" title={c.name}>{c.name}</span>
              {#if line}<span class="detail" title={line}>{line}</span>{/if}
            </span>
            <span class="cell"><TypeChip type={c.type} /></span>
            <span class="cell badges">{#each badges(c) as k (k)}<KeyBadge kind={k} />{/each}</span>
            <span class="cell">
              {#if c.default !== null}
                <span class="value" class:dim={c.default === ""} title={c.default === "" ? "Empty text" : c.default}>{c.default === "" ? "''" : c.default}</span>
              {:else if c.nullable && !autoIncrement(c)}
                <span class="null" title="Defaults to NULL">NULL</span>
              {:else}
                <span class="dash" title="No default value">—</span>
              {/if}
            </span>
            {#if canEdit}
              <span class="actions">
                <button type="button" class="icon" title="Edit Column" aria-label="Edit Column {c.name}" onclick={() => openSheet("editColumn", c.name)}><Pencil size={12} strokeWidth={2.2} /></button>
                <button type="button" class="icon danger" title="Delete Column" aria-label="Delete Column {c.name}" onclick={() => deleteColumn(c.name)}><Trash2 size={12} strokeWidth={2.2} /></button>
              </span>
            {/if}
          </div>
        {/each}
      {/if}
    </section>

    {#if data && !view}
      <section class="card" aria-labelledby="st-indexes">
        <header class="card-head">
          <IconTile icon={Zap} tint="var(--st-orange)" top="var(--st-orange-top)" size={24} glyph={0.46} />
          <h2 id="st-indexes">Indexes</h2>
          <span class="count">{data.indexes.length}</span>
          <span class="spacer"></span>
          <button type="button" class="cfg-btn small" disabled={!columns.length} onclick={() => openSheet("addIndex")}><Plus size={14} aria-hidden="true" />Add Index</button>
        </header>
        {#if !data.indexes.length}
          <p class="empty">This table has no indexes. Indexes make searching and sorting faster.</p>
        {:else}
          {#each data.indexes as ix (ix.name)}
            {@const primary = ix.name === "PRIMARY"}
            <!-- svelte-ignore a11y_no_noninteractive_element_interactions -->
            <div class="grid idx row" role="group" aria-label="Index {ix.name} on {ix.columns.join(', ')}" oncontextmenu={(e) => indexMenu(e, ix)}>
              <span class="mark" style:color={primary ? "var(--st-yellow)" : "var(--st-orange)"}>
                {#if primary}<KeyRound size={11} strokeWidth={2.6} aria-hidden="true" />{:else}<Zap size={11} strokeWidth={2.6} aria-hidden="true" />{/if}
              </span>
              <span class="name" title={ix.name}>{ix.name}</span>
              <span class="cell chips" title={ix.columns.join(", ")}>{#each ix.columns as col, j (j)}<span class="namechip">{col}</span>{/each}</span>
              <span class="cell">
                {#if primary}
                  <span class="tag" style:--c="var(--st-yellow)"><KeyRound size={8.5} strokeWidth={3} aria-hidden="true" />PRIMARY</span>
                {:else if ix.unique}
                  <span class="tag" style:--c="var(--st-orange)">UNIQUE</span>
                {/if}
              </span>
              <span class="itype">{ix.type}</span>
              <span class="actions">
                <button type="button" class="icon danger" title={primary ? "Delete Primary Key" : "Delete Index"} aria-label={primary ? "Delete Primary Key" : `Delete Index ${ix.name}`} onclick={() => deleteIndex(ix.name)}><Trash2 size={12} strokeWidth={2.2} /></button>
              </span>
            </div>
          {/each}
        {/if}
      </section>

      <section class="card" aria-labelledby="st-fks">
        <header class="card-head">
          <IconTile icon={Link} tint="var(--st-purple)" top="var(--st-purple-top)" size={24} glyph={0.46} />
          <h2 id="st-fks">Foreign Keys</h2>
          <span class="count">{data.foreignKeys.length}</span>
          <span class="spacer"></span>
          <button type="button" class="cfg-btn small" disabled={!columns.length} onclick={() => openSheet("addForeignKey")}><Plus size={14} aria-hidden="true" />Add Foreign Key</button>
        </header>
        {#if !data.foreignKeys.length}
          <p class="empty">This table has no foreign keys. A foreign key links a column to a row in another table.</p>
        {:else}
          {#each data.foreignKeys as fk (fk.name)}
            <!-- svelte-ignore a11y_no_noninteractive_element_interactions -->
            <div class="grid fk row" role="group" aria-label="Foreign key {fk.name}: {fk.columns.join(', ')} references {target(fk)}" oncontextmenu={(e) => foreignKeyMenu(e, fk)}>
              <span class="mark" style:color="var(--st-purple)"><Link size={11} strokeWidth={2.6} aria-hidden="true" /></span>
              <span class="fkmain">
                <span class="fkline">
                  <span class="namechip strong">{fk.columns.join(", ")}</span>
                  <span class="arrow" aria-hidden="true"><ArrowRight size={10} strokeWidth={2.6} /></span>
                  <button type="button" class="ref" title="Open “{fk.refTable}”" onclick={() => openReference(fk)}><b>{target(fk)}</b><span class="dot">.</span>{refColumns(fk)}</button>
                </span>
                <span class="fkname">{fk.name}</span>
              </span>
              <span class="rule" title="On update: {fk.onUpdate}"><span class="rl">ON UPDATE</span><span class="rv" class:cascade={fk.onUpdate === "CASCADE"}>{fk.onUpdate || "—"}</span></span>
              <span class="rule" title="On delete: {fk.onDelete}"><span class="rl">ON DELETE</span><span class="rv" class:cascade={fk.onDelete === "CASCADE"}>{fk.onDelete || "—"}</span></span>
              <span class="actions">
                <button type="button" class="icon danger" title="Delete Foreign Key" aria-label="Delete Foreign Key {fk.name}" onclick={() => deleteForeignKey(fk.name)}><Trash2 size={12} strokeWidth={2.2} /></button>
              </span>
            </div>
          {/each}
        {/if}
      </section>
    {/if}

    <section class="card" aria-labelledby="st-sql">
      <header class="card-head">
        <IconTile icon={ScrollText} tint="var(--st-gray)" top="var(--st-gray-top)" size={24} glyph={0.46} />
        <h2 id="st-sql">{view ? "CREATE VIEW statement" : "CREATE statement"}</h2>
        <span class="spacer"></span>
        <button type="button" class="cfg-btn small" disabled={!data?.createSql} title="Copy the statement to the clipboard" onclick={() => data && copy(data.createSql)}><Copy size={13} aria-hidden="true" />Copy</button>
        <button type="button" class="cfg-btn small" aria-expanded={showCreate} onclick={() => (showCreate = !showCreate)}>
          {#if showCreate}<ChevronUp size={13} aria-hidden="true" />Hide{:else}<ChevronDown size={13} aria-hidden="true" />Show{/if}
        </button>
      </header>
      {#if showCreate}
        {#if data?.createSql}
          <pre class="sql">{#each segments ?? [{ text: data.createSql, kind: null }] as s, k (k)}{#if s.kind}<span class={s.kind}>{s.text}</span>{:else}{s.text}{/if}{/each}</pre>
        {:else}
          <p class="empty">The statement is not available.</p>
        {/if}
      {:else}
        <button type="button" class="reveal" onclick={() => (showCreate = true)}>The SQL that creates this {view ? "view" : "table"}. Click to show it.</button>
      {/if}
    </section>
  </div>
</div>

{#if menu}
  <ContextMenu items={menu.items} x={menu.x} y={menu.y} onclose={() => (menu = null)} />
{/if}

<style>
  .page { height: 100%; overflow: auto; }
  .content { display: flex; flex-direction: column; gap: 18px; max-width: 1048px; margin: 0 auto; padding: 20px 24px; }

  .pagehead { display: flex; align-items: center; gap: 15px; padding: 0 4px; }
  .meta { display: inline-flex; align-items: center; gap: 9px; color: var(--text-2); font-size: 11px; white-space: nowrap; }
  .abc { margin-right: 1.5px; font-size: 10px; letter-spacing: -0.2px; }
  .readonly { display: flex; align-items: center; gap: 7px; margin: 0; color: var(--text-2); font-size: 12px; line-height: 15px; }
  .spacer { flex: 1; }
  .cfg-btn { display: inline-flex; align-items: center; gap: 9px; }
  .cfg-btn.small { gap: 6px; }

  .card { overflow: hidden; }
  .card-head { display: flex; align-items: center; gap: 10px; padding: 11.5px 16px; border-bottom: 1px solid var(--st-sep); }
  h2 { margin: 0; font-size: 13px; line-height: 16px; font-weight: 700; }
  .count {
    padding: 1.5px 7px;
    border-radius: 999px;
    background: color-mix(in srgb, var(--text) 7%, transparent);
    color: var(--text-2);
    font-size: 10px;
    font-weight: 500;
    line-height: 13px;
    font-variant-numeric: tabular-nums;
  }
  .empty { margin: 0; padding: 14px 16px; color: var(--text-2); font-size: 12px; line-height: 15px; }

  .grid { display: grid; align-items: center; column-gap: 12px; padding: 0 16px; }
  .cols { grid-template-columns: 22px minmax(110px, 1fr) minmax(90px, 190px) minmax(60px, 170px) minmax(70px, 170px) 52px; }
  .cols.ro { grid-template-columns: 22px minmax(110px, 1fr) minmax(90px, 190px) minmax(60px, 170px) minmax(70px, 170px); }
  .idx { grid-template-columns: 22px minmax(90px, 220px) minmax(0, 1fr) 76px 64px 28px; }
  .fk { grid-template-columns: 22px minmax(0, 1fr) auto auto 28px; }
  .heading {
    padding-top: 7px;
    padding-bottom: 7px;
    border-bottom: 1px solid var(--st-sep);
    color: var(--text-3);
    font-size: 9.5px;
    font-weight: 600;
    line-height: 12px;
    letter-spacing: 0.4px;
    text-transform: uppercase;
    white-space: nowrap;
  }
  .row { position: relative; padding-top: 9px; padding-bottom: 9px; transition: background-color 0.12s; }
  .row:hover { background: var(--hover); }
  /* Dividers sit between the rows (1 px of their own), inset like macOS. */
  .row + .row { margin-top: 1px; }
  .row + .row::before { content: ""; position: absolute; top: -1px; left: 50px; right: 0; height: 1px; background: var(--st-sep); }
  .cell { display: flex; align-items: center; min-width: 0; overflow: hidden; }
  .num { color: var(--text-3); font: 11px var(--mono); text-align: right; }
  .heading .num { font: inherit; }
  .namecell { display: flex; flex-direction: column; gap: 2px; min-width: 0; }
  .name { overflow: hidden; font: 600 12.5px/15px var(--mono); text-overflow: ellipsis; white-space: nowrap; }
  .detail { overflow: hidden; color: var(--text-2); font-size: 10px; line-height: 13px; text-overflow: ellipsis; white-space: nowrap; }
  .badges { gap: 4px; }
  .value { overflow: hidden; font: 11px var(--mono); text-overflow: ellipsis; white-space: nowrap; user-select: text; }
  .value.dim { color: var(--text-2); }
  .null { color: var(--text-3); font: italic 11px var(--mono); }
  .dash { color: var(--text-3); }

  .actions { display: flex; justify-content: flex-end; gap: 2px; opacity: 0; transition: opacity 0.12s; }
  .row:hover .actions, .row:focus-within .actions { opacity: 1; }
  .icon {
    display: grid;
    place-items: center;
    width: 24px;
    height: 22px;
    min-height: 0;
    padding: 0;
    border: none;
    border-radius: 5px;
    background: none;
    box-shadow: none;
    color: var(--text-2);
  }
  .icon:hover:not(:disabled) { background: none; color: var(--text); }
  .icon.danger, .icon.danger:hover:not(:disabled) { color: color-mix(in srgb, var(--red) 85%, transparent); }

  .mark { display: grid; place-items: center; }
  .chips { gap: 4px; }
  .namechip {
    flex: none;
    padding: 2px 6px;
    border-radius: 4px;
    background: color-mix(in srgb, var(--text) 7%, transparent);
    font: 11px/14px var(--mono);
    white-space: nowrap;
  }
  .namechip.strong { font-weight: 600; }
  .tag {
    display: inline-flex;
    align-items: center;
    gap: 3px;
    padding: 2px 5px;
    border-radius: 999px;
    background: color-mix(in srgb, var(--c) 15%, transparent);
    color: var(--c);
    font-size: 9.5px;
    font-weight: 600;
    line-height: 12px;
    white-space: nowrap;
  }
  .itype { overflow: hidden; color: var(--text-2); font-size: 10px; text-overflow: ellipsis; white-space: nowrap; }

  .fkmain { display: flex; flex-direction: column; gap: 3px; min-width: 0; }
  .fkline { display: flex; align-items: center; gap: 8px; min-width: 0; }
  .arrow { display: grid; color: var(--text-3); }
  .ref {
    min-width: 0;
    min-height: 0;
    overflow: hidden;
    padding: 0;
    border: none;
    background: none;
    box-shadow: none;
    font: 12px/15px var(--mono);
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .ref:hover:not(:disabled) { background: none; text-decoration: underline; }
  .ref b { font-weight: 600; }
  .dot { color: var(--text-2); }
  .fkname { overflow: hidden; color: var(--text-2); font-size: 10px; line-height: 13px; text-overflow: ellipsis; white-space: nowrap; }
  .rule {
    display: inline-flex;
    align-items: baseline;
    gap: 4px;
    padding: 2.5px 6px;
    border-radius: 999px;
    background: color-mix(in srgb, var(--text) 5%, transparent);
    white-space: nowrap;
  }
  .rl { color: var(--text-3); font-size: 9px; font-weight: 600; }
  .rv { color: var(--text-2); font: 500 10.5px var(--mono); }
  .rv.cascade { color: var(--st-orange); }

  .sql { margin: 0; padding: 16px; overflow-x: auto; font: 12px/17px var(--mono); white-space: pre; user-select: text; cursor: text; }
  .sql .keyword { color: var(--st-purple); }
  .sql .function { color: var(--st-blue); }
  .sql .type { color: var(--st-teal); }
  .sql .string { color: var(--red); }
  .sql .number { color: var(--green); }
  .sql .comment { color: var(--text-2); }
  .sql .identifier { color: var(--st-orange); }
  .reveal {
    display: block;
    width: 100%;
    min-height: 0;
    padding: 11px 16px;
    border: none;
    border-radius: 0;
    background: none;
    box-shadow: none;
    color: var(--text-2);
    font-size: 12px;
    line-height: 15px;
    text-align: left;
  }
  .reveal:hover:not(:disabled) { background: none; }
  .reveal:focus-visible { outline-offset: -2px; }
</style>
