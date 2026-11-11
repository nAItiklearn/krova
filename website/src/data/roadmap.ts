/**
 * The Roadmap page. Source: docs/ROADMAP.md (the "Krova's version" column, shortened to one plain sentence).
 *
 * Rules for this file:
 *   - "available" is only for things that are in Krova v0.1.0 (stable). "beta" is for things that are in the
 *     v0.2.0-beta.1 pre-release. Everything else is "progress" or "planned".
 *   - Never write a planned item as if it exists. No competitor names; Krova's own names for everything.
 *   - Dates and order are not promises. Say so on the page (it does), and keep sentences short.
 *   - Update LAST_UPDATED_ISO / LAST_UPDATED when the roadmap changes.
 */
import type { GlyphName } from './glyphs';

export type Status = 'available' | 'beta' | 'progress' | 'planned';

export interface RoadmapItem {
  name: string;
  text: string;
  status: Status;
  glyph: GlyphName;
  /** Small label next to the status, for example "Windows" or "Early". */
  tag?: string;
}

export interface RoadmapGroup {
  id: string;
  title: string;
  lead: string;
  items: readonly RoadmapItem[];
}

export const LAST_UPDATED_ISO = '2026-11-11';
export const LAST_UPDATED = '11 November 2026';

export const statusLabel: Record<Status, string> = {
  available: 'Available now',
  beta: 'In beta',
  progress: 'In progress',
  planned: 'Planned',
};

export const available: RoadmapGroup = {
  id: 'available',
  title: 'Available now',
  lead: 'What is in Krova v0.1.0, the stable release for Windows. Several of these have not yet been tried on real hardware; the changelog says which.',
  items: [
    { name: 'Zones editor', text: 'Draw zones by splitting the screen, or start from a template. Undo, save, switch layouts.', status: 'available', glyph: 'zones' },
    { name: 'Hold a key and drag', text: 'Hold Shift, Ctrl, Alt or the Windows key while dragging a window, release over a zone to snap.', status: 'available', glyph: 'snap' },
    { name: 'Edge snapping', text: 'Halves, quarters and maximize from the screen edges. Off by default.', status: 'available', glyph: 'edge' },
    { name: 'Layouts per monitor', text: 'Each monitor keeps its own layouts, and they come back when you plug it in again.', status: 'available', glyph: 'monitors' },
    { name: 'Keyboard zone control', text: 'Move the focused window to the next, previous or numbered zone, or to the next monitor.', status: 'available', glyph: 'keyboard', tag: 'Windows' },
    { name: 'Windows come back', text: 'When a monitor returns, the windows you snapped there go back into their zones.', status: 'available', glyph: 'memory', tag: 'Windows' },
    { name: 'Linked dividers', text: 'Resize a snapped window by its edge and the windows that share it follow.', status: 'available', glyph: 'dividers', tag: 'Windows, early' },
    { name: 'Space between zones', text: 'Leave 0 to 32 px around snapped windows.', status: 'available', glyph: 'gap' },
    { name: 'Layout files', text: 'Copy a layout as plain text and paste it on another monitor or computer.', status: 'available', glyph: 'files' },
    { name: 'Tray and autostart', text: 'Krova waits in the tray, and can start when you sign in.', status: 'available', glyph: 'tray' },
    { name: 'Light and dark', text: 'Follows your system, or pick one.', status: 'available', glyph: 'theme' },
  ],
};

export const progress: RoadmapGroup = {
  id: 'in-beta',
  title: 'In the 0.2 beta',
  lead: 'In v0.2.0-beta.1, a pre-release for Windows, along with every item marked “In beta” below. They pass their automated tests; most have not been tried by hand on real hardware yet.',
  items: [
    { name: 'Layout switcher', text: 'Your saved layouts as a stack of cards on the monitor under the cursor. Flip, apply, reorder, or delete with undo.', status: 'beta', glyph: 'switcher' },
    { name: 'A calmer look', text: 'A new main window with a sidebar, and a new icon, in light and dark.', status: 'beta', glyph: 'look' },
    { name: 'The notch', text: 'A small pill at the edge of the screen. Hover for the layout name, click for the quick panel, right-click for a menu.', status: 'beta', glyph: 'notch' },
    { name: 'Hot corners', text: 'Rest the cursor in a screen corner to open the switcher, toggle calm desk or focus dim, or pause.', status: 'beta', glyph: 'corners' },
    { name: 'Security hardening', text: 'Fixes from the second pass of the security review.', status: 'progress', glyph: 'shield' },
  ],
};

export const waves: readonly RoadmapGroup[] = [
  {
    id: 'wave-1',
    title: 'Wave 1: arrange without dragging',
    lead: 'Let Krova place windows for you, while you still decide where the zones are.',
    items: [
      { name: 'Auto-arrange', text: 'On by default, per monitor: a new window goes to the next free zone of your layout, whatever layout you drew.', status: 'beta', glyph: 'arrange' },
      { name: 'Preset families', text: 'More ready-made layouts: grid, columns, rows and main plus side, for one to five windows. They are ordinary layouts you can edit. In the beta, each monitor gets one that fits its shape on first run; a picker for the rest is not built yet.', status: 'progress', glyph: 'presets' },
      { name: 'Stacks', text: 'When every zone is taken, a new window stacks in the focused zone, with a slim tab strip to cycle through the stack.', status: 'beta', glyph: 'stacks' },
      { name: 'Swap', text: 'Drop a window on a full zone with the modifier held and the two windows trade places.', status: 'beta', glyph: 'swap' },
      { name: 'App rules', text: 'Per program, by file name and never by window title: leave it alone, always float it, or give it a preferred zone.', status: 'beta', glyph: 'rules' },
    ],
  },
  {
    id: 'wave-2',
    title: 'Wave 2: keyboard and control',
    lead: 'Reach everything without the mouse, and open less of the editor.',
    items: [
      { name: 'Rebind any shortcut', text: 'Record your own keys for each action, turn one off, or reset it. Clashes are flagged and free keys suggested.', status: 'beta', glyph: 'rebind' },
      { name: 'Shortcut per layout', text: 'A key combination applies layout 1 to 9 on the monitor under the cursor.', status: 'beta', glyph: 'layoutkeys' },
      { name: 'Cheat sheet', text: 'Every shortcut on one sheet, with your own keys.', status: 'beta', glyph: 'cheat' },
      { name: 'Pause', text: 'A shortcut and a tray item pause Krova. Windows stay where they are.', status: 'beta', glyph: 'pause' },
      { name: 'Quick panel', text: 'Click the tray icon or the notch for a small panel: layouts, pause, focus dim, calm desk, share-safe. The editor stays closed until you need it.', status: 'beta', glyph: 'panel' },
    ],
  },
  {
    id: 'wave-3',
    title: 'Wave 3: workspaces and focus',
    lead: 'Save a whole arrangement, and keep attention on one window.',
    items: [
      { name: 'Workspaces', text: 'Save which apps are on screen and where. Restore starts missing apps and places all of them. Stored on your computer, program paths only.', status: 'beta', glyph: 'workspaces' },
      { name: 'Focus', text: 'Dim everything except the active window, and follow focus.', status: 'beta', glyph: 'focus' },
      { name: 'Calm desk', text: 'Minimise every window that is not in a zone of the active layout, and bring them back by switching it off.', status: 'beta', glyph: 'calm' },
      { name: 'Share-safe', text: 'While on, Krova shows no app names or icons in its overlays, and keeps its own overlays and pop-ups out of screen capture.', status: 'beta', glyph: 'share' },
      { name: 'Window highlight', text: 'Mark the active window with a border or a soft glow, in your colour and width.', status: 'beta', glyph: 'highlight' },
    ],
  },
  {
    id: 'wave-4',
    title: 'Wave 4: fit and finish',
    lead: 'The details that make it feel finished.',
    items: [
      { name: 'Reserved edges', text: 'Keep room for third-party docks and bars, per monitor.', status: 'beta', glyph: 'reserved' },
      { name: 'Focus follows mouse', text: 'Opt-in, with a short delay. Windows may refuse some activations.', status: 'beta', glyph: 'follow' },
      { name: 'Hover actions', text: 'A small zone picker on a window’s title bar while the modifier is held. Off by default.', status: 'beta', glyph: 'hover' },
      { name: 'Languages', text: 'English first, then Spanish, German, Japanese and Simplified Chinese. The beta has the language setting; the translations are not done.', status: 'progress', glyph: 'languages' },
      { name: 'App icons', text: 'A few alternative Krova icon designs.', status: 'planned', glyph: 'icons' },
      { name: 'First-run tour', text: 'Five short steps you can skip, and a settings page that shows what Krova can do.', status: 'planned', glyph: 'tour' },
      { name: 'Check for updates', text: 'A tray item that opens the releases page in your browser. The app itself stays offline.', status: 'beta', glyph: 'updates' },
    ],
  },
];

export const own: RoadmapGroup = {
  id: 'own-ideas',
  title: 'Krova’s own ideas',
  lead: 'Things only a drawn-zone manager can do. Still ideas: the order is open and some may only work in part.',
  items: [
    { name: 'Layouts per virtual desktop', text: 'Keep a different layout for each virtual desktop.', status: 'planned', glyph: 'desktops' },
    { name: 'Maximize inside a zone', text: 'The maximize button fills your zone instead of the whole monitor. Full-screen video belongs to the app, so this may only work in part.', status: 'planned', glyph: 'maximize' },
    { name: 'Layouts from a terminal', text: 'List and apply layouts from the command line.', status: 'planned', glyph: 'cli' },
  ],
};

/** Names shown in the "Coming next" strip. Every one of them is in beta, in progress or planned. */
export const comingNext: readonly { name: string; status: Status }[] = [
  { name: 'Layout switcher', status: 'beta' },
  { name: 'Auto-arrange', status: 'beta' },
  { name: 'Stacks', status: 'beta' },
  { name: 'Workspaces', status: 'beta' },
  { name: 'Languages', status: 'progress' },
  { name: 'Layouts per virtual desktop', status: 'planned' },
];
