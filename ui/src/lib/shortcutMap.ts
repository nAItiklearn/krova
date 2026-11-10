// The shortcut sheet groups some backend actions into families (the four "Move"
// arrows, "Zone 1–9", "Layout 1–9") and binds only their modifiers. This module
// maps between those sheet rows and the backend's per-action shortcuts
// (`AppState.shortcut_actions`, `set_shortcut`, `probe_shortcut`).

import type { ShortcutActionState } from './bindings';

interface Family {
  ids: string[];
  keys: string[];
}

const DIRS = ['left', 'right', 'up', 'down'];
const ARROWS = ['Left', 'Right', 'Up', 'Down'];
const DIGITS = Array.from({ length: 9 }, (_, i) => String(i + 1));

/** Sheet rows that stand for several backend actions. */
export const FAMILIES: Record<string, Family> = {
  move: { ids: DIRS.map((d) => `move_${d}`), keys: ARROWS },
  swap: { ids: DIRS.map((d) => `swap_${d}`), keys: ARROWS },
  zone_n: { ids: DIGITS.map((n) => `zone_${n}`), keys: DIGITS },
  layout_n: { ids: DIGITS.map((n) => `layout_${n}`), keys: DIGITS },
};

/** Sheet rows that are one backend action (sheet id -> action id). */
export const SINGLES: Record<string, string> = {
  maximize_zone: 'maximize_in_zone',
  stack_next: 'stack_next',
  stack_prev: 'stack_prev',
  switcher: 'switcher',
  pause: 'pause',
  focus: 'focus',
  cheat_sheet: 'cheat_sheet',
};

/** The backend action ids behind a sheet row, or [] when the backend has none. */
export function membersOf(uiId: string): string[] {
  if (FAMILIES[uiId]) return FAMILIES[uiId].ids;
  if (SINGLES[uiId]) return [SINGLES[uiId]];
  return [];
}

/** `Ctrl+Alt+Left` minus its key: `Ctrl+Alt`. */
function modifiersOf(chord: string): string {
  const i = chord.lastIndexOf('+');
  return i > 0 ? chord.slice(0, i) : chord;
}

/**
 * The chord a sheet row shows: for a family only the modifiers (taken from its
 * first action). `undefined` when no backend action stands behind the row.
 */
export function uiChord(uiId: string, rows: readonly ShortcutActionState[]): string | null | undefined {
  const ids = membersOf(uiId);
  if (ids.length === 0) return undefined;
  const row = rows.find((r) => r.id === ids[0]);
  if (!row) return undefined;
  if (row.chord === null) return null;
  return FAMILIES[uiId] ? modifiersOf(row.chord) : row.chord;
}

/** What to send to `set_shortcut` for a sheet row: `[action id, chord | null]` pairs. */
export function backendChords(uiId: string, chord: string | null): [string, string | null][] {
  const fam = FAMILIES[uiId];
  if (fam) return fam.ids.map((id, i) => [id, chord === null ? null : `${chord}+${fam.keys[i]}`]);
  const single = SINGLES[uiId];
  return single ? [[single, chord]] : [];
}

/** A suggestion for a family member (`Ctrl+Shift+Left`) as the family's modifiers. */
export function suggestionFor(uiId: string, suggestion: string): string {
  return FAMILIES[uiId] ? modifiersOf(suggestion) : suggestion;
}

/** The first refusal (`in_use` / `error`) among a row's actions, for the warning. */
export function refusalOf(
  uiId: string,
  rows: readonly ShortcutActionState[],
): ShortcutActionState | null {
  const ids = new Set(membersOf(uiId));
  return rows.find((r) => ids.has(r.id) && (r.status === 'in_use' || r.status === 'error')) ?? null;
}

/** True when every action has its default chord (nothing to reset). */
export function allDefault(rows: readonly ShortcutActionState[]): boolean {
  return rows.every((r) => r.chord === r.default_chord);
}
