<script lang="ts">
  // A pop-up menu: a borderless trigger (text and a round ⌄ badge) and a popover anchored under it.
  // Keyboard: Enter, Space or ↓ open it on the first item; ↑ ↓ Home End move; Esc closes; Tab leaves.
  import type { Snippet } from "svelte";
  import { ChevronDown } from "@lucide/svelte";
  let {
    label,
    trigger,
    children,
    role = "menu",
    disabled = false,
    title,
  }: {
    /** Accessible name of the trigger and the menu. */
    label: string;
    /** Trigger content; the label when missing. */
    trigger?: Snippet;
    /** The items (buttons, <hr> separators); gets `close`. */
    children: Snippet<[() => void]>;
    role?: "menu" | "dialog";
    disabled?: boolean;
    title?: string;
  } = $props();

  const id = $props.id();
  let button: HTMLButtonElement | undefined = $state();
  let menu: HTMLDivElement | undefined = $state();
  let expanded = $state(false);
  let enter = false;

  const items = () => [...(menu?.querySelectorAll<HTMLElement>("button:not(:disabled), input") ?? [])];
  const close = () => menu?.hidePopover();

  $effect(() => {
    const m = menu;
    if (!m) return;
    const toggled = (e: Event) => {
      expanded = (e as ToggleEvent).newState === "open";
      if (!expanded) return;
      if (role === "menu") m.querySelectorAll("button:not([role])").forEach((b) => b.setAttribute("role", "menuitem"));
      if (!CSS.supports("position-area: block-end") && button) {
        // No anchor positioning: place it under the trigger, right-aligned.
        const r = button.getBoundingClientRect();
        m.style.left = `${Math.max(8, r.right - m.offsetWidth)}px`;
        m.style.top = `${r.bottom + 4}px`;
      }
      // Move focus here, not on keydown: the menu is open only now.
      if (enter) items()[0]?.focus();
      enter = false;
    };
    m.addEventListener("toggle", toggled);
    return () => m.removeEventListener("toggle", toggled);
  });

  function triggerKey(e: KeyboardEvent) {
    if (e.key === "Enter" || e.key === " ") enter = true;
    else if (e.key === "ArrowDown") {
      e.preventDefault();
      if (menu?.matches(":popover-open")) items()[0]?.focus();
      else {
        enter = true;
        button?.click();
      }
    }
  }

  function menuKey(e: KeyboardEvent) {
    const list = items();
    if (!list.length) return;
    const i = list.indexOf(document.activeElement as HTMLElement);
    const inField = document.activeElement instanceof HTMLInputElement;
    let n: number | undefined;
    if (e.key === "ArrowDown") n = (i + 1) % list.length;
    else if (e.key === "ArrowUp") n = i <= 0 ? list.length - 1 : i - 1;
    else if (e.key === "Home" && !inField) n = 0;
    else if (e.key === "End" && !inField) n = list.length - 1;
    if (n === undefined) return;
    e.preventDefault();
    list[n].focus();
  }

  // Hover follows focus, so the pointer and the keyboard highlight the same item.
  function hover(e: PointerEvent) {
    const b = (e.target as Element).closest("button");
    if (b && b !== document.activeElement && menu?.contains(b)) b.focus({ preventScroll: true });
  }
</script>

<button
  bind:this={button}
  type="button"
  class="trigger"
  aria-haspopup={role}
  aria-expanded={expanded}
  {disabled}
  {title}
  onkeydown={triggerKey}
  {...{ commandfor: id, command: "toggle-popover" } as any}
>
  {#if trigger}{@render trigger()}{:else}<span class="text">{label}</span>{/if}
  <span class="badge" aria-hidden="true"><ChevronDown size={10} strokeWidth={2.6} /></span>
</button>
<div bind:this={menu} {id} popover="auto" class="menu" {role} tabindex="-1" aria-label={label} onkeydown={menuKey} onpointermove={hover}>
  {@render children(close)}
</div>

<style>
  .trigger {
    display: inline-flex;
    align-items: center;
    gap: 15px;
    min-width: 0;
    height: 23px;
    min-height: 0;
    padding: 0 5px 0 4px;
    border: none;
    border-radius: 6px;
    background: none;
    box-shadow: none;
    color: var(--text);
    font-size: 13px;
  }
  .trigger:hover:not(:disabled) { background: none; }
  .text { overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .badge {
    flex: none;
    display: grid;
    place-items: center;
    width: 17px;
    height: 17px;
    border-radius: 50%;
    background: color-mix(in srgb, var(--text) 8%, transparent);
  }
  .trigger[aria-expanded="true"] .badge { background: color-mix(in srgb, var(--text) 16%, transparent); }

  .menu {
    position: fixed;
    inset: auto;
    margin: 4px 0;
    position-area: block-end span-inline-start;
    position-try-fallbacks: flip-block, flip-inline, flip-block flip-inline;
    min-width: 190px;
    max-height: min(440px, 70vh);
    overflow: auto;
    padding: 5px;
    border: 1px solid var(--border-strong);
    border-radius: 10px;
    background: color-mix(in srgb, var(--text) 7%, var(--bg));
    box-shadow: 0 10px 30px rgba(0, 0, 0, 0.35);
    color: var(--text);
    font-size: 13px;
  }
  .menu :global(button) {
    display: flex;
    align-items: center;
    gap: 8px;
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
  .menu :global(button:focus) { outline: none; background: var(--accent); color: var(--accent-text); }
  .menu :global(button:disabled) { opacity: 0.4; }
  .menu :global(hr) { height: 1px; margin: 5px 10px; border: none; background: var(--border-strong); }
  @media (prefers-reduced-motion: no-preference) {
    .menu { opacity: 0; transition: opacity 0.12s, display 0.12s allow-discrete, overlay 0.12s allow-discrete; }
    .menu:popover-open { opacity: 1; }
    @starting-style { .menu:popover-open { opacity: 0; } }
  }
</style>
