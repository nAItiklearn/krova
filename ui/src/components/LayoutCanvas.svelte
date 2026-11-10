<script lang="ts">
  import type { Axis, Layout, PxRect, Rect, Zone } from '../lib/bindings';
  import type { Tool } from '../lib/editor.svelte';
  import {
    MIN_ZONE,
    clamp,
    computeDividers,
    dividerKey,
    hitTest,
    pct,
    previewMoveDivider,
    rectStyle,
    tidy,
    zonePixels,
    type Divider,
    type DividerSegment,
  } from '../lib/geometry';

  interface Props {
    layout: Layout;
    /** Work area in pixels; its aspect ratio sets the canvas shape. */
    work: PxRect;
    tool: Tool;
    selected: number | null;
    /** Accessible name of the canvas, e.g. "Layout Halves on Display 1". */
    label: string;
    onselect: (zoneId: number | null) => void;
    onsplit: (zoneId: number, axis: Axis, at: number) => Promise<void>;
    onmove: (axis: Axis, from: number, to: number) => Promise<void>;
    onremove: (zoneId: number) => void;
  }

  let { layout, work, tool, selected, label, onselect, onsplit, onmove, onremove }: Props =
    $props();

  interface SplitPreview {
    zoneId: number;
    axis: Axis;
    at: number;
    rect: Rect;
  }

  interface DragState {
    axis: Axis;
    from: number;
    to: number;
    lo: number;
    hi: number;
  }

  let root: HTMLDivElement | undefined = $state();
  let preview = $state<SplitPreview | null>(null);
  /** Position chosen with the arrow keys while a zone has focus in a split tool. */
  let keyboardAt = $state<{ zoneId: number; at: number } | null>(null);
  let drag = $state<DragState | null>(null);
  let hoverDivider = $state<string | null>(null);

  const splitAxis = $derived<Axis | null>(
    tool === 'split-vertical' ? 'vertical' : tool === 'split-horizontal' ? 'horizontal' : null,
  );

  /** What is drawn: the layout, or the live preview while a divider is dragged. */
  const shown = $derived<Layout>(
    drag ? previewMoveDivider(layout, drag.axis, drag.from, drag.to) : layout,
  );
  const dividers = $derived<Divider[]>(computeDividers(shown));
  const draggedKey = $derived<string | null>(drag ? dividerKey(drag.axis, drag.to) : null);

  // Leaving a split tool (or changing layout) drops any stale preview.
  $effect(() => {
    void tool;
    void layout;
    preview = null;
    keyboardAt = null;
  });

  // ---- pointer helpers -----------------------------------------------------

  function normalized(e: { clientX: number; clientY: number }): { x: number; y: number } | null {
    if (!root) return null;
    const b = root.getBoundingClientRect();
    if (b.width <= 0 || b.height <= 0) return null;
    return {
      x: clamp((e.clientX - b.left) / b.width, 0, 1 - 1e-9),
      y: clamp((e.clientY - b.top) / b.height, 0, 1 - 1e-9),
    };
  }

  const startOf = (r: Rect, axis: Axis): number => (axis === 'vertical' ? r.x : r.y);
  const lengthOf = (r: Rect, axis: Axis): number => (axis === 'vertical' ? r.w : r.h);
  const middleOf = (r: Rect, axis: Axis): number => startOf(r, axis) + lengthOf(r, axis) / 2;

  /** Tidy the pointer position; snap to the zone centre when close to it. */
  function splitPosition(r: Rect, axis: Axis, v: number): number {
    const mid = middleOf(r, axis);
    return Math.abs(v - mid) < 0.01 ? mid : tidy(v);
  }

  /** Cosmetic only: would this position leave both parts >= MIN_ZONE? */
  function splitIsValid(p: SplitPreview): boolean {
    const s = startOf(p.rect, p.axis);
    const e = s + lengthOf(p.rect, p.axis);
    return p.at - s >= MIN_ZONE && e - p.at >= MIN_ZONE;
  }

  function onCanvasMove(e: PointerEvent): void {
    if (!splitAxis || drag) return;
    const p = normalized(e);
    if (!p) return;
    const z = hitTest(layout, p.x, p.y);
    if (!z) {
      preview = null;
      return;
    }
    keyboardAt = null;
    preview = {
      zoneId: z.id,
      axis: splitAxis,
      at: splitPosition(z.rect, splitAxis, splitAxis === 'vertical' ? p.x : p.y),
      rect: z.rect,
    };
  }

  function onCanvasLeave(): void {
    if (keyboardAt === null) preview = null;
  }

  // ---- zones ---------------------------------------------------------------

  async function commitSplit(zone: Zone, axis: Axis, at: number): Promise<void> {
    preview = null;
    keyboardAt = null;
    await onsplit(zone.id, axis, at);
  }

  function onZoneClick(e: MouseEvent, z: Zone): void {
    if (!splitAxis) {
      onselect(z.id);
      return;
    }
    if (e.detail === 0) {
      // Keyboard activation (Enter/Space): split where the arrow keys left the guide.
      const at = keyboardAt?.zoneId === z.id ? keyboardAt.at : middleOf(z.rect, splitAxis);
      void commitSplit(z, splitAxis, at);
      return;
    }
    const p = normalized(e);
    if (!p) return;
    void commitSplit(z, splitAxis, splitPosition(z.rect, splitAxis, splitAxis === 'vertical' ? p.x : p.y));
  }

  function onZoneKey(e: KeyboardEvent, z: Zone): void {
    if (splitAxis) {
      const dec = splitAxis === 'vertical' ? 'ArrowLeft' : 'ArrowUp';
      const inc = splitAxis === 'vertical' ? 'ArrowRight' : 'ArrowDown';
      if (e.key !== dec && e.key !== inc) return;
      e.preventDefault();
      const base = keyboardAt?.zoneId === z.id ? keyboardAt.at : middleOf(z.rect, splitAxis);
      const step = (e.shiftKey ? 0.05 : 0.01) * (e.key === inc ? 1 : -1);
      const start = startOf(z.rect, splitAxis);
      const at = clamp(Math.round((base + step) * 1e6) / 1e6, start, start + lengthOf(z.rect, splitAxis));
      keyboardAt = { zoneId: z.id, at };
      preview = { zoneId: z.id, axis: splitAxis, at, rect: z.rect };
      return;
    }
    if (e.key === 'Delete' || e.key === 'Backspace') {
      e.preventDefault();
      onselect(z.id);
      onremove(z.id);
    }
  }

  function onZoneFocus(e: FocusEvent, z: Zone): void {
    // Show the keyboard split guide, but not when focus came from a mouse click.
    if (!splitAxis) return;
    const el = e.currentTarget as HTMLElement;
    if (!el.matches(':focus-visible')) return;
    preview = {
      zoneId: z.id,
      axis: splitAxis,
      at: keyboardAt?.zoneId === z.id ? keyboardAt.at : middleOf(z.rect, splitAxis),
      rect: z.rect,
    };
  }

  function onZoneBlur(): void {
    if (keyboardAt !== null || preview !== null) {
      keyboardAt = null;
      preview = null;
    }
  }

  // WCAG 2.5.3 (label in name): the accessible name has to contain the visible
  // strings, in the same order: the number, "1280 × 1400" and "50% × 100%".
  // The spelled-out meaning and the keyboard help go into the description.
  function zoneName(z: Zone, px: PxRect): string {
    return `Zone ${z.id}, ${px.w} × ${px.h}, ${pct(z.rect.w)} × ${pct(z.rect.h)}`;
  }

  function zoneDescription(z: Zone, px: PxRect): string {
    const base = `${px.w} by ${px.h} pixels, ${pct(z.rect.w)} wide by ${pct(z.rect.h)} tall.`;
    if (splitAxis === 'vertical') {
      return `${base} Press Enter to split with a vertical divider; Left and Right arrows move the guide.`;
    }
    if (splitAxis === 'horizontal') {
      return `${base} Press Enter to split with a horizontal divider; Up and Down arrows move the guide.`;
    }
    return base;
  }

  // ---- dividers ------------------------------------------------------------

  function segmentStyle(d: Divider, s: DividerSegment): string {
    const len = (s.to - s.from) * 100;
    return d.axis === 'vertical'
      ? `left:${d.pos * 100}%;top:${s.from * 100}%;height:${len}%`
      : `top:${d.pos * 100}%;left:${s.from * 100}%;width:${len}%`;
  }

  function startDrag(e: PointerEvent, d: Divider): void {
    if (tool !== 'select' || e.button !== 0 || drag || !root) return;
    e.preventDefault();
    const box = root.getBoundingClientRect();
    const size = d.axis === 'vertical' ? box.width : box.height;
    if (size <= 0) return;
    const origin = d.axis === 'vertical' ? e.clientX : e.clientY;
    drag = { axis: d.axis, from: d.pos, to: d.pos, lo: d.lo, hi: d.hi };

    const stop = (): void => {
      window.removeEventListener('pointermove', move);
      window.removeEventListener('pointerup', up);
      window.removeEventListener('pointercancel', cancel);
      window.removeEventListener('keydown', key);
    };
    const move = (ev: PointerEvent): void => {
      if (!drag) return;
      const cur = d.axis === 'vertical' ? ev.clientX : ev.clientY;
      const raw = tidy(d.pos + (cur - origin) / size);
      drag.to = drag.lo <= drag.hi ? clamp(raw, drag.lo, drag.hi) : drag.from;
    };
    const up = async (): Promise<void> => {
      stop();
      const g = drag;
      if (g && Math.abs(g.to - g.from) > 1e-9) {
        await onmove(g.axis, g.from, g.to); // commit on release
      }
      drag = null;
    };
    const cancel = (): void => {
      stop();
      drag = null;
    };
    const key = (ev: KeyboardEvent): void => {
      if (ev.key === 'Escape') cancel();
    };
    window.addEventListener('pointermove', move);
    window.addEventListener('pointerup', up);
    window.addEventListener('pointercancel', cancel);
    window.addEventListener('keydown', key);
  }

  function onDividerKey(e: KeyboardEvent, d: Divider): void {
    const dec = d.axis === 'vertical' ? 'ArrowLeft' : 'ArrowUp';
    const inc = d.axis === 'vertical' ? 'ArrowRight' : 'ArrowDown';
    if (e.key !== dec && e.key !== inc) return;
    e.preventDefault();
    const step = (e.shiftKey ? 0.05 : 0.01) * (e.key === inc ? 1 : -1);
    // The backend decides whether the new position is allowed.
    void onmove(d.axis, d.pos, Math.round((d.pos + step) * 1e6) / 1e6);
  }

  const dividerName = (d: Divider): string =>
    `${d.axis === 'vertical' ? 'Vertical' : 'Horizontal'} divider at ${pct(d.pos)}`;
</script>

<!-- svelte-ignore a11y_no_noninteractive_element_interactions -->
<div
  class="canvas"
  class:splitting={splitAxis !== null}
  class:dragging={drag !== null}
  class:dragging-vertical={drag?.axis === 'vertical'}
  class:dragging-horizontal={drag?.axis === 'horizontal'}
  role="group"
  aria-label={label}
  data-testid="canvas"
  data-tool={tool}
  bind:this={root}
  style:--ar={work.w / work.h}
  onpointermove={onCanvasMove}
  onpointerleave={onCanvasLeave}
>
  {#each shown.zones as z (z.id)}
    {@const px = zonePixels(work, z.rect)}
    <button
      type="button"
      class="zone"
      class:selected={selected === z.id}
      style={rectStyle(z.rect)}
      aria-pressed={splitAxis ? undefined : selected === z.id}
      aria-label={zoneName(z, px)}
      aria-describedby="zone-desc-{z.id}"
      data-testid="zone-{z.id}"
      data-zone-id={z.id}
      onclick={(e) => onZoneClick(e, z)}
      onkeydown={(e) => onZoneKey(e, z)}
      onfocus={(e) => onZoneFocus(e, z)}
      onblur={onZoneBlur}
    >
      <span class="face">
        <span class="num">{z.id}</span>
        <span class="dim">{px.w} × {px.h}</span>
        <span class="pctline">{pct(z.rect.w)} × {pct(z.rect.h)}</span>
      </span>
    </button>
    <!-- Referenced by aria-describedby; hidden elements still count for that. -->
    <span id="zone-desc-{z.id}" hidden>{zoneDescription(z, px)}</span>
  {/each}

  {#each dividers as d}
    {#each d.segments as seg, si}
      <!-- A focusable separator is the ARIA "window splitter" widget. -->
      <!-- svelte-ignore a11y_no_noninteractive_tabindex, a11y_no_noninteractive_element_interactions -->
      <div
        class="divider {d.axis}"
        class:hot={hoverDivider === d.key}
        class:active={draggedKey === d.key}
        role="separator"
        aria-orientation={d.axis}
        aria-valuenow={Math.round(d.pos * 100)}
        aria-valuemin={Math.round(d.lo * 100)}
        aria-valuemax={Math.round(d.hi * 100)}
        aria-valuetext={pct(d.pos)}
        aria-label={dividerName(d)}
        aria-describedby="divider-help"
        aria-hidden={si === 0 ? undefined : true}
        tabindex={si === 0 && tool === 'select' ? 0 : -1}
        data-testid="divider"
        data-axis={d.axis}
        data-pos={Math.round(d.pos * 10000) / 10000}
        data-divider={d.key}
        style={segmentStyle(d, seg)}
        onpointerdown={(e) => startDrag(e, d)}
        onpointerenter={() => (hoverDivider = d.key)}
        onpointerleave={() => (hoverDivider = null)}
        onkeydown={(e) => onDividerKey(e, d)}
      ></div>
    {/each}
  {/each}

  {#if preview && splitAxis}
    <div
      class="guide {preview.axis}"
      class:bad={!splitIsValid(preview)}
      data-testid="split-preview"
      style={preview.axis === 'vertical'
        ? `left:${preview.at * 100}%;top:${preview.rect.y * 100}%;height:${preview.rect.h * 100}%`
        : `top:${preview.at * 100}%;left:${preview.rect.x * 100}%;width:${preview.rect.w * 100}%`}
    >
      <span class="chip">{pct(preview.at)}{splitIsValid(preview) ? '' : ' (too close to the edge)'}</span>
    </div>
  {/if}

  {#if drag}
    <div
      class="chip dragchip"
      class:vertical={drag.axis === 'vertical'}
      style={drag.axis === 'vertical' ? `left:${drag.to * 100}%` : `top:${drag.to * 100}%`}
    >
      {pct(drag.to)}
    </div>
  {/if}
</div>

<p id="divider-help" class="sr-only">
  Drag the divider, or use the arrow keys to move it by 1 percent. Hold Shift for 5 percent steps.
</p>

<style>
  .canvas {
    position: relative;
    width: 100%;
    /* Keep the monitor's aspect ratio and never grow taller than the window allows. */
    aspect-ratio: var(--ar);
    max-width: max(440px, calc((100vh - var(--canvas-chrome, 400px)) * var(--ar)));
    min-width: 280px;
    margin-inline: auto;
    background: var(--canvas-bg);
    border: 1px solid var(--border-strong);
    border-radius: var(--radius);
    box-shadow: var(--shadow);
    overflow: hidden;
    touch-action: none;
    user-select: none;
    -webkit-user-select: none;
  }

  .canvas.splitting {
    cursor: crosshair;
  }

  .canvas.dragging-vertical {
    cursor: col-resize;
  }

  .canvas.dragging-horizontal {
    cursor: row-resize;
  }

  .zone {
    position: absolute;
    z-index: 1;
    appearance: none;
    margin: 0;
    padding: 0;
    border: 0;
    background: transparent;
    color: var(--text);
    font: inherit;
    cursor: inherit;
    container-type: size;
  }

  .canvas:not(.splitting):not(.dragging) .zone {
    cursor: pointer;
  }

  .zone:focus-visible {
    outline: none;
  }

  .face {
    position: absolute;
    inset: 3px;
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    gap: 2px;
    overflow: hidden;
    background: var(--zone-bg);
    border: 1.5px solid var(--zone-border);
    border-radius: var(--radius-sm);
    transition: background-color 0.12s ease;
  }

  .zone:hover .face {
    background: var(--zone-bg-hover);
  }

  .zone.selected .face {
    background: var(--zone-bg-selected);
    border: 2.5px solid var(--accent);
  }

  .zone:focus-visible .face {
    outline: var(--ring) solid var(--focus);
    outline-offset: 1px;
  }

  .num {
    font-size: clamp(1.1rem, 7cqmin, 3rem);
    font-weight: 700;
    line-height: 1;
  }

  .dim,
  .pctline {
    font: 0.78rem/1.2 var(--mono);
    color: var(--text-muted);
    white-space: nowrap;
  }

  /* The selected fill is the strongest tint, so the small lines use the full
     text colour there (muted text on the dark selected fill is only 3.4:1). */
  .zone.selected .dim,
  .zone.selected .pctline {
    color: var(--text);
  }

  @container (max-width: 110px) {
    .dim,
    .pctline {
      display: none;
    }
  }

  @container (max-height: 70px) {
    .pctline {
      display: none;
    }
  }

  @container (max-height: 52px) {
    .dim {
      display: none;
    }
  }

  /* ---- dividers ---- */

  .divider {
    position: absolute;
    z-index: 2;
    touch-action: none;
  }

  .divider.vertical {
    width: 18px;
    margin-left: -9px;
    cursor: col-resize;
  }

  .divider.horizontal {
    height: 18px;
    margin-top: -9px;
    cursor: row-resize;
  }

  /* the visible line, shown on hover / focus / drag */
  .divider::after {
    content: '';
    position: absolute;
    background: var(--divider);
    opacity: 0;
    border-radius: 2px;
    transition: opacity 0.12s ease;
  }

  .divider.vertical::after {
    left: 50%;
    top: 0;
    bottom: 0;
    width: 3px;
    margin-left: -1.5px;
  }

  .divider.horizontal::after {
    top: 50%;
    left: 0;
    right: 0;
    height: 3px;
    margin-top: -1.5px;
  }

  /* the grip pill hints that the divider can be dragged */
  .divider::before {
    content: '';
    position: absolute;
    background: var(--surface);
    border: 1.5px solid var(--divider);
    border-radius: 6px;
    opacity: 0.85;
  }

  .divider.vertical::before {
    left: 50%;
    top: 50%;
    width: 8px;
    height: 34px;
    transform: translate(-50%, -50%);
  }

  .divider.horizontal::before {
    left: 50%;
    top: 50%;
    width: 34px;
    height: 8px;
    transform: translate(-50%, -50%);
  }

  .divider:hover::after,
  .divider.hot::after,
  .divider:focus-visible::after,
  .divider.active::after {
    opacity: 1;
  }

  .divider:focus-visible {
    outline: none;
  }

  .divider:focus-visible::before {
    outline: var(--ring) solid var(--focus);
    outline-offset: 2px;
  }

  .canvas.splitting .divider {
    pointer-events: none;
    opacity: 0.5;
  }

  /* ---- split guide ---- */

  .guide {
    position: absolute;
    z-index: 3;
    pointer-events: none;
    border: 0 dashed var(--preview);
  }

  .guide.vertical {
    width: 0;
    border-left-width: 3px;
    margin-left: -1.5px;
  }

  .guide.horizontal {
    height: 0;
    border-top-width: 3px;
    margin-top: -1.5px;
  }

  .guide.bad {
    border-color: var(--preview-bad);
  }

  .chip {
    position: absolute;
    left: 8px;
    top: 8px;
    padding: 1px 7px;
    border-radius: 999px;
    background: var(--ink);
    color: var(--on-ink);
    font: 600 0.78rem/1.5 var(--mono);
    white-space: nowrap;
  }

  .guide.bad .chip {
    background: var(--preview-bad);
  }

  .dragchip {
    z-index: 4;
    pointer-events: none;
    background: var(--accent);
    color: var(--accent-contrast);
  }

  .dragchip.vertical {
    top: 8px;
    transform: translateX(-50%);
  }

  .dragchip:not(.vertical) {
    left: 8px;
    transform: translateY(-50%);
  }

  @media (forced-colors: active) {
    .zone.selected .face {
      border-color: Highlight;
    }
    .divider::before {
      border-color: ButtonText;
    }
  }
</style>
