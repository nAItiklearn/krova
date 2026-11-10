<script lang="ts">
  // The hover zone picker: the zones of the monitor under the cursor as a small
  // map. Hover a zone to light it, click it (or press its number) to put the
  // window there. Closes on `hover-picker-end` or Esc. Shows zone shapes only,
  // never a window title or app name.
  import { onMount } from 'svelte';
  import { api, demoControls, errorMessage } from '../lib/backend';
  import type { HoverPicker } from '../lib/bindings';
  import { cursorFraction, pickerMap, placeNear, zoneForKey } from '../lib/hoverPicker';

  const MAP_W = 220;
  const MAP_H = 150;
  const CARD_PAD = 10;

  let picker = $state<HoverPicker | null>(null);
  let hot = $state<number | null>(null);
  let error = $state('');
  let busy = $state(false);
  let placed = $state<number | null>(null);
  const map = $derived(picker ? pickerMap(picker.zones, MAP_W, MAP_H) : null);

  const isDemo = api.kind === 'mock';
  let stage: HTMLElement | undefined = $state();
  let stageSize = $state({ w: 0, h: 0 });

  // Demo only: where the card sits on the fake desktop.
  const pos = $derived.by(() => {
    if (!picker || !map || !isDemo || stageSize.w === 0) return null;
    const { fx, fy } = cursorFraction(picker);
    const cx = fx * stageSize.w;
    const cy = fy * stageSize.h;
    const p = placeNear(cx, cy, map.width + CARD_PAD * 2, map.height + CARD_PAD * 2 + 26, stageSize.w, stageSize.h);
    return { ...p, cx, cy };
  });

  function close(): void {
    picker = null;
    hot = null;
    error = '';
    if (!isDemo) {
      void import('@tauri-apps/api/window')
        .then(({ getCurrentWindow }) => getCurrentWindow().hide())
        .catch(() => {});
    }
  }

  async function choose(id: number): Promise<void> {
    if (busy) return;
    busy = true;
    error = '';
    try {
      await api.placeHovered(id);
      placed = id;
      close();
    } catch (e) {
      error = errorMessage(e);
    } finally {
      busy = false;
    }
  }

  function onKey(e: KeyboardEvent): void {
    if (!picker) return;
    if (e.key === 'Escape') {
      e.preventDefault();
      close();
      return;
    }
    const id = zoneForKey(e.key, picker.zones);
    if (id !== null) {
      e.preventDefault();
      void choose(id);
    }
  }

  onMount(() => {
    const offs: Array<() => void> = [];
    let disposed = false;
    const keep = (p: Promise<() => void>) =>
      p.then((off) => (disposed ? off() : offs.push(off))).catch(() => {});
    keep(
      api.onHoverPicker((p) => {
        picker = p;
        placed = null;
        hot = null;
        error = '';
      }),
    );
    keep(api.onHoverPickerEnd(() => close()));
    // The theme follows the main window's setting.
    void api
      .getState()
      .then((s) => {
        const t = s.config.settings.theme;
        if (t === 'light' || t === 'dark') document.documentElement.dataset.theme = t;
        const accent = (s.config.settings as unknown as { appearance?: { accent?: string } }).appearance?.accent;
        if (accent) document.documentElement.style.setProperty('--accent', accent);
      })
      .catch(() => {});
    const ro = new ResizeObserver(() => {
      if (stage) stageSize = { w: stage.clientWidth, h: stage.clientHeight };
    });
    if (stage) ro.observe(stage);
    return () => {
      disposed = true;
      offs.forEach((off) => off());
      ro.disconnect();
    };
  });

  function simulate(monitorId: string): void {
    demoControls?.hoverPicker(monitorId);
  }
</script>

<svelte:window onkeydown={onKey} />

{#snippet card(p: HoverPicker, m: NonNullable<typeof map>)}
  <div
    class="card"
    role="dialog"
    aria-label="Pick a zone"
    data-testid="picker-card"
    data-layout={p.layout_id}
    style:left={pos ? `${pos.left}px` : undefined}
    style:top={pos ? `${pos.top}px` : undefined}
    class:floating={!!pos}
  >
    <div class="map" style:width="{m.width}px" style:height="{m.height}px">
      {#each m.zones as z, i (z.id)}
        <button
          type="button"
          class="zone"
          class:hot={hot === z.id}
          style:left="{z.x * 100}%"
          style:top="{z.y * 100}%"
          style:width="{z.w * 100}%"
          style:height="{z.h * 100}%"
          style:animation-delay="{i * 35}ms"
          aria-label="Zone {i + 1}"
          data-testid="picker-zone-{z.id}"
          disabled={busy}
          onpointerenter={() => (hot = z.id)}
          onpointerleave={() => (hot = hot === z.id ? null : hot)}
          onfocus={() => (hot = z.id)}
          onblur={() => (hot = hot === z.id ? null : hot)}
          onclick={() => void choose(z.id)}
        >
          {#if i < 9}<span class="num">{i + 1}</span>{/if}
        </button>
      {/each}
    </div>
    <p class="hint" class:err={!!error} data-testid="picker-hint">{error || 'Click a zone · Esc to cancel'}</p>
  </div>
{/snippet}

{#if isDemo}
  <main class="demo" data-testid="picker-demo">
    <header>
      <h1>Hover zone picker</h1>
      <p>In the app this small window shows up next to the cursor while you hold the snap key on a title bar.</p>
      <div class="sim">
        <button type="button" class="btn primary" data-testid="picker-simulate" onclick={() => simulate('monitor-1')}>Simulate on Display 1</button>
        <button type="button" class="btn" data-testid="picker-simulate-2" onclick={() => simulate('monitor-2')}>Display 2</button>
        <button type="button" class="btn" data-testid="picker-simulate-end" disabled={!picker} onclick={() => demoControls?.hoverPickerEnd()}>End</button>
      </div>
      <p class="placed" role="status" data-testid="picker-placed">
        {#if !picker && placed !== null}Placed in zone {placed}.{/if}
      </p>
    </header>
    <div class="stage" bind:this={stage} aria-label="Demo desktop" role="region">
      {#if pos}<span class="cursor" style:left="{pos.cx}px" style:top="{pos.cy}px" aria-hidden="true"></span>{/if}
      {#if picker && map}{@render card(picker, map)}{/if}
    </div>
  </main>
{:else if picker && map}
  <main class="solo">{@render card(picker, map)}</main>
{/if}

<style>
  :global(body.picker-body) {
    background: transparent;
  }
  .solo {
    display: grid;
    place-items: center;
    min-height: 100vh;
  }
  .card {
    padding: 10px;
    border-radius: 16px;
    background: color-mix(in srgb, var(--surface) 92%, transparent);
    backdrop-filter: blur(18px) saturate(1.4);
    box-shadow:
      0 0 0 1px var(--border-strong),
      var(--shadow-lg);
    animation: pk-in 0.32s var(--pop) both;
    transform-origin: top left;
  }
  .card.floating {
    position: absolute;
  }
  @keyframes pk-in {
    from {
      opacity: 0;
      transform: scale(0.9) translateY(-4px);
    }
  }
  .map {
    position: relative;
    border-radius: 9px;
    background: var(--surface-3);
    overflow: hidden;
  }
  .zone {
    position: absolute;
    box-sizing: border-box;
    margin: 0;
    padding: 0;
    border: 2px solid var(--surface-3);
    border-radius: 7px;
    background: var(--surface);
    color: var(--text-muted);
    font: 650 0.85rem var(--font);
    cursor: pointer;
    animation: pk-zone 0.4s var(--spring) both;
    transition:
      background-color 0.15s ease,
      color 0.15s ease,
      transform 0.25s var(--pop);
  }
  @keyframes pk-zone {
    from {
      opacity: 0;
      transform: scale(0.85);
    }
  }
  .zone.hot {
    background: var(--zone-bg-selected);
    box-shadow: inset 0 0 0 2px var(--accent);
    color: var(--text);
    transform: scale(0.97);
  }
  .zone:focus-visible {
    outline: 2px solid var(--focus);
    outline-offset: -4px;
  }
  .hint {
    margin: 7px 2px 0;
    font-size: 0.76rem;
    color: var(--text-muted);
    text-align: center;
  }
  .hint.err {
    color: var(--danger);
  }

  .demo {
    display: flex;
    flex-direction: column;
    gap: 16px;
    min-height: 100vh;
    padding: 28px 32px;
    background: var(--bg);
  }
  .demo h1 {
    font-size: 1.4rem;
    letter-spacing: -0.03em;
  }
  .demo header p {
    margin-top: 4px;
    color: var(--text-muted);
  }
  .sim {
    display: flex;
    gap: 8px;
    margin-top: 12px;
  }
  .placed {
    min-height: 1.3em;
    margin-top: 6px;
    font-weight: 560;
  }
  .stage {
    position: relative;
    flex: 1;
    min-height: 380px;
    border-radius: var(--radius-xl);
    background: var(--wall);
    box-shadow: inset 0 0 0 1px var(--border);
    overflow: hidden;
  }
  .cursor {
    position: absolute;
    width: 12px;
    height: 12px;
    margin: -6px 0 0 -6px;
    border-radius: 50%;
    background: var(--ink);
    box-shadow: 0 0 0 3px var(--surface);
  }
</style>
