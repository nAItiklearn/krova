<script lang="ts">
  // Corners & notch: a screen drawing with the notch where it will sit and the
  // four hot corners; the panel that the notch opens is previewed beside it.
  import Page from '../ui/Page.svelte';
  import Group from '../ui/Group.svelte';
  import Row from '../ui/Row.svelte';
  import Toggle from '../ui/Toggle.svelte';
  import Segmented from '../ui/Segmented.svelte';
  import { app } from '../../lib/app.svelte';
  import { ext, type Corner, type HotCornerAction, type NotchPlace } from '../../lib/ext.svelte';

  const notch = $derived(ext.get('notch'));
  const hc = $derived(ext.get('hot_corners'));
  const auto = $derived(ext.get('auto_arrange'));
  const paused = $derived(ext.get('paused'));

  const ACTIONS: { value: HotCornerAction | 'none'; label: string }[] = [
    { value: 'none', label: 'Nothing' },
    { value: 'switcher', label: 'Layout switcher' },
    { value: 'panel', label: 'Open the panel' },
    { value: 'show_zones', label: 'Show zones' },
    { value: 'calm_desk', label: 'Calm desk' },
    { value: 'focus', label: 'Focus mode' },
    { value: 'cheat_sheet', label: 'Cheat sheet' },
    { value: 'pause', label: 'Pause' },
  ];
  const label = (a: HotCornerAction | null): string => ACTIONS.find((x) => x.value === (a ?? 'none'))!.label;
  const CORNERS: { id: Corner; name: string }[] = [
    { id: 'top_left', name: 'Top left' },
    { id: 'top_right', name: 'Top right' },
    { id: 'bottom_left', name: 'Bottom left' },
    { id: 'bottom_right', name: 'Bottom right' },
  ];
  const PLACES: { value: NotchPlace; label: string }[] = [
    { value: 'top_center', label: 'Top' },
    { value: 'top_left', label: 'Top left' },
    { value: 'top_right', label: 'Top right' },
    { value: 'bottom_left', label: 'Bottom left' },
    { value: 'bottom_right', label: 'Bottom right' },
  ];

  let open = $state<Corner | null>(null);
  function pick(c: Corner, v: HotCornerAction | 'none'): void {
    ext.patch('hot_corners', { [c]: v === 'none' ? null : v });
    open = null;
  }

  const monitor = $derived(app.state?.monitors.find((m) => m.primary) ?? app.state?.monitors[0]);
  const profile = $derived(app.state?.config.monitors.find((p) => p.id === monitor?.profile_id));
  const activeName = $derived(profile?.layouts.find((l) => l.id === profile.active_layout)?.name ?? 'Layout');
  let flash = $state<Corner | null>(null);
  function ping(c: Corner): void {
    flash = c;
    setTimeout(() => (flash = null), 900);
  }
</script>

<Page title="Corners & notch" sub="Control Krova from the edge of your screen, without opening this window." testid="view-corners" wide>
  <div class="hero">
    <div class="screen" style="--r: 14px">
      <div class="wall" aria-hidden="true"></div>

      {#if notch.enabled}
        <button
          type="button"
          class="notch at-{notch.corner}"
          class:paused={paused || !auto.enabled}
          aria-label="Preview of the notch"
        >
          <span class="dot"></span>
          <span class="nlabel">{activeName}</span>
        </button>
      {/if}

      {#each CORNERS as c (c.id)}
        <div class="corner c-{c.id}" class:flash={flash === c.id}>
          <button
            type="button"
            class="hot"
            class:set={!!hc[c.id]}
            aria-haspopup="listbox"
            aria-expanded={open === c.id}
            aria-label={`${c.name} corner: ${label(hc[c.id])}`}
            onclick={() => (open = open === c.id ? null : c.id)}
            onpointerenter={() => ping(c.id)}
          >
            <span class="arc" aria-hidden="true"></span>
            <span class="what">{label(hc[c.id])}</span>
          </button>
          {#if open === c.id}
            <ul class="menu" role="listbox" aria-label={`${c.name} corner action`}>
              {#each ACTIONS as a (a.value)}
                <li role="option" aria-selected={(hc[c.id] ?? 'none') === a.value}>
                  <button type="button" onclick={() => pick(c.id, a.value)}>{a.label}</button>
                </li>
              {/each}
            </ul>
          {/if}
        </div>
      {/each}
    </div>

    <aside class="panel" aria-label="Preview of the panel the notch opens">
      <p class="p-cap">The notch opens this</p>
      <div class="p-card">
        <div class="p-head">
          <span class="p-dot" class:on={auto.enabled && !paused}></span>
          <strong>{auto.enabled && !paused ? 'Arranging' : 'Paused'}</strong>
          <span class="p-mon">{monitor?.label ?? 'Display 1'}</span>
        </div>
        <div class="p-layouts">
          {#each (profile?.layouts ?? []).slice(0, 4) as l (l.id)}
            <span class="p-chip" class:on={l.id === profile?.active_layout}>
              <svg viewBox="0 0 32 20" aria-hidden="true">
                {#each l.zones as z (z.id)}
                  <rect x={z.rect.x * 32 + 0.8} y={z.rect.y * 20 + 0.8} width={z.rect.w * 32 - 1.6} height={z.rect.h * 20 - 1.6} rx="1.6" />
                {/each}
              </svg>
              {l.name}
            </span>
          {/each}
        </div>
        <div class="p-acts">
          <span>Switcher</span><span>Focus</span><span>Calm desk</span><span>Pause</span>
        </div>
      </div>
    </aside>
  </div>

  <div class="cols">
    <Group title="Notch" id="co-notch" desc="A small pill on your screen. Hover to peek, click for the panel, right-click for a menu.">
      <Row label="Show the notch" for="co-n-on">
        <Toggle id="co-n-on" checked={notch.enabled} onchange={(v) => ext.patch('notch', { enabled: v })} />
      </Row>
      <Row label="Position">
        {#snippet below()}
          <!-- Five choices: on its own line so the label is never squeezed. -->
          <Segmented name="notch-pos" label="Notch position" options={PLACES} value={notch.corner} onchange={(v) => ext.patch('notch', { corner: v })} />
        {/snippet}
      </Row>
      <Row label="Screen" desc="Stay on the main screen or follow the mouse.">
        <Segmented
          name="notch-mon"
          label="Notch screen"
          options={[{ value: 'primary', label: 'Main' }, { value: 'cursor', label: 'Under the mouse' }]}
          value={notch.monitor}
          onchange={(v) => ext.patch('notch', { monitor: v })}
        />
      </Row>
      <Row label="Hide during full screen" desc="Videos, games and slides stay clean." for="co-n-fs">
        <Toggle id="co-n-fs" checked={notch.hide_in_fullscreen} onchange={(v) => ext.patch('notch', { hide_in_fullscreen: v })} />
      </Row>
    </Group>

    <Group title="Screen corners" id="co-corners" desc="Push the mouse into a corner to run an action.">
      {#each CORNERS as c (c.id)}
        <Row label={c.name} for="co-{c.id}">
          <select id="co-{c.id}" value={hc[c.id] ?? 'none'} onchange={(e) => pick(c.id, e.currentTarget.value as HotCornerAction | 'none')}>
            {#each ACTIONS as a (a.value)}<option value={a.value}>{a.label}</option>{/each}
          </select>
        </Row>
      {/each}
      <Row label="Wait before acting" desc="Stops accidental triggers." for="co-dwell">
        <div class="slide">
          <input id="co-dwell" type="range" min="150" max="1000" step="50" value={hc.dwell_ms}
            oninput={(e) => ext.patch('hot_corners', { dwell_ms: Number(e.currentTarget.value) })} />
          <output for="co-dwell">{hc.dwell_ms} ms</output>
        </div>
      </Row>
      <Row label="Ignore during full screen" for="co-fs">
        <Toggle id="co-fs" checked={hc.ignore_fullscreen} onchange={(v) => ext.patch('hot_corners', { ignore_fullscreen: v })} />
      </Row>
    </Group>
  </div>
</Page>

<style>
  .hero {
    display: grid;
    grid-template-columns: minmax(0, 1.7fr) minmax(240px, 1fr);
    gap: 22px;
    margin-bottom: 28px;
  }
  .screen {
    position: relative;
    aspect-ratio: 16 / 10;
    border-radius: 18px;
    padding: 10px;
    background: #1b1b1a;
    box-shadow: var(--shadow-lg);
  }
  .wall {
    position: absolute;
    inset: 10px;
    border-radius: 10px;
    background: var(--wall);
  }

  .notch {
    position: absolute;
    z-index: 3;
    display: flex;
    align-items: center;
    gap: 7px;
    width: 46px;
    height: 13px;
    padding: 0 8px;
    border: 0;
    border-radius: 999px;
    background: #141413;
    color: #fff;
    font: inherit;
    font-size: 0.72rem;
    font-weight: 600;
    overflow: hidden;
    cursor: pointer;
    box-shadow: 0 6px 18px -6px rgb(0 0 0 / 0.5);
    transition:
      width 0.5s var(--spring),
      height 0.5s var(--spring),
      left 0.6s var(--spring),
      top 0.6s var(--spring),
      right 0.6s var(--spring),
      bottom 0.6s var(--spring);
  }
  .notch:hover,
  .notch:focus-visible {
    width: 168px;
    height: 28px;
  }
  .notch .dot {
    width: 6px;
    height: 6px;
    border-radius: 50%;
    flex: none;
    background: var(--accent);
    box-shadow: 0 0 0 3px color-mix(in srgb, var(--accent) 30%, transparent);
  }
  .notch.paused .dot {
    background: #8a877f;
    box-shadow: none;
  }
  .nlabel {
    white-space: nowrap;
    opacity: 0;
    transition: opacity 0.25s ease 0.1s;
  }
  .notch:hover .nlabel,
  .notch:focus-visible .nlabel {
    opacity: 1;
  }
  .at-top_center {
    top: 14px;
    left: calc(50% - 23px);
  }
  .at-top_center:hover,
  .at-top_center:focus-visible {
    left: calc(50% - 84px);
  }
  .at-top_left {
    top: 14px;
    left: 64px;
  }
  .at-top_right {
    top: 14px;
    right: 64px;
  }
  .at-bottom_left {
    bottom: 14px;
    left: 64px;
  }
  .at-bottom_right {
    bottom: 14px;
    right: 64px;
  }

  .corner {
    position: absolute;
    z-index: 4;
  }
  .c-top_left {
    top: 10px;
    left: 10px;
  }
  .c-top_right {
    top: 10px;
    right: 10px;
  }
  .c-bottom_left {
    bottom: 10px;
    left: 10px;
  }
  .c-bottom_right {
    bottom: 10px;
    right: 10px;
  }
  .hot {
    position: relative;
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 10px;
    border: 0;
    background: transparent;
    font: inherit;
    cursor: pointer;
  }
  .c-top_right .hot,
  .c-bottom_right .hot {
    flex-direction: row-reverse;
  }
  .c-bottom_left .hot,
  .c-bottom_right .hot {
    align-items: flex-end;
  }
  .arc {
    width: 26px;
    height: 26px;
    border: 2.5px solid color-mix(in srgb, var(--text) 25%, transparent);
    transition:
      border-color 0.3s ease,
      transform 0.4s var(--pop);
  }
  .c-top_left .arc {
    border-right: 0;
    border-bottom: 0;
    border-top-left-radius: 12px;
  }
  .c-top_right .arc {
    border-left: 0;
    border-bottom: 0;
    border-top-right-radius: 12px;
  }
  .c-bottom_left .arc {
    border-right: 0;
    border-top: 0;
    border-bottom-left-radius: 12px;
  }
  .c-bottom_right .arc {
    border-left: 0;
    border-top: 0;
    border-bottom-right-radius: 12px;
  }
  .hot.set .arc {
    border-color: var(--accent);
  }
  .hot:hover .arc,
  .flash .arc {
    transform: scale(1.18);
  }
  .what {
    padding: 4px 9px;
    border-radius: 999px;
    background: color-mix(in srgb, var(--surface) 85%, transparent);
    backdrop-filter: blur(8px);
    box-shadow: 0 0 0 1px var(--border);
    color: var(--text);
    font-size: 0.75rem;
    font-weight: 600;
    white-space: nowrap;
  }
  .hot:not(.set) .what {
    color: var(--text-soft);
  }
  .menu {
    position: absolute;
    z-index: 10;
    min-width: 180px;
    margin: 0;
    padding: 5px;
    list-style: none;
    border-radius: 12px;
    background: var(--surface);
    box-shadow:
      0 0 0 1px var(--border),
      0 18px 40px -12px rgb(0 0 0 / 0.35);
    animation: pop 0.35s var(--spring) both;
  }
  @keyframes pop {
    from {
      opacity: 0;
      transform: scale(0.94) translateY(-4px);
    }
  }
  .c-top_left .menu,
  .c-bottom_left .menu {
    left: 8px;
  }
  .c-top_right .menu,
  .c-bottom_right .menu {
    right: 8px;
  }
  .c-bottom_left .menu,
  .c-bottom_right .menu {
    bottom: 54px;
  }
  .menu button {
    display: block;
    width: 100%;
    padding: 7px 10px;
    border: 0;
    border-radius: 8px;
    background: transparent;
    color: var(--text);
    font: inherit;
    text-align: left;
    cursor: pointer;
  }
  .menu button:hover {
    background: var(--surface-2);
  }
  .menu [aria-selected='true'] button {
    background: var(--accent-soft);
    font-weight: 600;
  }

  .panel {
    display: flex;
    flex-direction: column;
    justify-content: center;
  }
  .p-cap {
    margin: 0 4px 8px;
    color: var(--text-soft);
    font-size: 0.78rem;
    font-weight: 600;
  }
  .p-card {
    padding: 14px;
    border-radius: 18px;
    background: var(--surface);
    box-shadow:
      0 0 0 1px var(--border),
      var(--shadow-lg);
  }
  .p-head {
    display: flex;
    align-items: center;
    gap: 8px;
  }
  .p-dot {
    width: 8px;
    height: 8px;
    border-radius: 50%;
    background: var(--text-soft);
  }
  .p-dot.on {
    background: var(--accent);
    box-shadow: 0 0 0 4px var(--accent-soft);
  }
  .p-mon {
    margin-left: auto;
    color: var(--text-soft);
    font-size: 0.78rem;
  }
  .p-layouts {
    display: grid;
    grid-template-columns: 1fr 1fr;
    gap: 6px;
    margin-top: 12px;
  }
  .p-chip {
    display: flex;
    align-items: center;
    gap: 7px;
    padding: 7px 8px;
    border-radius: 10px;
    background: var(--surface-2);
    font-size: 0.78rem;
    font-weight: 550;
    color: var(--text-muted);
    white-space: nowrap;
    overflow: hidden;
  }
  .p-chip svg {
    width: 26px;
    height: 16px;
    flex: none;
  }
  .p-chip rect {
    fill: var(--surface);
    stroke: var(--border-strong);
    stroke-width: 0.8;
  }
  .p-chip.on {
    background: var(--accent-soft);
    color: var(--text);
    box-shadow: inset 0 0 0 1.5px var(--accent);
  }
  .p-chip.on rect {
    stroke: var(--accent);
  }
  .p-acts {
    display: grid;
    grid-template-columns: repeat(4, 1fr);
    gap: 6px;
    margin-top: 10px;
  }
  .p-acts span {
    padding: 8px 4px;
    border-radius: 10px;
    background: var(--surface-2);
    text-align: center;
    font-size: 0.72rem;
    font-weight: 600;
    color: var(--text-muted);
  }

  .cols {
    display: grid;
    grid-template-columns: repeat(auto-fit, minmax(360px, 1fr));
    gap: 22px;
    align-items: start;
  }
  .cols :global(.group + .group) {
    margin-top: 0;
  }
  .slide {
    display: flex;
    align-items: center;
    gap: 10px;
  }
  .slide input {
    width: 150px;
  }
  output {
    min-width: 54px;
    text-align: right;
    color: var(--text-muted);
    font-weight: 550;
    font-variant-numeric: tabular-nums;
  }
  select {
    min-width: 170px;
  }
  @media (max-width: 980px) {
    .hero {
      grid-template-columns: 1fr;
    }
  }
</style>
