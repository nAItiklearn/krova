// Demo-mode mirror of the wave-3 backend (src-tauri/src/wave3.rs and
// crates/krova-core: workspace.rs, focus.rs, calm.rs): workspaces, focus dim,
// calm desk, share-safe and the window highlight. A small fake desktop of demo
// windows stands in for the real one; nothing is launched, moved or read.
//
// Like the Tauri commands, every function throws a plain string on error, with the
// same sentences as the Rust side.

import type {
  CalmDeskSettings,
  Config,
  EntryOutcome,
  FocusSettings,
  HighlightMode,
  HighlightSettings,
  RestoreReport,
  Settings,
  Wave3State,
  Workspace,
} from "../bindings";

export const MAX_WORKSPACES = 32;
export const MAX_WORKSPACE_ENTRIES = 64;
export const MAX_WORKSPACE_NAME_LEN = 64;
export const DEFAULT_ACCENT = "#ff5b1f";

export const messages = {
  emptyName: "give the workspace a name",
  nameTooLong: "the workspace name is too long (at most 64 characters)",
  hidden: "the workspace name contains hidden or control characters",
  tooMany: "at most 32 workspaces can be saved",
  unknown: "no workspace with this id",
  busy: "a workspace is already being restored",
} as const;

type Wave3Settings = Pick<
  Settings,
  "focus" | "calm_desk" | "share_safe" | "highlight"
>;

export function defaultWave3Settings(): Wave3Settings {
  return {
    focus: { enabled: false, opacity: 0.45 },
    calm_desk: { all_monitors: true },
    share_safe: false,
    highlight: { mode: "none", color: null, width_px: 2 },
  };
}

const MODES: HighlightMode[] = ["none", "active", "all", "glow"];

const clamp = (v: number, lo: number, hi: number) =>
  Math.min(hi, Math.max(lo, v));

/** Like `FocusSettings::sanitized` / `HighlightSettings::sanitized`; missing blocks get defaults. */
export function sanitizeWave3Settings(
  s: Partial<Wave3Settings>,
): Wave3Settings {
  const d = defaultWave3Settings();
  const focus: FocusSettings = { ...d.focus, ...(s.focus ?? {}) };
  const calm: CalmDeskSettings = { ...d.calm_desk, ...(s.calm_desk ?? {}) };
  const highlight: HighlightSettings = {
    ...d.highlight,
    ...(s.highlight ?? {}),
  };
  if (!MODES.includes(highlight.mode))
    throw `invalid highlight mode "${String(highlight.mode)}"`;
  const color =
    typeof highlight.color === "string" &&
    /^#?[0-9a-fA-F]{6}$/.test(highlight.color.trim())
      ? `#${highlight.color.trim().replace(/^#/, "").toLowerCase()}`
      : null;
  const width = Number.isFinite(highlight.width_px)
    ? Math.round(highlight.width_px)
    : 2;
  return {
    focus: {
      enabled: !!focus.enabled,
      opacity: Number.isFinite(focus.opacity)
        ? clamp(focus.opacity, 0.2, 0.8)
        : 0.45,
    },
    calm_desk: { all_monitors: calm.all_monitors !== false },
    share_safe: !!s.share_safe,
    highlight: { mode: highlight.mode, color, width_px: clamp(width, 1, 6) },
  };
}

/** Control, zero-width, direction and other invisible characters (workspace.rs `is_hidden_char`). */
const HIDDEN =
  // eslint-disable-next-line no-control-regex
  /[\u0000-\u001f\u007f-\u009f\u00ad\u061c\u180e\u200b-\u200f\u2028\u2029\u202a-\u202e\u2060-\u2064\u2066-\u206f\ufeff\ufff9-\ufffb]|[\u{e0000}-\u{e007f}]/u;

/** Trim and check a typed workspace name; throws the Rust sentence. */
export function normalizeWorkspaceName(raw: string): string {
  const name = raw.replace(/^ +| +$/g, "");
  if (name.trim() === "") throw messages.emptyName;
  if ([...name].length > MAX_WORKSPACE_NAME_LEN) throw messages.nameTooLong;
  if (HIDDEN.test(name) || /\s/.test(name.replace(/ /g, "")))
    throw messages.hidden;
  return name;
}

/** File name of an executable path, for the restore report. */
export const programName = (exe: string): string =>
  exe.split(/[\\/]/).pop() ?? exe;

/** A window on the demo desktop. `zone_id` null = floating (not in a zone). */
export interface DemoWindow {
  id: number;
  exe: string;
  monitor_id: string;
  zone_id: number | null;
  minimized: boolean;
}

export function demoWindows(): DemoWindow[] {
  return [
    {
      id: 101,
      exe: "C:\\Program Files\\Code\\Code.exe",
      monitor_id: "monitor-1",
      zone_id: 1,
      minimized: false,
    },
    {
      id: 102,
      exe: "C:\\Program Files\\Mail\\mail.exe",
      monitor_id: "monitor-1",
      zone_id: 2,
      minimized: false,
    },
    {
      id: 103,
      exe: "C:\\Program Files\\Chat\\chat.exe",
      monitor_id: "monitor-1",
      zone_id: null,
      minimized: false,
    },
    {
      id: 104,
      exe: "C:\\Program Files\\Music\\music.exe",
      monitor_id: "monitor-2",
      zone_id: null,
      minimized: false,
    },
  ];
}

const sameExe = (a: string, b: string) =>
  a.replace(/\//g, "\\").toLowerCase() === b.replace(/\//g, "\\").toLowerCase();

export interface Wave3Mock {
  status(): Wave3State;
  /** The demo desktop (tests and design reviews). */
  windows(): DemoWindow[];
  /** Replace the demo desktop (tests). */
  setWindows(w: DemoWindow[]): void;
  toggleFocus(): void;
  toggleCalmDesk(): void;
  capture(name: string, id?: string | null): Workspace;
  rename(id: string, name: string): void;
  remove(id: string): void;
  restore(id: string, launch: boolean): RestoreReport;
  reset(): void;
}

/** `config` is the mock backend's live config (mutated in place). */
export function createWave3Mock(config: () => Config): Wave3Mock {
  let windows = demoWindows();
  let calm: number[] | null = null;
  let nextId = 900;

  const settings = () => config().settings;
  const list = (): Workspace[] => {
    const c = config();
    if (!Array.isArray(c.workspaces)) c.workspaces = [];
    return c.workspaces;
  };
  const find = (id: string): Workspace => {
    const w = list().find((x) => x.id === id);
    if (!w) throw messages.unknown;
    return w;
  };

  return {
    status() {
      return {
        capabilities: {
          workspaces: true,
          calm_desk: true,
          focus_dim: true,
          highlight: true,
          dwm_borders: true,
        },
        calm_desk: { active: calm !== null, count: calm?.length ?? 0 },
        focus_active: settings().enabled && !!settings().focus?.enabled,
        share_safe: !!settings().share_safe,
        visuals_error: null,
        restoring: null,
      };
    },
    windows: () => windows.map((w) => ({ ...w })),
    setWindows(w) {
      windows = w.map((x) => ({ ...x }));
    },
    toggleFocus() {
      const s = settings();
      s.focus = {
        ...defaultWave3Settings().focus,
        ...s.focus,
        enabled: !s.focus?.enabled,
      };
    },
    toggleCalmDesk() {
      if (calm) {
        for (const id of calm) {
          const w = windows.find((x) => x.id === id);
          if (w?.minimized) w.minimized = false;
        }
        calm = null;
        return;
      }
      const all = settings().calm_desk?.all_monitors !== false;
      calm = windows
        .filter(
          (w) =>
            !w.minimized &&
            w.zone_id === null &&
            (all || w.monitor_id === "monitor-1"),
        )
        .map((w) => {
          w.minimized = true;
          return w.id;
        });
    },
    capture(rawName, id) {
      const name = normalizeWorkspaceName(rawName);
      const ws = list();
      let wsId: string;
      if (id) {
        find(id);
        wsId = id;
      } else {
        let n = 1;
        while (ws.some((w) => w.id === `ws-${n}`)) n++;
        wsId = `ws-${n}`;
        if (ws.length >= MAX_WORKSPACES) throw messages.tooMany;
      }
      const captured: Workspace = {
        id: wsId,
        name,
        layouts: config().monitors.map((m) => ({
          monitor_id: m.id,
          layout_id: m.active_layout,
        })),
        entries: windows
          .filter((w) => !w.minimized && w.zone_id !== null)
          .slice(0, MAX_WORKSPACE_ENTRIES)
          .map((w) => ({
            exe: w.exe,
            monitor_id: w.monitor_id,
            zone_id: w.zone_id as number,
          })),
      };
      const i = ws.findIndex((w) => w.id === wsId);
      if (i >= 0) ws[i] = captured;
      else ws.push(captured);
      return captured;
    },
    rename(id, rawName) {
      const name = normalizeWorkspaceName(rawName);
      find(id).name = name;
    },
    remove(id) {
      const ws = list();
      const i = ws.findIndex((w) => w.id === id);
      if (i < 0) throw messages.unknown;
      ws.splice(i, 1);
    },
    restore(id, launch) {
      const ws = find(id);
      for (const l of ws.layouts) {
        const p = config().monitors.find((m) => m.id === l.monitor_id);
        if (p?.layouts.some((x) => x.id === l.layout_id))
          p.active_layout = l.layout_id;
      }
      const used = new Set<number>();
      const entries = ws.entries.map((e) => {
        let outcome: EntryOutcome;
        const open = windows.find(
          (w) => !used.has(w.id) && sameExe(w.exe, e.exe),
        );
        let win = open;
        if (open) {
          outcome = "placed";
        } else if (launch) {
          win = {
            id: nextId++,
            exe: e.exe,
            monitor_id: e.monitor_id,
            zone_id: null,
            minimized: false,
          };
          windows.push(win);
          outcome = "launched";
        } else {
          outcome = "not_running";
        }
        if (win) {
          used.add(win.id);
          win.minimized = false;
          win.monitor_id = e.monitor_id;
          win.zone_id = e.zone_id;
        }
        return {
          program: programName(e.exe),
          monitor_id: e.monitor_id,
          zone_id: e.zone_id,
          outcome,
        };
      });
      let minimized = 0;
      for (const w of windows) {
        if (!used.has(w.id) && !w.minimized) {
          w.minimized = true;
          minimized++;
        }
      }
      return { workspace_id: ws.id, entries, minimized };
    },
    reset() {
      windows = demoWindows();
      calm = null;
      nextId = 900;
    },
  };
}

/** The accent the highlight uses when its colour is null. */
export function highlightColor(s: Pick<Settings, "highlight">): string {
  return s.highlight?.color ?? DEFAULT_ACCENT;
}
