<script lang="ts">
  // The card at the top of a settings page: icon tile, title, one line about the page and the file menu
  // (Edit File…, Show in Explorer, Copy Path, Restore Original File…).
  import type { Component } from "svelte";
  import { FileText } from "@lucide/svelte";
  import { revealItemInDir } from "@tauri-apps/plugin-opener";
  import { writeText } from "@tauri-apps/plugin-clipboard-manager";
  import { call } from "$lib/api";
  import IconTile from "$lib/components/IconTile.svelte";
  import Menu from "./Menu.svelte";
  import Dialog from "./Dialog.svelte";
  import type { ConfigFileId } from "$lib/types";

  let {
    icon,
    tint,
    top,
    title,
    detail,
    file,
    fileId,
    hasBackup = false,
    dirty = false,
    onrestore,
    onerror,
  }: {
    icon: Component<any>;
    tint: string;
    top: string;
    title: string;
    detail: string;
    /** Full path of the file the page edits. */
    file?: string;
    fileId?: ConfigFileId;
    hasBackup?: boolean;
    dirty?: boolean;
    onrestore?: () => Promise<void>;
    onerror?: (message: string) => void;
  } = $props();

  const name = $derived(file?.split(/[\\/]/).pop() ?? "");
  let confirmRestore = $state(false);
  let restoring = $state(false);

  async function run(work: () => Promise<unknown>) {
    try {
      await work();
    } catch (e) {
      onerror?.(String(e));
    }
  }

  async function restore() {
    restoring = true;
    await onrestore?.();
    restoring = false;
    confirmRestore = false;
  }
</script>

<div class="header">
  <IconTile {icon} {tint} {top} size={44} />
  <div class="text">
    <h2>{title}</h2>
    <p>{detail}</p>
  </div>
  {#if file && fileId}
    <Menu label="{name} file" title={file}>
      {#snippet trigger()}
        <span class="file"><FileText size={14} strokeWidth={1.8} aria-hidden="true" /><span>{name}</span></span>
      {/snippet}
      {#snippet children(close)}
        <button type="button" onclick={() => { close(); run(() => call("config_edit_file", { id: fileId })); }}>Edit File…</button>
        <button type="button" onclick={() => { close(); run(() => revealItemInDir(file ?? "")); }}>Show in Explorer</button>
        <button type="button" onclick={() => { close(); run(() => writeText(file ?? "")); }}>Copy Path</button>
        {#if onrestore}
          <hr />
          <button type="button" onclick={() => { close(); confirmRestore = true; }}>Restore Original File…</button>
        {/if}
      {/snippet}
    </Menu>
  {/if}
</div>

{#if onrestore}
  <Dialog
    bind:open={confirmRestore}
    title="Restore the original file?"
    message={hasBackup
      ? `This puts back ${name} as it was before Lampion first changed it.${dirty ? " Your unsaved changes on this page are discarded." : ""} Restart the server afterwards.`
      : "Lampion has not changed this file, so there is no original to restore."}
    cancel="Cancel"
    confirm="Restore"
    destructive
    disabled={!hasBackup}
    busy={restoring}
    onconfirm={restore}
  />
{/if}

<style>
  .header { display: flex; align-items: center; gap: 14px; min-height: 72px; padding: 14px 10px; border-radius: 10px; background: var(--surface-3); }
  .text { flex: 1; min-width: 0; }
  h2 { margin: 0; font-size: 17px; line-height: 20px; font-weight: 600; }
  p { margin: 3px 0 0; font-size: 12px; line-height: 15px; color: var(--text-2); }
  .file { display: inline-flex; align-items: center; gap: 5px; min-width: 0; }
  .file span { overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
</style>
