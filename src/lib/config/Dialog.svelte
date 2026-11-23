<script lang="ts">
  // A macOS-style alert as a native modal <dialog>: bold title, message, capsule buttons.
  // Esc and Cancel close it; the browser keeps focus inside and returns it afterwards.
  import type { Snippet } from "svelte";
  let {
    open = $bindable(false),
    title,
    message,
    confirm = "OK",
    cancel,
    destructive = false,
    busy = false,
    disabled = false,
    onconfirm,
    children,
  }: {
    open?: boolean;
    title: string;
    message?: string;
    confirm?: string;
    /** Label of the safe button; none for a plain alert. */
    cancel?: string;
    destructive?: boolean;
    busy?: boolean;
    disabled?: boolean;
    /** Without it, the confirm button just closes. */
    onconfirm?: () => void;
    children?: Snippet;
  } = $props();

  const titleId = $props.id();
  let dialog: HTMLDialogElement | undefined = $state();

  $effect(() => {
    if (!dialog) return;
    if (open && !dialog.open) dialog.showModal();
    else if (!open && dialog.open) dialog.close();
  });

  function confirmed(e: SubmitEvent) {
    e.preventDefault();
    if (onconfirm) onconfirm();
    else open = false;
  }
</script>

<dialog bind:this={dialog} aria-labelledby={titleId} onclose={() => (open = false)}>
  <form onsubmit={confirmed}>
    <h1 id={titleId}>{title}</h1>
    {#if message}<p>{message}</p>{/if}
    {@render children?.()}
    <div class="buttons">
      {#if cancel}<button type="button" onclick={() => (open = false)}>{cancel}</button>{/if}
      <button type="submit" class={destructive ? "destructive" : "default"} disabled={busy || disabled}>{busy ? "…" : confirm}</button>
    </div>
  </form>
</dialog>

<style>
  dialog {
    width: 260px;
    padding: 20px 16px 16px;
    border: 1px solid var(--border-strong);
    border-radius: 20px;
    background: color-mix(in srgb, var(--text) 12%, var(--bg));
    box-shadow: 0 20px 50px rgba(0, 0, 0, 0.45);
    color: var(--text);
  }
  dialog:modal { overscroll-behavior: contain; }
  dialog::backdrop { background: rgba(0, 0, 0, 0.2); overflow: hidden; overscroll-behavior: contain; }
  h1 { margin: 0 6px 8px; font-size: 13px; line-height: 17px; font-weight: 700; }
  p { margin: 0 6px 16px; font-size: 13px; line-height: 17px; white-space: pre-wrap; overflow-wrap: anywhere; user-select: text; max-height: 50vh; overflow: auto; }
  .buttons { display: flex; gap: 8px; margin-top: 4px; }
  .buttons button {
    flex: 1;
    height: 28px;
    min-height: 0;
    border: none;
    border-radius: 14px;
    background: color-mix(in srgb, var(--text) 9%, transparent);
    box-shadow: none;
    font-size: 13px;
  }
  .buttons .default { background: var(--accent); color: var(--accent-text); }
  .buttons .destructive { background: color-mix(in srgb, var(--red) 30%, transparent); color: var(--red); }
  .buttons button:hover:not(:disabled) { filter: brightness(1.1); }
  .buttons .default:hover:not(:disabled) { background: var(--accent); }
  .buttons .destructive:hover:not(:disabled) { background: color-mix(in srgb, var(--red) 30%, transparent); }
</style>
