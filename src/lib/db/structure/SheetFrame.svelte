<script lang="ts">
  // The shared look of every structure sheet, as a native modal <dialog>: an icon header, the content as a
  // grouped form (sheet.css), and a footer with a note on the left and Cancel / confirm on the right.
  // Enter confirms (when allowed), Esc and Cancel close; focus starts in the field marked `data-autofocus`.
  import type { Component, Snippet } from "svelte";
  import { onMount } from "svelte";
  import { Info, CodeXml, TriangleAlert } from "@lucide/svelte";
  import IconTile from "$lib/components/IconTile.svelte";
  import "./structure.css";
  import "./sheet.css";
  import "$lib/config/config.css";

  let {
    title,
    subtitle,
    icon,
    tint,
    top,
    confirm,
    canConfirm = true,
    busy = false,
    onconfirm,
    onclose,
    width = 460,
    note,
    noteIsCode = false,
    warning,
    showsCancel = true,
    maxFormHeight = 520,
    children,
  }: {
    title: string;
    subtitle?: string;
    icon: Component<any>;
    /** Tile colours (CSS), bottom and top of the gradient. */
    tint: string;
    top: string;
    confirm: string;
    canConfirm?: boolean;
    /** A statement is running: the confirm button waits. */
    busy?: boolean;
    onconfirm: () => void;
    /** The sheet closed (Esc, Cancel, or `close()` after success). */
    onclose: () => void;
    width?: number;
    note?: string | null;
    noteIsCode?: boolean;
    warning?: string | null;
    showsCancel?: boolean;
    maxFormHeight?: number;
    children: Snippet;
  } = $props();

  const titleId = $props.id();
  let dialog = $state<HTMLDialogElement | undefined>();

  onMount(() => {
    dialog?.showModal();
    const first = dialog?.querySelector<HTMLElement>("[data-autofocus]");
    if (first) {
      first.focus();
      if (first instanceof HTMLInputElement) first.select();
    }
  });

  /** Closes the sheet the way Cancel does (focus goes back where it was). */
  export function close() {
    dialog?.close();
  }

  function submitted(e: SubmitEvent) {
    e.preventDefault();
    if (canConfirm && !busy) onconfirm();
  }
</script>

<dialog bind:this={dialog} class="st st-sheet" style:width="{width}px" aria-labelledby={titleId} onclose={onclose}>
  <form onsubmit={submitted}>
    <header class="head">
      <IconTile {icon} {tint} {top} size={38} glyph={0.46} />
      <div class="titles">
        <h1 id={titleId}>{title}</h1>
        {#if subtitle}<p title={subtitle}>{subtitle}</p>{/if}
      </div>
    </header>
    <div class="form" style:max-height="{maxFormHeight}px">{@render children()}</div>
    <footer class="foot">
      {#if warning}
        <p class="note"><TriangleAlert size={12} color="var(--orange)" aria-hidden="true" /><span>{warning}</span></p>
      {:else if note}
        <p class="note" class:code={noteIsCode} title={noteIsCode ? "The SQL that will be sent" : undefined}>
          {#if noteIsCode}<CodeXml size={12} aria-hidden="true" />{:else}<Info size={12} aria-hidden="true" />{/if}<span>{note}</span>
        </p>
      {/if}
      <span class="spacer"></span>
      {#if showsCancel}<button type="button" class="btn" onclick={() => dialog?.close()}>Cancel</button>{/if}
      <button type="submit" class="btn primary" disabled={!canConfirm || busy}>{confirm}</button>
    </footer>
  </form>
</dialog>

<style>
  dialog {
    max-width: calc(100vw - 32px);
    max-height: calc(100vh - 32px);
    padding: 0;
    overflow: hidden;
    border: 1px solid var(--border-strong);
    border-radius: 20px;
    background: var(--bg);
    box-shadow: 0 20px 60px rgba(0, 0, 0, 0.45);
    color: var(--text);
  }
  dialog::backdrop { background: rgba(0, 0, 0, 0.2); }
  @media (prefers-reduced-motion: no-preference) {
    dialog[open] { animation: sheet-in 0.2s cubic-bezier(0.2, 0.9, 0.3, 1); }
  }
  @keyframes sheet-in {
    from { opacity: 0; transform: scale(0.97); }
  }
  form { display: flex; flex-direction: column; max-height: calc(100vh - 34px); }

  .head { display: flex; align-items: center; gap: 12px; padding: 18px 20px 2px; }
  .titles { min-width: 0; }
  h1 { margin: 0; font-size: 15px; line-height: 20px; font-weight: 600; }
  .titles p { margin: 2px 0 0; overflow: hidden; color: var(--text-2); font-size: 12px; line-height: 15px; text-overflow: ellipsis; white-space: nowrap; }

  .form { flex: 0 1 auto; min-height: 0; overflow: auto; padding: 20.5px 20px 28px; overscroll-behavior: contain; }

  .foot { display: flex; flex: none; align-items: center; gap: 10px; padding: 14px 20px; border-top: 1px solid var(--st-sep); }
  .note { display: flex; align-items: flex-start; gap: 6px; min-width: 0; margin: 0; color: var(--text-2); font-size: 12px; line-height: 15px; user-select: text; }
  .note :global(svg) { flex: none; margin-top: 1.5px; }
  .note span { display: -webkit-box; overflow: hidden; -webkit-box-orient: vertical; -webkit-line-clamp: 3; line-clamp: 3; overflow-wrap: anywhere; }
  .note.code { font: 11px/15px var(--mono); }
  .spacer { flex: 1; }
  .btn {
    flex: none;
    height: 28px;
    min-height: 0;
    padding: 0 15px;
    border: none;
    border-radius: 14px;
    background: color-mix(in srgb, var(--text) 9%, transparent);
    box-shadow: none;
    color: var(--text);
    font-size: 13px;
    white-space: nowrap;
  }
  .btn:hover:not(:disabled) { background: color-mix(in srgb, var(--text) 13%, transparent); }
  .btn.primary { background: var(--accent); color: var(--accent-text); }
  .btn.primary:hover:not(:disabled) { background: var(--accent); filter: brightness(1.08); }
  .btn.primary:disabled { background: color-mix(in srgb, var(--text) 5%, transparent); color: var(--text-3); opacity: 1; }
</style>
