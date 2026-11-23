// One pop-up menu for the whole window (context menus and pull-down buttons), drawn by MenuHost.svelte.

export type MenuItem =
  | { label: string; action: () => void; disabled?: boolean; checked?: boolean }
  | { separator: true };

export const menu = $state({
  open: false,
  items: [] as MenuItem[],
  x: 0,
  y: 0,
  /** Pull-down: the button's edge the menu lines up with, and its bottom (to flip above it). */
  alignRight: false,
  anchorTop: 0,
  minWidth: 0,
  /** Opened from the keyboard: focus the first item. */
  keyboard: false,
});

/** Right-click menu at the pointer. */
export function contextMenu(e: MouseEvent, items: MenuItem[]) {
  e.preventDefault();
  e.stopPropagation();
  Object.assign(menu, { items, x: e.clientX, y: e.clientY, alignRight: false, anchorTop: e.clientY, minWidth: 0, keyboard: e.button !== 2, open: true });
}

let closedAt = 0;
let anchor: Element | null = null;

/** MenuHost reports closing, so a second click on the same button closes instead of reopening. */
export function menuClosed() {
  closedAt = performance.now();
}

/** Menu under a button (pull-down). */
export function dropdown(el: HTMLElement, items: MenuItem[], opts: { alignRight?: boolean; keyboard?: boolean } = {}) {
  if (anchor === el && performance.now() - closedAt < 250) return;
  anchor = el;
  const r = el.getBoundingClientRect();
  Object.assign(menu, {
    items,
    x: opts.alignRight ? r.right : r.left,
    y: r.bottom + 4,
    alignRight: !!opts.alignRight,
    anchorTop: r.top - 4,
    minWidth: r.width,
    keyboard: !!opts.keyboard,
    open: true,
  });
}
