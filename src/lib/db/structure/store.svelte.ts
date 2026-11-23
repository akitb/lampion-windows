// The table the Structure tab shows, so a sheet opened from it starts at once (no second load).
import type { TableStructure } from "$lib/types";
import { structure } from "./api";

export const shown = $state({ database: "", table: "", data: null as TableStructure | null });

/** The table's structure: what the tab shows when it is the same table, else a fresh load. */
export function tableStructure(database: string, table: string): Promise<TableStructure> {
  if (shown.data && shown.database === database && shown.table === table) return Promise.resolve(shown.data);
  return structure.load(database, table);
}
