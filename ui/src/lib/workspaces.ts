// Pure helpers for the Workspaces view: what a saved workspace looks like per
// monitor, and how a restore outcome is worded. No titles, ever: a workspace
// entry is only an executable path, a monitor id and a zone id.

import type { AppState, EntryOutcome, Rect, Workspace } from './bindings';

/** File name of an executable path (`C:\Program Files\Code\Code.exe` -> `Code.exe`). */
export function exeName(exe: string): string {
  const name = exe.split(/[\\/]/).pop() ?? '';
  return name === '' ? exe : name;
}

export type OutcomeTone = 'ok' | 'info' | 'warn';

/** The words and colour for one line of a restore report. */
export function outcomeLabel(outcome: EntryOutcome): { label: string; tone: OutcomeTone } {
  switch (outcome) {
    case 'placed':
      return { label: 'Placed', tone: 'ok' };
    case 'launched':
      return { label: 'Launched', tone: 'ok' };
    case 'not_running':
      return { label: 'Not found', tone: 'info' };
    case 'timed_out':
      return { label: 'Did not open in time', tone: 'warn' };
    case 'refused':
      return { label: 'Refused', tone: 'warn' };
    case 'no_zone':
      return { label: 'Zone is gone', tone: 'warn' };
    default:
      return { label: 'Failed', tone: 'warn' };
  }
}

export interface MiniZone {
  id: number;
  rect: Rect;
  /** Executable names placed in this zone, in workspace order. */
  apps: string[];
}

export interface MiniMonitor {
  monitorId: string;
  label: string;
  /** width / height of the monitor, for the drawing's aspect ratio. */
  aspect: number;
  zones: MiniZone[];
  /** Entries whose zone is not in the layout any more. */
  stray: string[];
}

/**
 * One drawing per monitor the workspace touches: the layout recorded at capture
 * (or the monitor's active one if it is gone), with each entry in its zone.
 */
export function miniMonitors(ws: Workspace, state: AppState): MiniMonitor[] {
  const ids: string[] = [];
  for (const l of ws.layouts) if (!ids.includes(l.monitor_id)) ids.push(l.monitor_id);
  for (const e of ws.entries) if (!ids.includes(e.monitor_id)) ids.push(e.monitor_id);

  const out: MiniMonitor[] = [];
  for (const id of ids) {
    const entries = ws.entries.filter((e) => e.monitor_id === id);
    const profile = state.config.monitors.find((m) => m.id === id);
    const recorded = ws.layouts.find((l) => l.monitor_id === id)?.layout_id;
    const layout =
      profile?.layouts.find((l) => l.id === recorded) ??
      profile?.layouts.find((l) => l.id === profile.active_layout) ??
      null;
    // A monitor with no entries is drawn only if it is the only one.
    if (entries.length === 0 && ids.length > 1) continue;
    const snap = state.monitors.find((m) => m.profile_id === id);
    const r = snap?.rect ?? profile?.last_rect;
    const aspect = r && r.w > 0 && r.h > 0 ? r.w / r.h : 16 / 9;
    const zones: MiniZone[] = (layout?.zones ?? []).map((z) => ({
      id: z.id,
      rect: z.rect,
      apps: entries.filter((e) => e.zone_id === z.id).map((e) => exeName(e.exe)),
    }));
    const stray = entries.filter((e) => !zones.some((z) => z.id === e.zone_id)).map((e) => exeName(e.exe));
    out.push({ monitorId: id, label: profile?.label ?? snap?.label ?? id, aspect, zones, stray });
  }
  return out;
}

/** "1 app", "3 apps". */
export function appCount(n: number): string {
  return `${n} ${n === 1 ? 'app' : 'apps'}`;
}

/** The first "Workspace N" name not taken yet. */
export function nextWorkspaceName(existing: Workspace[]): string {
  const taken = new Set(existing.map((w) => w.name.toLowerCase()));
  for (let n = existing.length + 1; ; n++) {
    const name = `Workspace ${n}`;
    if (!taken.has(name.toLowerCase())) return name;
  }
}

/** Key in the shortcuts map that a workspace's restore shortcut is stored under. */
export const workspaceShortcutKey = (id: string): string => `workspace:${id}`;

/** The privacy line shown on the view and before the first save (security review F-27). */
export const PRIVACY_NOTE =
  "Workspaces save the full path of each app (it includes your Windows user name) in Krova's settings file on this PC. Nothing leaves your computer. Delete a workspace to remove it.";

/**
 * The full executable path behind one line of a restore report (the report itself
 * carries only the file name): the entry on the same monitor and zone with that
 * file name, else any entry with that file name, else the file name as given.
 */
export function entryPath(
  ws: Workspace | undefined,
  r: { program: string; monitor_id: string; zone_id: number },
): string {
  if (!ws) return r.program;
  const same = (exe: string) => exeName(exe).toLowerCase() === r.program.toLowerCase();
  const hit =
    ws.entries.find((e) => e.monitor_id === r.monitor_id && e.zone_id === r.zone_id && same(e.exe)) ??
    ws.entries.find((e) => same(e.exe));
  return hit?.exe ?? r.program;
}

/** Outcomes whose full path the report shows: something was or would be started, or Windows said no. */
export const SHOW_PATH: ReadonlySet<EntryOutcome> = new Set<EntryOutcome>(['launched', 'refused', 'timed_out', 'not_running']);

/** Distinct full paths of a workspace, in order: what "Open missing apps" may start. */
export function launchPaths(ws: Workspace): string[] {
  const seen = new Set<string>();
  const out: string[] = [];
  for (const e of ws.entries) {
    const k = e.exe.toLowerCase();
    if (!seen.has(k)) {
      seen.add(k);
      out.push(e.exe);
    }
  }
  return out;
}
