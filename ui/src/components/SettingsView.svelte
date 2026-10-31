<script lang="ts">
  import { app } from '../lib/app.svelte';
  import type { MoveMode, ShortcutModifier, Theme } from '../lib/bindings';
  import {
    MODIFIERS,
    SHORTCUT_MODIFIERS,
    modifierLabel,
    shortcutKeys,
    shortcutLabel,
  } from '../lib/platform';
  import { settings } from '../lib/settings.svelte';
  import Icon from './Icon.svelte';

  const s = $derived(settings.draft);

  // What this build can do here, and what the shortcut registration reported. Both
  // come from the backend; when they are missing nothing is offered.
  const caps = $derived(app.state?.capabilities ?? { shortcuts: false, window_memory: false });
  // (A backend that predates these fields shows both groups as "not available" instead
  // of breaking the page.)
  const failedShortcuts = $derived(app.state?.shortcuts?.failed ?? []);
  // The *saved* setting decides whether "not active" is news: the form value changes
  // 300 ms before the backend hears about it.
  const savedShortcutsOn = $derived(app.state?.config.settings.keyboard_shortcuts ?? false);
  const shortcutsActive = $derived(app.state?.shortcuts?.active ?? false);
  // The keys of the chosen modifier, e.g. ['Win', 'Alt']; Mac words only where they work.
  const modKeys = $derived(s ? shortcutKeys(s.shortcut_modifier, caps.shortcuts) : []);

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

<!-- The chosen modifier keys followed by the rest of the combination: Win + Alt + Left. -->
{#snippet combo(tail: string[], to?: string)}
  {#each [...modKeys, ...tail] as key, i (i)}{#if i > 0}{' + '}{/if}<kbd>{key}</kbd>{/each}{#if to}{' to '}<kbd>{to}</kbd>{/if}
{/snippet}

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

    <div
      class="card"
      role="group"
      aria-labelledby="shortcuts-heading"
      data-testid="settings-shortcuts"
    >
      <h3 class="card-title" id="shortcuts-heading">Keyboard shortcuts</h3>

      {#if !caps.shortcuts}
        <p class="notice" data-testid="shortcuts-unavailable">
          <Icon name="info" />
          <span>Keyboard shortcuts are not available on this system yet.</span>
        </p>
      {/if}

      <div class="row">
        <div class="text">
          <label for="set-shortcuts">Use keyboard shortcuts</label>
          <p class="desc" id="set-shortcuts-desc">
            Move the focused window to another zone or monitor without the mouse.
          </p>
        </div>
        <input
          id="set-shortcuts"
          class="switch"
          type="checkbox"
          role="switch"
          checked={s.keyboard_shortcuts}
          disabled={!caps.shortcuts}
          aria-describedby="set-shortcuts-desc"
          data-testid="setting-keyboard_shortcuts"
          onchange={(e) => settings.update({ keyboard_shortcuts: e.currentTarget.checked })}
        />
      </div>

      <div class="field">
        <label for="set-shortcut-mod">Modifier keys</label>
        <p class="desc" id="set-shortcut-mod-desc">
          Hold these keys together with the keys in the list below.
        </p>
        <select
          id="set-shortcut-mod"
          class="mod-select"
          disabled={!caps.shortcuts || !s.keyboard_shortcuts}
          aria-describedby="set-shortcut-mod-desc"
          data-testid="setting-shortcut_modifier"
          onchange={(e) =>
            settings.update({ shortcut_modifier: e.currentTarget.value as ShortcutModifier })}
        >
          {#each SHORTCUT_MODIFIERS as m (m)}
            <option value={m} selected={s.shortcut_modifier === m}>
              {shortcutLabel(m, caps.shortcuts)}
            </option>
          {/each}
        </select>
      </div>

      <!-- Polite live region: a conflict that shows up after a change is announced. -->
      <div class="live" role="status" aria-live="polite" data-testid="shortcuts-status">
        {#if caps.shortcuts && failedShortcuts.length > 0}
          <div class="notice warn" data-testid="shortcuts-failed">
            <Icon name="warning" />
            <div>
              <p class="strong">These shortcuts could not be registered:</p>
              <ul>
                {#each failedShortcuts as message, i (i)}
                  <li>{message}</li>
                {/each}
              </ul>
              <p>Another app is probably using them. Choose other modifier keys above, or close that app.</p>
            </div>
          </div>
        {:else if caps.shortcuts && savedShortcutsOn && !shortcutsActive}
          <p class="notice" data-testid="shortcuts-inactive">
            <Icon name="info" />
            <span>Keyboard shortcuts are not active right now.</span>
          </p>
        {/if}
      </div>

      {#if caps.shortcuts}
        <div class="keymap-box">
          <h4 class="sub-title">What the keys do</h4>
          <dl class="keymap" data-testid="shortcuts-keymap">
            <div class="km">
              <dt data-testid="keymap-left">{@render combo(['Left'])}</dt>
              <dd>Move the focused window to the previous zone</dd>
            </div>
            <div class="km">
              <dt data-testid="keymap-right">{@render combo(['Right'])}</dt>
              <dd>Move the focused window to the next zone</dd>
            </div>
            <div class="km">
              <dt data-testid="keymap-number">{@render combo(['1'], '9')}</dt>
              <dd>Move the focused window to that zone number</dd>
            </div>
            <div class="km">
              <dt data-testid="keymap-up">{@render combo(['Up'])}</dt>
              <dd>Send the focused window to the next monitor, keeping its zone number</dd>
            </div>
          </dl>
        </div>
      {/if}
    </div>

    <div class="card" role="group" aria-labelledby="windows-heading" data-testid="settings-windows">
      <h3 class="card-title" id="windows-heading">Windows</h3>

      {#if !caps.window_memory}
        <p class="notice" data-testid="window-memory-unavailable">
          <Icon name="info" />
          <span>Remembering snapped windows is not available on this system yet.</span>
        </p>
      {/if}

      <div class="row">
        <div class="text">
          <label for="set-restore">Restore windows on reconnect</label>
          <p class="desc" id="set-restore-desc">
            When a monitor comes back, windows you snapped to it return to their zones. Kept in
            memory only.
          </p>
        </div>
        <input
          id="set-restore"
          class="switch"
          type="checkbox"
          role="switch"
          checked={s.restore_on_reconnect}
          disabled={!caps.window_memory}
          aria-describedby="set-restore-desc"
          data-testid="setting-restore_on_reconnect"
          onchange={(e) => settings.update({ restore_on_reconnect: e.currentTarget.checked })}
        />
      </div>

      <div class="row">
        <div class="text">
          <label for="set-linked">Linked dividers</label>
          <p class="desc" id="set-linked-desc">
            Drag the edge between two snapped windows and both resize.
          </p>
        </div>
        <input
          id="set-linked"
          class="switch"
          type="checkbox"
          role="switch"
          checked={s.linked_dividers}
          disabled={!caps.window_memory}
          aria-describedby="set-linked-desc"
          data-testid="setting-linked_dividers"
          onchange={(e) => settings.update({ linked_dividers: e.currentTarget.checked })}
        />
      </div>

      <div class="field slider">
        <label for="set-gap">Space between zones (px)</label>
        <p class="desc" id="set-gap-desc">
          Leaves a gap between neighbouring snapped windows, from 0 to 32 pixels.
        </p>
        <div class="slide">
          <input
            id="set-gap"
            type="range"
            min="0"
            max="32"
            step="1"
            value={s.zone_gap_px}
            aria-describedby="set-gap-desc"
            aria-valuetext="{s.zone_gap_px} pixels"
            data-testid="setting-zone_gap_px"
            oninput={(e) => settings.update({ zone_gap_px: Math.round(Number(e.currentTarget.value)) })}
          />
          <output for="set-gap" class="value">{s.zone_gap_px} px</output>
        </div>
      </div>
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

  /* ---- keyboard shortcuts and window memory ---- */

  .mod-select {
    align-self: flex-start;
    min-width: 200px;
  }

  .switch:disabled,
  .mod-select:disabled {
    cursor: not-allowed;
    opacity: 0.5;
  }

  /* The live region is always in the page; when it is empty it must not add a gap. */
  .live:empty {
    margin-top: -16px;
  }

  .strong {
    font-weight: 700;
  }

  .sub-title {
    margin: 0 0 6px;
    font-size: 0.9rem;
  }

  .keymap {
    display: flex;
    flex-direction: column;
    gap: 6px;
    margin: 0;
  }

  .km {
    display: grid;
    grid-template-columns: minmax(190px, max-content) 1fr;
    align-items: baseline;
    column-gap: 14px;
    row-gap: 2px;
  }

  .km dt {
    white-space: nowrap;
  }

  .km dd {
    margin: 0;
    color: var(--text);
  }

  @media (max-width: 560px) {
    .km {
      grid-template-columns: minmax(0, 1fr);
    }
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
