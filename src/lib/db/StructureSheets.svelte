<script lang="ts">
  // Renders the open structure sheet (`db.sheet`): New Database, New Table, Table Options, Add/Edit Column,
  // Add Index, Add Foreign Key. Mounted once in the Database Manager window. A sheet about a table opens
  // when the table's structure is there (taken from the Structure tab when it shows the same table).
  import { untrack } from "svelte";
  import { db, server, showError } from "./dbstate.svelte";
  import { message } from "./dbapi.svelte";
  import { structure } from "./structure/api";
  import { tableStructure } from "./structure/store.svelte";
  import NewDatabaseSheet from "./structure/NewDatabaseSheet.svelte";
  import NewTableSheet from "./structure/NewTableSheet.svelte";
  import TableOptionsSheet from "./structure/TableOptionsSheet.svelte";
  import ColumnSheet from "./structure/ColumnSheet.svelte";
  import IndexSheet from "./structure/IndexSheet.svelte";
  import ForeignKeySheet from "./structure/ForeignKeySheet.svelte";
  import type { Sheet } from "./dbstate.svelte";
  import type { StructureChoices, TableStructure } from "$lib/types";

  /** The sheet being shown, with what it needs. */
  let open = $state<{ sheet: Sheet; choices: StructureChoices; data: TableStructure | null } | null>(null);

  $effect(() => {
    const sheet = db.sheet;
    if (!sheet) {
      open = null;
      return;
    }
    let current = true;
    const needsTable = sheet.kind !== "newDatabase" && sheet.kind !== "newTable";
    // Only a new sheet reloads: later changes of the version or the shown table don't.
    const loads = untrack(() => Promise.all([structure.choices(server.version), needsTable && sheet.table ? tableStructure(sheet.database, sheet.table) : null]));
    loads.then(
      ([choices, data]) => {
        if (!current) return;
        if (sheet.kind === "editColumn" && !data?.columns.some((c) => c.name === sheet.column)) {
          showError(`The column “${sheet.column}” is no longer in the table.`);
          db.sheet = null;
          return;
        }
        open = { sheet, choices, data };
      },
      (e) => {
        if (!current) return;
        showError(message(e));
        db.sheet = null;
      },
    );
    return () => (current = false);
  });

  /** The sheet closed (Esc, Cancel, Done, or after its change). */
  function closed() {
    open = null;
    db.sheet = null;
  }
</script>

{#if open}
  {@const { sheet, choices, data } = open}
  {#key sheet}
    {#if sheet.kind === "newDatabase"}
      <NewDatabaseSheet {choices} onclose={closed} />
    {:else if sheet.kind === "newTable"}
      <NewTableSheet database={sheet.database} {choices} onclose={closed} />
    {:else if data && sheet.table}
      {#if sheet.kind === "tableOptions"}
        <TableOptionsSheet database={sheet.database} table={sheet.table} {data} {choices} onclose={closed} />
      {:else if sheet.kind === "addColumn" || sheet.kind === "editColumn"}
        <ColumnSheet database={sheet.database} table={sheet.table} {data} editing={sheet.kind === "editColumn" ? (data.columns.find((c) => c.name === sheet.column) ?? null) : null} onclose={closed} />
      {:else if sheet.kind === "addIndex"}
        <IndexSheet database={sheet.database} table={sheet.table} {data} onclose={closed} />
      {:else if sheet.kind === "addForeignKey"}
        <ForeignKeySheet database={sheet.database} table={sheet.table} {data} {choices} onclose={closed} />
      {/if}
    {/if}
  {/key}
{/if}
