<script lang="ts">
  // The "…" menu on an Overview card: a popover menu under its button. Arrow keys, Home/End (the
  // browser's focusgroup, or the small script below where it is missing), Esc and clicking outside close it.
  import { Ellipsis } from "@lucide/svelte";

  /** `null` draws a separator. */
  type Item = { label: string; run: () => void } | null;
  let { items, label }: { items: Item[]; label: string } = $props();

  const id = $props.id();
  let trigger: HTMLButtonElement;
  let menu: HTMLDivElement;
  let open = $state(false);
  /** Opened from the keyboard: focus moves to the first item (a click keeps it on the button). */
  let enter = false;
  const native = typeof HTMLElement !== "undefined" && "focusGroup" in HTMLElement.prototype;

  const first = () => menu.querySelector<HTMLButtonElement>("button");

  // Focus moves in `toggle`: the menu is not open yet when the command fires.
  function onToggle(e: ToggleEvent) {
    open = e.newState === "open";
    if (open && enter) first()?.focus();
    enter = false;
  }

  function onTriggerKey(e: KeyboardEvent) {
    if (e.key === "Enter" || e.key === " ") enter = true; // the button's own click opens it
    else if (e.key === "ArrowDown") {
      e.preventDefault();
      if (open) first()?.focus();
      else {
        enter = true;
        trigger.click();
      }
    }
  }

  function choose(run: () => void) {
    menu.hidePopover(); // focus goes back to the button
    run();
  }

  // Tabbing out closes the menu, so focus never stays in hidden content.
  function onFocusOut(e: FocusEvent) {
    const to = e.relatedTarget as Node | null;
    if (open && to && to !== trigger && !menu.contains(to)) menu.hidePopover();
  }

  function onMenuKey(e: KeyboardEvent) {
    if (native || !["ArrowDown", "ArrowUp", "Home", "End"].includes(e.key)) return;
    const all = [...menu.querySelectorAll<HTMLButtonElement>("button:not(:disabled)")];
    const i = all.indexOf(document.activeElement as HTMLButtonElement);
    if (i < 0) return;
    const next = e.key === "Home" ? 0 : e.key === "End" ? all.length - 1 : (i + (e.key === "ArrowUp" ? -1 : 1) + all.length) % all.length;
    e.preventDefault();
    all[next].focus();
  }
</script>

<button
  bind:this={trigger}
  id="{id}-button"
  class="more"
  type="button"
  aria-label={label}
  title={label}
  aria-haspopup="menu"
  aria-expanded={open}
  commandfor="{id}-menu"
  command="toggle-popover"
  onkeydown={onTriggerKey}
>
  <Ellipsis size={17} />
</button>
<div bind:this={menu} id="{id}-menu" class="menu" popover="auto" ontoggle={onToggle} onfocusout={onFocusOut}>
  <!-- svelte-ignore a11y_no_static_element_interactions (the browser's focusgroup makes it a menu; the role is set where it is missing) -->
  <div class="items" {...{ focusgroup: "menu nomemory" }} role={native ? undefined : "menu"} aria-labelledby="{id}-button" onkeydown={onMenuKey}>
    {#each items as item, i (i)}
      {#if item}
        <button type="button" role={native ? undefined : "menuitem"} onclick={() => choose(item.run)} onpointermove={(e) => e.currentTarget.focus()}>
          {item.label}
        </button>
      {:else}
        <hr />
      {/if}
    {/each}
  </div>
</div>

<style>
  .more {
    display: inline-flex; align-items: center; justify-content: center; flex: none;
    width: 20px; height: 24px; min-height: 0; padding: 0; border: none; background: none; box-shadow: none; border-radius: 5px;
    color: var(--text-2);
  }
  .more:hover:not(:disabled), .more[aria-expanded="true"] { background: var(--hover); }

  .menu {
    padding: 5px; min-width: 190px;
    border: 1px solid var(--border-strong); border-radius: 8px;
    background: var(--surface-2); color: var(--text);
    box-shadow: 0 10px 28px rgba(0, 0, 0, 0.28);
  }
  /* Under the button, right edges aligned; flips where it would not fit. */
  @supports (position-area: block-end) {
    .menu {
      inset: auto; margin: 4px 0 0;
      position-area: block-end span-inline-start;
      position-try-fallbacks: flip-block, flip-inline, flip-block flip-inline;
    }
  }
  .items { display: flex; flex-direction: column; }
  .items button {
    display: flex; align-items: center; height: 22px; min-height: 0; padding: 0 10px;
    border: none; border-radius: 4px; background: none; box-shadow: none;
    color: inherit; font-size: 13px; text-align: left; white-space: nowrap;
  }
  /* Hover moves focus, so the focused item is the highlighted one, as in a native menu. */
  .items button:focus { outline: none; background: var(--accent); color: var(--accent-text); }
  .items hr { border: none; border-top: 1px solid var(--border-strong); margin: 4px 6px; }

  @media (prefers-reduced-motion: no-preference) {
    .menu {
      opacity: 0; translate: 0 -4px;
      transition: opacity 0.15s, translate 0.15s, display 0.15s, overlay 0.15s;
      transition-behavior: allow-discrete;
    }
    .menu:popover-open { opacity: 1; translate: 0 0; transition-duration: 0.1s; }
    @starting-style {
      .menu:popover-open { opacity: 0; translate: 0 -4px; }
    }
  }
</style>
