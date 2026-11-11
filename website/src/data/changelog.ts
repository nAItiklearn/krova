/**
 * Release notes. To add a release, put a new object at the TOP of `releases`.
 * The Changelog page and the "latest" badge are generated from this list.
 *
 *   status 'unreleased' -> shown as "not released yet", no date allowed
 *   status 'released'   -> `date` (YYYY-MM-DD) is required
 *   status 'prerelease' -> a beta published as a GitHub pre-release; `date` optional (add it once it is out)
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
  status: 'unreleased' | 'released' | 'prerelease';
  /** ISO date, only for released versions. */
  date?: string;
  summary: string;
  sections: readonly ChangelogSection[];
}

export const releases: readonly Release[] = [
  {
    version: '0.2.0-beta.1',
    status: 'prerelease',
    summary:
      'A beta of 0.2, published as a pre-release on GitHub, for Windows only. The installer is unsigned. Every feature below passes its automated tests, but most have not yet been tried by hand on a real PC, so expect bugs. v0.1.0 stays the stable version.',
    sections: [
      {
        title: 'Arrange without dragging',
        items: [
          'Auto-arrange, on by default for every monitor: a new window goes into the next free zone of the active layout about a third of a second after it opens. Dialogs, tool windows, pop-ups, full-screen and administrator windows are left alone. Switch it off, or choose “Main zone first”, “Fill gaps” and “Stack when full”, under Arrange.',
          'Stacks: when every zone is taken, a new window joins a zone’s stack. A slim tab strip on the zone’s top edge shows each program’s icon and file name (never the window title); click a tab or turn the wheel over the strip to switch.',
          'Swap: drag an arranged window onto another one’s zone with the snap key held and the two trade places; Ctrl+Alt+Shift+arrow swaps with the neighbour.',
          'App rules, by program file name only: never touch, always float, or a preferred zone.',
          'On first run, each monitor gets a main + side layout that fits its shape (landscape, ultrawide or portrait), added in front of your layouts.',
        ],
      },
      {
        title: 'Control',
        items: [
          'New shortcut scheme on Ctrl+Alt: arrows move to the neighbouring zone (and on to the next monitor), 1 to 9 pick a zone, Shift+arrows swap, Shift+1 to 9 apply a layout, Enter fills the screen and back, [ and ] cycle a stack, Space opens the layout switcher, T pauses, F toggles focus dim, / shows the cheat sheet.',
          'Rebind any shortcut, or switch it off. Krova checks a new combination against its own and asks Windows whether it is free, and suggests free ones when it is not. If you used Win+Alt or Win+Ctrl in 0.1, those keys are kept.',
          'The notch: a small pill at the top centre of the screen, or in a top or bottom corner. Hover to see the layout name, click for the quick panel, right-click for a menu, drag to move it. It hides during full-screen apps.',
          'The quick panel: pause, the layouts of that monitor, focus dim, calm desk, share-safe, and links to the switcher, the cheat sheet and Krova. It also opens from a left click on the tray icon.',
          'The layout switcher: your layouts as a stack of cards on the monitor under the cursor. Flip, apply, reorder, or delete with undo.',
          'Hot corners: rest the cursor in a screen corner to open the layout switcher, toggle calm desk or focus dim, or pause. By default the top-right corner opens the switcher and the bottom-right one toggles calm desk. Corners where two monitors meet never fire.',
          'Pause: Ctrl+Alt+T, the tray, the notch or the panel. New windows are not arranged while paused; windows stay where they are.',
          'A hover zone picker (off by default): hold the snap key on a window’s title bar and pick a zone from a small card next to the cursor.',
        ],
      },
      {
        title: 'Workspaces and focus',
        items: [
          'Workspaces: save which programs are in which zones on which monitors, and restore it later. Restore can start missing programs (it shows their paths and asks first, and refuses programs in places like Temp or Downloads), and it minimises the other windows instead of closing them.',
          'Focus dim: everything except the active window fades (20 to 80 %), and follows the focus.',
          'Calm desk: minimise every window that is not in a zone; toggle again to bring them back.',
          'Share-safe: while on, Krova’s overlays show no program names or icons, and Krova’s own overlays and pop-up windows are hidden from screen capture and recording.',
          'Window highlight: none, an active border, all borders or a soft glow, in your colour and width.',
        ],
      },
      {
        title: 'Look and fit',
        items: [
          'A new main window with a sidebar: Overview, Layouts, Arrange, Workspaces, Focus, Shortcuts, Corners & notch, Appearance, Displays, Screen edges and Privacy & about. A new ink and ember icon.',
          'Appearance: accent colour, corner radius and border width, used by the zone overlay, the stack tabs and the notch too.',
          'Screen edges: keep room per monitor for third-party docks and bars that Windows does not know about.',
          'Focus follows mouse (off by default): the window under a resting cursor becomes active after a short delay.',
          '“Check for updates” in the tray opens the releases page in your browser. Krova itself still makes no network requests.',
        ],
      },
      {
        title: 'Known limitations in 0.2.0-beta.1',
        items: [
          'Windows only. No macOS build: the macOS code has never run on a real Mac.',
          'The installer is not code-signed. SmartScreen shows “Windows protected your PC”: choose More info, then Run anyway.',
          'Most new features have only been checked by automated tests and have not been tried by hand on real hardware yet, including the notch, hot corners, stacks, workspaces, focus dim and mixed-DPI setups.',
          'Focus follows mouse may be refused by Windows’ foreground rules; how often is not measured yet.',
          'Ctrl+Alt+Left and Right clash with back and forward in some code editors, and some older Intel graphics drivers use Ctrl+Alt+arrows to rotate the screen. Rebind them under Shortcuts if needed.',
          'Programs from the Microsoft Store cannot be started by a workspace restore; they are reused when already open.',
          'Languages other than English are placeholders (English text) for now.',
          '“Show zones” (quick panel, hot corner) and the “Cheat sheet” hot corner do nothing yet. Ctrl+Alt+/ and the quick panel’s Cheat sheet link may not show the sheet in this build; it is always under Shortcuts, Cheat sheet.',
        ],
      },
    ],
  },
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

/** The newest stable release (pre-releases are skipped). */
export const latestRelease = releases.find((r) => r.status === 'released') ?? releases[0];

export function releaseLabel(r: Release): string {
  if (r.status === 'unreleased') return 'Not released yet';
  if (r.status === 'prerelease') return r.date ? `Beta, ${r.date}` : 'Beta · pre-release';
  return r.date ?? 'Released';
}
