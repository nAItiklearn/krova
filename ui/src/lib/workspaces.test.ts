import { describe, expect, it } from 'vitest';
import type { AppState, HoverZone, Workspace } from './bindings';
import { cleanSide, noneReserved, reservedBands } from './edges';
import { cursorFraction, pickerMap, placeNear, zoneForKey, zonesBounds } from './hoverPicker';
import { createMockBackend } from './mock/mockBackend';
import { appCount, exeName, miniMonitors, nextWorkspaceName, outcomeLabel } from './workspaces';

const ws = (over: Partial<Workspace> = {}): Workspace => ({
  id: 'ws-1',
  name: 'Coding',
  layouts: [
    { monitor_id: 'monitor-1', layout_id: 'halves' },
    { monitor_id: 'monitor-2', layout_id: 'thirds' },
  ],
  entries: [
    { exe: 'C:\\Program Files\\Code\\Code.exe', monitor_id: 'monitor-1', zone_id: 1 },
    { exe: 'C:/Apps/mail.exe', monitor_id: 'monitor-1', zone_id: 2 },
  ],
  ...over,
});

async function state(): Promise<AppState> {
  return createMockBackend({ persist: false, storage: null }).getState();
}

describe('workspaces', () => {
  it('exeName keeps only the file name', () => {
    expect(exeName('C:\\Program Files\\Code\\Code.exe')).toBe('Code.exe');
    expect(exeName('/usr/bin/app')).toBe('app');
    expect(exeName('plain.exe')).toBe('plain.exe');
    expect(exeName('C:\\dir\\')).toBe('C:\\dir\\');
  });

  it('outcomeLabel words every outcome', () => {
    expect(outcomeLabel('placed')).toEqual({ label: 'Placed', tone: 'ok' });
    expect(outcomeLabel('launched').tone).toBe('ok');
    expect(outcomeLabel('not_running').label).toBe('Not found');
    expect(outcomeLabel('refused')).toEqual({ label: 'Refused', tone: 'warn' });
    for (const o of ['timed_out', 'no_zone', 'failed'] as const) expect(outcomeLabel(o).tone).toBe('warn');
  });

  it('miniMonitors draws only monitors with entries, with apps in their zones', async () => {
    const s = await state();
    const m = miniMonitors(ws(), s);
    expect(m).toHaveLength(1);
    expect(m[0].monitorId).toBe('monitor-1');
    expect(m[0].aspect).toBeCloseTo(2560 / 1440);
    expect(m[0].zones.map((z) => z.apps)).toEqual([['Code.exe'], ['mail.exe']]);
    expect(m[0].stray).toEqual([]);
  });

  it('miniMonitors falls back to the active layout and lists entries whose zone is gone', async () => {
    const s = await state();
    const m = miniMonitors(
      ws({
        layouts: [{ monitor_id: 'monitor-2', layout_id: 'deleted' }],
        entries: [{ exe: 'x.exe', monitor_id: 'monitor-2', zone_id: 7 }],
      }),
      s,
    );
    expect(m[0].zones).toHaveLength(3); // thirds, the active layout of monitor 2
    expect(m[0].stray).toEqual(['x.exe']);
  });

  it('miniMonitors still draws the one monitor of an empty workspace', async () => {
    const s = await state();
    const m = miniMonitors(ws({ layouts: [{ monitor_id: 'monitor-1', layout_id: 'halves' }], entries: [] }), s);
    expect(m).toHaveLength(1);
    expect(m[0].zones.every((z) => z.apps.length === 0)).toBe(true);
  });

  it('appCount and nextWorkspaceName', () => {
    expect(appCount(1)).toBe('1 app');
    expect(appCount(0)).toBe('0 apps');
    expect(nextWorkspaceName([])).toBe('Workspace 1');
    expect(nextWorkspaceName([ws({ name: 'Workspace 2' })])).toBe('Workspace 3');
    expect(nextWorkspaceName([ws({ name: 'a' }), ws({ name: 'workspace 3' })])).toBe('Workspace 4');
  });
});

describe('screen edges', () => {
  const rect = { x: 0, y: 0, w: 2000, h: 1000 };
  it('cleanSide clamps and rounds', () => {
    expect(cleanSide('12.6')).toBe(13);
    expect(cleanSide(-4)).toBe(0);
    expect(cleanSide(9999)).toBe(400);
    expect(cleanSide('abc')).toBe(0);
    expect(cleanSide(undefined)).toBe(0);
  });
  it('reservedBands gives fractions of the monitor, times the scale', () => {
    const b = reservedBands(rect, 2, { top: 50, right: 0, bottom: 0, left: 100 });
    expect(b.top).toBeCloseTo(0.1);
    expect(b.left).toBeCloseTo(0.1);
    expect(b.ignoredX || b.ignoredY).toBe(false);
  });
  it('an axis that would keep less than 200 px is ignored', () => {
    const b = reservedBands({ x: 0, y: 0, w: 900, h: 1000 }, 1, { top: 0, right: 400, bottom: 0, left: 400 });
    expect(b.ignoredX).toBe(true);
    expect(b.left).toBe(0);
    expect(b.right).toBe(0);
  });
  it('noneReserved', () => {
    expect(noneReserved({ top: 0, right: 0, bottom: 0, left: 0 })).toBe(true);
    expect(noneReserved({ top: 0, right: 1, bottom: 0, left: 0 })).toBe(false);
  });
});

describe('hover picker', () => {
  const zones: HoverZone[] = [
    { id: 1, rect: { x: 100, y: 0, w: 1000, h: 1000 } },
    { id: 2, rect: { x: 1100, y: 0, w: 1000, h: 1000 } },
  ];
  it('zonesBounds is the union of the zones', () => {
    expect(zonesBounds(zones)).toEqual({ x: 100, y: 0, w: 2000, h: 1000 });
    expect(zonesBounds([])).toEqual({ x: 0, y: 0, w: 1, h: 1 });
  });
  it('pickerMap fits the aspect ratio and gives fractions', () => {
    const m = pickerMap(zones, 200, 200);
    expect(m.width).toBe(200);
    expect(m.height).toBe(100);
    expect(m.zones[1]).toEqual({ id: 2, x: 0.5, y: 0, w: 0.5, h: 1 });
  });
  it('cursorFraction clamps', () => {
    const p = { monitor_id: 'm', window_rect: zones[0].rect, layout_id: 'l', zones, cursor: [1100, 250] as [number, number] };
    expect(cursorFraction(p)).toEqual({ fx: 0.5, fy: 0.25 });
    expect(cursorFraction({ ...p, cursor: [-50, 5000] })).toEqual({ fx: 0, fy: 1 });
  });
  it('placeNear flips at the viewport edges and stays inside', () => {
    expect(placeNear(100, 100, 50, 40, 800, 600)).toEqual({ left: 114, top: 114 });
    expect(placeNear(780, 590, 50, 40, 800, 600)).toEqual({ left: 716, top: 536 });
    expect(placeNear(10, 10, 900, 700, 800, 600)).toEqual({ left: 8, top: 8 });
  });
  it('zoneForKey maps 1..9 to zones', () => {
    expect(zoneForKey('2', zones)).toBe(2);
    expect(zoneForKey('3', zones)).toBeNull();
    expect(zoneForKey('a', zones)).toBeNull();
    expect(zoneForKey('0', zones)).toBeNull();
  });
});

describe('workspace paths (F-27)', () => {
  it('entryPath finds the full path behind a report line', async () => {
    const { entryPath, launchPaths } = await import('./workspaces');
    const w = ws();
    const code = w.entries[0].exe;
    expect(entryPath(w, { program: 'Code.exe', monitor_id: 'monitor-1', zone_id: 1 })).toBe(code);
    expect(entryPath(w, { program: 'MAIL.EXE', monitor_id: 'monitor-2', zone_id: 9 })).toBe('C:/Apps/mail.exe');
    expect(entryPath(w, { program: 'other.exe', monitor_id: 'monitor-1', zone_id: 1 })).toBe('other.exe');
    expect(entryPath(undefined, { program: 'a.exe', monitor_id: 'm', zone_id: 1 })).toBe('a.exe');
    const dup = ws({ entries: [...w.entries, { exe: code.toLowerCase(), monitor_id: 'monitor-2', zone_id: 1 }] });
    expect(launchPaths(dup)).toEqual([code, 'C:/Apps/mail.exe']);
  });
});
