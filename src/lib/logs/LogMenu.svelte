<script module lang="ts">
  /** `null` draws a separator. */
  export type MenuItem = { label: string; run: () => void; disabled?: boolean } | null;
</script>

<script lang="ts">
  // A pop-up menu: under a button (right edges aligned) or at the pointer (a row's context menu).
  // Arrow keys, Home and End move, Enter chooses, Esc and a click outside close it; focus goes back.
  import { tick } from "svelte";

  let { items, label }: { items: MenuItem[]; label: string } = $props();

  let menu: HTMLDivElement;
  let back: HTMLElement | null = null;
  let from: unknown = null;
  let closedAt = 0;

  const enabled = () => [...menu.querySelectorAll<HTMLButtonElement>("button:not(:disabled)")];

  /** Opens it under `at` (an element) or at a point. `keyboard`: focus the first item. */
  export async function show(at: HTMLElement | { x: number; y: number }, keyboard = false) {
    // Clicking its button again closes it: the press outside already did, so it must not open again.
    if (at === from && performance.now() - closedAt < 300) return;
    from = at;
    back = document.activeElement as HTMLElement | null;
    await tick(); // items given just before are drawn
    menu.showPopover();
    const { offsetWidth: w, offsetHeight: h } = menu;
    let x: number, y: number;
    if (at instanceof HTMLElement) {
      const r = at.getBoundingClientRect();
      x = r.right - w;
      y = r.bottom + 4 + h > innerHeight - 8 ? r.top - 4 - h : r.bottom + 4;
    } else {
      x = at.x + w > innerWidth - 8 ? at.x - w : at.x;
      y = at.y + h > innerHeight - 8 ? at.y - h : at.y;
    }
    menu.style.left = `${Math.max(8, Math.min(x, innerWidth - w - 8))}px`;
    menu.style.top = `${Math.max(8, y)}px`;
    (keyboard ? enabled()[0] : menu)?.focus();
  }

  function toggled(e: ToggleEvent) {
    if (e.newState === "closed" && (menu.contains(document.activeElement) || document.activeElement === document.body)) back?.focus();
  }

  function choose(run: () => void) {
    menu.hidePopover();
    run();
  }

  function keys(e: KeyboardEvent) {
    const all = enabled();
    if (!all.length || !["ArrowDown", "ArrowUp", "Home", "End"].includes(e.key)) return;
    e.preventDefault();
    const i = all.indexOf(document.activeElement as HTMLButtonElement);
    const n = e.key === "Home" ? 0 : e.key === "End" ? all.length - 1 : e.key === "ArrowDown" ? (i + 1) % all.length : (i <= 0 ? all.length : i) - 1;
    all[n].focus();
  }
</script>

<div
  bind:this={menu}
  class="menu"
  popover="auto"
  role="menu"
  aria-label={label}
  tabindex="-1"
  onbeforetoggle={(e) => {
    if (e.newState === "closed") closedAt = performance.now(); // sync, before the click that caused it
  }}
  ontoggle={toggled}
  onkeydown={keys}
>
  {#each items as item, i (i)}
    {#if item}
      <button
        type="button"
        role="menuitem"
        disabled={item.disabled}
        onclick={() => choose(item.run)}
        onpointermove={(e) => e.currentTarget.focus({ preventScroll: true })}
      >
        {item.label}
      </button>
    {:else}
      <hr />
    {/if}
  {/each}
</div>

<style>
  .menu {
    position: fixed;
    inset: auto;
    margin: 0;
    min-width: 190px;
    padding: 5px;
    border: 1px solid var(--border-strong);
    border-radius: 8px;
    background: var(--surface-2);
    box-shadow: 0 10px 28px rgba(0, 0, 0, 0.28);
    color: var(--text);
    outline: none;
  }
  button {
    display: flex;
    align-items: center;
    width: 100%;
    height: 22px;
    min-height: 0;
    padding: 0 10px;
    border: none;
    border-radius: 4px;
    background: none;
    box-shadow: none;
    color: inherit;
    font-size: 13px;
    text-align: left;
    white-space: nowrap;
  }
  /* Hover moves focus, so the pointer and the keyboard highlight the same item. */
  button:focus { outline: none; background: var(--accent); color: var(--accent-text); }
  button:disabled { opacity: 0.4; }
  hr { border: none; border-top: 1px solid var(--border-strong); margin: 4px 6px; }
  @media (prefers-reduced-motion: no-preference) {
    .menu { opacity: 0; transition: opacity 0.12s, display 0.12s allow-discrete, overlay 0.12s allow-discrete; }
    .menu:popover-open { opacity: 1; }
    @starting-style { .menu:popover-open { opacity: 0; } }
  }
</style>
