/**
 * Release notes. To add a release, put a new object at the TOP of `releases`.
 * The Changelog page and the "latest" badge are generated from this list.
 *
 *   status 'unreleased' -> shown as "not released yet", no date allowed
 *   status 'released'   -> `date` (YYYY-MM-DD) is required
 *
 * Write for users: what changed for them, in plain sentences. Do not invent
 * dates, and do not list a thing that is not in the build.
 */

export interface ChangelogSection {
  title: string;
  items: readonly string[];
}

export interface Release {
  version: string;
  status: 'unreleased' | 'released';
  /** ISO date, only for released versions. */
  date?: string;
  summary: string;
  sections: readonly ChangelogSection[];
}

export const releases: readonly Release[] = [
  {
    version: '0.1.0',
    status: 'unreleased',
    summary:
      'The first release. It has not been published yet; this is what it will contain. Expect rough edges: the app is new and has had very little testing outside its automated tests.',
    sections: [
      {
        title: 'What the first release contains',
        items: [
          'Zones you draw, one set of layouts per monitor. The layout editor has four templates (Halves, Thirds, Grid 2x2, Priority), split-vertical and split-horizontal tools, draggable dividers, undo, and Save. Up to 64 zones per layout and 64 layouts per monitor.',
          'Snap by dragging: hold Shift (or Ctrl, Alt, or the Windows key / Command) while dragging a window, and the zones of the monitor under the cursor appear. Release the mouse over a zone and the window fills it. Release the key, or press Esc, to cancel.',
          'Layouts come back when a monitor is reconnected. Krova recognises a monitor by its hardware identity, then its device path, then its last size and position.',
          'Optional edge snapping, off by default: left and right edges give halves, corners give quarters, the top edge maximizes. The edge size is 2 to 64 px. It never triggers on the border between two monitors.',
          'Animated moves (about 120 ms) or instant moves.',
          'Settings for the activation key, highlight colour and opacity, move behaviour, edge snapping, theme (system, light or dark) and launch at login. Changes save automatically.',
          'A tray icon (menu bar on macOS) with Enabled, Edit layouts, Settings, Launch at login and Quit. There is no taskbar window; closing the window hides it.',
          'On Windows: mixed-DPI monitors, the invisible window borders, each window’s own minimum and maximum size, and the taskbar area are all taken into account. Minimized, fullscreen and administrator windows are skipped.',
          'A per-user Windows installer (x64) and a universal macOS disk image. Both are unsigned.',
          'Free under the MIT license. By design: no accounts, no telemetry, no network calls, and window titles are never read or stored.',
        ],
      },
      {
        title: 'Known limitations in 0.1.0',
        items: [
          'No keyboard-shortcut snapping. Global shortcuts for snapping are an idea for v0.2, not a promise.',
          'No window stacks, tabs, automatic tiling or hover-to-activate.',
          'Windows of apps running as administrator are skipped, and running Krova as administrator is not supported.',
          'The installers are not code-signed, and the macOS app is not notarised. Windows SmartScreen and macOS Gatekeeper will warn before the first run.',
          'The macOS build has never been tested on a real Mac. It needs the Accessibility permission and may also ask for Input Monitoring.',
          'Some apps enforce their own minimum or maximum size, so a snapped window may not fill its zone exactly.',
          'Krova’s own window can look blurry on scaled displays on Windows. This is known and is being checked.',
          'Two identical monitor models are told apart by the port they use, then by position. If you swap them between ports, their layouts can swap too.',
          'If config.json cannot be saved (for example because a newer version of Krova wrote it), Krova keeps running on defaults and does not tell you.',
          'Drag latency and resource use while dragging have not been measured. See the Performance page for what has.',
          'Windows and macOS only. There is no Linux version.',
        ],
      },
    ],
  },
];

export const latestRelease = releases[0];

export function releaseLabel(r: Release): string {
  if (r.status === 'unreleased') return 'Not released yet';
  return r.date ?? 'Released';
}
