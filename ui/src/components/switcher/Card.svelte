<script lang="ts">
  // One layout in the switcher deck: name, Active badge, zone count, reorder
  // grip and a live miniature of its zones. Position, scale, tilt and opacity
  // are written by Deck's frame loop straight to `el` (no reactive state per
  // frame); `shadeEl` darkens cards stacked behind, `hintEl` is the red edge
  // shown while the card is swiped away.
  import type { Layout } from '../../lib/bindings';

  let {
    layout,
    active,
    selected,
    position,
    count,
    leaving = false,
    el = $bindable(),
    shadeEl = $bindable(),
    hintEl = $bindable(),
  }: {
    layout: Layout;
    active: boolean;
    selected: boolean;
    /** 1-based position in the list */
    position: number;
    count: number;
    /** Being swiped away: still drawn, gone for assistive tech. */
    leaving?: boolean;
    el?: HTMLElement;
    shadeEl?: HTMLElement;
    hintEl?: HTMLElement;
  } = $props();

  const zones = $derived(layout.zones.length);
  const pct = (v: number): string => `${+(v * 100).toFixed(4)}%`;
</script>

<!-- svelte-ignore a11y_no_noninteractive_tabindex (role is option unless the card is leaving) -->
<div
  bind:this={el}
  class="card"
  class:leaving
  data-card={layout.id}
  data-testid={leaving ? undefined : 'switcher-card'}
  role={leaving ? 'presentation' : 'option'}
  id={leaving ? undefined : `sw-opt-${layout.id}`}
  tabindex={leaving ? undefined : selected ? 0 : -1}
  aria-selected={leaving ? undefined : selected}
  aria-labelledby={leaving ? undefined : `sw-n-${layout.id}${active ? ` sw-b-${layout.id}` : ''} sw-z-${layout.id}`}
  aria-posinset={leaving ? undefined : position}
  aria-setsize={leaving ? undefined : count}
  aria-hidden={leaving ? 'true' : undefined}
  inert={leaving}
>
  <div class="meta">
    <span class="name" id={leaving ? undefined : `sw-n-${layout.id}`}>{layout.name}</span>
    {#if active}<span class="badge" id={leaving ? undefined : `sw-b-${layout.id}`} data-testid="switcher-active-badge">Active</span>{/if}
    <span class="sub" id={leaving ? undefined : `sw-z-${layout.id}`}>{zones} {zones === 1 ? 'zone' : 'zones'}</span>
    <span class="grip" data-grip title="Drag to reorder" aria-hidden="true">
      <svg width="10" height="14" viewBox="0 0 10 14" fill="currentColor">
        <circle cx="2" cy="2" r="1.4" /><circle cx="8" cy="2" r="1.4" />
        <circle cx="2" cy="7" r="1.4" /><circle cx="8" cy="7" r="1.4" />
        <circle cx="2" cy="12" r="1.4" /><circle cx="8" cy="12" r="1.4" />
      </svg>
    </span>
  </div>
  <div class="thumb" aria-hidden="true">
    {#each layout.zones as z, i (z.id)}
      <span
        class="z"
        style:left={pct(z.rect.x)}
        style:top={pct(z.rect.y)}
        style:width={pct(z.rect.w)}
        style:height={pct(z.rect.h)}
        data-n={i + 1}
      ></span>
    {/each}
  </div>
  <div class="hint" bind:this={hintEl}></div>
  <div class="shade" bind:this={shadeEl}></div>
</div>

<style>
  .card {
    --card-w: min(380px, calc(100vw - 32px));
    position: absolute;
    top: 0;
    left: calc(50% - var(--card-w) / 2);
    width: var(--card-w);
    padding: 12px;
    border-radius: var(--radius-lg);
    background: var(--surface);
    box-shadow:
      0 0 0 1px var(--border),
      var(--shadow-lg);
    color: var(--text);
    cursor: pointer;
    outline: none;
    transform-origin: 50% 0;
    will-change: transform, opacity;
    /* Not drawn until the first frame places it. */
    opacity: 0;
    transition: box-shadow 0.2s ease;
  }
  .card:focus-visible {
    box-shadow:
      0 0 0 var(--ring) var(--focus),
      0 0 0 calc(var(--ring) + 3px) color-mix(in srgb, var(--focus) 22%, transparent),
      var(--shadow-lg);
  }
  .card:global(.lifted) {
    cursor: grabbing;
    box-shadow:
      0 0 0 1px var(--border),
      0 2px 4px rgb(20 20 19 / 0.08),
      0 30px 60px -14px rgb(40 20 10 / 0.38);
  }

  .meta {
    display: flex;
    align-items: center;
    gap: 8px;
    height: 22px;
    margin: 0 0 10px;
    padding: 0 2px;
  }
  .name {
    overflow: hidden;
    font-weight: 620;
    letter-spacing: -0.01em;
    white-space: nowrap;
    text-overflow: ellipsis;
  }
  .badge {
    display: inline-flex;
    flex: none;
    align-items: center;
    gap: 5px;
    padding: 1px 8px 1px 7px;
    border-radius: 999px;
    background: var(--accent-soft);
    color: var(--text);
    font-size: 0.74rem;
    font-weight: 620;
  }
  .badge::before {
    content: '';
    width: 6px;
    height: 6px;
    border-radius: 50%;
    background: var(--accent);
  }
  .sub {
    flex: none;
    margin-left: auto;
    color: var(--text-muted);
    font-size: 0.8rem;
    font-variant-numeric: tabular-nums;
  }
  .grip {
    display: grid;
    flex: none;
    place-items: center;
    width: 24px;
    height: 24px;
    margin-right: -4px;
    border-radius: 6px;
    color: var(--text-soft);
    cursor: grab;
    touch-action: none;
  }
  .grip:hover {
    background: var(--surface-2);
    color: var(--text);
  }

  .thumb {
    position: relative;
    aspect-ratio: 16 / 9;
    overflow: hidden;
    border-radius: var(--radius-sm);
    background: var(--canvas-bg);
    box-shadow: inset 0 0 0 1px var(--border);
  }
  .z {
    position: absolute;
    padding: 3px;
  }
  .z::before {
    content: '';
    position: absolute;
    inset: 3px;
    border-radius: 6px;
    background: var(--zone-bg);
    box-shadow: inset 0 0 0 1px var(--zone-border);
  }
  .z:first-child::before {
    background: color-mix(in srgb, var(--accent) 22%, var(--zone-bg));
    box-shadow: inset 0 0 0 1.5px var(--accent);
  }
  /* Zone numbers are CSS content: decorative, not part of the option's text. */
  .z::after {
    content: attr(data-n);
    position: absolute;
    top: 7px;
    left: 10px;
    color: var(--text-muted);
    font-size: 0.7rem;
    font-weight: 600;
    font-variant-numeric: tabular-nums;
  }
  .z:first-child::after {
    color: var(--text);
  }

  .hint,
  .shade {
    position: absolute;
    inset: 0;
    border-radius: inherit;
    pointer-events: none;
    opacity: 0;
  }
  .hint {
    box-shadow: inset 0 0 0 2px var(--danger);
    background: linear-gradient(to right, var(--danger-soft), transparent 30%, transparent 70%, var(--danger-soft));
  }
  .shade {
    background: var(--bg);
  }

  @media (forced-colors: active) {
    .card {
      border: 1px solid CanvasText;
    }
    .card[aria-selected='true'] {
      outline: 2px solid Highlight;
    }
    .badge::before,
    .z:first-child::before {
      background: Highlight;
    }
  }
</style>
