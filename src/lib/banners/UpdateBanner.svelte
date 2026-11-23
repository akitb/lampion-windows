<script lang="ts">
  // The update notice at the top of the main window (UpdateCard.swift): checking, up to date, a new
  // version (Later, What's New, Update Now), the download (Cancel), installing, and a failure (Dismiss,
  // Download Manually, Try Again). It fades in and settles from just above (with reduced motion it only
  // fades); its content cross-fades when the kind of notice changes, not on every progress step.
  // Renders nothing when idle.
  import { cubicOut } from "svelte/easing";
  import { fade } from "svelte/transition";
  import { CircleCheck, TriangleAlert } from "@lucide/svelte";
  import { core } from "$lib/api";
  import { startUpdater, update, updater } from "$lib/updater/updater.svelte";
  import ReleaseNotes from "$lib/updater/ReleaseNotes.svelte";
  import appIcon from "../../../src-tauri/icons/128x128.png";

  startUpdater();

  const st = $derived(updater.state);
  const reduce = matchMedia("(prefers-reduced-motion: reduce)").matches;
  /** Why an action failed (the failure state itself shows the updater's own errors). */
  let problem = $state<string | null>(null);
  const notesId = $props.id();
  let notesButton: HTMLButtonElement | undefined = $state();

  async function act(f: () => Promise<unknown>) {
    problem = null;
    try {
      await f();
    } catch (e) {
      problem = String(e);
    }
  }

  function notice(node: HTMLElement) {
    const height = node.offsetHeight;
    return {
      duration: reduce ? 180 : 280,
      easing: cubicOut,
      css: (t: number) =>
        reduce
          ? `opacity: ${t}`
          : `opacity: ${t}; height: ${t * height}px; overflow: hidden; transform: scale(${0.97 + 0.03 * t}); transform-origin: top center`,
    };
  }
</script>

{#if st.state !== "idle"}
  <div class="wrap" transition:notice>
    <div class="notice">
      {#key st.state}
        <div class="content" in:fade={{ duration: reduce ? 0 : 180 }}>
          {#if st.state === "checking"}
            <span class="icon" aria-hidden="true"><span class="spinner"></span></span>
            <div class="text"><strong aria-live="polite">Checking for updates…</strong></div>
          {:else if st.state === "upToDate"}
            <span class="icon ok" aria-hidden="true"><CircleCheck size={30} strokeWidth={1.6} /></span>
            <div class="text">
              <strong aria-live="polite">Lampion is up to date</strong>
              <span class="detail">Version {updater.version} is the latest.</span>
            </div>
          {:else if st.state === "available"}
            <img class="app" src={appIcon} alt="" />
            <div class="text">
              <strong aria-live="polite">Lampion {st.release.version} is available</strong>
              <span class="detail">Version {updater.version} is installed. Updating takes a few seconds and the servers keep running.</span>
              {#if problem}<span class="problem" role="alert">{problem}</span>{/if}
            </div>
            <div class="buttons">
              <button type="button" title="Hide this until the next time Lampion opens" onclick={() => act(update.later)}>Later</button>
              <button type="button" bind:this={notesButton} popovertarget={notesId}>What’s New</button>
              <button type="button" class="primary" onclick={() => act(update.install)}>Update Now</button>
            </div>
            <ReleaseNotes id={notesId} release={st.release} anchor={notesButton} onskip={() => act(update.skip)} />
          {:else if st.state === "downloading"}
            <img class="app" src={appIcon} alt="" />
            <div class="text progress">
              <strong>Downloading Lampion {st.release.version}</strong>
              <progress max="1" value={st.progress} aria-label="Download progress"></progress>
            </div>
            <span class="percent">{Math.round(st.progress * 100)}%</span>
            <div class="buttons"><button type="button" onclick={() => act(update.cancel)}>Cancel</button></div>
          {:else if st.state === "installing"}
            <img class="app" src={appIcon} alt="" />
            <div class="text progress">
              <strong aria-live="polite">Installing Lampion {st.release.version}</strong>
              <span class="busy" role="progressbar" aria-label="Installing"></span>
            </div>
            <span class="detail wide">Lampion reopens in a moment.</span>
          {:else if st.state === "failed"}
            <span class="icon warn" aria-hidden="true"><TriangleAlert size={30} strokeWidth={1.6} /></span>
            <div class="text">
              <strong aria-live="polite">The update could not be installed</strong>
              <span class="detail">{st.message}</span>
              {#if problem}<span class="problem" role="alert">{problem}</span>{/if}
            </div>
            <div class="buttons">
              <button type="button" onclick={() => act(update.later)}>Dismiss</button>
              <button type="button" onclick={() => act(() => core.openUrl(st.page))}>Download Manually</button>
              <!-- A failed check comes back as the failure state itself. -->
              <button type="button" onclick={() => update.check().catch(() => {})}>Try Again</button>
            </div>
          {/if}
        </div>
      {/key}
    </div>
  </div>
{/if}

<style>
  /* NoticeCard.swift: an inset card, 12 px radius, 14 × 12 px padding, 14 px between its parts. */
  .wrap { flex: none; padding: 12px 16px 4px; }
  .notice { padding: 12px 14px; border: 1px solid var(--border); border-radius: 12px; background: var(--surface); box-shadow: var(--shadow); }
  .content { display: flex; align-items: center; gap: 14px; min-height: 40px; }
  .icon { flex: none; display: flex; justify-content: center; width: 40px; }
  /* Hierarchical SF Symbols: a tinted shape with the mark in full colour. */
  .icon :global(svg) { fill: color-mix(in srgb, currentColor 25%, transparent); }
  .icon.ok { color: var(--green); }
  .icon.warn { color: var(--orange); }
  .app { flex: none; width: 40px; height: 40px; }
  .spinner {
    width: 16px;
    height: 16px;
    border: 2px solid color-mix(in srgb, var(--text-2) 35%, transparent);
    border-top-color: var(--text-2);
    border-radius: 50%;
    animation: spin 0.9s linear infinite;
  }
  @keyframes spin { to { transform: rotate(360deg); } }
  .text { flex: 1; min-width: 0; display: flex; flex-direction: column; gap: 2px; }
  .text.progress { gap: 6px; }
  strong { color: var(--text); font-size: 13px; line-height: 16px; font-weight: 600; }
  .detail { color: var(--text-2); font-size: 12px; line-height: 15px; user-select: text; }
  .detail.wide { flex: 1; }
  .problem { color: var(--red); font-size: 12px; line-height: 15px; user-select: text; }
  progress, .busy {
    display: block;
    width: 100%;
    max-width: 360px;
    height: 4px;
    border: none;
    border-radius: 2px;
    background: var(--control);
    overflow: hidden;
  }
  progress { appearance: none; }
  progress::-webkit-progress-bar { background: var(--control); border-radius: 2px; }
  progress::-webkit-progress-value { background: var(--accent); border-radius: 2px; transition: width var(--fade); }
  .busy { position: relative; }
  .busy::after {
    content: "";
    position: absolute;
    top: 0;
    bottom: 0;
    left: 0;
    width: 30%;
    border-radius: 2px;
    background: var(--accent);
    animation: slide 1.2s ease-in-out infinite;
  }
  @keyframes slide { from { transform: translateX(-100%); } to { transform: translateX(340%); } }
  @media (prefers-reduced-motion: reduce) {
    .spinner, .busy::after { animation: none; }
  }
  .percent { flex: none; width: 44px; color: var(--text-2); font-size: 12px; text-align: right; font-variant-numeric: tabular-nums; }
  .buttons { flex: none; display: flex; gap: 8px; margin-left: auto; }
  .buttons button {
    height: 24px;
    min-height: 0;
    padding: 0 12px;
    border: none;
    border-radius: 5px;
    background: var(--control);
    box-shadow: none;
    color: var(--text);
    font-size: 13px;
    white-space: nowrap;
  }
  .buttons button:hover:not(:disabled) { background: color-mix(in srgb, var(--control) 86%, var(--text)); }
  .buttons .primary { background: var(--accent); color: var(--accent-text); }
  .buttons .primary:hover:not(:disabled) { background: var(--accent); filter: brightness(1.08); }
</style>
