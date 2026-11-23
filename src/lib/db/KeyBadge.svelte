<script lang="ts" module>
  import type { ColumnInfo } from "./types";

  export type KeyKind = "primary" | "unique" | "index" | "foreign" | "autoIncrement" | "notNull";

  /** Badges for a column, most important first. */
  export function badges(c: ColumnInfo): KeyKind[] {
    const b: KeyKind[] = [];
    if (c.key === "PRI") b.push("primary");
    else if (c.key === "UNI") b.push("unique");
    else if (c.key === "MUL") b.push("index");
    if (c.autoIncrement) b.push("autoIncrement");
    if (!c.nullable && c.key !== "PRI") b.push("notNull");
    return b;
  }

  export const KEY_HELP: Record<KeyKind, string> = {
    primary: "Primary key",
    unique: "Unique: no two rows may have the same value",
    index: "Indexed, for faster searching",
    foreign: "Foreign key: refers to a row in another table",
    autoIncrement: "Auto increment: filled in automatically",
    notNull: "A value is required",
  };
</script>

<script lang="ts">
  // A key marker for columns and indexes: PK, UNIQUE, INDEX, FK, AUTO, NOT NULL.
  import { ArrowUpRight, KeyRound, Link } from "@lucide/svelte";
  let { kind }: { kind: KeyKind } = $props();
  const title: Record<KeyKind, string> = { primary: "PK", unique: "UNIQUE", index: "INDEX", foreign: "FK", autoIncrement: "AUTO", notNull: "NOT NULL" };
</script>

<span class="badge {kind}" title={KEY_HELP[kind]}>
  {#if kind === "primary"}<span class="filled key"><KeyRound size={9} strokeWidth={2.4} /></span>{/if}
  {#if kind === "foreign"}<Link size={9} strokeWidth={2.8} />{/if}
  {#if kind === "autoIncrement"}<ArrowUpRight size={10} strokeWidth={2.8} />{/if}
  {title[kind]}
</span>

<style>
  .badge {
    display: inline-flex;
    align-items: center;
    gap: 3px;
    flex: none;
    padding: 1px 5px;
    border-radius: 9px;
    background: color-mix(in srgb, var(--c) 15%, transparent);
    color: var(--c);
    font-size: 9.5px;
    font-weight: 600;
    line-height: 14px;
    white-space: nowrap;
  }
  .primary { --c: var(--db-yellow); }
  .unique { --c: var(--db-orange); }
  .index { --c: var(--db-blue); }
  .foreign { --c: var(--db-purple); }
  .autoIncrement { --c: var(--db-teal); }
  .notNull { --c: var(--text-2); }
  .key { display: inline-flex; --cut: currentColor; }
</style>
