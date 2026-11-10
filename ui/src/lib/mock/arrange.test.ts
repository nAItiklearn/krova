import { describe, expect, it } from 'vitest';
import { createMockBackend, defaultArrange, exeNameError } from './mockBackend';
import { allPresets, variantForSize } from './presets';

const backend = () => createMockBackend({ persist: false });

describe('mock backend: auto-arrange (wave 1)', () => {
  it('every monitor starts with auto-arrange on', async () => {
    const s = await backend().getState();
    for (const m of s.config.monitors) {
      expect(m.auto_arrange).toEqual(defaultArrange());
    }
    expect(s.capabilities.auto_arrange).toBe(true);
    expect(s.front_app).toEqual({ exe: 'code.exe', arranged: true, reason: null });
    expect(s.config.settings.app_rules).toEqual([]);
  });

  it('auto-arrange is set per monitor', async () => {
    const b = backend();
    const s = await b.setAutoArrange('monitor-2', {
      enabled: false,
      policy: 'main_first',
      fill_gaps: true,
      stack_when_full: false,
    });
    expect(s.config.monitors[0].auto_arrange.enabled).toBe(true);
    expect(s.config.monitors[1].auto_arrange).toEqual({
      enabled: false,
      policy: 'main_first',
      fill_gaps: true,
      stack_when_full: false,
    });
    await expect(b.setAutoArrange('monitor-9', defaultArrange())).rejects.toBe(
      'monitor monitor-9 not found',
    );
  });

  it('app rules are added, replaced ignoring case and removed', async () => {
    const b = backend();
    await b.setAppRule({ exe: 'Spotify.exe', action: { kind: 'preferred_zone', value: 3 } });
    let s = await b.setAppRule({ exe: 'spotify.EXE', action: { kind: 'float' } });
    expect(s.config.settings.app_rules).toEqual([
      { exe: 'spotify.EXE', action: { kind: 'float' } },
    ]);
    s = await b.removeAppRule('SPOTIFY.exe');
    expect(s.config.settings.app_rules).toEqual([]);
    await expect(b.removeAppRule('nope.exe')).rejects.toBe('there is no rule for nope.exe');
  });

  it('bad rules are refused with a readable sentence', async () => {
    const b = backend();
    await expect(b.setAppRule({ exe: 'C:/Apps/x.exe', action: { kind: 'float' } })).rejects.toMatch(
      'is a path',
    );
    await expect(
      b.setAppRule({ exe: 'x.exe', action: { kind: 'preferred_zone', value: 0 } }),
    ).rejects.toMatch('between 1 and 64');
    expect(exeNameError('a\u202eb.exe')).toMatch('invisible');
    expect(exeNameError('ok.exe')).toBeNull();
  });

  it('the front app can be faked for the chip', async () => {
    const b = backend();
    b.demo.setFrontApp({ exe: 'calc.exe', arranged: false, reason: 'float' });
    expect((await b.getState()).front_app?.reason).toBe('float');
  });

  it('arrange now only works for a connected monitor', async () => {
    const b = backend();
    await expect(b.arrangeMonitor('monitor-1')).resolves.toBeUndefined();
    await expect(b.arrangeMonitor('monitor-7')).rejects.toBe('monitor monitor-7 is not connected');
  });
});

describe('mock presets', () => {
  it('ids are unique and zones are numbered in order', () => {
    const all = allPresets();
    expect(new Set(all.map((p) => p.id)).size).toBe(all.length);
    for (const p of all) {
      expect(p.layout.id).toBe(p.id);
      expect(p.layout.zones.map((z) => z.id)).toEqual(
        Array.from({ length: p.count }, (_, i) => i + 1),
      );
      const area = p.layout.zones.reduce((a, z) => a + z.rect.w * z.rect.h, 0);
      expect(area).toBeCloseTo(1, 6);
    }
    expect(all.some((p) => p.id === 'main-side-3-60')).toBe(true);
    expect(all.some((p) => p.id === 'main-side-3-ultrawide-60')).toBe(true);
  });

  it('the screen shape picks the variant', async () => {
    expect(variantForSize(1920, 1080)).toBe('landscape');
    expect(variantForSize(3440, 1440)).toBe('ultrawide');
    expect(variantForSize(1080, 1920)).toBe('portrait');
    const presets = await backend().listPresets('monitor-1');
    expect(presets.filter((p) => p.recommended).every((p) => p.variant === 'landscape')).toBe(true);
    expect((await backend().listPresets()).some((p) => p.recommended)).toBe(false);
  });
});
