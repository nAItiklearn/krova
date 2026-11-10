<script lang="ts" module>
  import type { Layout } from '../../lib/bindings';

  /** What the deck asks its owner to do. Every promise rejects with a user-facing message. */
  export interface DeckActions {
    apply(layoutId: string): Promise<void>;
    remove(layoutId: string): Promise<void>;
    /** Undo of `remove`: back at `index`, active again if it was. */
    restore(layout: Layout, index: number, activate: boolean): Promise<void>;
    move(layoutId: string, toIndex: number): Promise<void>;
    close(): void;
  }
</script>

<script lang="ts">
  // The layout switcher deck (docs/prototypes/layout-switcher): the focused
  // layout in front, the following ones stacked behind it, flipped-past ones
  // sliding away below. Everything that moves is a spring from lib/motion.ts,
  // stepped by one requestAnimationFrame loop that only runs while something
  // is moving; positions are written straight to the card elements.
  import { onMount, tick, untrack } from 'svelte';
  import { errorMessage } from '../../lib/backend';
  import {
    Loop,
    Spring,
    clamp,
    projectIndex,
    rubberClamp,
    releaseVelocity,
    trimHistory,
    watchReducedMotion,
    type Sample,
  } from '../../lib/motion';
  import Card from './Card.svelte';
  import {
    DRAG_PX,
    TAP_MS,
    classify,
    dismissHint,
    dismisses,
    focusY,
    moved,
    place,
    slotAt,
    swipeFade,
    swipeTilt,
    unplace,
    type Gesture,
  } from './deck';

  let {
    layouts,
    activeId,
    open,
    title,
    subtitle,
    monitorLabel,
    actions,
  }: {
    layouts: Layout[];
    activeId: string | null;
    open: boolean;
    title: string;
    subtitle: string;
    /** For messages: "… is now active on Display 1". */
    monitorLabel: string;
    actions: DeckActions;
  } = $props();

  interface Motion {
    slot: Spring;
    x: Spring;
    lift: Spring;
    alpha: Spring;
    el?: HTMLElement;
    shadeEl?: HTMLElement;
    hintEl?: HTMLElement;
  }

  /** Per-layout springs and elements; plain (not reactive): written every frame. */
  const refs: Record<string, Motion> = {};
  const motionFor = (id: string, slot: number): Motion =>
    (refs[id] ??= {
      slot: new Spring(slot),
      x: new Spring(0, { k: 360, zeta: 0.8 }),
      lift: new Spring(0, { k: 500, zeta: 0.72 }),
      alpha: new Spring(1, { k: 300, zeta: 1 }),
    });

  /** The deck's own order (optimistic: changes before the backend answers). */
  let items = $state.raw<Layout[]>([]);
  /** Cards being swiped away: still drawn until they are gone. */
  let ghosts = $state.raw<Layout[]>([]);
  let focusIdx = $state(0);
  const rendered = $derived([
    ...items.map((l) => ({ l, leaving: false })),
    ...ghosts.map((l) => ({ l, leaving: true })),
  ]);

  const scroll = new Spring(0, { k: 300, zeta: 0.9 });
  const openness = new Spring(0, { k: 340, zeta: 0.92 });
  const toastS = new Spring(0, { k: 380, zeta: 0.8 });

  let stageEl = $state<HTMLElement>();
  let scrimEl = $state<HTMLElement>();
  let toastEl = $state<HTMLElement>();
  let stageH = 800;
  let cardH = 260;

  interface Drag {
    id: string | null;
    x0: number;
    y0: number;
    t0: number;
    scroll0: number;
    mode: Gesture;
    hist: Sample[];
    /** reorder: pointer offset inside the card, card top (stage px), start index */
    grab: number;
    y: number;
    from: number;
  }
  let drag: Drag | null = null;
  let suppressClick = false;
  /** Backend order that arrived during a reorder drag; applied on drop. */
  let deferred: Layout[] | null = null;

  const loop = new Loop(frame);

  function frame(dt: number): boolean {
    let moving = false;
    for (const s of [scroll, openness, toastS]) {
      s.step(dt);
      moving ||= !s.settled;
    }
    const o = openness.value;
    const fy = focusY(stageH, cardH);
    if (scrimEl) scrimEl.style.opacity = String(clamp(o, 0, 1));
    if (toastEl) {
      const v = toastS.value;
      toastEl.style.opacity = String(clamp(v, 0, 1));
      toastEl.style.transform = `translate(-50%, ${(1 - v) * 20}px) scale(${0.96 + 0.04 * v})`;
    }
    const n = Object.keys(refs).length;
    for (const id in refs) {
      const m = refs[id];
      for (const s of [m.slot, m.x, m.lift, m.alpha]) {
        s.step(dt);
        moving ||= !s.settled;
      }
      if (!m.el) continue;
      const p = place(scroll.value - m.slot.value, cardH);
      const held = drag?.mode === 'reorder' && drag.id === id;
      const y = (held ? drag!.y : fy + p.y) + (1 - o) * 120;
      const s = (held ? 1 : p.s) * (0.94 + 0.06 * o) * (1 + m.lift.value * 0.035);
      const x = m.x.value;
      m.el.style.transform = `translate3d(${x}px, ${y}px, 0) scale(${s}) rotate(${swipeTilt(x)}deg)`;
      m.el.style.opacity = String(
        (held ? 1 : p.o) * Math.min(1, Math.max(0, o) * 1.6) * clamp(m.alpha.value, 0, 1) * swipeFade(x),
      );
      m.el.style.zIndex = String(held || m.lift.value > 0.02 ? 1000 : Math.round(100 + (n - m.slot.value) * 10));
      m.el.classList.toggle('lifted', held || m.lift.target > 0);
      if (m.shadeEl) m.shadeEl.style.opacity = String(held ? 0 : p.shade);
      if (m.hintEl) m.hintEl.style.opacity = String(dismissHint(x));
    }
    return moving || drag !== null;
  }

  // ---- data -----------------------------------------------------------------

  function sync(ls: Layout[]): void {
    if (drag?.mode === 'reorder') {
      deferred = ls;
      return;
    }
    ls.forEach((l, i) => {
      const m = motionFor(l.id, i);
      m.slot.set(i);
      if (ghosts.some((g) => g.id === l.id)) {
        m.x.set(0);
        m.alpha.set(1);
      }
    });
    items = ls;
    ghosts = ghosts.filter((g) => !ls.some((l) => l.id === g.id));
    for (const id in refs) {
      if (!ls.some((l) => l.id === id) && !ghosts.some((g) => g.id === id)) delete refs[id];
    }
    focusIdx = clamp(focusIdx, 0, Math.max(0, ls.length - 1));
    void measure();
    loop.kick();
  }

  $effect(() => {
    const ls = layouts;
    untrack(() => sync(ls));
  });

  $effect(() => {
    const o = open;
    untrack(() => (o ? show() : hide()));
  });

  async function measure(): Promise<void> {
    await tick();
    const first = items[0] && refs[items[0].id]?.el;
    if (first && first.offsetHeight) cardH = first.offsetHeight;
    loop.kick();
  }

  // ---- focus ----------------------------------------------------------------

  async function setFocus(i: number, animate = true): Promise<void> {
    if (!items.length) return;
    focusIdx = clamp(i, 0, items.length - 1);
    if (animate) scroll.set(focusIdx);
    else scroll.jump(focusIdx);
    loop.kick();
    await tick();
    const el = items[focusIdx] && refs[items[focusIdx].id]?.el;
    el?.focus({ preventScroll: true });
  }

  function show(): void {
    const i = Math.max(0, items.findIndex((l) => l.id === activeId));
    void setFocus(i, false);
    openness.set(1);
    loop.kick();
  }

  function hide(): void {
    openness.set(0);
    if (drag) endDrag();
    loop.kick();
  }

  // ---- live region and toast --------------------------------------------------

  let liveText = $state('');
  function announce(text: string): void {
    liveText = '';
    requestAnimationFrame(() => (liveText = text));
  }

  let toastText = $state('');
  let toastUndo = $state(false);
  let toastShown = $state(false);
  let toastTimer: ReturnType<typeof setTimeout> | undefined;
  function toast(text: string, withUndo = false): void {
    toastText = text;
    toastUndo = withUndo;
    toastShown = true;
    toastS.set(1);
    loop.kick();
    clearTimeout(toastTimer);
    toastTimer = setTimeout(
      () => {
        toastS.set(0);
        toastShown = false;
        if (withUndo) undoState = null;
        loop.kick();
      },
      withUndo ? 5000 : 2200,
    );
  }

  function failed(e: unknown): void {
    const msg = errorMessage(e);
    toast(msg);
    announce(msg);
  }

  // ---- actions --------------------------------------------------------------

  let undoState: { layout: Layout; index: number; wasActive: boolean } | null = null;

  function apply(i: number): void {
    const l = items[i];
    if (!l) return;
    void setFocus(i);
    const m = refs[l.id];
    m.lift.set(1, { vel: 10 });
    setTimeout(() => {
      m.lift.set(0);
      loop.kick();
    }, 150);
    loop.kick();
    actions.apply(l.id).then(() => {
      toast(`“${l.name}” is now active on ${monitorLabel}`);
      announce(`${l.name} is now active on ${monitorLabel}.`);
    }, failed);
  }

  function remove(i: number, dir: number, vel: number): void {
    const l = items[i];
    if (!l) return;
    if (items.length < 2) {
      announce('The last layout cannot be deleted.');
      return;
    }
    const m = refs[l.id];
    m.x.set(dir * 600, { vel: vel || dir * 1800 });
    m.alpha.set(0);
    undoState = { layout: l, index: i, wasActive: l.id === activeId };
    items = items.filter((x) => x.id !== l.id);
    ghosts = [...ghosts, l];
    items.forEach((x, j) => refs[x.id].slot.set(j));
    setTimeout(() => {
      if (ghosts.some((g) => g.id === l.id)) {
        ghosts = ghosts.filter((g) => g.id !== l.id);
        if (!items.some((x) => x.id === l.id)) delete refs[l.id];
      }
    }, 450);
    void setFocus(Math.min(i, items.length - 1));
    toast(`Deleted “${l.name}”`, true);
    announce(`Deleted ${l.name}. Press Control Z to undo.`);
    actions.remove(l.id).catch(failed);
  }

  function undo(): void {
    const u = undoState;
    if (!u) return;
    undoState = null;
    const { layout: l, wasActive } = u;
    const index = clamp(u.index, 0, items.length);
    ghosts = ghosts.filter((g) => g.id !== l.id);
    const m = motionFor(l.id, index);
    m.slot.jump(index);
    m.x.set(0);
    m.alpha.set(1);
    const next = items.slice();
    next.splice(index, 0, l);
    items = next;
    items.forEach((x, j) => refs[x.id].slot.set(j));
    void setFocus(index);
    toast(`Restored “${l.name}”`);
    announce(`Restored ${l.name}.`);
    actions.restore(l, index, wasActive).catch(failed);
  }

  function moveBy(delta: number): void {
    const from = focusIdx;
    const to = clamp(from + delta, 0, items.length - 1);
    if (to === from) return;
    const l = items[from];
    items = moved(items, from, to);
    items.forEach((x, j) => refs[x.id].slot.set(j));
    const m = refs[l.id];
    m.lift.set(1, { vel: 6 });
    setTimeout(() => {
      m.lift.set(0);
      loop.kick();
    }, 160);
    void setFocus(to);
    announce(`${l.name} moved to position ${to + 1} of ${items.length}.`);
    actions.move(l.id, to).catch(failed);
  }

  // ---- pointer --------------------------------------------------------------

  const stageY = (clientY: number): number => clientY - (stageEl?.getBoundingClientRect().top ?? 0);

  function onpointerdown(e: PointerEvent): void {
    if (e.button !== 0 || !open || drag) return;
    suppressClick = false;
    const target = e.target as Element;
    const cardEl = target.closest<HTMLElement>('[data-card]');
    const id = cardEl?.dataset.card ?? null;
    const known = id !== null && items.some((l) => l.id === id);
    const now = performance.now();
    drag = {
      id: known ? id : null,
      x0: e.clientX,
      y0: e.clientY,
      t0: now,
      scroll0: scroll.value,
      mode: known && target.closest('[data-grip]') ? 'reorder' : 'pending',
      hist: [{ x: e.clientX, y: e.clientY, t: now }],
      grab: 0,
      y: 0,
      from: known ? items.findIndex((l) => l.id === id) : -1,
    };
    if (drag.mode === 'reorder') {
      const m = refs[id!];
      const top = focusY(stageH, cardH) + place(scroll.value - m.slot.value, cardH).y;
      drag.grab = stageY(e.clientY) - top;
      drag.y = top;
      m.lift.set(1);
    }
    stageEl?.setPointerCapture(e.pointerId);
    loop.kick();
  }

  function onpointermove(e: PointerEvent): void {
    if (!drag) return;
    const now = performance.now();
    drag.hist.push({ x: e.clientX, y: e.clientY, t: now });
    trimHistory(drag.hist, now);
    const dx = e.clientX - drag.x0;
    const dy = e.clientY - drag.y0;
    if (drag.mode === 'pending') drag.mode = classify(dx, dy, drag.id !== null, items.length);
    if (drag.mode === 'scroll') {
      scroll.jump(rubberClamp(drag.scroll0 + dy / DRAG_PX, 0, Math.max(0, items.length - 1)));
    } else if (drag.mode === 'swipe') {
      refs[drag.id!].x.jump(dx);
    } else if (drag.mode === 'reorder') {
      drag.y = stageY(e.clientY) - drag.grab;
      const from = items.findIndex((l) => l.id === drag!.id);
      const to = slotAt(drag.y, scroll.value, stageH, cardH, items.length);
      if (from >= 0 && to !== from) {
        items = moved(items, from, to);
        items.forEach((l, i) => {
          if (l.id !== drag!.id) refs[l.id].slot.set(i);
        });
        refs[drag.id!].slot.jump(to);
      }
    }
    loop.kick();
  }

  function endDrag(): void {
    const d = drag;
    if (!d) return;
    drag = null;
    const { vx, vy } = releaseVelocity(d.hist, performance.now());
    // With pointer capture the click lands on the stage; only a tap on the backdrop closes.
    suppressClick = d.mode !== 'pending' || d.id !== null;
    if (d.mode === 'pending') {
      if (d.id !== null && performance.now() - d.t0 < TAP_MS) {
        apply(items.findIndex((l) => l.id === d.id));
      }
    } else if (d.mode === 'scroll') {
      const v = (vy * 1000) / DRAG_PX; // cards per second
      focusIdx = projectIndex(scroll.value, v, items.length);
      scroll.set(focusIdx, { vel: v });
      void setFocus(focusIdx);
    } else if (d.mode === 'swipe') {
      const m = refs[d.id!];
      const x = m.x.value;
      if (dismisses(x, vx)) remove(items.findIndex((l) => l.id === d.id), Math.sign(x || vx), vx * 1000);
      else m.x.set(0, { vel: vx * 1000 });
    } else if (d.mode === 'reorder') {
      const i = items.findIndex((l) => l.id === d.id);
      const m = refs[d.id!];
      m.slot.jump(scroll.value - unplace(d.y - focusY(stageH, cardH), cardH));
      m.slot.set(i);
      m.lift.set(0);
      if (i !== d.from && i >= 0) {
        announce(`${items[i].name} moved to position ${i + 1} of ${items.length}.`);
        actions.move(d.id!, i).catch(failed);
      }
      if (deferred) {
        const ls = deferred;
        deferred = null;
        sync(ls);
      }
    }
    loop.kick();
  }

  function onclick(e: MouseEvent): void {
    // A tap on the empty backdrop closes, a drag that ended there does not.
    if (e.target === stageEl && !suppressClick && open) actions.close();
    suppressClick = false;
  }

  let wheelSnap: ReturnType<typeof setTimeout> | undefined;
  function onwheel(e: WheelEvent): void {
    e.preventDefault();
    if (!open || !items.length) return;
    scroll.set(clamp(scroll.target + e.deltaY / DRAG_PX, 0, items.length - 1));
    clearTimeout(wheelSnap);
    wheelSnap = setTimeout(() => void setFocus(Math.round(scroll.target)), 120);
    loop.kick();
  }

  // ---- keyboard -------------------------------------------------------------

  function onkeydown(e: KeyboardEvent): void {
    if (!open) return;
    const k = e.key;
    if (k === 'Escape') {
      e.preventDefault();
      actions.close();
      return;
    }
    if ((k === 'z' || k === 'Z') && (e.ctrlKey || e.metaKey) && !e.shiftKey) {
      e.preventDefault();
      undo();
      return;
    }
    // Buttons (Undo) keep their own Enter/Space.
    if (e.target instanceof HTMLButtonElement) return;
    if (!items.length) return;
    if ((k === 'ArrowDown' || k === 'ArrowUp') && e.altKey) {
      e.preventDefault();
      moveBy(k === 'ArrowDown' ? 1 : -1);
    } else if (k === 'ArrowDown') {
      e.preventDefault();
      void setFocus(focusIdx + 1);
    } else if (k === 'ArrowUp') {
      e.preventDefault();
      void setFocus(focusIdx - 1);
    } else if (k === 'Home') {
      e.preventDefault();
      void setFocus(0);
    } else if (k === 'End') {
      e.preventDefault();
      void setFocus(items.length - 1);
    } else if (k === 'Enter' || k === ' ') {
      e.preventDefault();
      apply(focusIdx);
    } else if (k === 'Delete' || k === 'Backspace') {
      e.preventDefault();
      remove(focusIdx, 1, 0);
    }
  }

  onMount(() => {
    const unwatch = watchReducedMotion();
    const ro = new ResizeObserver(() => {
      if (stageEl) stageH = stageEl.clientHeight;
      void measure();
    });
    if (stageEl) {
      stageH = stageEl.clientHeight;
      ro.observe(stageEl);
      // Not passive: the wheel flips cards instead of scrolling the page.
      stageEl.addEventListener('wheel', onwheel, { passive: false });
    }
    return () => {
      unwatch();
      ro.disconnect();
      stageEl?.removeEventListener('wheel', onwheel);
      loop.stop();
      clearTimeout(toastTimer);
      clearTimeout(wheelSnap);
    };
  });
</script>

<svelte:window {onkeydown} />

<div
  bind:this={scrimEl}
  class="scrim"
  class:open
  role="dialog"
  aria-modal="true"
  aria-labelledby="sw-title"
  inert={!open}
  data-testid="switcher"
  data-open={open}
>
  <header class="head">
    <h1 id="sw-title">{title}</h1>
    <p class="sub">{subtitle}</p>
  </header>

  <!-- svelte-ignore a11y_click_events_have_key_events (the keyboard is handled on the window) -->
  <div
    bind:this={stageEl}
    class="stage"
    role="listbox"
    tabindex="-1"
    aria-labelledby="sw-title"
    aria-describedby="sw-help"
    data-testid="switcher-deck"
    {onpointerdown}
    {onpointermove}
    onpointerup={endDrag}
    onpointercancel={endDrag}
    {onclick}
  >
    {#each rendered as r, i (r.l.id)}
      {@const m = motionFor(r.l.id, i)}
      <Card
        layout={r.l}
        active={r.l.id === activeId}
        selected={!r.leaving && i === focusIdx}
        position={i + 1}
        count={items.length}
        leaving={r.leaving}
        bind:el={m.el}
        bind:shadeEl={m.shadeEl}
        bind:hintEl={m.hintEl}
      />
    {/each}
    {#if !items.length && !ghosts.length}
      <p class="empty">No layouts on this display yet.</p>
    {/if}
  </div>

  <p id="sw-help" class="sr-only">
    Up and Down arrows flip through the layouts, Enter applies one. Alt with Up or Down moves it
    in the list. Delete removes it, Control Z undoes that. Escape closes.
  </p>
  <div class="hints" aria-hidden="true">
    <span><kbd>↑</kbd><kbd>↓</kbd> or drag to flip through</span>
    <span><kbd>Enter</kbd> apply</span>
    <span><kbd>Alt</kbd><kbd>↑↓</kbd> or grip to reorder</span>
    <span><kbd>Del</kbd> or swipe sideways to delete</span>
    <span><kbd>Esc</kbd> close</span>
  </div>
</div>

<div
  bind:this={toastEl}
  class="toast"
  class:shown={toastShown}
  inert={!toastShown}
  aria-hidden={!toastShown}
  data-testid="switcher-toast"
>
  <span>{toastText}</span>
  {#if toastUndo}
    <button type="button" onclick={undo}>Undo <kbd>Ctrl Z</kbd></button>
  {/if}
</div>
<p class="sr-only" aria-live="polite" data-testid="switcher-live">{liveText}</p>

<style>
  .scrim {
    position: fixed;
    inset: 0;
    opacity: 0;
    pointer-events: none;
    background: rgb(246 245 242 / 0.66);
    -webkit-backdrop-filter: blur(22px) saturate(1.3);
    backdrop-filter: blur(22px) saturate(1.3);
  }
  .scrim.open {
    pointer-events: auto;
  }
  :global(:root[data-theme='dark']) .scrim {
    background: rgb(12 12 11 / 0.62);
  }
  @media (prefers-color-scheme: dark) {
    :global(:root:not([data-theme='light'])) .scrim {
      background: rgb(12 12 11 / 0.62);
    }
  }

  .head {
    position: absolute;
    top: 26px;
    right: 0;
    left: 0;
    z-index: 2000;
    text-align: center;
    pointer-events: none;
  }
  h1 {
    font-size: 1.05rem;
    font-weight: 650;
  }
  .sub {
    margin-top: 3px;
    color: var(--text-muted);
    font-size: 0.82rem;
    font-variant-numeric: tabular-nums;
  }

  .stage {
    position: absolute;
    inset: 0;
    outline: none;
    touch-action: none;
    user-select: none;
    -webkit-user-select: none;
  }
  .empty {
    position: absolute;
    top: 45%;
    right: 0;
    left: 0;
    color: var(--text-muted);
    text-align: center;
  }

  .hints {
    position: absolute;
    right: 0;
    bottom: 22px;
    left: 0;
    z-index: 2000;
    display: flex;
    flex-wrap: wrap;
    justify-content: center;
    gap: 6px 18px;
    padding: 0 16px;
    color: var(--text-muted);
    font-size: 0.8rem;
    pointer-events: none;
  }
  .hints span {
    display: inline-flex;
    align-items: center;
    gap: 3px;
  }
  .hints kbd {
    min-width: 1.7em;
    height: 1.7em;
    font-size: 0.72rem;
  }

  .toast {
    position: fixed;
    bottom: 64px;
    left: 50%;
    z-index: 3000;
    display: flex;
    align-items: center;
    gap: 12px;
    max-width: calc(100vw - 32px);
    padding: 8px 8px 8px 14px;
    border-radius: var(--radius);
    background: var(--ink);
    box-shadow: var(--shadow-lg);
    color: var(--on-ink);
    font-weight: 550;
    opacity: 0;
    pointer-events: none;
    transform: translate(-50%, 20px);
  }
  .toast.shown {
    pointer-events: auto;
  }
  .toast button {
    display: inline-flex;
    align-items: center;
    gap: 7px;
    padding: 4px 6px 4px 9px;
    border: 0;
    border-radius: 8px;
    background: color-mix(in srgb, var(--on-ink) 14%, transparent);
    color: var(--on-ink);
    font: inherit;
    font-weight: 650;
    cursor: pointer;
  }
  .toast button:hover {
    background: color-mix(in srgb, var(--on-ink) 22%, transparent);
  }
  .toast button:focus-visible {
    outline-color: var(--on-ink);
  }
  .toast kbd {
    height: 1.6em;
    background: transparent;
    box-shadow: inset 0 0 0 1px color-mix(in srgb, var(--on-ink) 40%, transparent);
    color: var(--on-ink);
    font-size: 0.68rem;
  }

  @media (forced-colors: active) {
    .scrim {
      background: Canvas;
    }
    .toast {
      border: 1px solid CanvasText;
    }
  }
</style>
