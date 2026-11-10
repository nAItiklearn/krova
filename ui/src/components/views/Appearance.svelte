<script lang="ts">
  // Appearance: a live preview desk on top; theme, accent, window highlight,
  // border, corners and the zone overlay below. Every change shows in the desk.
  import Page from '../ui/Page.svelte';
  import Group from '../ui/Group.svelte';
  import Row from '../ui/Row.svelte';
  import Segmented from '../ui/Segmented.svelte';
  import { app } from '../../lib/app.svelte';
  import { ext, type HighlightMode } from '../../lib/ext.svelte';
  import { settings } from '../../lib/settings.svelte';
  import type { Theme } from '../../lib/bindings';

  const s = $derived(settings.draft ?? app.state!.config.settings);
  const look = $derived(ext.get('appearance'));
  const hl = $derived(ext.get('highlight'));

  const ACCENTS = [
    { name: 'Ember', v: '#ff5b1f' },
    { name: 'Rose', v: '#f43f6b' },
    { name: 'Amber', v: '#f59e0b' },
    { name: 'Moss', v: '#2f9e5f' },
    { name: 'Teal', v: '#14a3a3' },
    { name: 'Violet', v: '#8b5cf6' },
    { name: 'Graphite', v: '#57534e' },
  ];
  const THEMES: { value: Theme; label: string; testid: string }[] = [
    { value: 'system', label: 'System', testid: 'setting-theme-system' },
    { value: 'light', label: 'Light', testid: 'setting-theme-light' },
    { value: 'dark', label: 'Dark', testid: 'setting-theme-dark' },
  ];
  const MODES: { value: HighlightMode; label: string }[] = [
    { value: 'none', label: 'None' },
    { value: 'active', label: 'Active border' },
    { value: 'all', label: 'All borders' },
    { value: 'glow', label: 'Glow' },
  ];

  // Zone overlay colour: the hex field keeps what was typed until it is valid.
  let hexText = $state('');
  let hexInvalid = $state(false);
  $effect(() => {
    hexText = s.highlight_color;
    hexInvalid = false;
  });
  function onHex(raw: string): void {
    hexText = raw.trim();
    const v = hexText.startsWith('#') ? hexText : `#${hexText}`;
    if (/^#[0-9a-fA-F]{6}$/.test(v)) {
      hexInvalid = false;
      settings.update({ highlight_color: v.toLowerCase() });
    } else hexInvalid = true;
  }

  // Preview: the focused window cycles every few seconds so the highlight moves.
  let focus = $state(0);
  $effect(() => {
    const t = setInterval(() => (focus = (focus + 1) % 3), 2600);
    return () => clearInterval(t);
  });
  const gap = $derived(Math.max(4, Math.min(16, s.zone_gap_px * 0.6 + 4)));
  function winStyle(i: number): string {
    const on = i === focus;
    const w = look.border_width_px;
    let ring = '0 0 0 1px rgb(0 0 0 / 0.1)';
    if ((hl.mode === 'active' && on) || hl.mode === 'all') ring = `0 0 0 ${w}px ${look.accent}`;
    if (hl.mode === 'glow' && on)
      ring = `0 0 0 ${Math.max(1, w - 1)}px ${look.accent}, 0 0 22px 3px color-mix(in srgb, ${look.accent} 50%, transparent)`;
    return `border-radius:${look.corner_radius_px}px; box-shadow:${ring}, 0 14px 30px -14px rgb(40 20 10 / 0.4)`;
  }
</script>

<Page title="Appearance" sub="Make Krova look like yours. The desk below follows every change." testid="view-appearance">
  <div class="desk" aria-hidden="true" style="--g: {gap}px">
    <div class="w w1" style={winStyle(0)}>
      <div class="bar"><i></i><span>Browser</span></div>
      <div class="hero"></div>
      <div class="ln"></div>
      <div class="ln s"></div>
    </div>
    <div class="w w2" style={winStyle(1)}>
      <div class="bar"><i class="y"></i><span>Notes</span></div>
      <div class="todo"></div>
      <div class="todo"></div>
    </div>
    <div class="w w3 dark" style={winStyle(2)}>
      <div class="bar"><i class="k"></i><span>Terminal</span></div>
      <div class="code">$ cargo test<br /><b>ok.</b> 350 passed</div>
    </div>
    <div class="zone-pv" style="background:{s.highlight_color}; opacity:{s.highlight_opacity}; border-radius:{look.corner_radius_px}px"></div>
    <span class="tag">Zone overlay</span>
  </div>

  <Group title="Theme" id="ap-theme">
    <Row label="Look" desc="System follows your Windows light or dark mode.">
      <Segmented name="theme" label="Theme" options={THEMES} value={s.theme} onchange={(v) => settings.update({ theme: v })} />
    </Row>
  </Group>

  <Group title="Accent" id="ap-accent" desc="Used for highlights, the notch and the active zone.">
    <div class="swatches" role="radiogroup" aria-label="Accent colour">
      {#each ACCENTS as a (a.v)}
        <button
          type="button"
          role="radio"
          aria-checked={look.accent === a.v}
          aria-label={a.name}
          class="sw"
          style="--c: {a.v}"
          onclick={() => ext.patch('appearance', { accent: a.v })}
        ></button>
      {/each}
      <label class="sw custom" aria-label="Custom accent colour">
        <input type="color" value={look.accent} oninput={(e) => ext.patch('appearance', { accent: e.currentTarget.value })} />
      </label>
      <span class="sw-name">{ACCENTS.find((a) => a.v === look.accent)?.name ?? look.accent}</span>
    </div>
  </Group>

  <Group title="Windows" id="ap-windows">
    <Row label="Highlight" desc="How Krova marks the window you are working in.">
      <Segmented name="hl" label="Highlight" options={MODES} value={hl.mode} onchange={(v) => ext.patch('highlight', { mode: v })} />
    </Row>
    <Row label="Border width" for="ap-bw">
      <div class="slide">
        <input id="ap-bw" type="range" min="1" max="6" step="1" value={look.border_width_px}
          oninput={(e) => ext.patch('appearance', { border_width_px: Number(e.currentTarget.value) })} />
        <output for="ap-bw">{look.border_width_px} px</output>
      </div>
    </Row>
    <Row label="Corner radius" desc="For zones, borders and Krova's overlays." for="ap-rad">
      <div class="slide">
        <input id="ap-rad" type="range" min="0" max="16" step="1" value={look.corner_radius_px}
          oninput={(e) => ext.patch('appearance', { corner_radius_px: Number(e.currentTarget.value) })} />
        <output for="ap-rad">{look.corner_radius_px} px</output>
      </div>
    </Row>
  </Group>

  <Group title="Zone overlay" id="ap-overlay" desc="The zones you see while you drag a window.">
    <Row label="Colour" for="set-color-hex">
      <div class="color">
        <input type="color" aria-label="Highlight color picker" value={s.highlight_color} data-testid="setting-highlight_color-picker"
          oninput={(e) => settings.update({ highlight_color: e.currentTarget.value.toLowerCase() })} />
        <input id="set-color-hex" type="text" class="hex" maxlength="7" spellcheck="false" autocomplete="off" value={hexText}
          aria-invalid={hexInvalid} aria-describedby={hexInvalid ? 'set-color-err' : undefined}
          data-testid="setting-highlight_color" oninput={(e) => onHex(e.currentTarget.value)} />
        <button type="button" class="btn small" onclick={() => settings.update({ highlight_color: look.accent.toLowerCase() })}>Use accent</button>
      </div>
    </Row>
    {#if hexInvalid}<p class="err" id="set-color-err">Use a colour like #ff5b1f</p>{/if}
    <Row label="Opacity" for="set-opacity">
      <div class="slide">
        <input id="set-opacity" type="range" min="0" max="1" step="0.01" value={s.highlight_opacity} data-testid="setting-highlight_opacity"
          oninput={(e) => settings.update({ highlight_opacity: Number(e.currentTarget.value) })} />
        <output for="set-opacity">{Math.round(s.highlight_opacity * 100)}%</output>
      </div>
    </Row>
  </Group>
</Page>

<style>
  .desk {
    position: relative;
    display: grid;
    grid-template-columns: 1.5fr 1fr;
    grid-template-rows: 1fr 1fr;
    gap: var(--g);
    height: 230px;
    margin-bottom: 28px;
    padding: var(--g);
    padding-bottom: calc(var(--g) + 26px);
    border-radius: var(--radius-xl);
    background: var(--wall);
    box-shadow: inset 0 0 0 1px var(--border);
    overflow: hidden;
    transition: gap 0.4s var(--spring), padding 0.4s var(--spring);
  }
  .w {
    background: #fff;
    overflow: hidden;
    transition:
      box-shadow 0.45s var(--spring),
      border-radius 0.3s var(--spring);
  }
  .w1 {
    grid-row: span 2;
  }
  .dark {
    background: #161615;
  }
  .bar {
    display: flex;
    align-items: center;
    gap: 6px;
    height: 20px;
    padding: 0 8px;
    background: #f3f1ed;
    font-size: 9px;
    color: #5c5a55;
  }
  .dark .bar {
    background: #222;
    color: #aaa;
  }
  .bar i {
    width: 8px;
    height: 8px;
    border-radius: 2px;
    background: linear-gradient(135deg, #ff8a5c, #ff5b1f);
  }
  .bar i.y {
    background: #f5b301;
  }
  .bar i.k {
    background: #555;
  }
  .hero {
    height: 54px;
    margin: 10px;
    border-radius: 6px;
    background: linear-gradient(120deg, #f08a5d, #b8405e 60%, #6a2c70);
  }
  .ln {
    height: 6px;
    margin: 0 10px 6px;
    border-radius: 9px;
    background: #ebe8e2;
  }
  .ln.s {
    width: 50%;
  }
  .todo {
    height: 7px;
    margin: 10px 10px 0;
    border-radius: 9px;
    background: #ebe8e2;
  }
  .code {
    padding: 8px 10px;
    font: 9px/1.6 var(--mono);
    color: #ccc;
  }
  .code b {
    color: #6fd08c;
  }
  .zone-pv {
    position: absolute;
    right: 14px;
    bottom: 8px;
    width: 90px;
    height: 14px;
    transition: background 0.3s ease, opacity 0.3s ease;
  }
  .tag {
    position: absolute;
    right: 112px;
    bottom: 7px;
    font-size: 0.72rem;
    color: var(--text-muted);
  }

  .swatches {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 10px;
    padding: 16px;
  }
  .sw {
    position: relative;
    width: 30px;
    height: 30px;
    padding: 0;
    border: 0;
    border-radius: 50%;
    background: var(--c);
    box-shadow: inset 0 0 0 1px rgb(0 0 0 / 0.12);
    cursor: pointer;
    transition: transform 0.3s var(--pop), box-shadow 0.2s ease;
  }
  .sw:hover {
    transform: scale(1.1);
  }
  .sw[aria-checked='true'] {
    box-shadow:
      0 0 0 2.5px var(--surface),
      0 0 0 4.5px var(--c);
  }
  .custom {
    display: grid;
    place-items: center;
    background: conic-gradient(#ff5b1f, #f59e0b, #2f9e5f, #14a3a3, #8b5cf6, #f43f6b, #ff5b1f);
    overflow: hidden;
  }
  .custom input {
    position: absolute;
    inset: 0;
    opacity: 0;
    cursor: pointer;
  }
  .sw-name {
    margin-left: 6px;
    color: var(--text-muted);
    font-weight: 550;
  }
  .slide {
    display: flex;
    align-items: center;
    gap: 10px;
  }
  .slide input {
    width: 180px;
  }
  output {
    min-width: 42px;
    text-align: right;
    font-variant-numeric: tabular-nums;
    color: var(--text-muted);
    font-weight: 550;
  }
  .color {
    display: flex;
    align-items: center;
    gap: 8px;
  }
  .color input[type='color'] {
    width: 34px;
    height: 34px;
    padding: 0;
    border: 0;
    border-radius: 9px;
    background: none;
    cursor: pointer;
  }
  .hex {
    width: 96px;
    font-family: var(--mono);
  }
  .err {
    padding: 0 16px 10px;
    color: var(--danger);
    font-size: 0.82rem;
  }
</style>
