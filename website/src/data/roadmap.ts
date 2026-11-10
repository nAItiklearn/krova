/**
 * The Roadmap page. Source: docs/ROADMAP.md (the "Krova's version" column, shortened to one plain sentence).
 *
 * Rules for this file:
 *   - "available" is only for things that are in Krova v0.1.0 today. Everything else is "progress" or "planned".
 *   - Never write a planned item as if it exists. No competitor names; Krova's own names for everything.
 *   - Dates and order are not promises. Say so on the page (it does), and keep sentences short.
 *   - Update LAST_UPDATED_ISO / LAST_UPDATED when the roadmap changes.
 */
import type { GlyphName } from './glyphs';

export type Status = 'available' | 'progress' | 'planned';

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

export const LAST_UPDATED_ISO = '2026-11-10';
export const LAST_UPDATED = '10 November 2026';

export const statusLabel: Record<Status, string> = {
  available: 'Available now',
  progress: 'In progress',
  planned: 'Planned',
};

export const available: RoadmapGroup = {
  id: 'available',
  title: 'Available now',
  lead: 'What is in Krova v0.1.0, an early preview for Windows. Several of these have not yet been tried on real hardware; the changelog says which.',
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
  id: 'in-progress',
  title: 'In progress',
  lead: 'Being worked on now. None of it is in the app you can download.',
  items: [
    { name: 'Layout switcher', text: 'Flip through your saved layouts as a stack of cards and apply one. The engine is being built; the interface follows once the design is approved.', status: 'progress', glyph: 'switcher' },
    { name: 'A calmer look', text: 'A cleaner, more consistent look for the editor and settings, light and dark.', status: 'progress', glyph: 'look' },
    { name: 'Security hardening', text: 'Fixes from the second pass of the security review.', status: 'progress', glyph: 'shield' },
  ],
};

export const waves: readonly RoadmapGroup[] = [
  {
    id: 'wave-1',
    title: 'Wave 1: arrange without dragging',
    lead: 'Let Krova place windows for you, while you still decide where the zones are.',
    items: [
      { name: 'Auto-arrange', text: 'Off by default. When on for a monitor, a new window goes to the next free zone of your layout, whatever layout you drew.', status: 'planned', glyph: 'arrange' },
      { name: 'Preset families', text: 'More ready-made layouts: grid, columns, rows and main plus side, for one to five windows. They are ordinary layouts you can edit.', status: 'planned', glyph: 'presets' },
      { name: 'Stacks', text: 'When every zone is taken, a new window stacks in the focused zone, with a slim tab strip to cycle through the stack.', status: 'planned', glyph: 'stacks' },
      { name: 'Swap', text: 'Drop a window on a full zone with the modifier held and the two windows trade places.', status: 'planned', glyph: 'swap' },
      { name: 'App rules', text: 'Per program, by name and never by window title: leave it alone, always float it, or give it a preferred zone or monitor.', status: 'planned', glyph: 'rules' },
    ],
  },
  {
    id: 'wave-2',
    title: 'Wave 2: keyboard and control',
    lead: 'Reach everything without the mouse, and open less of the editor.',
    items: [
      { name: 'Rebind any shortcut', text: 'Record your own keys for each action, turn one off, or reset it. Clashes are flagged and free keys suggested.', status: 'planned', glyph: 'rebind' },
      { name: 'Shortcut per layout', text: 'A key combination applies layout 1 to 9 on the monitor under the cursor.', status: 'planned', glyph: 'layoutkeys' },
      { name: 'Cheat sheet', text: 'One key shows every shortcut, with your own keys.', status: 'planned', glyph: 'cheat' },
      { name: 'Pause', text: 'A shortcut and a tray item pause Krova. Windows stay where they are.', status: 'planned', glyph: 'pause' },
      { name: 'Quick panel', text: 'Click the tray icon for a small panel: the layout per monitor, auto-arrange, pause. The editor stays closed until you need it.', status: 'planned', glyph: 'panel' },
    ],
  },
  {
    id: 'wave-3',
    title: 'Wave 3: workspaces and focus',
    lead: 'Save a whole arrangement, and keep attention on one window.',
    items: [
      { name: 'Workspaces', text: 'Save which apps are on screen and where. Restore starts missing apps and places all of them. Stored on your computer, program paths only.', status: 'planned', glyph: 'workspaces' },
      { name: 'Focus', text: 'Dim everything except the zone of the active window, and follow focus.', status: 'planned', glyph: 'focus' },
      { name: 'Calm desk', text: 'Minimise every window that is not in the active layout, and bring them back with the same key.', status: 'planned', glyph: 'calm' },
      { name: 'Share-safe', text: 'While on, Krova’s overlays show only zone shapes, never app names or window contents. For screen sharing.', status: 'planned', glyph: 'share' },
      { name: 'Window highlight', text: 'Mark the active window with a border or a soft glow, in your colour and width.', status: 'planned', glyph: 'highlight' },
    ],
  },
  {
    id: 'wave-4',
    title: 'Wave 4: fit and finish',
    lead: 'The details that make it feel finished.',
    items: [
      { name: 'Reserved edges', text: 'Keep room for third-party docks and bars, per monitor.', status: 'planned', glyph: 'reserved' },
      { name: 'Focus follows mouse', text: 'Opt-in, with a short delay.', status: 'planned', glyph: 'follow' },
      { name: 'Hover actions', text: 'A small zone picker on a window’s title bar while the modifier is held.', status: 'planned', glyph: 'hover' },
      { name: 'Languages', text: 'English first, then Spanish, German, Japanese and Simplified Chinese.', status: 'planned', glyph: 'languages' },
      { name: 'App icons', text: 'A few alternative Krova icon designs.', status: 'planned', glyph: 'icons' },
      { name: 'First-run tour', text: 'Five short steps you can skip, and a settings page that shows what Krova can do.', status: 'planned', glyph: 'tour' },
      { name: 'Check for updates', text: 'Opens the releases page in your browser. The app itself stays offline.', status: 'planned', glyph: 'updates' },
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

/** Names shown in the "Coming next" strip on the home page. Every one of them is planned or in progress. */
export const comingNext: readonly { name: string; status: Status }[] = [
  { name: 'Layout switcher', status: 'progress' },
  { name: 'Auto-arrange', status: 'planned' },
  { name: 'Stacks', status: 'planned' },
  { name: 'Swap', status: 'planned' },
  { name: 'App rules', status: 'planned' },
  { name: 'Quick panel', status: 'planned' },
  { name: 'Workspaces', status: 'planned' },
];
