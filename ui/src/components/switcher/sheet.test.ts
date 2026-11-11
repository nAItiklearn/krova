import { describe, expect, it } from 'vitest';
import type { ShortcutActionState } from '../../lib/bindings';
import { sheetRows } from './sheet';

const row = (id: string, chord: string | null, group: ShortcutActionState['group'] = 'Windows'): ShortcutActionState => ({
  id,
  label: id,
  group,
  chord,
  default_chord: chord,
  status: chord ? 'active' : 'off',
  message: null,
});

const arrows = (prefix: string, mods: string) =>
  ['Left', 'Right', 'Up', 'Down'].map((k, i) => row(`${prefix}${['left', 'right', 'up', 'down'][i]}`, `${mods}+${k}`));

describe('sheetRows', () => {
  it('collapses a family that shares its modifiers into one row', () => {
    const rows = sheetRows([...arrows('move_', 'Ctrl+Alt'), row('pause', 'Ctrl+Alt+T', 'Krova')]);
    expect(rows).toEqual([
      { id: 'move_n', label: 'Move to the next zone', group: 'Windows', keys: ['Ctrl', 'Alt'], tail: '← ↑ → ↓' },
      { id: 'pause', label: 'pause', group: 'Krova', keys: ['Ctrl', 'Alt', 'T'], tail: null },
    ]);
  });

  it('collapses zone 1-9', () => {
    const zones = Array.from({ length: 9 }, (_, i) => row(`zone_${i + 1}`, `Ctrl+Alt+${i + 1}`));
    expect(sheetRows(zones)).toEqual([
      { id: 'zone_n', label: 'Move to zone 1–9', group: 'Windows', keys: ['Ctrl', 'Alt'], tail: '1–9' },
    ]);
  });

  it('keeps members apart when one was rebound or switched off', () => {
    const moves = arrows('move_', 'Ctrl+Alt');
    moves[1] = row('move_right', 'Ctrl+Shift+Right');
    moves[3] = row('move_down', null);
    const rows = sheetRows(moves);
    expect(rows.map((r) => r.id)).toEqual(['move_left', 'move_right', 'move_up', 'move_down']);
    expect(rows[3].keys).toEqual([]);
    expect(rows[1].keys).toEqual(['Ctrl', 'Shift', 'Right']);
  });

  it('keeps a partial family as single rows', () => {
    const rows = sheetRows([row('zone_1', 'Ctrl+Alt+1'), row('zone_2', 'Ctrl+Alt+2')]);
    expect(rows.map((r) => r.id)).toEqual(['zone_1', 'zone_2']);
  });
});
