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
    status: 'released',
    date: '2026-10-31',
    summary:
      'The first release, for Windows only. Expect rough edges: the app is new and has had very little testing outside its automated tests and a few runs on one Windows PC.',
    sections: [
      {
        title: 'What the first release contains',
        items: [
          'Zones you draw, one set of layouts per monitor. The layout editor has four templates (Halves, Thirds, Grid 2x2, Priority), split-vertical and split-horizontal tools, draggable dividers, undo, and Save. Up to 64 zones per layout and 64 layouts per monitor.',
          'Snap by dragging: hold Shift (or Ctrl, Alt, or the Windows key / Command) while dragging a window, and the zones of the monitor under the cursor appear. Release the mouse over a zone and the window fills it. Release the key, or press Esc, to cancel.',
          'Keyboard zone control (Windows only): global shortcuts move the focused window to the previous or next zone (Left, Right), to zone 1 to 9, or to the next monitor keeping its zone (Up). The default keys are Ctrl+Alt+Shift; you can choose Ctrl+Alt, Win+Alt or Win+Ctrl. Windows can keep some combinations for itself, and Settings lists the ones Krova could not register.',
          'Windows return to their zones when a monitor comes back (Windows only): windows Krova snapped to a monitor go back into their zones when that monitor comes back, for example after docking or waking the PC. This is kept in memory only, no window titles are stored, and a window you move yourself is left alone. It can be switched off.',
          'Linked dividers (Windows only, early): resize a snapped window by its edge and the snapped neighbours that share that edge follow when you let go. It can be switched off.',
          'Layout files: Export layout shows a layout as plain JSON text to copy, and Import layout adds pasted text to a monitor as a new layout. Zones are proportions of the screen, so one text should work on any monitor and on both Windows and macOS (it has not been tried on a Mac). At most 64 KiB, strict format.',
          'Space between zones, 0 to 32 px (0 by default). It applies to zone snaps, the keyboard shortcuts, restored windows and the preview while you drag. Edge snapping ignores it.',
          'Layouts come back when a monitor is reconnected. Krova recognises a monitor by its hardware identity, then its device path, then its last size and position.',
          'Optional edge snapping, off by default: left and right edges give halves, corners give quarters, the top edge maximizes. The edge size is 2 to 64 px. It never triggers on the border between two monitors.',
          'Animated moves (about 120 ms) or instant moves.',
          'Settings for the activation key, highlight colour and opacity, move behaviour, edge snapping, theme (system, light or dark), launch at login, the keyboard shortcuts and their modifier keys, window memory on reconnect, linked dividers and the space between zones. Changes save automatically.',
          'A tray icon (menu bar on macOS) with Enabled, Edit layouts, Settings, Launch at login and Quit. There is no taskbar window. Closing the editor window closes it for good, together with the embedded browser, so Krova waits in the tray using little memory; reopen it from the tray menu or by starting Krova again.',
          'On Windows: mixed-DPI monitors, the invisible window borders, each window’s own minimum and maximum size, and the taskbar area are all taken into account, and the app declares per-monitor DPI awareness. Minimized, fullscreen and administrator windows are skipped.',
          'A per-user Windows installer (x64), unsigned. No macOS download in this release: macOS support is coming once it has been tested on real hardware.',
          'Free under the MIT license. By design: no accounts, no telemetry, no network calls, and window titles are never read or stored.',
        ],
      },
      {
        title: 'Known limitations in 0.1.0',
        items: [
          'macOS has no keyboard shortcuts and no window memory or linked dividers yet; those settings are unavailable there. The macOS build has never been tested on a real Mac, needs the Accessibility permission and may also ask for Input Monitoring.',
          'Window memory has been tested with a simulated monitor change and a real Notepad, not with a real monitor unplug and replug. It applies only to windows Krova snapped since it started; nothing is kept after you quit Krova.',
          'Linked dividers are early. The part that moves the neighbours works in tests with synthetic events, but it has not been proven that Windows reports a real mouse resize to Krova. If you try it, tell us whether it works.',
          'Some shortcut combinations can be taken by Windows or by other apps. Windows refused every Win+Alt and most Win+Ctrl combinations on the PC we tested. Krova lists the shortcuts it could not register in Settings; the default Ctrl+Alt+Shift was accepted for all twelve there.',
          'The keyboard shortcuts do nothing for the desktop, the taskbar, minimized windows, administrator windows and Krova’s own windows.',
          'Per-monitor DPI awareness is built in, but it has not been verified on a mixed-DPI setup (for example a 100% and a 150% monitor).',
          'No window stacks, tabs, automatic tiling or hover-to-activate.',
          'Windows of apps running as administrator are skipped, and running Krova as administrator is not supported.',
          'The installers are not code-signed, and the macOS app is not notarised. Windows SmartScreen and macOS Gatekeeper will warn before the first run. An unsigned program that registers global shortcuts and watches the mouse may also be flagged by antivirus software; that has not been tested.',
          'Some apps enforce their own minimum or maximum size, so a snapped window may not fill its zone exactly.',
          'Two identical monitor models are told apart by the port they use, then by position. If you swap them between ports, their layouts can swap too.',
          'If config.json cannot be saved (for example because a newer version of Krova wrote it), Krova keeps running on defaults and does not tell you.',
          'Drag latency and resource use while dragging have not been measured. See the Performance page for what has: single runs on one PC.',
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
