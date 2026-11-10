import { describe, expect, it } from 'vitest';
import { createMockBackend } from './mockBackend';
import { ACTIONS, normalizeChord } from './shortcuts';

const backend = () => createMockBackend({ persist: false });

describe('mock shortcuts (wave 2)', () => {
  it('lists every action with the Ctrl+Alt defaults, active', async () => {
    const s = await backend().getState();
    expect(s.shortcut_actions).toHaveLength(33);
    expect(ACTIONS).toHaveLength(33);
    const row = (id: string) => s.shortcut_actions.find((r) => r.id === id)!;
    expect(row('move_left')).toMatchObject({
      label: 'Move left',
      group: 'Windows',
      chord: 'Ctrl+Alt+Left',
      default_chord: 'Ctrl+Alt+Left',
      status: 'active',
    });
    expect(row('swap_up').chord).toBe('Ctrl+Alt+Shift+Up');
    expect(row('layout_3').chord).toBe('Ctrl+Alt+Shift+3');
    expect(row('pause').chord).toBe('Ctrl+Alt+T');
    expect(row('switcher').group).toBe('Layouts');
    expect(s.paused).toBe(false);
  });

  it('chords are normalised like Rust and nonsense is refused', () => {
    expect(normalizeChord(' shift+alt+ctrl+t ')).toBe('Ctrl+Alt+Shift+T');
    expect(normalizeChord('Cmd+Option+Space')).toBe('Win+Alt+Space');
    let err = '';
    try {
      normalizeChord('Shift+A');
    } catch (e) {
      err = String(e);
    }
    expect(err).toMatch('Ctrl, Alt or Win');
  });

  it('probe reports conflicts, other programs and suggestions', async () => {
    const b = backend();
    const conflict = await b.probeShortcut('pause', 'ctrl+alt+left');
    expect(conflict.status).toBe('conflict');
    expect(conflict.conflict_with).toBe('move_left');
    expect(conflict.suggestions.length).toBeGreaterThan(0);
    expect(conflict.suggestions.every((c) => c.endsWith('Left'))).toBe(true);
    expect((await b.probeShortcut('focus', 'Ctrl+Shift+F')).status).toBe('in_use');
    expect((await b.probeShortcut('pause', 'Ctrl+Alt+T')).status).toBe('current');
    expect((await b.probeShortcut('pause', 'Ctrl+Shift+P')).status).toBe('free');
    expect((await b.probeShortcut('pause', 'P')).status).toBe('invalid');
  });

  it('rebinding stores only changes, off is null, reset clears them', async () => {
    const b = backend();
    let s = await b.setShortcut('pause', 'Ctrl+Shift+P');
    expect(s.config.settings.shortcuts).toEqual({ pause: 'Ctrl+Shift+P' });
    s = await b.setShortcut('focus', null);
    expect(s.shortcut_actions.find((r) => r.id === 'focus')?.status).toBe('off');
    s = await b.setShortcut('pause', 'Ctrl+Alt+T');
    expect(s.config.settings.shortcuts).toEqual({ focus: null });
    await expect(b.setShortcut('pause', 'Ctrl+Alt+Left')).rejects.toBe(
      'Ctrl+Alt+Left is already Move left',
    );
    s = await b.resetShortcuts();
    expect(s.config.settings.shortcuts).toEqual({});
  });

  it('pause and the popups behave like the backend', async () => {
    const b = backend();
    let pushed = false;
    await b.onStateChanged((s) => (pushed = s.paused));
    await b.setPaused(true);
    expect(pushed).toBe(true);
    expect((await b.getState()).paused).toBe(true);
    await expect(b.hidePopup('panel')).resolves.toBeUndefined();
    let anchor = null as unknown;
    await b.onPanelAnchor((a) => (anchor = a));
    const a = { x: 1, y: 2, w: 3, h: 4, corner: 'top_center' as const, monitor_id: 'monitor-1' };
    b.demo.anchorPanel(a);
    expect(anchor).toEqual(a);
  });
});
