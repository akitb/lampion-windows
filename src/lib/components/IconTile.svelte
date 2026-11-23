<script lang="ts">
  // A rounded tile with a vertical gradient and a white glyph, like the macOS sidebar and card icons.
  // `tint` is a CSS colour; `top` (optional) the lighter top colour of the gradient.
  import type { Component } from "svelte";
  let { icon: Icon, tint, top, size = 20 }: { icon: Component<any>; tint: string; top?: string; size?: number } = $props();
</script>

<span class="tile" style="width:{size}px;height:{size}px;border-radius:{Math.round(size * 0.24)}px;background:linear-gradient(180deg,{top ?? `color-mix(in srgb, ${tint} 78%, white)`},{tint})">
  <Icon size={Math.round(size * 0.62)} strokeWidth={2} color="#fff" />
</span>

<style>
  .tile { display: inline-flex; align-items: center; justify-content: center; flex: none; }
</style>
