import { describe, expect, it } from 'vitest';
import type { HotCornerEvent, NavigateTarget } from '../bindings';
import { createMockBackend } from './mockBackend';
import { defaultHotCorners, hotCornersStatus, sanitizeHotCorners } from './cornersMock';

const mock = () => createMockBackend({ persist: false });

describe('hot corner settings', () => {
  it('defaults match the Rust side', () => {
    expect(defaultHotCorners()).toEqual({
      top_left: null,
      top_right: 'switcher',
      bottom_left: null,
      bottom_right: 'calm_desk',
      dwell_ms: 300,
      ignore_fullscreen: true,
    });
  });

  it('sanitising never fails and clamps the dwell', () => {
    const s = sanitizeHotCorners({
      top_left: 'warp_drive',
      top_right: 7,
      bottom_left: 'panel',
      dwell_ms: -4,
      ignore_fullscreen: 'yes',
    });
    expect(s).toEqual({
      top_left: null,
      top_right: null,
      bottom_left: 'panel',
      bottom_right: 'calm_desk',
      dwell_ms: 150,
      ignore_fullscreen: true,
    });
    expect(sanitizeHotCorners({ dwell_ms: 99999 }).dwell_ms).toBe(1000);
    expect(sanitizeHotCorners({ dwell_ms: 420.4 }).dwell_ms).toBe(420);
    expect(sanitizeHotCorners(null)).toEqual(defaultHotCorners());
    expect(sanitizeHotCorners({ top_right: null }).top_right).toBeNull();
  });

  it('saving goes through the same rules and old saved settings get the defaults', async () => {
    const b = mock();
    const cur = (await b.getState()).config.settings;
    expect(cur.hot_corners).toEqual(defaultHotCorners());
    const s = await b.saveSettings({
      ...cur,
      hot_corners: { ...cur.hot_corners, top_left: 'focus', dwell_ms: 5 },
    });
    expect(s.config.settings.hot_corners.top_left).toBe('focus');
    expect(s.config.settings.hot_corners.dwell_ms).toBe(150);
    const { hot_corners: _drop, ...old } = cur;
    const s2 = await b.saveSettings(old as typeof cur);
    expect(s2.config.settings.hot_corners).toEqual(defaultHotCorners());
  });
});

describe('hot corner events in the demo', () => {
  it('a set corner sends hot-corner and the switcher opens on that monitor', async () => {
    const b = mock();
    const events: HotCornerEvent[] = [];
    const views: NavigateTarget[] = [];
    const off = await b.onHotCorner((e) => events.push(e));
    await b.onNavigate((v) => views.push(v));
    const ev = b.demo.hotCorner('top_right', 'monitor-2');
    expect(ev).toEqual({ corner: 'top_right', action: 'switcher', monitor_id: 'monitor-2' });
    expect(events).toEqual([ev]);
    expect(views).toEqual(['switcher']);
    expect(await b.switcherMonitor()).toBe('monitor-2');

    // calm_desk only sends the event.
    expect(b.demo.hotCorner('bottom_right')?.action).toBe('calm_desk');
    expect(views).toEqual(['switcher']);
    // An unset corner does nothing.
    expect(b.demo.hotCorner('top_left')).toBeNull();
    expect(events).toHaveLength(2);
    off();
    b.demo.hotCorner('top_right');
    expect(events).toHaveLength(2);
  });

  it('panel opens the settings view', async () => {
    const b = mock();
    const cur = (await b.getState()).config.settings;
    await b.saveSettings({ ...cur, hot_corners: { ...cur.hot_corners, top_left: 'panel' } });
    const views: NavigateTarget[] = [];
    await b.onNavigate((v) => views.push(v));
    b.demo.hotCorner('top_left');
    expect(views).toEqual(['settings']);
  });

  it('nothing fires while Krova is off or the platform cannot watch corners', async () => {
    const b = mock();
    expect(await b.hotCornersStatus()).toEqual({ supported: true, active: true });
    b.demo.toggleEnabled();
    expect(b.demo.hotCorner('top_right')).toBeNull();
    expect(await b.hotCornersStatus()).toEqual({ supported: true, active: false });
    b.demo.toggleEnabled();
    b.demo.setCapabilities('none');
    expect(b.demo.hotCorner('top_right')).toBeNull();
    expect(await b.hotCornersStatus()).toEqual({ supported: false, active: false });
  });

  it('status is inactive with no corner set', () => {
    const settings = {
      enabled: true,
      hot_corners: { ...defaultHotCorners(), top_right: null, bottom_right: null },
    } as Parameters<typeof hotCornersStatus>[0];
    expect(hotCornersStatus(settings, true)).toEqual({ supported: true, active: false });
  });
});
