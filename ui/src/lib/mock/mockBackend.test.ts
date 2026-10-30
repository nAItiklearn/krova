import { describe, expect, it } from 'vitest';
import { STORAGE_KEY, createMockBackend } from './mockBackend';

const fresh = () => createMockBackend({ persist: false });

describe('mock backend: state', () => {
  it('fakes the two demo monitors', async () => {
    const s = await fresh().getState();
    expect(s.monitors.map((m) => m.label)).toEqual(['Display 1 (primary)', 'Display 2']);
    expect(s.monitors[0].rect).toEqual({ x: 0, y: 0, w: 2560, h: 1440 });
    expect(s.monitors[0].work).toEqual({ x: 0, y: 0, w: 2560, h: 1400 });
    expect(s.monitors[0].primary).toBe(true);
    expect(s.monitors[1].rect).toEqual({ x: 2560, y: 0, w: 1920, h: 1080 });
    for (const p of s.config.monitors) {
      expect(p.layouts.map((l) => l.id)).toEqual(['halves', 'thirds', 'grid', 'priority']);
      expect(p.layouts.some((l) => l.id === p.active_layout)).toBe(true);
    }
    expect(s.permission).toBe('not_required');
  });

  it('returns copies, not live references', async () => {
    const b = fresh();
    const s = await b.getState();
    s.config.monitors[0].layouts.length = 0;
    expect((await b.getState()).config.monitors[0].layouts).toHaveLength(4);
  });

  it('honours the permission option', async () => {
    const s = await createMockBackend({ persist: false, permission: 'denied' }).getState();
    expect(s.permission).toBe('denied');
  });
});

describe('mock backend: settings', () => {
  it('saves, clamps numbers and applies launch_at_login to autostart', async () => {
    const b = fresh();
    const cur = (await b.getState()).config.settings;
    const s = await b.saveSettings({
      ...cur,
      launch_at_login: true,
      edge_size_px: 500,
      highlight_opacity: 3,
      highlight_color: '#AABBCC',
      theme: 'dark',
    });
    expect(s.autostart).toBe(true);
    expect(s.config.settings.edge_size_px).toBe(64);
    expect(s.config.settings.highlight_opacity).toBe(1);
    expect(s.config.settings.highlight_color).toBe('#aabbcc');
    expect(s.config.settings.theme).toBe('dark');
  });

  it('rejects bad enum values and colors with a string', async () => {
    const b = fresh();
    const cur = (await b.getState()).config.settings;
    await expect(b.saveSettings({ ...cur, highlight_color: 'blue' })).rejects.toBe(
      'invalid highlight_color "blue"',
    );
    await expect(
      b.saveSettings({ ...cur, modifier: 'meta' as unknown as 'shift' }),
    ).rejects.toBe('invalid modifier "meta"');
  });
});

describe('mock backend: layouts', () => {
  it('upserts by id and keeps others', async () => {
    const b = fresh();
    const [halves] = await b.listTemplates();
    const renamed = { ...halves, name: 'My halves' };
    let s = await b.saveLayout('monitor-1', renamed);
    const layouts = s.config.monitors[0].layouts;
    expect(layouts).toHaveLength(4);
    expect(layouts.find((l) => l.id === 'halves')?.name).toBe('My halves');

    s = await b.saveLayout('monitor-1', { ...halves, id: 'custom', name: 'Custom' });
    expect(s.config.monitors[0].layouts).toHaveLength(5);
    // The other monitor is untouched.
    expect(s.config.monitors[1].layouts).toHaveLength(4);
  });

  it('rejects invalid layouts and unknown monitors', async () => {
    const b = fresh();
    await expect(b.saveLayout('monitor-1', { id: 'x', name: 'x', zones: [] })).rejects.toBe(
      'layout has no zones',
    );
    await expect(b.saveLayout('nope', { id: 'x', name: 'x', zones: [] })).rejects.toBe(
      'monitor nope not found',
    );
  });

  it('deletes layouts, repairs the active id, keeps at least one', async () => {
    const b = fresh();
    let s = await b.deleteLayout('monitor-1', 'halves'); // halves was active
    expect(s.config.monitors[0].layouts.map((l) => l.id)).toEqual(['thirds', 'grid', 'priority']);
    expect(s.config.monitors[0].active_layout).toBe('thirds');
    await expect(b.deleteLayout('monitor-1', 'halves')).rejects.toBe('layout halves not found');
    s = await b.deleteLayout('monitor-1', 'thirds');
    s = await b.deleteLayout('monitor-1', 'grid');
    await expect(b.deleteLayout('monitor-1', 'priority')).rejects.toBe(
      'a monitor needs at least one layout',
    );
    expect(s.config.monitors[0].layouts).toHaveLength(1);
  });

  it('sets the active layout', async () => {
    const b = fresh();
    const s = await b.setActiveLayout('monitor-2', 'priority');
    expect(s.config.monitors[1].active_layout).toBe('priority');
    await expect(b.setActiveLayout('monitor-2', 'zzz')).rejects.toBe('layout zzz not found');
  });
});

describe('mock backend: edit commands', () => {
  it('split / remove / divider return new layouts and reject with strings', async () => {
    const b = fresh();
    const [halves] = await b.listTemplates();
    const split = await b.editSplitZone(halves, 2, 'horizontal', 0.5);
    expect(split.zones).toHaveLength(3);
    expect(halves.zones).toHaveLength(2);

    const moved = await b.editMoveDivider(halves, 'vertical', 0.5, 0.4);
    expect(moved.zones[0].rect.w).toBeCloseTo(0.4);

    const removed = await b.editRemoveZone(split, 3);
    expect(removed.zones).toHaveLength(2);

    await expect(b.editSplitZone(halves, 1, 'vertical', 0.01)).rejects.toBe(
      'the change would leave a zone smaller than the minimum size',
    );
    await expect(b.editRemoveZone({ ...halves, zones: [halves.zones[0]] }, 1)).rejects.toBe(
      'a layout needs at least one zone',
    );
    await expect(b.editMoveDivider(halves, 'vertical', 0, 0.2)).rejects.toBe(
      'there is no divider at that position',
    );
  });
});

describe('mock backend: persistence and events', () => {
  const memory = () => {
    const m = new Map<string, string>();
    return {
      m,
      storage: {
        getItem: (k: string) => m.get(k) ?? null,
        setItem: (k: string, v: string) => void m.set(k, v),
        removeItem: (k: string) => void m.delete(k),
      },
    };
  };

  it('persists under krova-demo and restores, and reset clears it', async () => {
    const { m, storage } = memory();
    const a = createMockBackend({ storage });
    await a.setActiveLayout('monitor-1', 'grid');
    expect(m.has(STORAGE_KEY)).toBe(true);

    const b = createMockBackend({ storage });
    expect((await b.getState()).config.monitors[0].active_layout).toBe('grid');

    const c = createMockBackend({ storage, reset: true });
    expect((await c.getState()).config.monitors[0].active_layout).toBe('halves');
    expect(m.has(STORAGE_KEY)).toBe(false);
  });

  it('falls back to defaults on a corrupt persisted state', async () => {
    const { m, storage } = memory();
    m.set(STORAGE_KEY, '{ not json');
    const s = await createMockBackend({ storage }).getState();
    expect(s.config.monitors).toHaveLength(2);
  });

  it('delivers navigate and state-changed events until unlistened', async () => {
    const b = fresh();
    const views: string[] = [];
    const perms: string[] = [];
    const offNav = await b.onNavigate((v) => views.push(v));
    const offState = await b.onStateChanged((s) => perms.push(s.permission));
    b.demo.navigate('settings');
    b.demo.setPermission('denied');
    offNav();
    offState();
    b.demo.navigate('editor');
    b.demo.setPermission('granted');
    expect(views).toEqual(['settings']);
    expect(perms).toEqual(['denied']);
  });
});
