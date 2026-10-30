/**
 * Compare table data.
 *
 * Source of truth: docs/compare-research.md (accessed 2026-10-30). Only cells
 * the research tagged [V] (verified against the product's own documentation)
 * are written as claims. Everything tagged [U] is `unknown`, rendered as an
 * em dash with a footnote. Do not add a claim here that is not in that file.
 *
 * The Krova column describes v0.1 as designed. The macOS build has never been
 * run on a Mac, and "no network calls" is a design rule plus a source review,
 * not a proof.
 *
 * To update: edit the rows below, change CHECKED_ISO / CHECKED_LABEL, and keep
 * SOURCES in step. An empty `rows` array renders a clearly marked empty state.
 */

export type CellKind = 'yes' | 'no' | 'part' | 'text' | 'unknown';

export interface Cell {
  kind: CellKind;
  /** Short text. Omitted for `unknown`. */
  text?: string;
}

export type ColumnId = 'snap' | 'fz' | 'rect' | 'magnet' | 'mac' | 'krova';

export interface Column {
  id: ColumnId;
  name: string;
  note: string;
}

export const columns: readonly Column[] = [
  { id: 'krova', name: 'Krova', note: 'v0.1' },
  { id: 'snap', name: 'Windows Snap', note: 'Windows 11 and 10' },
  { id: 'fz', name: 'PowerToys FancyZones', note: 'Windows' },
  { id: 'rect', name: 'Rectangle (free)', note: 'macOS' },
  { id: 'magnet', name: 'Magnet', note: 'macOS' },
  { id: 'mac', name: 'macOS tiling', note: 'macOS 15+' },
];

export interface Row {
  label: string;
  cells: Record<ColumnId, Cell>;
}

const unknown: Cell = { kind: 'unknown' };

export const rows: readonly Row[] = [
  {
    label: 'Platforms',
    cells: {
      krova: { kind: 'text', text: 'Windows 10/11 and macOS 13+ (macOS build untested)' },
      snap: { kind: 'text', text: 'Windows 11 and 10' },
      fz: { kind: 'text', text: 'Windows 11, and Windows 10 2004 or newer' },
      rect: { kind: 'text', text: 'macOS 14+' },
      magnet: { kind: 'text', text: 'macOS 13+' },
      mac: { kind: 'text', text: 'macOS 15 or later' },
    },
  },
  {
    label: 'Price and licence',
    cells: {
      krova: { kind: 'yes', text: 'Free, MIT' },
      snap: { kind: 'text', text: 'Part of Windows' },
      fz: { kind: 'text', text: 'MIT licence' },
      rect: { kind: 'text', text: 'Free, MIT. Rectangle Pro is a separate paid app' },
      magnet: { kind: 'text', text: '$4.99 one-time (US Mac App Store price)' },
      mac: { kind: 'text', text: 'Part of macOS' },
    },
  },
  {
    label: 'Zones you draw yourself',
    cells: {
      krova: { kind: 'yes', text: 'Per monitor' },
      snap: { kind: 'no', text: 'Predetermined layouts' },
      fz: { kind: 'yes', text: 'Grid and Canvas editors' },
      rect: { kind: 'no', text: 'No zone editor; fixed set of sizes' },
      magnet: unknown,
      mac: { kind: 'no', text: 'Documented layouts are a fixed set' },
    },
  },
  {
    label: 'Hold a key to show zones while dragging',
    cells: {
      krova: { kind: 'yes', text: 'Shift by default; or Ctrl, Alt, Windows/Command' },
      snap: { kind: 'no', text: 'Layouts show while dragging, no key' },
      fz: { kind: 'yes', text: 'Shift by default; configurable' },
      rect: { kind: 'no', text: 'No zone overlay; edge drag shows a preview' },
      magnet: unknown,
      mac: { kind: 'part', text: 'Option-drag tiles to preset areas, not your zones' },
    },
  },
  {
    label: 'A separate layout per monitor',
    cells: {
      krova: { kind: 'yes' },
      snap: unknown,
      fz: { kind: 'yes', text: 'Pick a monitor in the editor' },
      rect: unknown,
      magnet: unknown,
      mac: unknown,
    },
  },
  {
    label: 'Edge and corner snapping',
    cells: {
      krova: { kind: 'part', text: 'Optional, off by default' },
      snap: { kind: 'yes', text: 'Edges and corners' },
      fz: unknown,
      rect: { kind: 'yes' },
      magnet: { kind: 'yes' },
      mac: { kind: 'yes' },
    },
  },
  {
    label: 'Keyboard-shortcut snapping',
    cells: {
      krova: { kind: 'no', text: 'Not yet; an idea for v0.2' },
      snap: { kind: 'yes', text: 'Win+Arrow keys' },
      fz: { kind: 'yes', text: 'Opt-in Win+Arrow, and layout hotkeys' },
      rect: { kind: 'yes', text: 'Configurable shortcuts' },
      magnet: { kind: 'yes', text: 'Optional shortcuts' },
      mac: { kind: 'yes', text: 'Built-in shortcuts' },
    },
  },
  {
    label: 'Windows of apps run as administrator',
    cells: {
      krova: { kind: 'no', text: 'Skipped; no administrator mode' },
      snap: unknown,
      fz: { kind: 'part', text: 'Only if PowerToys itself runs as administrator' },
      rect: unknown,
      magnet: unknown,
      mac: unknown,
    },
  },
  {
    label: 'Network and telemetry',
    cells: {
      krova: { kind: 'yes', text: 'No network calls, by design' },
      snap: unknown,
      fz: {
        kind: 'text',
        text: 'Docs: diagnostic data “completely optional”, “off by default in v0.86 and beyond”. Checks for updates',
      },
      rect: {
        kind: 'text',
        text: 'Privacy policy: “No user data is gathered”. Checks for updates every 2 days',
      },
      magnet: { kind: 'text', text: 'Developer states it collects no personal data' },
      mac: unknown,
    },
  },
  {
    label: 'Release status',
    cells: {
      krova: { kind: 'no', text: 'v0.1, not released; unsigned; macOS untested' },
      snap: { kind: 'text', text: 'Part of Windows. Windows 10 support ended 14 Oct 2025' },
      fz: { kind: 'text', text: 'v0.102.3001, 27 Oct 2026' },
      rect: { kind: 'text', text: 'v2.0.4, 20 Oct 2026' },
      magnet: { kind: 'text', text: '3.0.7, 12 Apr 2025 (US App Store)' },
      mac: { kind: 'text', text: 'Part of macOS' },
    },
  },
];

export const CHECKED_ISO = '2026-10-30';
export const CHECKED_LABEL = '30 October 2026';

export interface Source {
  name: string;
  url: string;
}

export const sources: readonly Source[] = [
  { name: 'Windows Snap: Microsoft Support, “Snap your windows”', url: 'https://support.microsoft.com/en-us/windows/snap-your-windows-885a9b1e-a983-a3b1-16cd-c531795e6241' },
  { name: 'FancyZones: Microsoft Learn', url: 'https://learn.microsoft.com/en-us/windows/powertoys/fancyzones' },
  { name: 'PowerToys administrator mode: Microsoft Learn', url: 'https://learn.microsoft.com/en-us/windows/powertoys/administrator' },
  { name: 'PowerToys install and requirements: Microsoft Learn', url: 'https://learn.microsoft.com/en-us/windows/powertoys/install' },
  { name: 'PowerToys data and privacy', url: 'https://raw.githubusercontent.com/microsoft/PowerToys/main/DATA_AND_PRIVACY.md' },
  { name: 'PowerToys repository and latest release', url: 'https://github.com/microsoft/PowerToys' },
  { name: 'Rectangle README and LICENSE', url: 'https://github.com/rxhanson/Rectangle' },
  { name: 'Rectangle site and Pro page', url: 'https://rectangleapp.com' },
  { name: 'Rectangle privacy policy', url: 'https://rectangleapp.com/terms' },
  { name: 'Magnet: US Mac App Store listing', url: 'https://apps.apple.com/us/app/magnet/id441258766?mt=12' },
  { name: 'Magnet site and FAQ', url: 'https://magnet.crowdcafe.com/' },
  { name: 'macOS tiling: Apple Support, tile app windows', url: 'https://support.apple.com/guide/mac-help/tile-app-windows-mchlef287e5d/15.0/mac/15.0' },
  { name: 'macOS tiling settings and shortcuts: Apple Support', url: 'https://support.apple.com/guide/mac-help/mac-window-tiling-icons-keyboard-shortcuts-mchl9674d0b0/15.0/mac/15.0' },
];
