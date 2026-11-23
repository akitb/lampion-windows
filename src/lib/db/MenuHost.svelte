<script lang="ts">
  // The window's pop-up menu (see menu.svelte.ts). Keyboard: ↑ ↓ Home End move, Enter picks, Esc closes.
  import { Check } from "@lucide/svelte";
  import { menu, menuClosed, type MenuItem } from "./menu.svelte";

  let el: HTMLDivElement | undefined = $state();
  const checks = $derived(menu.items.some((i) => "label" in i && i.checked !== undefined));

  $effect(() => {
    const m = el;
    if (!m) return;
    if (menu.open) {
      // Read the items so a new menu while one is open re-positions.
      void menu.items;
      if (!m.matches(":popover-open")) m.showPopover();
      place(m);
    } else if (m.matches(":popover-open")) m.hidePopover();
  });

  function place(m: HTMLDivElement) {
    m.style.minWidth = `${Math.max(menu.minWidth, 120)}px`;
    const w = m.offsetWidth;
    const h = m.offsetHeight;
    let x = menu.alignRight ? menu.x - w : menu.x;
    let y = menu.y;
    x = Math.max(4, Math.min(x, innerWidth - w - 4));
    if (y + h > innerHeight - 4) y = Math.max(4, menu.anchorTop - h);
    m.style.left = `${x}px`;
    m.style.top = `${y}px`;
    if (menu.keyboard) buttons()[0]?.focus({ preventScroll: true });
    else m.focus({ preventScroll: true });
  }

  const buttons = () => [...(el?.querySelectorAll<HTMLButtonElement>("button:not(:disabled)") ?? [])];

  function pick(item: MenuItem) {
    if (!("label" in item)) return;
    menu.open = false;
    item.action();
  }

  function keys(e: KeyboardEvent) {
    const list = buttons();
    if (!list.length) return;
    const i = list.indexOf(document.activeElement as HTMLButtonElement);
    let n: number | undefined;
    if (e.key === "ArrowDown") n = (i + 1) % list.length;
    else if (e.key === "ArrowUp") n = i <= 0 ? list.length - 1 : i - 1;
    else if (e.key === "Home") n = 0;
    else if (e.key === "End") n = list.length - 1;
    else if (e.key === "Tab") {
      menu.open = false;
      return;
    }
    if (n === undefined) return;
    e.preventDefault();
    list[n].focus();
  }

  function hover(e: PointerEvent) {
    const b = (e.target as Element).closest("button");
    if (b && !b.disabled && b !== document.activeElement) b.focus({ preventScroll: true });
  }
</script>

<div
  bind:this={el}
  popover="auto"
  class="menu"
  role="menu"
  tabindex="-1"
  onkeydown={keys}
  onpointermove={hover}
  ontoggle={(e) => {
    if ((e as ToggleEvent).newState === "closed") {
      menu.open = false;
      menuClosed();
    }
  }}
>
  {#each menu.items as item, i (i)}
    {#if "separator" in item}
      <hr />
    {:else}
      <button type="button" role="menuitem" disabled={item.disabled} onclick={() => pick(item)}>
        {#if checks}<span class="check">{#if item.checked}<Check size={12} strokeWidth={2.6} />{/if}</span>{/if}
        {item.label}
      </button>
    {/if}
  {/each}
</div>

<style>
  .menu {
    position: fixed;
    inset: auto;
    margin: 0;
    max-height: min(480px, 80vh);
    overflow: auto;
    padding: 5px;
    border: 1px solid var(--border-strong);
    border-radius: 10px;
    background: var(--db-menu);
    box-shadow: 0 10px 30px rgba(0, 0, 0, 0.35);
    color: var(--text);
    font-size: 13px;
    outline: none;
  }
  button {
    display: flex;
    align-items: center;
    gap: 4px;
    width: 100%;
    height: 22px;
    min-height: 0;
    padding: 0 10px;
    border: none;
    border-radius: 5px;
    background: none;
    box-shadow: none;
    color: inherit;
    font-size: 13px;
    text-align: left;
    white-space: nowrap;
  }
  button:focus { outline: none; background: var(--accent); color: var(--accent-text); }
  button:disabled { opacity: 0.4; }
  .check { display: inline-flex; width: 14px; margin-left: -4px; flex: none; }
  hr { height: 1px; margin: 5px 10px; border: none; background: var(--border-strong); }
  @media (prefers-reduced-motion: no-preference) {
    .menu { opacity: 0; transition: opacity 0.1s, display 0.1s allow-discrete, overlay 0.1s allow-discrete; }
    .menu:popover-open { opacity: 1; }
    @starting-style { .menu:popover-open { opacity: 0; } }
  }
</style>
