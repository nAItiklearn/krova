import { afterEach, describe, expect, it, vi } from 'vitest';
import type { Layout } from '../bindings';
import {
  DEMO_FAILED_SHORTCUTS,
  DEMO_FAILED_SWITCHER_SHORTCUT,
  STORAGE_KEY,
  createMockBackend,
  defaultSettings,
} from './mockBackend';

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
    await expect(b.saveSettings({ ...cur, modifier: 'meta' as unknown as 'shift' })).rejects.toBe(
      'invalid modifier "meta"',
    );
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

// ---------------------------------------------------------------------------
// Keyboard shortcuts, window memory and zone gap settings
// ---------------------------------------------------------------------------

describe('mock backend: new settings', () => {
  it('defaults match the contract', async () => {
    const s = (await fresh().getState()).config.settings;
    expect(s.keyboard_shortcuts).toBe(true);
    expect(s.shortcut_modifier).toBe('ctrl_alt_shift');
    expect(s.restore_on_reconnect).toBe(true);
    expect(s.linked_dividers).toBe(true);
    expect(s.zone_gap_px).toBe(0);
    expect(s.layout_switcher_shortcut).toBe(true);
    expect(defaultSettings()).toEqual(s);
  });

  it('saves every new field', async () => {
    const b = fresh();
    const cur = (await b.getState()).config.settings;
    const s = await b.saveSettings({
      ...cur,
      keyboard_shortcuts: false,
      shortcut_modifier: 'win_ctrl',
      restore_on_reconnect: false,
      linked_dividers: false,
      zone_gap_px: 12,
    });
    expect(s.config.settings).toMatchObject({
      keyboard_shortcuts: false,
      shortcut_modifier: 'win_ctrl',
      restore_on_reconnect: false,
      linked_dividers: false,
      zone_gap_px: 12,
    });
  });

  it.each(['ctrl_alt', 'win_alt', 'ctrl_alt_shift', 'win_ctrl'] as const)(
    'accepts the shortcut modifier %s',
    async (m) => {
      const b = fresh();
      const cur = (await b.getState()).config.settings;
      expect(
        (await b.saveSettings({ ...cur, shortcut_modifier: m })).config.settings.shortcut_modifier,
      ).toBe(m);
    },
  );

  it('rejects an unknown shortcut modifier with a string, leaving the settings alone', async () => {
    const b = fresh();
    const cur = (await b.getState()).config.settings;
    for (const bad of ['alt', 'win_shift', '', 'CTRL_ALT', 'super']) {
      await expect(
        b.saveSettings({ ...cur, shortcut_modifier: bad as unknown as 'ctrl_alt_shift' }),
      ).rejects.toBe(`invalid shortcut_modifier "${bad}"`);
    }
    expect((await b.getState()).config.settings).toEqual(cur);
  });

  it('clamps zone_gap_px to 0..32 and rounds it to an integer', async () => {
    const b = fresh();
    const cur = (await b.getState()).config.settings;
    const gap = async (v: number) =>
      (await b.saveSettings({ ...cur, zone_gap_px: v })).config.settings.zone_gap_px;
    expect(await gap(-5)).toBe(0);
    expect(await gap(0)).toBe(0);
    expect(await gap(7.6)).toBe(8);
    expect(await gap(32)).toBe(32);
    expect(await gap(33)).toBe(32);
    expect(await gap(1e9)).toBe(32);
    expect(await gap(Number.POSITIVE_INFINITY)).toBe(0); // not finite: back to the default
    expect(await gap(Number.NaN)).toBe(0);
  });

  it('keeps booleans boolean and fills in missing fields with their defaults', async () => {
    const b = fresh();
    const cur = (await b.getState()).config.settings;
    const old = { ...cur } as Partial<typeof cur>;
    delete old.keyboard_shortcuts;
    delete old.restore_on_reconnect;
    delete old.linked_dividers;
    delete old.zone_gap_px;
    delete old.shortcut_modifier;
    delete old.layout_switcher_shortcut;
    const s = (await b.saveSettings(old as typeof cur)).config.settings;
    expect(s.layout_switcher_shortcut).toBe(true);
    expect(s.keyboard_shortcuts).toBe(true);
    expect(s.restore_on_reconnect).toBe(true);
    expect(s.linked_dividers).toBe(true);
    expect(s.zone_gap_px).toBe(0);
    expect(s.shortcut_modifier).toBe('ctrl_alt_shift');

    const odd = await b.saveSettings({
      ...cur,
      keyboard_shortcuts: 'no' as unknown as boolean,
      linked_dividers: 0 as unknown as boolean,
    });
    expect(odd.config.settings.keyboard_shortcuts).toBe(true); // not a boolean: default
    expect(odd.config.settings.linked_dividers).toBe(true);
  });

  it('persists the new settings and fills defaults into an older persisted state', async () => {
    const m = new Map<string, string>();
    const storage = {
      getItem: (k: string) => m.get(k) ?? null,
      setItem: (k: string, v: string) => void m.set(k, v),
      removeItem: (k: string) => void m.delete(k),
    };
    const a = createMockBackend({ storage });
    const cur = (await a.getState()).config.settings;
    await a.saveSettings({
      ...cur,
      shortcut_modifier: 'win_ctrl',
      zone_gap_px: 20,
      linked_dividers: false,
    });
    const b = createMockBackend({ storage });
    expect((await b.getState()).config.settings).toMatchObject({
      shortcut_modifier: 'win_ctrl',
      zone_gap_px: 20,
      linked_dividers: false,
    });

    // A persisted state from before these settings existed.
    const persisted = JSON.parse(m.get(STORAGE_KEY) ?? '{}') as {
      config: { settings: Record<string, unknown> };
    };
    for (const k of [
      'keyboard_shortcuts',
      'shortcut_modifier',
      'restore_on_reconnect',
      'linked_dividers',
      'zone_gap_px',
      'layout_switcher_shortcut',
    ]) {
      delete persisted.config.settings[k];
    }
    m.set(STORAGE_KEY, JSON.stringify(persisted));
    const c = createMockBackend({ storage });
    expect((await c.getState()).config.settings).toEqual(defaultSettings());
  });
});

describe('mock backend: shortcuts and capabilities', () => {
  afterEach(() => vi.unstubAllGlobals());

  it('defaults to active shortcuts and full capabilities', async () => {
    const s = await fresh().getState();
    expect(s.shortcuts).toEqual({ active: true, failed: [] });
    expect(s.capabilities).toEqual({ shortcuts: true, window_memory: true, auto_arrange: true });
  });

  it('shortcuts=failed reports the demo conflict and stays active', async () => {
    const s = await createMockBackend({ persist: false, shortcuts: 'failed' }).getState();
    expect(s.shortcuts).toEqual({
      active: true,
      failed: ['Ctrl+Alt+Shift+3 is already used by another app'],
    });
    expect(DEMO_FAILED_SHORTCUTS).toEqual(['Ctrl+Alt+Shift+3 is already used by another app']);
  });

  it('shortcuts=off reports inactive shortcuts', async () => {
    const s = await createMockBackend({ persist: false, shortcuts: 'off' }).getState();
    expect(s.shortcuts).toEqual({ active: false, failed: [] });
  });

  it('capabilities=none turns both capabilities off, and the shortcuts with them', async () => {
    const s = await createMockBackend({
      persist: false,
      capabilities: 'none',
      shortcuts: 'failed',
    }).getState();
    expect(s.capabilities).toEqual({ shortcuts: false, window_memory: false, auto_arrange: false });
    expect(s.shortcuts).toEqual({ active: false, failed: [] });
  });

  it('switching the setting off deactivates the shortcuts and clears the conflicts', async () => {
    const b = createMockBackend({ persist: false, shortcuts: 'failed' });
    const cur = (await b.getState()).config.settings;
    let s = await b.saveSettings({ ...cur, keyboard_shortcuts: false });
    expect(s.shortcuts).toEqual({ active: false, failed: [] });
    s = await b.saveSettings({ ...cur, keyboard_shortcuts: true });
    expect(s.shortcuts.active).toBe(true);
    expect(s.shortcuts.failed).toHaveLength(1);
  });

  it('reads ?shortcuts= and ?capabilities= from the URL', async () => {
    vi.stubGlobal('location', { search: '?nopersist&shortcuts=failed&capabilities=none' });
    const s = await createMockBackend().getState();
    expect(s.capabilities).toEqual({ shortcuts: false, window_memory: false, auto_arrange: false });
    vi.stubGlobal('location', { search: '?shortcuts=failed' });
    expect((await createMockBackend({ persist: false }).getState()).shortcuts.failed).toHaveLength(
      1,
    );
    vi.stubGlobal('location', { search: '?shortcuts=off' });
    expect((await createMockBackend({ persist: false }).getState()).shortcuts.active).toBe(false);
    vi.stubGlobal('location', { search: '?shortcuts=bogus&capabilities=bogus' });
    const d = await createMockBackend({ persist: false }).getState();
    expect(d.shortcuts).toEqual({ active: true, failed: [] });
    expect(d.capabilities).toEqual({ shortcuts: true, window_memory: true, auto_arrange: true });
  });

  it('the demo hooks change them live, emit state-changed and reset restores the start', async () => {
    const b = createMockBackend({ persist: false });
    const seen: string[] = [];
    await b.onStateChanged((s) =>
      seen.push(`${s.shortcuts.active}/${s.shortcuts.failed.length}/${s.capabilities.shortcuts}`),
    );
    b.demo.setShortcuts('failed');
    b.demo.setCapabilities('none');
    b.demo.setCapabilities('all');
    b.demo.setShortcuts('off');
    expect(seen).toEqual(['true/1/true', 'false/0/false', 'true/1/true', 'false/0/true']);
    b.demo.reset();
    expect((await b.getState()).shortcuts).toEqual({ active: true, failed: [] });
  });
});

// ---------------------------------------------------------------------------
// Layout export and import
// ---------------------------------------------------------------------------

const customLayout = (id: string, name = id): Layout => ({
  id,
  name,
  zones: [
    { id: 1, rect: { x: 0, y: 0, w: 0.25, h: 1 } },
    { id: 2, rect: { x: 0.25, y: 0, w: 0.75, h: 1 } },
  ],
});

describe('mock backend: layout save limits', () => {
  it('saveLayout enforces the shared rules: 64 zones, 64 characters, 0.01 sides', async () => {
    const b = fresh();
    const many = Array.from({ length: 65 }, (_, i) => ({
      id: i + 1,
      rect: { x: 0, y: 0, w: 1, h: 1 },
    }));
    await expect(b.saveLayout('monitor-1', { id: 'x', name: 'x', zones: many })).rejects.toBe(
      'layout has too many zones (at most 64)',
    );
    await expect(b.saveLayout('monitor-1', customLayout('x'.repeat(65)))).rejects.toBe(
      'layout id or name is too long (at most 64 characters)',
    );
    await expect(b.saveLayout('monitor-1', customLayout('ok', 'n'.repeat(65)))).rejects.toBe(
      'layout id or name is too long (at most 64 characters)',
    );
    const thin = customLayout('thin');
    thin.zones[0].rect.w = 0.005;
    await expect(b.saveLayout('monitor-1', thin)).rejects.toBe('zone 1 has an invalid rectangle');
  });

  it('a monitor holds at most 64 layouts, but existing ones can still be replaced', async () => {
    const b = fresh();
    for (let i = 4; i < 64; i++) await b.saveLayout('monitor-1', customLayout(`l${i}`));
    expect((await b.getState()).config.monitors[0].layouts).toHaveLength(64);
    await expect(b.saveLayout('monitor-1', customLayout('one-too-many'))).rejects.toBe(
      'a monitor can hold at most 64 layouts',
    );
    const s = await b.saveLayout('monitor-1', customLayout('l10', 'Renamed'));
    expect(s.config.monitors[0].layouts).toHaveLength(64);
    expect(s.config.monitors[0].layouts.find((l) => l.id === 'l10')?.name).toBe('Renamed');
    // The other monitor has its own allowance.
    expect(
      (await b.saveLayout('monitor-2', customLayout('free'))).config.monitors[1].layouts,
    ).toHaveLength(5);
  });
});

describe('mock backend: export_layout', () => {
  it('returns the envelope text for a valid layout', async () => {
    const b = fresh();
    const [halves] = await b.listTemplates();
    const text = await b.exportLayout(halves);
    const parsed = JSON.parse(text) as { krova_layout: number; layout: Layout };
    expect(parsed.krova_layout).toBe(1);
    expect(parsed.layout).toEqual(halves);
    expect(text).toContain('\n  "layout": {');
  });

  it('rejects an invalid layout with a string', async () => {
    const b = fresh();
    await expect(b.exportLayout({ id: 'e', name: 'e', zones: [] })).rejects.toBe(
      'layout has no zones',
    );
    const bad = customLayout('bad');
    bad.zones[1].id = 1;
    await expect(b.exportLayout(bad)).rejects.toBe('duplicate zone id 1');
  });
});

describe('mock backend: import_layout', () => {
  it('imports an exported layout onto the other monitor and keeps it unchanged', async () => {
    const b = fresh();
    await b.saveLayout('monitor-1', customLayout('side-bar', 'Side bar'));
    const text = await b.exportLayout(customLayout('side-bar', 'Side bar'));

    const s = await b.importLayout('monitor-2', text);
    const layouts = s.config.monitors[1].layouts;
    expect(layouts.map((l) => l.id)).toEqual(['halves', 'thirds', 'grid', 'priority', 'side-bar']);
    expect(layouts[4]).toEqual(customLayout('side-bar', 'Side bar'));
    // Importing never changes which layout is active, and the other monitor is untouched.
    expect(s.config.monitors[1].active_layout).toBe('thirds');
    expect(s.config.monitors[0].layouts).toHaveLength(5);
  });

  it('round-trips every template through export and import (ids get suffixes)', async () => {
    const b = fresh();
    const originals = await b.listTemplates();
    let state = await b.getState();
    for (const t of originals) state = await b.importLayout('monitor-1', await b.exportLayout(t));
    const added = state.config.monitors[0].layouts.slice(4);
    expect(added.map((l) => l.id)).toEqual(['halves-2', 'thirds-2', 'grid-2', 'priority-2']);
    expect(added.map((l) => ({ name: l.name, zones: l.zones }))).toEqual(
      originals.map((l) => ({ name: l.name, zones: l.zones })),
    );
  });

  it('makes the id unique with a suffix each time and never overwrites', async () => {
    const b = fresh();
    const text = await b.exportLayout(customLayout('x', 'Same name'));
    let s = await b.importLayout('monitor-1', text);
    expect(s.config.monitors[0].layouts.at(-1)?.id).toBe('x');
    s = await b.importLayout('monitor-1', text);
    expect(s.config.monitors[0].layouts.at(-1)?.id).toBe('x-2');
    s = await b.importLayout('monitor-1', text);
    expect(s.config.monitors[0].layouts.at(-1)?.id).toBe('x-3');
    const ids = s.config.monitors[0].layouts.map((l) => l.id);
    expect(new Set(ids).size).toBe(ids.length);
    expect(s.config.monitors[0].layouts.filter((l) => l.name === 'Same name')).toHaveLength(3);
  });

  it('uses a suffix that still fits a 64-character id', async () => {
    const b = fresh();
    const id = 'a'.repeat(64);
    const text = await b.exportLayout(customLayout(id));
    await b.importLayout('monitor-1', text);
    const s = await b.importLayout('monitor-1', text);
    const imported = s.config.monitors[0].layouts.at(-1);
    expect(imported?.id).toBe(`${'a'.repeat(62)}-2`);
    expect(imported?.id).toHaveLength(64);
    // And the result is still a layout the save rules accept.
    await expect(b.saveLayout('monitor-1', imported as Layout)).resolves.toBeDefined();
  });

  it('refuses the 65th layout of a monitor, saying so, and changes nothing', async () => {
    const b = fresh();
    const text = await b.exportLayout(customLayout('extra'));
    for (let i = 0; i < 60; i++) await b.importLayout('monitor-1', text);
    const full = await b.getState();
    expect(full.config.monitors[0].layouts).toHaveLength(64);
    await expect(b.importLayout('monitor-1', text)).rejects.toBe(
      'a monitor can hold at most 64 layouts',
    );
    expect(await b.getState()).toEqual(full);
    // Another monitor is not full; and deleting one makes room again.
    await expect(b.importLayout('monitor-2', text)).resolves.toBeDefined();
    await b.deleteLayout('monitor-1', 'extra');
    await expect(b.importLayout('monitor-1', text)).resolves.toBeDefined();
  });

  it('rejects with the exact strings of the contract', async () => {
    const b = fresh();
    const before = await b.getState();
    const unit = { x: 0, y: 0, w: 1, h: 1 };
    const cases: Array<[string, string]> = [
      ['', 'that is not valid JSON'],
      ['{ nope', 'that is not valid JSON'],
      ['[]', 'not a Krova layout file'],
      ['{"layout":{}}', 'not a Krova layout file'],
      [JSON.stringify({ krova_layout: 2, layout: {} }), 'unsupported layout file version 2'],
      [JSON.stringify({ krova_layout: 0, layout: {} }), 'unsupported layout file version 0'],
      [' '.repeat(65 * 1024), 'layout file is larger than 64 KiB'],
      [
        JSON.stringify({ krova_layout: 1, layout: { id: 'a', name: 'n', zones: [] } }),
        'layout has no zones',
      ],
      [
        JSON.stringify({
          krova_layout: 1,
          layout: {
            id: 'a',
            name: 'n',
            zones: Array.from({ length: 65 }, (_, i) => ({ id: i + 1, rect: unit })),
          },
        }),
        'layout has too many zones (at most 64)',
      ],
      [
        JSON.stringify({
          krova_layout: 1,
          layout: { id: 'a'.repeat(65), name: 'n', zones: [{ id: 1, rect: unit }] },
        }),
        'layout id or name is too long (at most 64 characters)',
      ],
      [
        JSON.stringify({
          krova_layout: 1,
          layout: { id: 'a', name: 'n', zones: [{ id: 9, rect: { ...unit, w: 3 } }] },
        }),
        'zone 9 has an invalid rectangle',
      ],
      [
        JSON.stringify({
          krova_layout: 1,
          layout: {
            id: 'a',
            name: 'n',
            zones: [
              { id: 4, rect: { x: 0, y: 0, w: 0.5, h: 1 } },
              { id: 4, rect: { x: 0.5, y: 0, w: 0.5, h: 1 } },
            ],
          },
        }),
        'duplicate zone id 4',
      ],
      [
        '{"krova_layout":1,"layout":{"id":"a","name":"n","zones":[{"id":1,"rect":{"x":0,"y":0,"w":1e999,"h":1}}]}}',
        'zone 1 has an invalid rectangle',
      ],
    ];
    for (const [text, message] of cases) {
      await expect(b.importLayout('monitor-1', text), message).rejects.toBe(message);
    }
    await expect(b.importLayout('nope', await b.exportLayout(customLayout('ok')))).rejects.toBe(
      'monitor nope not found',
    );
    expect(await b.getState()).toEqual(before);
  });

  it('rejects with plain strings, never Error objects', async () => {
    const b = fresh();
    for (const text of ['', '[]', '{"krova_layout":9}', 'x'.repeat(70_000)]) {
      const e: unknown = await b.importLayout('monitor-1', text).catch((err: unknown) => err);
      expect(typeof e).toBe('string');
    }
    expect(
      typeof (await b.exportLayout({ id: 'e', name: 'e', zones: [] }).catch((e: unknown) => e)),
    ).toBe('string');
  });

  it('survives hostile input without changing anything', async () => {
    const b = fresh();
    const before = await b.getState();
    const hostile = [
      'x'.repeat(5 * 1024 * 1024),
      '['.repeat(30_000) + ']'.repeat(30_000),
      '{"krova_layout":1,"layout":' + '{"a":'.repeat(5_000) + '1' + '}'.repeat(5_000) + '}',
      JSON.stringify({
        krova_layout: 1,
        layout: {
          id: 'a',
          name: 'n',
          zones: Array.from({ length: 10_000 }, (_, i) => ({
            id: i,
            rect: { x: 0, y: 0, w: 1, h: 1 },
          })),
        },
      }),
      '{"krova_layout":1,"layout":{"id":"a","name":"n","zones":[{"id":1,"rect":{"x":NaN,"y":0,"w":1,"h":1}}]}}',
      '{"__proto__":{"krova_layout":1}}',
    ];
    for (const text of hostile) {
      await expect(b.importLayout('monitor-1', text)).rejects.toEqual(expect.any(String));
    }
    expect(await b.getState()).toEqual(before);
  });

  it('persists imported layouts like any other edit', async () => {
    const m = new Map<string, string>();
    const storage = {
      getItem: (k: string) => m.get(k) ?? null,
      setItem: (k: string, v: string) => void m.set(k, v),
      removeItem: (k: string) => void m.delete(k),
    };
    const a = createMockBackend({ storage });
    await a.importLayout('monitor-2', await a.exportLayout(customLayout('kept', 'Kept')));
    const b = createMockBackend({ storage });
    expect((await b.getState()).config.monitors[1].layouts.at(-1)).toEqual(
      customLayout('kept', 'Kept'),
    );
  });

  it('returns a state that does not alias the imported object', async () => {
    const b = fresh();
    const s = await b.importLayout('monitor-1', await b.exportLayout(customLayout('z')));
    s.config.monitors[0].layouts.at(-1)!.zones.length = 0;
    expect((await b.getState()).config.monitors[0].layouts.at(-1)?.zones).toHaveLength(2);
  });
});

// ---------------------------------------------------------------------------
// Layout switcher: reorder, undo of delete, target monitor, its own shortcut
// ---------------------------------------------------------------------------

describe('mock backend: layout switcher', () => {
  const ids = async (b: ReturnType<typeof fresh>, monitor = 'monitor-1') =>
    (await b.getState()).config.monitors.find((m) => m.id === monitor)!.layouts.map((l) => l.id);
  const mine = (id: string): Layout => ({
    id,
    name: id,
    zones: [{ id: 1, rect: { x: 0, y: 0, w: 1, h: 1 } }],
  });

  it('moveLayout puts the layout at the final index and leaves the rest in order', async () => {
    const b = fresh();
    expect(await ids(b)).toEqual(['halves', 'thirds', 'grid', 'priority']);
    await b.moveLayout('monitor-1', 'priority', 0);
    expect(await ids(b)).toEqual(['priority', 'halves', 'thirds', 'grid']);
    await b.moveLayout('monitor-1', 'priority', 3);
    expect(await ids(b)).toEqual(['halves', 'thirds', 'grid', 'priority']);
    await b.moveLayout('monitor-1', 'thirds', 2);
    expect(await ids(b)).toEqual(['halves', 'grid', 'thirds', 'priority']);
    // Another monitor is untouched.
    expect(await ids(b, 'monitor-2')).toEqual(['halves', 'thirds', 'grid', 'priority']);
  });

  it('moveLayout to where the layout already is succeeds and keeps the active layout', async () => {
    const b = fresh();
    const before = await b.getState();
    const after = await b.moveLayout('monitor-1', 'thirds', 1);
    expect(after.config.monitors[0].layouts).toEqual(before.config.monitors[0].layouts);
    expect(after.config.monitors[0].active_layout).toBe('halves');
    const moved = await b.moveLayout('monitor-1', 'halves', 3);
    expect(moved.config.monitors[0].active_layout).toBe('halves');
  });

  it('moveLayout refuses unknown ids and positions out of range, changing nothing', async () => {
    const b = fresh();
    const before = await b.getState();
    await expect(b.moveLayout('nope', 'halves', 0)).rejects.toBe('monitor nope not found');
    await expect(b.moveLayout('monitor-1', 'ghost', 0)).rejects.toBe('layout ghost not found');
    for (const bad of [4, 5, -1, 1.5, Number.NaN]) {
      await expect(b.moveLayout('monitor-1', 'halves', bad)).rejects.toBe(
        `position ${bad} is out of range (the monitor has 4 layouts)`,
      );
    }
    expect(await b.getState()).toEqual(before);
  });

  it('insertLayout undoes a delete: same index, and the active layout when asked', async () => {
    const b = fresh();
    const profile = (await b.getState()).config.monitors[0];
    const index = profile.layouts.findIndex((l) => l.id === 'halves');
    const deleted = profile.layouts[index];
    expect(profile.active_layout).toBe('halves');

    const after = await b.deleteLayout('monitor-1', 'halves');
    expect(after.config.monitors[0].active_layout).toBe('thirds');
    const restored = await b.insertLayout('monitor-1', deleted, index, true);
    expect(restored.config.monitors[0].layouts).toEqual(profile.layouts);
    expect(restored.config.monitors[0].active_layout).toBe('halves');

    await b.deleteLayout('monitor-1', 'grid');
    const again = await b.insertLayout('monitor-1', profile.layouts[2], 2, false);
    expect(again.config.monitors[0].layouts.map((l) => l.id)).toEqual([
      'halves',
      'thirds',
      'grid',
      'priority',
    ]);
    expect(again.config.monitors[0].active_layout).toBe('halves');
  });

  it('insertLayout accepts index == length (append) and refuses beyond it', async () => {
    const b = fresh();
    await b.insertLayout('monitor-1', mine('last'), 4, false);
    expect((await ids(b)).at(-1)).toBe('last');
    await expect(b.insertLayout('monitor-1', mine('far'), 6, false)).rejects.toBe(
      'position 6 is out of range (the monitor has 5 layouts)',
    );
    await expect(b.insertLayout('monitor-1', mine('neg'), -1, false)).rejects.toMatch(
      /out of range/,
    );
  });

  it('insertLayout never replaces, validates the layout and respects the 64 cap', async () => {
    const b = fresh();
    const before = await b.getState();
    await expect(b.insertLayout('monitor-1', mine('halves'), 0, false)).rejects.toBe(
      'layout halves already exists',
    );
    const bad = { ...mine('bad'), zones: [] };
    await expect(b.insertLayout('monitor-1', bad, 0, false)).rejects.toMatch(/no zones/);
    await expect(b.insertLayout('nope', mine('x'), 0, false)).rejects.toBe(
      'monitor nope not found',
    );
    expect(await b.getState()).toEqual(before);
    for (let i = 4; i < 64; i += 1) await b.insertLayout('monitor-1', mine(`l${i}`), i, false);
    await expect(b.insertLayout('monitor-1', mine('one-too-many'), 0, false)).rejects.toBe(
      'a monitor can hold at most 64 layouts',
    );
  });

  it('switcherMonitor reports the demo cursor monitor, or null for an unknown one', async () => {
    const b = fresh();
    expect(await b.switcherMonitor()).toBe('monitor-1');
    b.demo.setSwitcherMonitor('monitor-2');
    expect(await b.switcherMonitor()).toBe('monitor-2');
    b.demo.setSwitcherMonitor('monitor-9');
    expect(await b.switcherMonitor()).toBeNull();
    b.demo.setSwitcherMonitor(null);
    expect(await b.switcherMonitor()).toBeNull();
  });

  it('the demo can send a navigate to the switcher', async () => {
    const b = fresh();
    const seen: string[] = [];
    const off = await b.onNavigate((v) => seen.push(v));
    b.demo.navigate('switcher');
    b.demo.navigate('settings');
    off();
    b.demo.navigate('editor');
    expect(seen).toEqual(['switcher', 'settings']);
  });

  it('the switcher shortcut has its own status, apart from the window-move shortcuts', async () => {
    const b = fresh();
    const cur = (await b.getState()).config.settings;
    expect((await b.getState()).layout_switcher).toEqual({ active: true, failed: [] });
    const both = async (move: boolean, switcher: boolean) => {
      const s = await b.saveSettings({
        ...cur,
        keyboard_shortcuts: move,
        layout_switcher_shortcut: switcher,
      });
      return [s.shortcuts.active, s.layout_switcher.active];
    };
    expect(await both(true, true)).toEqual([true, true]);
    expect(await both(false, true)).toEqual([false, true]);
    expect(await both(true, false)).toEqual([true, false]);
    expect(await both(false, false)).toEqual([false, false]);
    const s = await b.saveSettings({ ...cur, layout_switcher_shortcut: false });
    expect(s.config.settings.layout_switcher_shortcut).toBe(false);
  });

  it('shortcuts=failed also refuses the switcher chord, with one sentence', async () => {
    const failed = createMockBackend({ persist: false, shortcuts: 'failed' });
    const s = await failed.getState();
    expect(s.layout_switcher).toEqual({ active: false, failed: [DEMO_FAILED_SWITCHER_SHORTCUT] });
    const cur = s.config.settings;
    const off = await failed.saveSettings({ ...cur, layout_switcher_shortcut: false });
    expect(off.layout_switcher).toEqual({ active: false, failed: [] });
    expect(off.shortcuts.failed).toEqual(DEMO_FAILED_SHORTCUTS);
    // No platform support, or shortcuts=off: nothing registers, nothing is refused.
    const none = createMockBackend({ persist: false, capabilities: 'none' });
    expect((await none.getState()).layout_switcher).toEqual({ active: false, failed: [] });
    const quiet = createMockBackend({ persist: false, shortcuts: 'off' });
    expect((await quiet.getState()).layout_switcher).toEqual({ active: false, failed: [] });
  });
});
