<script lang="ts">
  // The notice at the top of the main window until the servers are set up: the folder choice on first
  // launch, the Visual C++ runtime, download progress (Cancel), and missing files or a failure (Try Again).
  // It fades in and settles from just above; with reduced motion it only fades. Renders nothing when ready.
  import { cubicOut } from "svelte/easing";
  import { FolderOpen, Package, Wrench } from "@lucide/svelte";
  import { core } from "$lib/api";
  import { servers, setup, startSetupState } from "$lib/servers.svelte";

  startSetupState();

  const st = $derived(setup.state);
  /** Why a chosen folder was refused, or why an action failed. */
  let problem = $state<string | null>(null);
  const packages = ["Apache", "PHP", "MariaDB", "phpMyAdmin"];
  const onlyConfig = $derived(st?.phase === "missing" && !st.missing.some((m) => packages.includes(m)));

  async function act(f: () => Promise<unknown>) {
    problem = null;
    try {
      await f();
    } catch (e) {
      problem = String(e);
    }
  }

  function changeFolder() {
    return act(async () => {
      const path = await core.pickPath({ kind: "folder", title: "Choose the Lampion folder", startDir: st?.root });
      if (path) await servers.setupChooseRoot(path);
    });
  }

  function notice(node: HTMLElement) {
    const reduce = matchMedia("(prefers-reduced-motion: reduce)").matches;
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

{#if st && st.phase !== "ready"}
  <div class="wrap" transition:notice>
    <div class="notice">
      <span class="icon" aria-hidden="true">
        {#if st.phase === "running"}<span class="spinner"></span>
        {:else if onlyConfig}<Wrench size={26} strokeWidth={1.8} />
        {:else}<Package size={26} strokeWidth={1.8} />{/if}
      </span>
      <div class="text">
        <strong aria-live="polite">{st.title}</strong>
        {#if st.detail}<span class="detail">{st.detail}</span>{/if}
        {#if st.phase === "chooseFolder"}<code class="path">{st.root}</code>{/if}
        {#if st.phase === "running" && st.progress !== null}
          <progress max="1" value={st.progress} aria-label={st.title}></progress>
        {/if}
        {#if problem}<span class="problem" role="alert">{problem}</span>{/if}
      </div>
      <div class="buttons">
        {#if st.canChangeRoot && st.phase !== "running"}
          <button onclick={changeFolder}><FolderOpen size={14} strokeWidth={2} /> Change Folder…</button>
        {/if}
        {#if st.phase === "chooseFolder"}
          <button class="primary" onclick={() => act(servers.setupRun)}>Set Up</button>
        {:else if st.phase === "needsRuntime"}
          <button class="primary" onclick={() => act(servers.setupInstallRuntime)}>Install Runtime</button>
        {:else if st.phase === "failed" || st.phase === "missing"}
          <button class="primary" onclick={() => act(servers.setupRetry)}>Try Again</button>
        {:else if st.phase === "running" && st.canCancel}
          <button onclick={() => act(servers.setupCancel)}>Cancel</button>
        {/if}
      </div>
    </div>
  </div>
{/if}

<style>
  .wrap { padding: 12px 16px 4px; flex: none; }
  .notice {
    display: flex; align-items: center; gap: 14px; padding: 11px 13px;
    border: 1px solid var(--border); border-radius: 12px; background: var(--surface); box-shadow: var(--shadow);
  }
  .icon { width: 40px; flex: none; display: flex; justify-content: center; color: var(--orange); }
  .spinner {
    width: 18px; height: 18px; border-radius: 50%;
    border: 2px solid color-mix(in srgb, var(--text-2) 35%, transparent); border-top-color: var(--text-2);
    animation: spin 0.9s linear infinite;
  }
  @keyframes spin { to { transform: rotate(360deg); } }
  .text { flex: 1; min-width: 0; display: flex; flex-direction: column; gap: 2px; }
  strong { font-size: 13px; line-height: 16px; font-weight: 600; color: var(--text); }
  .detail { font-size: 12px; line-height: 15px; color: var(--text-2); }
  .path { font-family: var(--mono); font-size: 12px; line-height: 15px; color: var(--text); user-select: text; overflow-wrap: anywhere; }
  .problem { font-size: 12px; line-height: 15px; color: var(--red); user-select: text; }
  progress {
    appearance: none; width: 100%; max-width: 420px; height: 4px; margin-top: 5px;
    border: none; border-radius: 2px; background: var(--control); overflow: hidden;
  }
  progress::-webkit-progress-bar { background: var(--control); border-radius: 2px; }
  progress::-webkit-progress-value { background: var(--accent); border-radius: 2px; transition: width var(--fade); }
  .buttons { display: flex; gap: 8px; flex: none; }
  .buttons button {
    display: inline-flex; align-items: center; gap: 6px; height: 24px; min-height: 0; padding: 0 12px;
    border: none; border-radius: 5px; box-shadow: none; background: var(--control); color: var(--text); font-size: 13px; white-space: nowrap;
  }
  .buttons button:hover:not(:disabled) { background: color-mix(in srgb, var(--control) 86%, var(--text)); }
  .buttons .primary { background: var(--accent); color: var(--accent-text); }
  .buttons .primary:hover:not(:disabled) { background: var(--accent); filter: brightness(1.08); }
</style>
