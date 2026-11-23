<script lang="ts">
  // The bar at the bottom of a settings page while there are unsaved changes (or a save result):
  // the first problem, Revert, and Save (or "Save & Restart <server>" plus "Save Only" while it runs).
  // Ctrl+S saves. Esc does not revert.
  import { slide } from "svelte/transition";
  import { CircleCheck, OctagonX, Pencil } from "@lucide/svelte";
  import { motion, type Message } from "./forms.svelte";
  let {
    dirty,
    problem,
    saving,
    restartName,
    message,
    onrevert,
    onsave,
  }: {
    dirty: boolean;
    problem?: string;
    saving: boolean;
    /** The server to restart after saving, when it runs. */
    restartName: string | null;
    message: Message;
    onrevert: () => void;
    onsave: (restart: boolean) => void;
  } = $props();

  const blocked = $derived(saving || !!problem);

  function key(e: KeyboardEvent) {
    if (e.ctrlKey && !e.altKey && !e.shiftKey && e.key.toLowerCase() === "s" && dirty) {
      e.preventDefault();
      if (!blocked) onsave(restartName !== null);
    }
  }
</script>

<svelte:window onkeydown={key} />

{#if dirty || message}
  <div class="bar" transition:slide={{ duration: motion(280) }}>
    <div class="status" role="status">
      {#if dirty && problem}
        <svg class="icon" viewBox="0 0 16 16" aria-hidden="true">
          <path d="M8 1.6c.5 0 .9.3 1.2.8l5.6 10c.5.9-.1 2-1.2 2H2.4c-1 0-1.7-1.1-1.2-2l5.6-10c.3-.5.7-.8 1.2-.8Z" fill="var(--orange)" />
          <rect x="7.25" y="5" width="1.5" height="5" rx="0.75" fill="var(--bg)" />
          <circle cx="8" cy="12" r="0.95" fill="var(--bg)" />
        </svg>
        <span class="text">{problem}</span>
      {:else if dirty}
        <span class="pencil" aria-hidden="true"><Pencil size={8} strokeWidth={3} /></span>
        <span class="text secondary">{restartName ? `Unsaved changes. ${restartName} restarts to apply them.` : "Unsaved changes."}</span>
      {:else if message}
        {#if message.ok}
          <CircleCheck size={16} color="var(--green)" aria-hidden="true" />
        {:else}
          <OctagonX size={16} color="var(--red)" aria-hidden="true" />
        {/if}
        <span class="text">{message.text}</span>
      {/if}
    </div>
    {#if saving}<span class="spinner" aria-label="Saving"></span>{/if}
    {#if dirty}
      <button type="button" class="b" disabled={saving} onclick={onrevert} title="Discard the unsaved changes on this page">Revert</button>
      {#if restartName}
        <button type="button" class="b" disabled={blocked} onclick={() => onsave(false)}>Save Only</button>
      {/if}
      <button type="button" class="b primary" disabled={blocked} onclick={() => onsave(restartName !== null)} title="Save (Ctrl+S)">
        {restartName ? `Save & Restart ${restartName}` : "Save"}
      </button>
    {/if}
  </div>
{/if}

<style>
  .bar {
    flex: none;
    display: flex;
    align-items: center;
    gap: 12px;
    padding: 12px 20px;
    border-top: 1px solid var(--border-strong);
    background: var(--bg);
  }
  .status { flex: 1; display: flex; align-items: center; gap: 12px; min-width: 0; }
  .icon { flex: none; width: 16px; height: 16px; }
  .text { font-size: 13px; line-height: 16px; display: -webkit-box; -webkit-line-clamp: 2; line-clamp: 2; -webkit-box-orient: vertical; overflow: hidden; user-select: text; }
  .secondary { color: var(--text-2); }
  .pencil { flex: none; display: grid; place-items: center; width: 16px; height: 16px; border-radius: 50%; background: var(--text-3); color: var(--bg); }
  .b {
    flex: none;
    height: 23px;
    min-height: 0;
    padding: 0 12px;
    border: none;
    border-radius: 6px;
    background: color-mix(in srgb, var(--text) 13%, transparent);
    box-shadow: none;
    color: var(--text);
    font-size: 13px;
    white-space: nowrap;
  }
  .b:hover:not(:disabled) { background: color-mix(in srgb, var(--text) 18%, transparent); }
  .b.primary { background: var(--accent); color: var(--accent-text); }
  .b.primary:hover:not(:disabled) { background: var(--accent); filter: brightness(1.08); }
  /* macOS dims a disabled default button instead of fading it out. */
  .b.primary:disabled { opacity: 1; background: color-mix(in srgb, var(--accent) 70%, var(--bg)); color: color-mix(in srgb, var(--accent-text) 36%, color-mix(in srgb, var(--accent) 70%, var(--bg))); }
  .spinner { width: 14px; height: 14px; border: 2px solid var(--border-strong); border-top-color: var(--text-2); border-radius: 50%; animation: spin 0.8s linear infinite; }
  @keyframes spin { to { transform: rotate(360deg); } }
  @media (prefers-reduced-motion: reduce) { .spinner { animation-duration: 2.4s; } }
</style>
