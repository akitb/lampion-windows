<script lang="ts">
  // Placeholder: replaced by the structure/designer owner (see docs/AGENTS.md).
  let { database, table = null }: { database: string; table?: string | null } = $props();
</script>

<p style="padding:24px;color:var(--text-3)">DesignerView {database} {table ?? ""}</p>
