<script lang="ts">
  // "What's New": the release notes in a popover under the banner's button (ReleaseNotes in
  // UpdateCard.swift), with Skip This Version and View on GitHub. The button opens and closes it
  // (`popovertarget={id}`); Esc or a click outside closes it too.
  import { core } from "$lib/api";
  import type { Release } from "$lib/types";

  let { id, release, anchor, onskip }: { id: string; release: Release; anchor: HTMLElement | undefined; onskip: () => void } = $props();

  const titleId = $props.id();
  let pop: HTMLDivElement | undefined = $state();

  /** Under the button, centred on it, inside the window (its width is fixed, so no measuring). */
  function place(e: ToggleEvent) {
    if (e.newState !== "open" || !pop || !anchor) return;
    const r = anchor.getBoundingClientRect();
    const w = Math.min(440, innerWidth - 16);
    pop.style.left = `${Math.max(8, Math.min(r.left + r.width / 2 - w / 2, innerWidth - w - 8))}px`;
    pop.style.top = `${r.bottom + 6}px`;
  }

  type Piece = { text: string; kind: "plain" | "bold" | "italic" | "code" | "link"; url?: string };

  /** Inline Markdown: **bold**, *italic*, `code` and [links](https://…). */
  function inline(md: string): Piece[] {
    const out: Piece[] = [];
    const re = /\*\*(.+?)\*\*|`([^`]+)`|\[([^\]]+)\]\((https?:\/\/[^)\s]+)\)|(?<![\w*])\*(?!\s)([^*]+?)\*(?![\w*])|(?<!\w)_(?!\s)([^_]+?)_(?!\w)/g;
    let at = 0;
    for (const m of md.matchAll(re)) {
      const start = m.index ?? 0;
      if (start > at) out.push({ text: md.slice(at, start), kind: "plain" });
      if (m[1] !== undefined) out.push({ text: m[1], kind: "bold" });
      else if (m[2] !== undefined) out.push({ text: m[2], kind: "code" });
      else if (m[3] !== undefined) out.push({ text: m[3], kind: "link", url: m[4] });
      else out.push({ text: m[5] ?? m[6] ?? "", kind: "italic" });
      at = start + m[0].length;
    }
    if (at < md.length) out.push({ text: md.slice(at), kind: "plain" });
    return out;
  }

  type Line = { kind: "heading" | "bullet" | "text"; text: string };
  const lines = $derived(
    release.notes
      .split("\n")
      .map((l) => l.trim())
      .filter(Boolean)
      .flatMap((l): Line[] => {
        if (l.startsWith("#")) {
          const text = l.replace(/^[#\s]+/, "");
          // The popover has its own title, so a leading "What's new" heading is dropped.
          return text.toLowerCase().startsWith("what") ? [] : [{ kind: "heading", text }];
        }
        if (l.startsWith("- ") || l.startsWith("* ")) return [{ kind: "bullet", text: l.slice(2) }];
        return [{ kind: "text", text: l }];
      }),
  );
</script>

{#snippet markdown(md: string)}
  {#each inline(md) as p, i (i)}
    {#if p.kind === "bold"}<strong>{p.text}</strong>
    {:else if p.kind === "italic"}<em>{p.text}</em>
    {:else if p.kind === "code"}<code>{p.text}</code>
    {:else if p.kind === "link"}<a
        href={p.url}
        onclick={(e) => {
          e.preventDefault();
          if (p.url) void core.openUrl(p.url);
        }}>{p.text}</a
      >
    {:else}{p.text}{/if}
  {/each}
{/snippet}

<div bind:this={pop} {id} class="notes" popover="auto" role="dialog" aria-labelledby={titleId} onbeforetoggle={place}>
  <h2 id={titleId}>What’s New in {release.version}</h2>
  <div class="body">
    {#if lines.length === 0}<p class="none">No release notes were published for this version.</p>{/if}
    {#each lines as line, i (i)}
      {#if line.kind === "heading"}
        <h3>{line.text}</h3>
      {:else if line.kind === "bullet"}
        <div class="bullet"><span class="mark" aria-hidden="true">•</span><span>{@render markdown(line.text)}</span></div>
      {:else}
        <p>{@render markdown(line.text)}</p>
      {/if}
    {/each}
  </div>
  <div class="foot">
    <button
      type="button"
      title="Don’t offer {release.version} again"
      onclick={() => {
        pop?.hidePopover();
        onskip();
      }}>Skip This Version</button
    >
    <span class="grow"></span>
    <button type="button" onclick={() => core.openUrl(release.page)}>View on GitHub</button>
  </div>
</div>

<style>
  .notes {
    position: fixed;
    inset: auto;
    width: 440px;
    max-width: calc(100vw - 16px);
    margin: 0;
    padding: 0;
    border: 1px solid var(--border-strong);
    border-radius: 10px;
    background: var(--surface-2);
    box-shadow: 0 10px 30px rgba(0, 0, 0, 0.3);
    color: var(--text);
  }
  h2 { margin: 0; padding: 16px; font-size: 13px; line-height: 16px; font-weight: 600; }
  .body {
    display: flex;
    flex-direction: column;
    gap: 8px;
    max-height: 300px;
    overflow: auto;
    padding: 16px;
    border-block: 1px solid color-mix(in srgb, var(--text) 12%, transparent);
    font-size: 13px;
    line-height: 17px;
    user-select: text;
  }
  h3 { margin: 0; font-size: 11px; font-weight: 600; }
  p { margin: 0; }
  .none { color: var(--text-2); }
  .bullet { display: flex; gap: 8px; }
  .mark { color: var(--text-2); }
  code { font-size: 12px; }
  a { color: var(--link); text-decoration: none; }
  a:hover { text-decoration: underline; }
  .foot { display: flex; align-items: center; padding: 12px; }
  .grow { flex: 1; }
  .foot button {
    height: 24px;
    min-height: 0;
    padding: 0 12px;
    border: none;
    border-radius: 5px;
    background: var(--control);
    box-shadow: none;
    color: var(--text);
    font-size: 13px;
  }
  .foot button:hover:not(:disabled) { background: color-mix(in srgb, var(--control) 86%, var(--text)); }
  @media (prefers-reduced-motion: no-preference) {
    .notes { opacity: 0; transition: opacity 0.15s, display 0.15s allow-discrete, overlay 0.15s allow-discrete; }
    .notes:popover-open { opacity: 1; }
    @starting-style { .notes:popover-open { opacity: 0; } }
  }
</style>
