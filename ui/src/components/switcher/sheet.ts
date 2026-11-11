// Rows of the cheat sheet (Ctrl+Alt+/ in the switcher window), built from
// `AppState.shortcut_actions`. Families of actions that differ only in the
// last key (move_left..move_down, zone_1..zone_9, ...) collapse into one row
// when they share the same modifiers: "Ctrl+Alt" + "← ↑ → ↓".
import type { ShortcutActionState, ShortcutGroup } from '../../lib/bindings';

export interface SheetRow {
  id: string;
  label: string;
  group: ShortcutGroup;
  /** Keycaps to draw; empty when the action is off. */
  keys: string[];
  /** Shown after the keycaps for a collapsed family ("← ↑ → ↓", "1–9"). */
  tail: string | null;
}

const FAMILIES: { prefix: string; members: string[]; label: string; tail: string }[] = [
  { prefix: 'move_', members: ['left', 'right', 'up', 'down'], label: 'Move to the next zone', tail: '← ↑ → ↓' },
  { prefix: 'swap_', members: ['left', 'right', 'up', 'down'], label: 'Swap with the neighbour', tail: '← ↑ → ↓' },
  { prefix: 'zone_', members: ['1', '2', '3', '4', '5', '6', '7', '8', '9'], label: 'Move to zone 1–9', tail: '1–9' },
  { prefix: 'layout_', members: ['1', '2', '3', '4', '5', '6', '7', '8', '9'], label: 'Apply layout 1–9', tail: '1–9' },
];

export const GROUP_ORDER: ShortcutGroup[] = ['Windows', 'Layouts', 'Krova'];

const split = (chord: string | null): string[] => (chord ? chord.split('+').filter(Boolean) : []);

/** The modifiers every member shares (all keys but the last), or null if they differ or one is off. */
function sharedMods(members: ShortcutActionState[]): string[] | null {
  let mods: string | null = null;
  for (const m of members) {
    const keys = split(m.chord);
    if (keys.length < 2) return null;
    const head = keys.slice(0, -1).join('+');
    if (mods === null) mods = head;
    else if (mods !== head) return null;
  }
  return mods === null ? null : mods.split('+');
}

export function sheetRows(actions: readonly ShortcutActionState[]): SheetRow[] {
  const byId = new Map(actions.map((a) => [a.id, a]));
  const used = new Set<string>();
  const out: SheetRow[] = [];
  for (const a of actions) {
    if (used.has(a.id)) continue;
    const fam = FAMILIES.find((f) => a.id.startsWith(f.prefix) && f.members.includes(a.id.slice(f.prefix.length)));
    if (fam) {
      const members = fam.members.map((m) => byId.get(fam.prefix + m)).filter((m): m is ShortcutActionState => !!m);
      const mods = members.length === fam.members.length ? sharedMods(members) : null;
      if (mods) {
        for (const m of members) used.add(m.id);
        out.push({ id: fam.prefix + 'n', label: fam.label, group: a.group, keys: mods, tail: fam.tail });
        continue;
      }
    }
    used.add(a.id);
    out.push({ id: a.id, label: a.label, group: a.group, keys: split(a.chord), tail: null });
  }
  return out;
}
