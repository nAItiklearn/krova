<script lang="ts">
  import type { Layout } from '../lib/bindings';
  import { rectStyle } from '../lib/geometry';

  // Decorative miniature of a layout (the name is always shown next to it).
  let { layout, ratio = 16 / 9 }: { layout: Layout; ratio?: number } = $props();
</script>

<span class="thumb" style:aspect-ratio={ratio} aria-hidden="true">
  {#each layout.zones as z (z.id)}
    <span class="z" style={rectStyle(z.rect)}></span>
  {/each}
</span>

<style>
  .thumb {
    position: relative;
    display: block;
    width: 52px;
    flex: none;
    background: var(--canvas-bg);
    border: 1px solid var(--border-strong);
    border-radius: 4px;
    overflow: hidden;
  }

  .z {
    position: absolute;
    box-sizing: border-box;
    border: 1px solid var(--zone-border);
    background: var(--zone-bg);
    border-radius: 2px;
  }
</style>
