<script lang="ts">
  // The quick panel: a small card that opens from the notch (or the tray icon).
  // Header with the pause switch, the layouts of the monitor under the notch,
  // quick toggles, actions and a link to the notch settings. Esc, a click outside
  // the card or the window losing focus closes it.
  import { onMount, tick, untrack } from 'svelte';
  import { app } from '../../lib/app.svelte';
  import { api, type PanelAnchor } from '../../lib/backend';
  import { ext } from '../../lib/ext.svelte';
  import { settings } from '../../lib/settings.svelte';
  import { prefersReducedMotion, Spring } from '../../lib/motion';
  import { monitorFor, placePanel, type Placement } from '../../lib/panelPlacement';
  import { hideWindow, openMain, placeWindow, type PanelTarget } from '../../lib/panelWindow';
  import LayoutChips from './LayoutChips.svelte';
  import PanelIcon from './PanelIcon.svelte';
  import QuickToggles from './QuickToggles.svelte';

  const CARD_W = 340;
  const PAD = 16;

  let anchor = $state<PanelAnchor | null>(null);
  let open = $state(false);
  let placement = $state<Placement | null>(null);
  let cardEl: HTMLElement | undefined = $state();
  let switchEl: HTMLButtonElement | undefined = $state();
  let openedAt = 0;

  // ---- theme and accent (same rules as the main window) -------------------
  const theme = $derived(app.state?.config.settings.theme ?? 'system');
  $effect(() => {
    const root = document.documentElement;
    if (theme === 'light' || theme === 'dark') root.dataset.theme = theme;
    else delete root.dataset.theme;
  });
  const accent = $derived(ext.get('appearance').accent);
  $effect(() => {
    const s = document.documentElement.style;
    s.setProperty('--accent', accent);
    s.setProperty('--accent-soft', `color-mix(in srgb, ${accent} 12%, transparent)`);
    s.setProperty('--accent-softer', `color-mix(in srgb, ${accent} 7%, transparent)`);
  });

  // ext.set writes through the settings form store once the backend has a field.
  $effect(() => {
    const state = app.state;
    if (state) untrack(() => settings.sync(state));
  });

  // ---- what the panel shows --------------------------------------------------
  const monitor = $derived(anchor && app.state ? monitorFor(anchor, app.state.monitors) : null);
  const profile = $derived(
    app.state?.config.monitors.find((p) => p.id === (monitor?.profile_id ?? anchor?.monitor_id)) ??
      app.state?.config.monitors[0],
  );
  const monitorName = $derived(
    app.state?.monitors.find((m) => m.profile_id === profile?.id)?.label ?? profile?.label ?? 'Display',
  );
  const auto = $derived(ext.get('auto_arrange'));
  const paused = $derived(ext.get('paused'));
  const arranging = $derived(auto.enabled && !paused);

  function setArranging(v: boolean): void {
    ext.set('paused', !v);
    if (v && !auto.enabled) ext.patch('auto_arrange', { enabled: true });
  }

  // ---- motion -----------------------------------------------------------------
  const spring = new Spring(0, { stiffness: 520, damping: 30, precision: 0.002 });
  let progress = $state(0);
  let raf = 0;
  let last = 0;
  let onSettled: (() => void) | null = null;

  function run(): void {
    cancelAnimationFrame(raf);
    last = performance.now();
    const frame = (now: number): void => {
      spring.step((now - last) / 1000);
      last = now;
      progress = spring.value;
      if (spring.settled) {
        const done = onSettled;
        onSettled = null;
        done?.();
      } else raf = requestAnimationFrame(frame);
    };
    raf = requestAnimationFrame(frame);
  }

  function animateTo(target: number, done?: () => void): void {
    onSettled = done ?? null;
    if (prefersReducedMotion()) {
      spring.jump(target);
      progress = target;
      onSettled = null;
      done?.();
      return;
    }
    spring.set(target);
    run();
  }

  const side = $derived(placement?.side ?? 'top');
  const cardStyle = $derived.by(() => {
    const p = progress;
    const y = (side === 'top' ? -8 : 8) * (1 - p);
    const scale = 0.96 + 0.04 * p;
    const ox = placement?.originX ?? CARD_W / 2;
    const oy = side === 'top' ? '0px' : '100%';
    return `transform-origin:${ox}px ${oy};transform:translateY(${y.toFixed(2)}px) scale(${scale.toFixed(4)});opacity:${Math.min(Math.max(p, 0), 1).toFixed(3)}`;
  });

  // ---- open / close -------------------------------------------------------------
  async function show(a: PanelAnchor): Promise<void> {
    anchor = a;
    if (!app.state) await app.load();
    else void app.refresh();
    open = true;
    await tick();
    const h = cardEl?.offsetHeight ?? 320;
    placement = placePanel(a, app.state?.monitors ?? [], { w: CARD_W, h }, { pad: PAD });
    spring.jump(0);
    progress = 0;
    openedAt = performance.now();
    await placeWindow(placement).catch(() => undefined);
    animateTo(1);
    switchEl?.focus({ preventScroll: true });
  }

  let closing = false;
  function close(): void {
    if (!open || closing) return;
    closing = true;
    animateTo(0, () => {
      open = false;
      closing = false;
      void hideWindow().catch(() => undefined);
    });
  }

  async function go(target: PanelTarget): Promise<void> {
    await openMain(target).catch(() => undefined);
    close();
  }

  function onKey(e: KeyboardEvent): void {
    if (e.key === 'Escape' && open) {
      e.preventDefault();
      close();
    }
  }
  function onBlur(): void {
    // The OS can bounce focus right after showing the window; ignore that.
    if (performance.now() - openedAt > 150) close();
  }
  function onPointerDown(e: PointerEvent): void {
    if (open && cardEl && !cardEl.contains(e.target as Node)) close();
  }

  onMount(() => {
    void app.load();
    let disposed = false;
    const offs: Array<() => void> = [];
    const keep = (p: Promise<() => void>): void => {
      void p.then((off) => (disposed ? off() : offs.push(off))).catch(() => undefined);
    };
    keep(api.onPanelAnchor((a) => void show(a)));
    keep(api.onStateChanged((s) => app.setState(s)));
    return () => {
      disposed = true;
      cancelAnimationFrame(raf);
      for (const off of offs) off();
    };
  });
</script>

<svelte:window onkeydown={onKey} onblur={onBlur} onpointerdown={onPointerDown} />

<div class="win" class:demo={api.kind === 'mock'} style:padding="{PAD}px">
  <div
    bind:this={cardEl}
    class="card"
    role="dialog"
    aria-modal="true"
    aria-label="Krova quick panel"
    data-testid="panel"
    data-state={open ? 'open' : 'closed'}
    data-side={side}
    hidden={!open}
    style={cardStyle}
  >
    <header class="head">
      <span class="dot" class:on={arranging} aria-hidden="true"></span>
      <div class="status">
        <strong data-testid="panel-status">{arranging ? 'Arranging' : 'Paused'}</strong>
        <span class="mon" data-testid="panel-monitor">{monitorName}</span>
      </div>
      <button
        bind:this={switchEl}
        type="button"
        class="switch"
        class:on={arranging}
        role="switch"
        aria-checked={arranging}
        aria-label="Arrange windows"
        data-testid="panel-pause"
        onclick={() => setArranging(!arranging)}
      >
        <span class="knob"></span>
      </button>
    </header>

    {#if profile}
      <LayoutChips {profile} />
    {/if}

    <QuickToggles />

    <nav class="acts" aria-label="Krova">
      <button type="button" class="act" data-testid="panel-act-switcher" onclick={() => go('switcher')}>
        <PanelIcon name="switcher" /><span>Layout switcher</span><PanelIcon name="chevron" />
      </button>
      <button type="button" class="act" data-testid="panel-act-cheat" onclick={() => go('cheat_sheet')}>
        <PanelIcon name="sheet" /><span>Cheat sheet</span><PanelIcon name="chevron" />
      </button>
      <button type="button" class="act" data-testid="panel-act-open" onclick={() => go('settings')}>
        <PanelIcon name="open" /><span>Open Krova</span><PanelIcon name="chevron" />
      </button>
    </nav>

    <footer class="foot">
      <button type="button" class="link" data-testid="panel-customise" onclick={() => go('corners')}>Customise the notch…</button>
    </footer>
  </div>
</div>

<style>
  :global(html),
  :global(body) {
    background: transparent;
  }
  :global(body) {
    overflow: hidden;
  }
  .win {
    width: max-content;
  }
  /* Demo: draw a desk so the transparent window can be seen in a browser. */
  .win.demo {
    margin: 24px auto 0;
  }
  :global(body:has(.win.demo)) {
    background: var(--wall);
    min-height: 100vh;
  }

  .card {
    width: 340px;
    padding: 14px;
    border-radius: 18px;
    background: var(--surface);
    color: var(--text);
    box-shadow:
      0 0 0 1px var(--border),
      var(--shadow-lg);
    will-change: transform, opacity;
  }

  .head {
    display: flex;
    align-items: center;
    gap: 10px;
  }
  .dot {
    width: 9px;
    height: 9px;
    flex: none;
    border-radius: 50%;
    background: var(--text-soft);
    transition:
      background-color 0.25s ease,
      box-shadow 0.35s var(--spring);
  }
  .dot.on {
    background: var(--accent);
    box-shadow: 0 0 0 4px var(--accent-soft);
  }
  .status {
    display: flex;
    flex: 1;
    flex-direction: column;
    min-width: 0;
    line-height: 1.25;
  }
  .status strong {
    font-size: 0.98rem;
    font-weight: 650;
    letter-spacing: -0.01em;
  }
  .mon {
    overflow: hidden;
    color: var(--text-muted);
    font-size: 0.76rem;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .switch {
    position: relative;
    flex: none;
    width: 52px;
    height: 31px;
    padding: 0;
    border: 0;
    border-radius: 999px;
    background: var(--surface-3);
    box-shadow: inset 0 0 0 1px var(--border-strong);
    cursor: pointer;
    transition: background-color 0.25s ease;
  }
  .switch.on {
    background: var(--accent);
    box-shadow: none;
  }
  .knob {
    position: absolute;
    top: 3px;
    left: 3px;
    width: 25px;
    height: 25px;
    border-radius: 999px;
    background: #fff;
    box-shadow:
      0 1px 2px rgb(0 0 0 / 0.18),
      0 3px 8px -2px rgb(0 0 0 / 0.22);
    transition:
      transform 0.42s var(--pop),
      width 0.22s var(--spring);
  }
  .switch.on .knob {
    transform: translateX(21px);
  }
  .switch:active .knob {
    width: 30px;
  }
  .switch.on:active .knob {
    transform: translateX(16px);
  }

  .acts {
    display: flex;
    flex-direction: column;
    gap: 2px;
    margin-top: 12px;
    padding-top: 10px;
    border-top: 1px solid var(--border);
  }
  .act {
    display: flex;
    align-items: center;
    gap: 10px;
    min-height: 36px;
    padding: 0 9px;
    border: 0;
    border-radius: 10px;
    background: transparent;
    color: var(--text);
    font: inherit;
    font-size: 0.86rem;
    font-weight: 550;
    text-align: left;
    cursor: pointer;
    transition:
      background-color 0.15s ease,
      transform 0.2s var(--spring);
  }
  .act > :global(svg:first-child) {
    color: var(--text-muted);
  }
  .act span {
    flex: 1;
  }
  .act > :global(svg:last-child) {
    width: 15px;
    height: 15px;
    color: var(--text-soft);
    transition: transform 0.25s var(--spring);
  }
  .act:hover {
    background: var(--surface-2);
  }
  .act:hover > :global(svg:last-child) {
    transform: translateX(2px);
  }
  .act:active {
    transform: scale(0.98);
  }

  .foot {
    display: flex;
    justify-content: center;
    margin-top: 8px;
  }
  .link {
    padding: 3px 6px;
    border: 0;
    border-radius: 6px;
    background: transparent;
    color: var(--text-muted);
    font: inherit;
    font-size: 0.74rem;
    font-weight: 550;
    cursor: pointer;
    text-decoration: underline;
    text-decoration-color: var(--border-strong);
    text-underline-offset: 3px;
  }
  .link:hover {
    color: var(--text);
    text-decoration-color: currentColor;
  }
</style>
