// Demo-mode mirror of krova-core's shortcuts.rs: the action table, default chords,
// chord parsing/formatting and the conflict probe with suggestions.

import type {
  ProbeResult,
  ShortcutActionState,
  ShortcutGroup,
  ShortcutMap,
  Settings,
} from '../bindings';

interface ActionDef {
  id: string;
  label: string;
  group: ShortcutGroup;
  chord: string;
}

const DIRS = ['left', 'right', 'up', 'down'] as const;
const cap = (s: string) => s[0].toUpperCase() + s.slice(1);

/** Every action, in sheet order, with its default chord (base Ctrl+Alt). */
export const ACTIONS: ActionDef[] = [
  ...DIRS.map((d) => ({
    id: `move_${d}`,
    label: `Move ${d}`,
    group: 'Windows' as const,
    chord: `Ctrl+Alt+${cap(d)}`,
  })),
  ...DIRS.map((d) => ({
    id: `swap_${d}`,
    label: `Swap ${d}`,
    group: 'Windows' as const,
    chord: `Ctrl+Alt+Shift+${cap(d)}`,
  })),
  ...Array.from({ length: 9 }, (_, i) => ({
    id: `zone_${i + 1}`,
    label: `Zone ${i + 1}`,
    group: 'Windows' as const,
    chord: `Ctrl+Alt+${i + 1}`,
  })),
  { id: 'maximize_in_zone', label: 'Fill screen', group: 'Windows', chord: 'Ctrl+Alt+Enter' },
  { id: 'stack_prev', label: 'Previous tab', group: 'Windows', chord: 'Ctrl+Alt+[' },
  { id: 'stack_next', label: 'Next tab', group: 'Windows', chord: 'Ctrl+Alt+]' },
  ...Array.from({ length: 9 }, (_, i) => ({
    id: `layout_${i + 1}`,
    label: `Layout ${i + 1}`,
    group: 'Layouts' as const,
    chord: `Ctrl+Alt+Shift+${i + 1}`,
  })),
  { id: 'switcher', label: 'Layout switcher', group: 'Layouts', chord: 'Ctrl+Alt+Space' },
  { id: 'pause', label: 'Pause arranging', group: 'Krova', chord: 'Ctrl+Alt+T' },
  { id: 'cheat_sheet', label: 'All shortcuts', group: 'Krova', chord: 'Ctrl+Alt+/' },
  { id: 'focus', label: 'Focus', group: 'Krova', chord: 'Ctrl+Alt+F' },
];

/** Chords the demo pretends another program holds. */
export const DEMO_TAKEN = ['Ctrl+Alt+Shift+Win+F', 'Ctrl+Shift+F', 'Win+Alt+T'];

const KEYS: Record<string, string> = {
  left: 'Left',
  right: 'Right',
  up: 'Up',
  down: 'Down',
  space: 'Space',
  enter: 'Enter',
  return: 'Enter',
  slash: '/',
  '/': '/',
  '[': '[',
  ']': ']',
  ',': ',',
  '.': '.',
  '-': '-',
  '=': '=',
};

/** `ctrl+alt+left` -> `Ctrl+Alt+Left`, like Rust's `Chord` parse + display; throws. */
export function normalizeChord(text: string): string {
  const raw = text.trim();
  if (!raw || raw.length > 40)
    throw `"${raw.slice(0, 40)}" is not a shortcut (write it like Ctrl+Alt+Left)`;
  const parts = raw.split('+').map((p) => p.trim());
  const keyText = parts.pop() ?? '';
  const lower = keyText.toLowerCase();
  let key = KEYS[lower];
  if (!key && /^[0-9a-z]$/i.test(keyText)) key = keyText.toUpperCase();
  if (!key && /^f([1-9]|1[0-2])$/i.test(keyText)) key = keyText.toUpperCase();
  const bad = `"${raw.slice(0, 40)}" is not a shortcut (write it like Ctrl+Alt+Left)`;
  if (!key) throw bad;
  const mods = { win: false, ctrl: false, alt: false, shift: false };
  for (const m of parts) {
    const name = (
      {
        ctrl: 'ctrl',
        control: 'ctrl',
        alt: 'alt',
        option: 'alt',
        shift: 'shift',
        win: 'win',
        super: 'win',
        meta: 'win',
        cmd: 'win',
      } as Record<string, keyof typeof mods>
    )[m.toLowerCase()];
    if (!name || mods[name]) throw bad;
    mods[name] = true;
  }
  if (!mods.ctrl && !mods.alt && !mods.win) throw 'a shortcut needs Ctrl, Alt or Win';
  const out: string[] = [];
  if (mods.win) out.push('Win');
  if (mods.ctrl) out.push('Ctrl');
  if (mods.alt) out.push('Alt');
  if (mods.shift) out.push('Shift');
  out.push(key);
  return out.join('+');
}

export function chordOf(map: ShortcutMap, id: string): string | null {
  if (Object.prototype.hasOwnProperty.call(map, id)) return map[id];
  return ACTIONS.find((a) => a.id === id)?.chord ?? null;
}

function ownerOf(map: ShortcutMap, chord: string, except: string): ActionDef | undefined {
  return ACTIONS.find((a) => a.id !== except && chordOf(map, a.id) === chord);
}

function keyOf(chord: string): string {
  return chord.split('+').pop() ?? '';
}

const ALT_MODS = [
  'Ctrl+Alt',
  'Ctrl+Alt+Shift',
  'Ctrl+Shift',
  'Alt+Shift',
  'Win+Ctrl',
  'Win+Alt',
  'Win+Ctrl+Alt',
  'Win+Shift',
];

export function probe(map: ShortcutMap, actionId: string, text: string): ProbeResult {
  let chord: string;
  try {
    chord = normalizeChord(text);
  } catch (e) {
    return {
      status: 'invalid',
      chord: null,
      conflict_with: null,
      message: String(e),
      suggestions: [],
    };
  }
  const current = chordOf(map, actionId) === chord;
  const free = (c: string) =>
    !ownerOf(map, c, actionId) && (c === chordOf(map, actionId) || !DEMO_TAKEN.includes(c));
  const suggestions = () =>
    ALT_MODS.map((m) => `${m}+${keyOf(chord)}`)
      .filter((c) => c !== chord && free(c))
      .slice(0, 3);
  const other = ownerOf(map, chord, actionId);
  if (other) {
    return {
      status: 'conflict',
      chord,
      conflict_with: other.id,
      message: `${chord} is already ${other.label}`,
      suggestions: suggestions(),
    };
  }
  if (current) {
    return {
      status: 'current',
      chord,
      conflict_with: null,
      message: `${chord} is set`,
      suggestions: [],
    };
  }
  if (DEMO_TAKEN.includes(chord)) {
    return {
      status: 'in_use',
      chord,
      conflict_with: null,
      message: `${chord} is already used by another app`,
      suggestions: suggestions(),
    };
  }
  return {
    status: 'free',
    chord,
    conflict_with: null,
    message: `${chord} is free`,
    suggestions: [],
  };
}

/** The rows of `AppState.shortcut_actions`. */
export function rows(
  settings: Settings,
  supported: boolean,
  failed: string[],
): ShortcutActionState[] {
  return ACTIONS.map((a) => {
    const chord = chordOf(settings.shortcuts, a.id);
    const featureOn =
      settings.enabled &&
      (a.id === 'switcher' ? settings.layout_switcher_shortcut : settings.keyboard_shortcuts);
    const refusal = failed.find((f) => chord !== null && f.startsWith(`${chord} `));
    const status = !supported
      ? 'unsupported'
      : chord === null || !featureOn
        ? 'off'
        : refusal
          ? 'in_use'
          : 'active';
    return {
      id: a.id,
      label: a.label,
      group: a.group,
      chord,
      default_chord: a.chord,
      status,
      message: status === 'in_use' ? (refusal ?? null) : null,
    };
  });
}

/** Store only the changes (the default chord removes the override), like Rust. */
export function setChord(map: ShortcutMap, id: string, chord: string | null): ShortcutMap {
  const def = ACTIONS.find((a) => a.id === id);
  if (!def) throw `there is no shortcut "${id}"`;
  const next = { ...map };
  if (chord === def.chord) delete next[id];
  else next[id] = chord;
  return next;
}

/** Drop unknown ids and unreadable chords (lenient load). */
export function sanitizeMap(map: unknown): ShortcutMap {
  const out: ShortcutMap = {};
  if (!map || typeof map !== 'object' || Array.isArray(map)) return out;
  for (const [id, value] of Object.entries(map as Record<string, unknown>)) {
    if (!ACTIONS.some((a) => a.id === id)) continue;
    if (value === null) out[id] = null;
    else if (typeof value === 'string') {
      try {
        out[id] = normalizeChord(value);
      } catch {
        // skipped, like Rust
      }
    }
  }
  return out;
}
