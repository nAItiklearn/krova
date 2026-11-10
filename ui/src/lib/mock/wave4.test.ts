import { describe, expect, it } from 'vitest';
import type { HoverPicker } from '../bindings';
import { createMockBackend, defaultSettings } from './mockBackend';
import { applyReserved, clampFocusDelay, resolveLanguage, sanitizeReserved } from './wave4';

const fresh = () => createMockBackend({ persist: false });
const WORK = { x: 0, y: 0, w: 1920, h: 1040 };

describe('wave 4 rules (mirrors of krova-core)', () => {
  it('reserved edges are logical px times the scale, clamped, ignored per axis when too big', () => {
    expect(applyReserved(WORK, 1, { top: 10, right: 20, bottom: 30, left: 40 })).toEqual({
      x: 40,
      y: 10,
      w: 1860,
      h: 1000,
    });
    expect(applyReserved(WORK, 2, { top: 0, right: 0, bottom: 0, left: 48 }).x).toBe(96);
    expect(applyReserved(WORK, 1.25, { top: 10, right: 0, bottom: 0, left: 0 }).y).toBe(13);
    const small = { x: 0, y: 0, w: 800, h: 600 };
    expect(applyReserved(small, 1, { top: 50, right: 400, bottom: 0, left: 400 })).toEqual({
      x: 0,
      y: 50,
      w: 800,
      h: 550,
    });
    expect(sanitizeReserved({ top: -5, right: 12.6, bottom: 1e9 })).toEqual({
      top: 0,
      right: 13,
      bottom: 400,
      left: 0,
    });
  });

  it('the focus delay is clamped to 150..800', () => {
    expect(clampFocusDelay(0)).toBe(150);
    expect(clampFocusDelay(350)).toBe(350);
    expect(clampFocusDelay(5000)).toBe(800);
    expect(clampFocusDelay(Number.NaN)).toBe(350);
  });

  it('system follows the browser language, with English for anything unknown', () => {
    expect(resolveLanguage('ja', 'de-DE')).toBe('ja');
    expect(resolveLanguage('system', 'de-AT')).toBe('de');
    expect(resolveLanguage('system', 'zh-CN')).toBe('zh-Hans');
    expect(resolveLanguage('system', 'zh-TW')).toBe('en');
    expect(resolveLanguage('system', 'fr')).toBe('en');
    expect(resolveLanguage('system', undefined)).toBe('en');
  });
});

describe('mock backend: wave 4', () => {
  it('defaults and the new state fields match the contract', async () => {
    const s = await fresh().getState();
    const d = defaultSettings();
    expect(d.focus_follows_mouse).toBe(false);
    expect(d.focus_follows_mouse_delay_ms).toBe(350);
    expect(d.hover_picker).toBe(false);
    expect(d.language).toBe('system');
    expect(s.config.monitors[0].reserved).toEqual({ top: 0, right: 0, bottom: 0, left: 0 });
    expect(s.wave4_capabilities).toEqual({
      focus_follows_mouse: true,
      hover_picker: true,
      open_releases_page: true,
    });
    expect(s.pointer).toEqual({ active: false, focus_refused: 0 });
    expect(s.languages.map((l) => l.id)).toEqual(['system', 'en', 'es', 'de', 'ja', 'zh-Hans']);
    expect(s.language).not.toBe('system');
  });

  it('settings are sanitised and the watcher turns on with a pointer feature', async () => {
    const b = fresh();
    const cur = (await b.getState()).config.settings;
    const s = await b.saveSettings({
      ...cur,
      focus_follows_mouse: true,
      focus_follows_mouse_delay_ms: 20,
      language: 'fr' as never,
    });
    expect(s.config.settings.focus_follows_mouse_delay_ms).toBe(150);
    expect(s.config.settings.language).toBe('system');
    expect(s.pointer.active).toBe(true);
    const de = await b.saveSettings({ ...s.config.settings, language: 'de', enabled: false });
    expect(de.language).toBe('de');
    expect(de.pointer.active, 'Krova disabled').toBe(false);
  });

  it('reserved edges shrink the work area of that monitor only', async () => {
    const b = fresh();
    const s = await b.setReservedEdges('monitor-1', { top: 0, right: 0, bottom: 0, left: 9999 });
    expect(s.config.monitors[0].reserved?.left).toBe(400);
    expect(s.monitors[0].work).toEqual({ x: 400, y: 0, w: 2160, h: 1400 });
    expect(s.monitors[1].work).toEqual({ x: 2560, y: 0, w: 1920, h: 1080 });
    await expect(b.setReservedEdges('monitor-9', sanitizeReserved({}))).rejects.toBe(
      'monitor monitor-9 not found',
    );
  });

  it('the hover picker can be faked, placed into a zone and closed', async () => {
    const b = fresh();
    const seen: HoverPicker[] = [];
    let ended = 0;
    await b.onHoverPicker((p) => seen.push(p));
    await b.onHoverPickerEnd(() => (ended += 1));
    await expect(b.placeHovered(1)).rejects.toBe('no window is under the picker');

    const picker = b.demo.hoverPicker('monitor-1');
    expect(picker?.monitor_id).toBe('monitor-1');
    expect(seen).toHaveLength(1);
    // The default layout of monitor 1 is halves: the left half of its work area.
    expect(seen[0].zones[0].rect).toEqual({ x: 0, y: 0, w: 1280, h: 1400 });
    await expect(b.placeHovered(99)).rejects.toBe('zone 99 is not in the active layout');
    await b.placeHovered(2);
    expect(b.demo.lastPlaced()).toEqual({ monitorId: 'monitor-1', zoneId: 2 });
    expect(ended).toBe(1);
  });

  it('the hover picker applies the zone gap like Rust does', async () => {
    const b = fresh();
    const cur = (await b.getState()).config.settings;
    await b.saveSettings({ ...cur, zone_gap_px: 10 });
    const p = b.demo.hoverPicker('monitor-1');
    // lo = hi = 5: a 5 px margin, then 5 px on each side of every zone.
    expect(p?.zones[0].rect).toEqual({ x: 10, y: 10, w: 1265, h: 1380 });
  });

  it('opening the releases page makes no request in the demo', async () => {
    const b = fresh();
    await b.openReleasesPage();
    expect(b.demo.releasesOpened()).toBe(1);
  });
});
