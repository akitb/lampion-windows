<script lang="ts">
  // A rounded colored tile with a white glyph, like the macOS sidebar icons.
  import type { Component } from "svelte";
  let { icon: Icon, tint, size = 20 }: { icon: Component<any>; tint: string; size?: number } = $props();
</script>

<span class="tile" style="width:{size}px;height:{size}px;background:{tint}">
  <Icon size={Math.round(size * 0.62)} strokeWidth={2.2} color="#fff" />
</span>

<style>
  .tile { display: inline-flex; align-items: center; justify-content: center; border-radius: 5px; flex: none; }
</style>
