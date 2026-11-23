<script lang="ts" module>
  export type MenuItem = { label: string; action: () => void; destructive?: boolean } | "separator";

  /** Where a context menu opens: the pointer, or the element when it came from the keyboard. */
  export function menuPoint(e: MouseEvent): { x: number; y: number } {
    if (e.clientX || e.clientY) return { x: e.clientX, y: e.clientY };
    const r = (e.target as Element).getBoundingClientRect();
    return { x: r.left + 8, y: r.bottom };
  }
</script>

<script lang="ts">
  // A right-click menu at a point, as a popover: ↑ ↓ Home End move, Enter runs, Esc or a click outside closes.
  import { onMount } from "svelte";
  let { items, x, y, onclose }: { items: MenuItem[]; x: number; y: number; onclose: () => void } = $props();
  let menu = $state<HTMLDivElement | undefined>();
  let left = $state(0);
  let top = $state(0);

  onMount(() => {
    if (!menu) return;
    menu.showPopover();
    // Keep it inside the window.
    left = Math.max(4, Math.min(x, innerWidth - menu.offsetWidth - 4));
    top = Math.max(4, y + menu.offsetHeight > innerHeight - 4 ? y - menu.offsetHeight : y);
    menu.querySelector<HTMLElement>("button")?.focus();
  });

  const buttons = () => [...(menu?.querySelectorAll<HTMLElement>("button") ?? [])];

  function key(e: KeyboardEvent) {
    const list = buttons();
    const i = list.indexOf(document.activeElement as HTMLElement);
    const n = { ArrowDown: (i + 1) % list.length, ArrowUp: i <= 0 ? list.length - 1 : i - 1, Home: 0, End: list.length - 1 }[e.key];
    if (n === undefined) return;
    e.preventDefault();
    list[n]?.focus();
  }

  function run(action: () => void) {
    menu?.hidePopover();
    action();
  }
</script>

<div
  bind:this={menu}
  class="menu"
  popover="auto"
  role="menu"
  tabindex="-1"
  style:left="{left}px"
  style:top="{top}px"
  onkeydown={key}
  ontoggle={(e) => (e as ToggleEvent).newState === "closed" && onclose()}
  onpointermove={(e) => (e.target as Element).closest("button")?.focus({ preventScroll: true })}
>
  {#each items as item, i (i)}
    {#if item === "separator"}
      <hr />
    {:else}
      <button type="button" role="menuitem" class:destructive={item.destructive} onclick={() => run(item.action)}>{item.label}</button>
    {/if}
  {/each}
</div>

<style>
  .menu {
    position: fixed;
    inset: auto;
    margin: 0;
    min-width: 170px;
    padding: 5px;
    border: 1px solid var(--border-strong);
    border-radius: 10px;
    background: color-mix(in srgb, var(--text) 7%, var(--bg));
    box-shadow: 0 10px 30px rgba(0, 0, 0, 0.35);
    color: var(--text);
    font-size: 13px;
  }
  button {
    display: flex;
    align-items: center;
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
  button.destructive { color: var(--red); }
  button:focus { outline: none; background: var(--accent); color: var(--accent-text); }
  hr { height: 1px; margin: 5px 10px; border: none; background: var(--border-strong); }
</style>
