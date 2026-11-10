<script lang="ts">
  // The shortcut sheet: every Krova action with its keys, click to record new
  // keys, Backspace to turn one off, conflicts flagged with free suggestions.
  // A keyboard above lights up the keys of the shortcut you point at.
  import Page from '../ui/Page.svelte';
  import Group from '../ui/Group.svelte';
  import Row from '../ui/Row.svelte';
  import Toggle from '../ui/Toggle.svelte';
  import SaveIndicator from '../SaveIndicator.svelte';
  import { app } from '../../lib/app.svelte';
  import { ext } from '../../lib/ext.svelte';
  import { settings } from '../../lib/settings.svelte';
  import type { ShortcutModifier } from '../../lib/bindings';

  type GroupName = 'Windows' | 'Layouts' | 'Krova';
  interface Action {
    id: string;
    label: string;
    group: GroupName;
    def: string;
    /** A family binds modifiers only; the keys after them are fixed (arrows, 1–9). */
    family?: 'Arrows' | '1–9';
  }
  const ACTIONS: Action[] = [
    { id: 'move', label: 'Move to the next zone', group: 'Windows', def: 'Ctrl+Alt', family: 'Arrows' },
    { id: 'swap', label: 'Swap with the neighbour', group: 'Windows', def: 'Ctrl+Alt+Shift', family: 'Arrows' },
    { id: 'zone_n', label: 'Move to zone 1–9', group: 'Windows', def: 'Ctrl+Alt', family: '1–9' },
    { id: 'maximize_zone', label: 'Fill the zone / restore', group: 'Windows', def: 'Ctrl+Alt+Enter' },
    { id: 'stack_next', label: 'Next window in the stack', group: 'Windows', def: 'Ctrl+Alt+]' },
    { id: 'stack_prev', label: 'Previous window in the stack', group: 'Windows', def: 'Ctrl+Alt+[' },
    { id: 'layout_n', label: 'Apply layout 1–9', group: 'Layouts', def: 'Ctrl+Alt+Shift', family: '1–9' },
    { id: 'switcher', label: 'Layout switcher', group: 'Layouts', def: 'Ctrl+Alt+Space' },
    { id: 'pause', label: 'Pause auto-arrange', group: 'Krova', def: 'Ctrl+Alt+T' },
    { id: 'focus', label: 'Focus mode', group: 'Krova', def: 'Ctrl+Alt+F' },
    { id: 'calm_desk', label: 'Calm desk', group: 'Krova', def: 'Ctrl+Alt+D' },
    { id: 'cheat_sheet', label: 'Show this sheet', group: 'Krova', def: 'Ctrl+Alt+/' },
  ];
  const GROUPS: GroupName[] = ['Windows', 'Layouts', 'Krova'];

  const map = $derived(ext.get('shortcuts'));
  const chordOf = (a: Action): string | null => (a.id in map ? map[a.id] : a.def);
  const fullChords = (a: Action, c: string | null): string[] => {
    if (!c) return [];
    if (a.family === 'Arrows') return ['Left', 'Right', 'Up', 'Down'].map((k) => `${c}+${k}`);
    if (a.family === '1–9') return Array.from({ length: 9 }, (_, i) => `${c}+${i + 1}`);
    return [c];
  };

  // Conflicts: two actions that would claim the same final chord.
  const conflicts = $derived.by(() => {
    const owner = new Map<string, string>();
    const bad = new Set<string>();
    for (const a of ACTIONS) {
      for (const c of fullChords(a, chordOf(a))) {
        const prev = owner.get(c);
        if (prev && prev !== a.id) {
          bad.add(prev);
          bad.add(a.id);
        } else owner.set(c, a.id);
      }
    }
    return bad;
  });
  const used = $derived(new Set(ACTIONS.flatMap((a) => fullChords(a, chordOf(a)))));
  // Windows keeps a few chords for itself.
  const RESERVED = new Set(['Ctrl+Alt+Delete', 'Win+L', 'Ctrl+Shift+Escape', 'Alt+Tab', 'Alt+F4']);

  function suggestions(a: Action): string[] {
    const c = chordOf(a);
    if (!c) return [];
    const key = a.family ? '' : c.split('+').pop()!;
    const combos = [['Ctrl', 'Alt'], ['Ctrl', 'Alt', 'Shift'], ['Ctrl', 'Shift'], ['Alt', 'Shift'], ['Win', 'Alt'], ['Ctrl', 'Win']];
    const out: string[] = [];
    for (const m of combos) {
      const cand = a.family ? m.join('+') : [...m, key].join('+');
      if (cand === c) continue;
      if (fullChords(a, cand).some((f) => used.has(f) || RESERVED.has(f))) continue;
      out.push(cand);
      if (out.length === 3) break;
    }
    return out;
  }
  function setChord(a: Action, c: string | null): void {
    ext.set('shortcuts', { ...map, [a.id]: c });
  }

  // ---- recording ----
  let recording = $state<string | null>(null);
  let held = $state<string[]>([]);
  const NAMED: Record<string, string> = {
    Space: 'Space', Enter: 'Enter', Slash: '/', BracketLeft: '[', BracketRight: ']', Minus: '-',
    Equal: '=', Comma: ',', Period: '.', Semicolon: ';', Quote: "'", Backquote: '`', Backslash: '\\',
    Home: 'Home', End: 'End', PageUp: 'PageUp', PageDown: 'PageDown',
  };
  function keyName(e: KeyboardEvent): string | null {
    const c = e.code;
    if (c.startsWith('Key')) return c.slice(3);
    if (c.startsWith('Digit')) return c.slice(5);
    if (c.startsWith('Arrow')) return c.slice(5);
    if (NAMED[c]) return NAMED[c];
    if (/^F\d{1,2}$/.test(c)) return c;
    return null;
  }
  function modsOf(e: KeyboardEvent): string[] {
    const m: string[] = [];
    if (e.ctrlKey) m.push('Ctrl');
    if (e.altKey) m.push('Alt');
    if (e.shiftKey) m.push('Shift');
    if (e.metaKey) m.push('Win');
    return m;
  }
  function onKey(e: KeyboardEvent): void {
    if (!recording) return;
    const a = ACTIONS.find((x) => x.id === recording)!;
    e.preventDefault();
    e.stopPropagation();
    const mods = modsOf(e);
    held = mods;
    if (e.type === 'keyup') return;
    if (e.key === 'Escape') return stop();
    if ((e.key === 'Backspace' || e.key === 'Delete') && mods.length === 0) {
      setChord(a, null);
      return stop();
    }
    if (['Control', 'Alt', 'Shift', 'Meta'].includes(e.key)) return;
    if (!mods.some((m) => m !== 'Shift')) return; // needs Ctrl, Alt or Win
    if (a.family) {
      setChord(a, mods.join('+'));
      return stop();
    }
    const k = keyName(e);
    if (!k) return;
    setChord(a, [...mods, k].join('+'));
    stop();
  }
  function start(id: string): void {
    recording = id;
    held = [];
  }
  function stop(): void {
    recording = null;
    held = [];
  }
  $effect(() => {
    if (!recording) return;
    window.addEventListener('keydown', onKey, true);
    window.addEventListener('keyup', onKey, true);
    return () => {
      window.removeEventListener('keydown', onKey, true);
      window.removeEventListener('keyup', onKey, true);
    };
  });

  // ---- keyboard illustration ----
  let hovered = $state<string | null>(null);
  const lit = $derived.by(() => {
    if (recording) return new Set(held);
    if (!hovered) return new Set<string>();
    const a = ACTIONS.find((x) => x.id === hovered)!;
    const c = chordOf(a);
    if (!c) return new Set<string>();
    const parts = c.split('+');
    if (a.family === 'Arrows') parts.push('Left', 'Right', 'Up', 'Down');
    if (a.family === '1–9') parts.push(...'123456789'.split(''));
    return new Set(parts);
  });
  type K = { k: string; w?: number; label?: string };
  const ROWS: K[][] = [
    [{ k: '`' }, ...'1234567890'.split('').map((k) => ({ k })), { k: '-' }, { k: '=' }, { k: 'Backspace', w: 1.9, label: '⌫' }],
    [{ k: 'Tab', w: 1.45 }, ...'QWERTYUIOP'.split('').map((k) => ({ k })), { k: '[' }, { k: ']' }, { k: '\\', w: 1.45 }],
    [{ k: 'Caps', w: 1.75 }, ...'ASDFGHJKL'.split('').map((k) => ({ k })), { k: ';' }, { k: "'" }, { k: 'Enter', w: 2.15 }],
    [{ k: 'Shift', w: 2.3 }, ...'ZXCVBNM'.split('').map((k) => ({ k })), { k: ',' }, { k: '.' }, { k: '/' }, { k: 'Shift', w: 2.6 }],
    [
      { k: 'Ctrl', w: 1.4 }, { k: 'Win', w: 1.2 }, { k: 'Alt', w: 1.2 }, { k: 'Space', w: 5.6, label: '' },
      { k: 'Alt', w: 1.2 }, { k: 'Ctrl', w: 1.2 }, { k: 'Left', label: '←' }, { k: 'Up', label: '↑' },
      { k: 'Down', label: '↓' }, { k: 'Right', label: '→' },
    ],
  ];

  // ---- v0.1 switches (the current backend reads these) ----
  const s = $derived(settings.draft ?? app.state!.config.settings);
  const capabilities = $derived(app.state?.capabilities);
  const failed = $derived(app.state?.shortcuts.failed ?? []);
  const MOD_LABEL: Record<ShortcutModifier, string> = {
    ctrl_alt_shift: 'Ctrl+Alt+Shift',
    ctrl_alt: 'Ctrl+Alt',
    win_alt: 'Win+Alt',
    win_ctrl: 'Win+Ctrl',
  };

  let sheet = $state(false);
  let sheetEl: HTMLElement | undefined = $state();
  $effect(() => {
    if (sheet) sheetEl?.focus();
  });
</script>

{#snippet keycaps(chord: string | null, family?: string)}
  {#if chord}
    <span class="keys">
      {#each chord.split('+') as k, i (i)}<kbd>{k}</kbd>{/each}
      {#if family}<span class="fam">+ {family === 'Arrows' ? '← ↑ → ↓' : '1–9'}</span>{/if}
    </span>
  {:else}
    <span class="off">Off</span>
  {/if}
{/snippet}

<Page title="Shortcuts" sub="Click a shortcut and press new keys. Backspace turns it off." testid="view-shortcuts" wide>
  {#snippet actions()}
    <SaveIndicator />
    <button type="button" class="btn" data-testid="open-cheat-sheet" onclick={() => (sheet = true)}>Cheat sheet</button>
    <button type="button" class="btn" onclick={() => ext.set('shortcuts', {})} disabled={Object.keys(map).length === 0}>
      Reset all
    </button>
  {/snippet}

  <div class="kb-wrap">
    <div class="kb" aria-hidden="true">
      {#each ROWS as row, r (r)}
        <div class="kr">
          {#each row as key, i (i)}
            <span class="key" class:lit={lit.has(key.k)} style="flex: {key.w ?? 1}">{key.label ?? key.k}</span>
          {/each}
        </div>
      {/each}
    </div>
    <p class="kb-cap" aria-live="polite">
      {#if recording}
        Press the new keys{ACTIONS.find((a) => a.id === recording)?.family ? ' (only the modifiers)' : ''}. Esc
        cancels.
      {:else}
        Point at a shortcut to see its keys.
      {/if}
    </p>
  </div>

  <div class="cols">
    {#each GROUPS as g (g)}
      <Group title={g} id="sc-{g.toLowerCase()}">
        {#each ACTIONS.filter((a) => a.group === g) as a (a.id)}
          {@const c = chordOf(a)}
          {@const bad = conflicts.has(a.id)}
          <div
            class="sc"
            class:bad
            role="group"
            aria-label={a.label}
            onpointerenter={() => (hovered = a.id)}
            onpointerleave={() => (hovered = null)}
          >
            <span class="sc-label">{a.label}</span>
            <button
              type="button"
              class="rec-btn"
              class:recording={recording === a.id}
              aria-label={`${a.label}: ${c ?? 'off'}. Press to record new keys.`}
              data-testid="shortcut-{a.id}"
              onclick={() => (recording === a.id ? stop() : start(a.id))}
              onfocus={() => (hovered = a.id)}
              onblur={() => (hovered = null)}
            >
              {#if recording === a.id}
                <span class="listening">{held.length ? held.join(' + ') : 'Press keys…'}</span>
              {:else}
                {@render keycaps(c, a.family)}
              {/if}
            </button>
            {#if bad}
              <div class="conflict" role="alert">
                <span>Used twice. Try</span>
                {#each suggestions(a) as sug (sug)}
                  <button type="button" class="chip" onclick={() => setChord(a, sug)}>{sug}{a.family ? ' + …' : ''}</button>
                {/each}
              </div>
            {:else if c === 'Ctrl+Alt' && a.family === 'Arrows'}
              <p class="hint">Can clash with code editors and some Intel display drivers.</p>
            {/if}
          </div>
        {/each}
      </Group>
    {/each}
  </div>

  <Group title="Keyboard" id="sc-legacy" testid="settings-shortcuts">
    {#if capabilities && !capabilities.shortcuts}
      <div class="pad"><p class="notice" data-testid="shortcuts-unavailable">Keyboard shortcuts are not available on this system yet.</p></div>
    {/if}
    <Row label="Use keyboard shortcuts" desc="Turn every Krova shortcut on or off." for="set-shortcuts">
      <Toggle
        id="set-shortcuts"
        checked={s.keyboard_shortcuts}
        testid="setting-keyboard_shortcuts"
        disabled={capabilities ? !capabilities.shortcuts : false}
        onchange={(v) => settings.update({ keyboard_shortcuts: v })}
      />
    </Row>
    <Row label="Zone keys today" desc="The keys this build registers for moving between zones." for="set-shortcut-mod">
      <select
        id="set-shortcut-mod"
        data-testid="setting-shortcut_modifier"
        value={s.shortcut_modifier}
        disabled={(capabilities ? !capabilities.shortcuts : false) || !s.keyboard_shortcuts}
        onchange={(e) => settings.update({ shortcut_modifier: e.currentTarget.value as ShortcutModifier })}
      >
        {#each Object.entries(MOD_LABEL) as [v, l] (v)}<option value={v}>{l}</option>{/each}
      </select>
    </Row>
    {#if failed.length}
      <div class="pad" role="status" aria-live="polite" data-testid="shortcuts-status">
        <div class="notice warn" data-testid="shortcuts-failed">
          <div>
            Some keys are taken by another app:
            <ul>{#each failed as f (f)}<li>{f}</li>{/each}</ul>
          </div>
        </div>
      </div>
    {/if}
  </Group>
</Page>

{#if sheet}
  <div class="sheet-scrim" role="presentation" onclick={() => (sheet = false)}>
    <div
      class="sheet"
      role="dialog"
      aria-modal="true"
      aria-label="Krova shortcuts"
      tabindex="-1"
      bind:this={sheetEl}
      onclick={(e) => e.stopPropagation()}
      onkeydown={(e) => e.key === 'Escape' && (sheet = false)}
    >
      <header>
        <h3>Krova shortcuts</h3>
        <button type="button" class="btn small" onclick={() => (sheet = false)}>Done</button>
      </header>
      <div class="sheet-cols">
        {#each GROUPS as g (g)}
          <div>
            <p class="sheet-g">{g}</p>
            {#each ACTIONS.filter((a) => a.group === g) as a (a.id)}
              <div class="sheet-row"><span>{a.label}</span>{@render keycaps(chordOf(a), a.family)}</div>
            {/each}
          </div>
        {/each}
      </div>
    </div>
  </div>
{/if}

<style>
  .kb-wrap {
    margin-bottom: 26px;
    padding: 24px 22px 14px;
    border-radius: var(--radius-xl);
    background: var(--wall);
    box-shadow: inset 0 0 0 1px var(--border);
  }
  .kb {
    display: flex;
    flex-direction: column;
    gap: 5px;
    max-width: 660px;
    margin: 0 auto;
    padding: 10px;
    border-radius: 16px;
    background: color-mix(in srgb, var(--surface) 72%, transparent);
    backdrop-filter: blur(10px);
    box-shadow:
      0 0 0 1px var(--border),
      var(--shadow-lg);
  }
  .kr {
    display: flex;
    gap: 5px;
  }
  .key {
    display: grid;
    place-items: center;
    min-width: 0;
    height: 30px;
    border-radius: 6px;
    background: var(--surface);
    box-shadow:
      inset 0 0 0 1px var(--border),
      inset 0 -1.5px 0 var(--border-strong);
    color: var(--text-muted);
    font-size: 0.68rem;
    font-weight: 600;
    transition:
      background 0.25s ease,
      color 0.25s ease,
      transform 0.35s var(--pop),
      box-shadow 0.25s ease;
  }
  .key.lit {
    background: var(--accent);
    color: #fff;
    box-shadow: 0 4px 12px -4px color-mix(in srgb, var(--accent) 70%, transparent);
    transform: translateY(1px);
  }
  .kb-cap {
    margin-top: 12px;
    text-align: center;
    color: var(--text-muted);
    font-size: 0.82rem;
  }

  .cols {
    display: grid;
    grid-template-columns: repeat(auto-fit, minmax(330px, 1fr));
    gap: 22px;
    align-items: start;
    margin-bottom: 26px;
  }
  .cols :global(.group + .group) {
    margin-top: 0;
  }
  .sc {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 6px 10px;
    padding: 9px 10px 9px 16px;
    transition: background 0.2s ease;
  }
  .sc:hover {
    background: var(--surface-2);
  }
  .sc.bad {
    background: var(--danger-soft);
  }
  .sc-label {
    flex: 1;
    min-width: 0;
    font-weight: 550;
  }
  .rec-btn {
    display: inline-flex;
    align-items: center;
    min-height: 32px;
    padding: 0 6px;
    border: 0;
    border-radius: 10px;
    background: transparent;
    font: inherit;
    cursor: pointer;
    transition:
      background 0.2s ease,
      box-shadow 0.2s ease;
  }
  .rec-btn:hover {
    background: var(--surface);
    box-shadow: inset 0 0 0 1px var(--border);
  }
  .rec-btn.recording {
    background: var(--surface);
    box-shadow:
      inset 0 0 0 1.5px var(--accent),
      0 0 0 4px var(--accent-soft);
    animation: listen 1.4s ease-in-out infinite;
  }
  @keyframes listen {
    50% {
      box-shadow:
        inset 0 0 0 1.5px var(--accent),
        0 0 0 7px var(--accent-softer);
    }
  }
  .listening {
    padding: 0 8px;
    color: var(--accent-ink);
    font-weight: 600;
    font-size: 0.85rem;
  }
  .keys {
    display: inline-flex;
    align-items: center;
    gap: 4px;
  }
  .fam {
    margin-left: 2px;
    color: var(--text-muted);
    font-size: 0.82rem;
    font-weight: 550;
  }
  .off {
    padding: 0 8px;
    color: var(--text-soft);
    font-weight: 550;
  }
  .conflict {
    flex-basis: 100%;
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 6px;
    color: var(--danger);
    font-size: 0.82rem;
    font-weight: 550;
  }
  .chip {
    padding: 3px 9px;
    border: 0;
    border-radius: 999px;
    background: var(--surface);
    box-shadow: inset 0 0 0 1px var(--border-strong);
    color: var(--text);
    font: inherit;
    font-size: 0.8rem;
    cursor: pointer;
  }
  .chip:hover {
    box-shadow: inset 0 0 0 1.5px var(--accent);
  }
  .hint {
    flex-basis: 100%;
    color: var(--text-soft);
    font-size: 0.78rem;
  }
  .pad {
    padding: 12px 16px;
  }
  select {
    min-width: 170px;
  }

  .sheet-scrim {
    position: fixed;
    inset: 0;
    z-index: 100;
    display: grid;
    place-items: center;
    background: rgb(10 10 10 / 0.35);
    backdrop-filter: blur(14px) saturate(1.2);
    animation: fade 0.25s ease both;
  }
  @keyframes fade {
    from {
      opacity: 0;
    }
  }
  .sheet {
    width: min(880px, calc(100vw - 48px));
    max-height: calc(100vh - 64px);
    overflow: auto;
    padding: 22px 24px 24px;
    border-radius: 22px;
    background: color-mix(in srgb, var(--surface) 94%, transparent);
    box-shadow:
      0 0 0 1px var(--border),
      0 40px 80px -24px rgb(0 0 0 / 0.45);
    animation: sheet-in 0.45s var(--spring) both;
    outline: none;
  }
  @keyframes sheet-in {
    from {
      opacity: 0;
      transform: translateY(18px) scale(0.97);
    }
  }
  .sheet header {
    display: flex;
    justify-content: space-between;
    align-items: center;
    margin-bottom: 16px;
  }
  .sheet h3 {
    font-size: 1.2rem;
    font-weight: 700;
    letter-spacing: -0.02em;
  }
  .sheet-cols {
    display: grid;
    grid-template-columns: repeat(3, 1fr);
    gap: 24px;
  }
  .sheet-g {
    margin-bottom: 8px;
    font-size: 0.75rem;
    font-weight: 650;
    color: var(--text-soft);
  }
  .sheet-row {
    display: flex;
    justify-content: space-between;
    align-items: center;
    gap: 10px;
    padding: 6px 0;
    border-top: 1px solid var(--border);
    font-size: 0.85rem;
  }
  @media (max-width: 900px) {
    .sheet-cols {
      grid-template-columns: 1fr;
    }
  }
</style>
