<script lang="ts" module>
  import { KeyRound, Link, ArrowUpRight } from "@lucide/svelte";
  import type { Component } from "svelte";

  export type KeyKind = "primary" | "unique" | "index" | "foreign" | "autoIncrement" | "notNull";

  const look: Record<KeyKind, { title: string; icon?: Component<any>; color: string; help: string }> = {
    primary: { title: "PK", icon: KeyRound, color: "var(--st-yellow)", help: "Primary key" },
    unique: { title: "UNIQUE", color: "var(--st-orange)", help: "Unique: no two rows may have the same value" },
    index: { title: "INDEX", color: "var(--st-blue)", help: "Indexed, for faster searching" },
    foreign: { title: "FK", icon: Link, color: "var(--st-purple)", help: "Foreign key: refers to a row in another table" },
    autoIncrement: { title: "AUTO", icon: ArrowUpRight, color: "var(--st-teal)", help: "Auto increment: filled in automatically" },
    notNull: { title: "NOT NULL", color: "var(--text-2)", help: "A value is required" },
  };

  /** A column's badges, most important first: key kind, AUTO, NOT NULL (not on the primary key). */
  export function columnBadges(c: { key: string; extra: string; nullable: boolean }): KeyKind[] {
    const b: KeyKind[] = [];
    if (c.key === "PRI") b.push("primary");
    else if (c.key === "UNI") b.push("unique");
    else if (c.key === "MUL") b.push("index");
    if (c.extra.toLowerCase().includes("auto_increment")) b.push("autoIncrement");
    if (!c.nullable && c.key !== "PRI") b.push("notNull");
    return b;
  }
</script>

<script lang="ts">
  // A key marker for columns: PK, UNIQUE, INDEX, FK, AUTO, NOT NULL.
  let { kind }: { kind: KeyKind } = $props();
  const l = $derived(look[kind]);
</script>

<span class="badge" style:--c={l.color} title={l.help}>
  {#if l.icon}<l.icon size={9} strokeWidth={3} aria-hidden="true" />{/if}{l.title}
</span>

<style>
  .badge {
    display: inline-flex;
    flex: none;
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
</style>
