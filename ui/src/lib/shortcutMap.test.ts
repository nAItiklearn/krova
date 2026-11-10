import { describe, expect, it } from 'vitest';
import type { ShortcutActionState } from './bindings';
import { createMockBackend } from './mock/mockBackend';
import { allDefault, backendChords, membersOf, refusalOf, suggestionFor, uiChord } from './shortcutMap';

async function rows(): Promise<ShortcutActionState[]> {
  const b = createMockBackend({ persist: false });
  return (await b.getState()).shortcut_actions;
}

describe('shortcut sheet <-> backend actions', () => {
  it('every sheet row maps to backend actions that exist', async () => {
    const ids = new Set((await rows()).map((r) => r.id));
    for (const id of ['move', 'swap', 'zone_n', 'layout_n', 'maximize_zone', 'stack_next', 'stack_prev', 'switcher', 'pause', 'focus', 'cheat_sheet']) {
      const members = membersOf(id);
      expect(members.length).toBeGreaterThan(0);
      for (const m of members) expect(ids.has(m)).toBe(true);
    }
    expect(membersOf('calm_desk')).toEqual([]);
  });

  it('families show their modifiers, singles their chord', async () => {
    const r = await rows();
    expect(uiChord('move', r)).toBe('Ctrl+Alt');
    expect(uiChord('swap', r)).toBe('Ctrl+Alt+Shift');
    expect(uiChord('layout_n', r)).toBe('Ctrl+Alt+Shift');
    expect(uiChord('pause', r)).toBe('Ctrl+Alt+T');
    expect(uiChord('calm_desk', r)).toBeUndefined();
    expect(allDefault(r)).toBe(true);
    expect(refusalOf('move', r)).toBeNull();
  });

  it('a family chord is sent for each of its keys', () => {
    expect(backendChords('move', 'Ctrl+Shift')).toEqual([
      ['move_left', 'Ctrl+Shift+Left'],
      ['move_right', 'Ctrl+Shift+Right'],
      ['move_up', 'Ctrl+Shift+Up'],
      ['move_down', 'Ctrl+Shift+Down'],
    ]);
    expect(backendChords('zone_n', null)).toHaveLength(9);
    expect(backendChords('maximize_zone', 'Ctrl+Alt+M')).toEqual([['maximize_in_zone', 'Ctrl+Alt+M']]);
    expect(backendChords('calm_desk', 'Ctrl+Alt+D')).toEqual([]);
    expect(suggestionFor('move', 'Ctrl+Shift+Left')).toBe('Ctrl+Shift');
    expect(suggestionFor('pause', 'Ctrl+Shift+T')).toBe('Ctrl+Shift+T');
  });
});
