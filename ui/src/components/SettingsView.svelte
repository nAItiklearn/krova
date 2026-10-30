<script lang="ts">
  import type { MoveMode, Theme } from '../lib/bindings';
  import { MODIFIERS, modifierLabel } from '../lib/platform';
  import { settings } from '../lib/settings.svelte';
  import Icon from './Icon.svelte';

  const s = $derived(settings.draft);

  const MOVE_MODES: { value: MoveMode; label: string; hint: string }[] = [
    { value: 'animate', label: 'Animate', hint: 'Glide into the zone (about 120 ms)' },
    { value: 'instant', label: 'Instant', hint: 'Jump straight into the zone' },
  ];

  const THEMES: { value: Theme; label: string }[] = [
    { value: 'system', label: 'System' },
    { value: 'light', label: 'Light' },
    { value: 'dark', label: 'Dark' },
  ];

  // The hex field keeps what the user typed until it is a valid colour.
  const color = $derived(s?.highlight_color);
  let hexText = $state('');
  let hexInvalid = $state(false);
  $effect(() => {
    if (color !== undefined) {
      hexText = color;
      hexInvalid = false;
    }
  });

  function onHexInput(e: Event & { currentTarget: HTMLInputElement }): void {
    const raw = e.currentTarget.value.trim();
    hexText = raw;
    const withHash = raw.startsWith('#') ? raw : `#${raw}`;
    if (/^#[0-9a-fA-F]{6}$/.test(withHash)) {
      hexInvalid = false;
      settings.update({ highlight_color: withHash.toLowerCase() });
    } else {
      hexInvalid = true;
    }
  }

  const opacityPct = $derived(s ? Math.round(s.highlight_opacity * 100) : 0);
</script>

<section class="settings" aria-labelledby="settings-heading" data-testid="settings">
  <div class="title">
    <h2 id="settings-heading">Settings</h2>
    <p
      class="save-state"
      role="status"
      aria-live="polite"
      data-testid="save-indicator"
      data-state={settings.status}
    >
      {#if settings.status === 'saving'}
        Saving...
      {:else if settings.status === 'saved'}
        <Icon name="check" /> Saved
      {:else if settings.status === 'error'}
        <span class="err"
          ><Icon name="warning" /> Could not save: {settings.error}</span
        >
        <button type="button" class="btn small" onclick={() => settings.flush()}>Retry</button>
      {/if}
    </p>
  </div>

  {#if s}
    <p class="muted intro">Changes are saved automatically.</p>

    <div class="card">
      <h3 class="card-title">General</h3>

      <div class="row">
        <div class="text">
          <label for="set-enabled">Enable Krova</label>
          <p class="desc" id="set-enabled-desc">When off, dragging windows never shows or uses zones.</p>
        </div>
        <input
          id="set-enabled"
          class="switch"
          type="checkbox"
          role="switch"
          checked={s.enabled}
          aria-describedby="set-enabled-desc"
          data-testid="setting-enabled"
          onchange={(e) => settings.update({ enabled: e.currentTarget.checked })}
        />
      </div>

      <div class="row">
        <div class="text">
          <label for="set-login">Launch at login</label>
          <p class="desc" id="set-login-desc">Start Krova in the tray or menu bar when you sign in.</p>
        </div>
        <input
          id="set-login"
          class="switch"
          type="checkbox"
          role="switch"
          checked={s.launch_at_login}
          aria-describedby="set-login-desc"
          data-testid="setting-launch_at_login"
          onchange={(e) => settings.update({ launch_at_login: e.currentTarget.checked })}
        />
      </div>
    </div>

    <div class="card">
      <h3 class="card-title">Snapping</h3>

      <fieldset class="field">
        <legend>Activation modifier</legend>
        <p class="desc" id="set-modifier-desc">
          Hold {modifierLabel(s.modifier)} while dragging a window to show the zones. Let go of it to cancel.
        </p>
        <div class="segmented" role="radiogroup" aria-describedby="set-modifier-desc">
          {#each MODIFIERS as m (m)}
            <label class="seg">
              <input
                type="radio"
                name="modifier"
                value={m}
                checked={s.modifier === m}
                data-testid="setting-modifier-{m}"
                onchange={() => settings.update({ modifier: m })}
              />
              <span>{modifierLabel(m)}</span>
            </label>
          {/each}
        </div>
      </fieldset>

      <fieldset class="field">
        <legend>Move behavior</legend>
        <div class="segmented" role="radiogroup">
          {#each MOVE_MODES as m (m.value)}
            <label class="seg" title={m.hint}>
              <input
                type="radio"
                name="move_mode"
                value={m.value}
                checked={s.move_mode === m.value}
                data-testid="setting-move_mode-{m.value}"
                onchange={() => settings.update({ move_mode: m.value })}
              />
              <span>{m.label}</span>
            </label>
          {/each}
        </div>
      </fieldset>

      <div class="row">
        <div class="text">
          <label for="set-edge">Edge snapping</label>
          <p class="desc" id="set-edge-desc">
            Drag a window to a screen edge or corner to snap it to a half or quarter. Never triggers
            on the border between two monitors.
          </p>
        </div>
        <input
          id="set-edge"
          class="switch"
          type="checkbox"
          role="switch"
          checked={s.edge_snapping}
          aria-describedby="set-edge-desc"
          data-testid="setting-edge_snapping"
          onchange={(e) => settings.update({ edge_snapping: e.currentTarget.checked })}
        />
      </div>

      <div class="field slider" class:off={!s.edge_snapping}>
        <label for="set-edge-size">Edge size</label>
        <div class="slide">
          <input
            id="set-edge-size"
            type="range"
            min="2"
            max="64"
            step="1"
            value={s.edge_size_px}
            disabled={!s.edge_snapping}
            data-testid="setting-edge_size_px"
            oninput={(e) => settings.update({ edge_size_px: Math.round(Number(e.currentTarget.value)) })}
          />
          <output for="set-edge-size" class="value">{s.edge_size_px} px</output>
        </div>
      </div>
    </div>

    <div class="card">
      <h3 class="card-title">Appearance</h3>

      <div class="field">
        <label for="set-color-hex">Highlight color</label>
        <div class="color">
          <input
            type="color"
            aria-label="Highlight color picker"
            value={s.highlight_color}
            data-testid="setting-highlight_color-picker"
            oninput={(e) => settings.update({ highlight_color: e.currentTarget.value.toLowerCase() })}
          />
          <input
            id="set-color-hex"
            type="text"
            class="hex"
            maxlength="7"
            spellcheck="false"
            autocomplete="off"
            value={hexText}
            aria-invalid={hexInvalid}
            aria-describedby={hexInvalid ? 'set-color-err' : undefined}
            data-testid="setting-highlight_color"
            oninput={onHexInput}
          />
          {#if hexInvalid}
            <span id="set-color-err" class="err small">Use a color like #3b82f6</span>
          {/if}
        </div>
      </div>

      <div class="field slider">
        <label for="set-opacity">Highlight opacity</label>
        <div class="slide">
          <input
            id="set-opacity"
            type="range"
            min="0"
            max="1"
            step="0.01"
            value={s.highlight_opacity}
            data-testid="setting-highlight_opacity"
            oninput={(e) => settings.update({ highlight_opacity: Number(e.currentTarget.value) })}
          />
          <output for="set-opacity" class="value">{opacityPct}%</output>
        </div>
      </div>

      <div class="preview" aria-hidden="true">
        <div class="pv-zone">
          <div class="pv-hl" style:background={s.highlight_color} style:opacity={s.highlight_opacity}></div>
          <span>Zone under the cursor</span>
        </div>
        <div class="pv-zone"><span>Other zone</span></div>
      </div>

      <fieldset class="field">
        <legend>Theme</legend>
        <div class="segmented" role="radiogroup">
          {#each THEMES as t (t.value)}
            <label class="seg">
              <input
                type="radio"
                name="theme"
                value={t.value}
                checked={s.theme === t.value}
                data-testid="setting-theme-{t.value}"
                onchange={() => settings.update({ theme: t.value })}
              />
              <span>{t.label}</span>
            </label>
          {/each}
        </div>
        <p class="desc">System follows your operating system's light or dark mode.</p>
      </fieldset>
    </div>
  {:else}
    <p class="muted">Loading settings...</p>
  {/if}
</section>

<style>
  .settings {
    max-width: 720px;
    margin-inline: auto;
    display: flex;
    flex-direction: column;
    gap: 14px;
  }

  .title {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 12px;
    min-height: 34px;
  }

  .save-state {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    color: var(--success);
    font-weight: 600;
  }

  .save-state :global(svg) {
    width: 18px;
    height: 18px;
  }

  .err {
    color: var(--danger);
    display: inline-flex;
    align-items: center;
    gap: 6px;
  }

  .small {
    font-size: 0.88em;
  }

  .intro {
    margin-top: -8px;
  }

  .card {
    display: flex;
    flex-direction: column;
    gap: 16px;
    padding: 16px 18px;
    background: var(--surface);
    border: 1px solid var(--border);
    border-radius: var(--radius);
    box-shadow: var(--shadow);
  }

  .card-title {
    font-size: 0.8rem;
    letter-spacing: 0.06em;
    text-transform: uppercase;
    color: var(--text-muted);
  }

  .row {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 16px;
  }

  .text label,
  legend,
  .field > label {
    font-weight: 600;
  }

  .desc {
    color: var(--text-muted);
    font-size: 0.9em;
    margin-top: 2px;
  }

  fieldset {
    border: 0;
    margin: 0;
    padding: 0;
    min-width: 0;
  }

  legend {
    padding: 0;
    margin-bottom: 4px;
  }

  .field {
    display: flex;
    flex-direction: column;
    gap: 6px;
  }

  /* ---- switch ---- */

  .switch {
    appearance: none;
    -webkit-appearance: none;
    flex: none;
    position: relative;
    width: 44px;
    height: 26px;
    margin: 0;
    border-radius: 999px;
    border: 1.5px solid var(--border-strong);
    background: var(--surface-2);
    cursor: pointer;
    transition: background-color 0.15s ease;
  }

  .switch::after {
    content: '';
    position: absolute;
    top: 2.5px;
    left: 2.5px;
    width: 18px;
    height: 18px;
    border-radius: 50%;
    background: var(--text-muted);
    transition: transform 0.15s ease;
  }

  .switch:checked {
    background: var(--accent);
    border-color: var(--accent);
  }

  .switch:checked::after {
    transform: translateX(18px);
    background: var(--accent-contrast);
  }

  /* ---- segmented radios ---- */

  .segmented {
    display: inline-flex;
    flex-wrap: wrap;
    border: 1px solid var(--border-strong);
    border-radius: var(--radius-sm);
    overflow: hidden;
    align-self: flex-start;
  }

  .seg {
    position: relative;
    display: inline-flex;
  }

  .seg + .seg {
    border-left: 1px solid var(--border-strong);
  }

  .seg input {
    position: absolute;
    inset: 0;
    margin: 0;
    opacity: 0;
    cursor: pointer;
  }

  .seg span {
    display: inline-flex;
    align-items: center;
    min-height: var(--control-h);
    padding: 0 14px;
    background: var(--surface);
    color: var(--text);
    font-weight: 500;
  }

  .seg input:checked + span {
    background: var(--accent);
    color: var(--accent-contrast);
  }

  .seg input:focus-visible + span {
    outline: var(--ring) solid var(--focus);
    outline-offset: -3px;
  }

  /* ---- sliders ---- */

  .slider.off {
    opacity: 0.6;
  }

  .slide {
    display: flex;
    align-items: center;
    gap: 12px;
  }

  input[type='range'] {
    flex: 1;
    min-width: 0;
    accent-color: var(--accent);
    height: 24px;
    margin: 0;
  }

  .value {
    min-width: 56px;
    text-align: right;
    font: 0.95rem var(--mono);
  }

  /* ---- color ---- */

  .color {
    display: flex;
    align-items: center;
    gap: 10px;
    flex-wrap: wrap;
  }

  input[type='color'] {
    width: 44px;
    height: var(--control-h);
    padding: 2px;
    border: 1px solid var(--border-strong);
    border-radius: var(--radius-sm);
    background: var(--surface);
    cursor: pointer;
  }

  .hex {
    width: 110px;
    font-family: var(--mono);
  }

  .preview {
    display: grid;
    grid-template-columns: 1fr 1fr;
    gap: 8px;
    padding: 10px;
    background: var(--canvas-bg);
    border: 1px solid var(--border);
    border-radius: var(--radius-sm);
  }

  .pv-zone {
    position: relative;
    display: flex;
    align-items: center;
    justify-content: center;
    min-height: 56px;
    border: 1.5px solid var(--zone-border);
    border-radius: var(--radius-sm);
    background: var(--zone-bg);
    overflow: hidden;
    font-size: 0.85em;
    color: var(--text);
  }

  .pv-hl {
    position: absolute;
    inset: 0;
  }

  .pv-zone span {
    position: relative;
  }
</style>
