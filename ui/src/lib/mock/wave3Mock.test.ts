import { describe, expect, it } from 'vitest';
import { createMockBackend, defaultSettings } from './mockBackend';
import {
  MAX_WORKSPACES,
  messages,
  normalizeWorkspaceName,
  programName,
  sanitizeWave3Settings,
} from './wave3Mock';

const fresh = () => createMockBackend({ persist: false });

describe('wave 3 settings', () => {
  it('are off by default and part of the state', async () => {
    const s = await fresh().getState();
    expect(s.config.settings.focus).toEqual({ enabled: false, opacity: 0.45 });
    expect(s.config.settings.highlight).toEqual({ mode: 'none', color: null, width_px: 2 });
    expect(s.config.settings.share_safe).toBe(false);
    expect(s.config.settings.calm_desk.all_monitors).toBe(true);
    expect(s.config.workspaces).toEqual([]);
    expect(s.wave3.capabilities.workspaces).toBe(true);
    expect(s.wave3.calm_desk).toEqual({ active: false, count: 0 });
  });

  it('are clamped like the Rust side', () => {
    const s = sanitizeWave3Settings({
      focus: { enabled: true, opacity: 3 },
      highlight: { mode: 'glow', color: 'ABCDEF', width_px: 40 },
    });
    expect(s.focus.opacity).toBe(0.8);
    expect(s.highlight).toEqual({ mode: 'glow', color: '#abcdef', width_px: 6 });
    expect(sanitizeWave3Settings({ highlight: { mode: 'all', color: 'red', width_px: 0 } }).highlight)
      .toEqual({ mode: 'all', color: null, width_px: 1 });
    expect(() =>
      sanitizeWave3Settings({ highlight: { mode: 'rainbow' as never, color: null, width_px: 2 } }),
    ).toThrow();
  });

  it('survive a settings save and drive focus_active and share_safe', async () => {
    const b = fresh();
    let s = await b.saveSettings({
      ...defaultSettings(),
      share_safe: true,
      focus: { enabled: false, opacity: 0.1 },
    });
    expect(s.config.settings.focus.opacity).toBe(0.2);
    expect(s.wave3.share_safe).toBe(true);
    s = await b.toggleFocusDim();
    expect(s.config.settings.focus.enabled).toBe(true);
    expect(s.wave3.focus_active).toBe(true);
    s = await b.toggleFocusDim();
    expect(s.wave3.focus_active).toBe(false);
  });
});

describe('workspace names', () => {
  it('follow the layout name rules', () => {
    expect(normalizeWorkspaceName('  Writing ')).toBe('Writing');
    expect(normalizeWorkspaceName('日本 & more')).toBe('日本 & more');
    expect(() => normalizeWorkspaceName('   ')).toThrow(messages.emptyName);
    expect(() => normalizeWorkspaceName('n'.repeat(65))).toThrow(messages.nameTooLong);
    expect(normalizeWorkspaceName('é'.repeat(64))).toHaveLength(64);
    for (const c of ['\u202e', '\u2066', '\u200b', '\u2060', '\ufeff', '\u2028', '\n', '\t']) {
      expect(() => normalizeWorkspaceName(`a${c}b`)).toThrow(messages.hidden);
    }
    expect(programName('C:\\Apps\\Code.exe')).toBe('Code.exe');
  });
});

describe('workspaces in the demo', () => {
  it('capture the windows in zones and restore them', async () => {
    const b = fresh();
    let s = await b.captureWorkspace('Coding');
    expect(s.config.workspaces).toHaveLength(1);
    const ws = s.config.workspaces[0];
    expect(ws.id).toBe('ws-1');
    // Two demo windows are in zones; the floating ones are not captured.
    expect(ws.entries.map((e) => programName(e.exe))).toEqual(['Code.exe', 'mail.exe']);
    expect(ws.layouts.map((l) => l.monitor_id)).toEqual(['monitor-1', 'monitor-2']);
    expect(JSON.stringify(ws)).not.toMatch(/title/);

    const report = await b.restoreWorkspace(ws.id, true);
    expect(report.entries.map((e) => e.outcome)).toEqual(['placed', 'placed']);
    expect(report.minimized).toBe(2);

    s = await b.renameWorkspace(ws.id, 'Deep work');
    expect(s.config.workspaces[0].name).toBe('Deep work');
    await expect(b.renameWorkspace('ws-9', 'x')).rejects.toBe(messages.unknown);
    s = await b.deleteWorkspace(ws.id);
    expect(s.config.workspaces).toEqual([]);
    await expect(b.deleteWorkspace(ws.id)).rejects.toBe(messages.unknown);
  });

  it('are forgotten with the rest of the demo on reset', async () => {
    const b = fresh();
    const ws = (await b.captureWorkspace('A')).config.workspaces[0];
    await b.demo.reset();
    // After a reset the workspace is gone with the rest of the demo state.
    await expect(b.restoreWorkspace(ws.id, false)).rejects.toBe(messages.unknown);
  });

  it('refuse bad names and more than 32 workspaces', async () => {
    const b = fresh();
    await expect(b.captureWorkspace('x\u202ey')).rejects.toBe(messages.hidden);
    for (let i = 0; i < MAX_WORKSPACES; i++) await b.captureWorkspace(`W${i}`);
    await expect(b.captureWorkspace('one more')).rejects.toBe(messages.tooMany);
    // Replacing an existing one still works at the limit.
    const s = await b.captureWorkspace('Again', 'ws-3');
    expect(s.config.workspaces.find((w) => w.id === 'ws-3')?.name).toBe('Again');
  });
});

describe('calm desk in the demo', () => {
  it('minimises the floating windows and brings them back', async () => {
    const b = fresh();
    let s = await b.toggleCalmDesk();
    expect(s.wave3.calm_desk).toEqual({ active: true, count: 2 });
    s = await b.toggleCalmDesk();
    expect(s.wave3.calm_desk).toEqual({ active: false, count: 0 });
  });

  it('pushes state-changed', async () => {
    const b = fresh();
    const seen: boolean[] = [];
    await b.onStateChanged((st) => seen.push(st.wave3.calm_desk.active));
    await b.toggleCalmDesk();
    expect(seen).toEqual([true]);
  });
});
