import { describe, expect, it } from 'vitest';
import type { MonitorSnapshot } from '../bindings';
import { createMockBackend, defaultSettings } from './mockBackend';
import { demoAnchor, sanitizeNotch } from './notch';

const MONITOR: MonitorSnapshot = {
  profile_id: 'monitor-1',
  label: 'Display 1',
  rect: { x: 0, y: 0, w: 1920, h: 1080 },
  work: { x: 0, y: 0, w: 1920, h: 1032 },
  scale: 1,
  primary: true,
};

describe('notch settings', () => {
  it('defaults to an enabled top-centre notch on the primary monitor', () => {
    expect(defaultSettings().notch).toEqual({
      enabled: true,
      corner: 'top_center',
      monitor: 'primary',
      hide_in_fullscreen: true,
    });
  });

  it('falls back field by field, like Rust', () => {
    expect(sanitizeNotch({ enabled: false, corner: 'middle', monitor: 'cursor' })).toEqual({
      enabled: false,
      corner: 'top_center',
      monitor: 'cursor',
      hide_in_fullscreen: true,
    });
    expect(sanitizeNotch(undefined)).toEqual(defaultSettings().notch);
    expect(sanitizeNotch('x')).toEqual(defaultSettings().notch);
  });

  it('saving settings keeps a valid notch and repairs a broken one', async () => {
    const api = createMockBackend({ persist: false });
    const s = defaultSettings();
    let state = await api.saveSettings({ ...s, notch: { ...s.notch, corner: 'bottom_right' } });
    expect(state.config.settings.notch.corner).toBe('bottom_right');
    state = await api.saveSettings({ ...s, notch: { corner: 7 } as never });
    expect(state.config.settings.notch).toEqual(s.notch);
  });
});

describe('panel anchor', () => {
  it('matches the Rust positions (physical px)', () => {
    expect(demoAnchor(MONITOR, 'top_center')).toEqual({
      x: 850,
      y: 4,
      w: 220,
      h: 28,
      corner: 'top_center',
      monitor_id: 'monitor-1',
    });
    expect(demoAnchor(MONITOR, 'bottom_right')).toMatchObject({ x: 1696, y: 1000 });
    expect(demoAnchor({ ...MONITOR, scale: 1.5 }, 'top_left')).toMatchObject({
      x: 6,
      y: 6,
      w: 330,
      h: 42,
    });
  });

  it('demo mode sends a fake anchor on subscribe and on clickNotch', async () => {
    const api = createMockBackend({ persist: false });
    const got: string[] = [];
    const views: string[] = [];
    api.demo.onNotchNavigate((v) => views.push(v));
    const stop = await api.onPanelAnchor((a) => got.push(a.corner));
    await Promise.resolve();
    expect(got).toEqual(['top_center']);
    expect(await api.panelAnchor()).toMatchObject({ monitor_id: 'monitor-1' });
    api.demo.clickNotch('bottom_left');
    expect(got).toEqual(['top_center', 'bottom_left']);
    expect(views).toEqual(['panel', 'panel']);
    stop();
    api.demo.clickNotch();
    expect(got).toHaveLength(2);
    await expect(api.hidePanel()).resolves.toBeUndefined();
  });
});
