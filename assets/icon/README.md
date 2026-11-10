# Krova icon

A screen sliced into four zones: one tall, one large, two small, and the smallest one in ember.
Ink and off-white do the structure; ember appears exactly once and marks the zone a window just snapped into.

## Colours

| Role | Hex | Used for |
| --- | --- | --- |
| Ink | `#141413` | app tile, and the zones of the mark on light backgrounds |
| Off-white | `#f4f1ea` | the three neutral zones (on the tile, and on dark backgrounds) |
| Ember | `#ff5b1f` | the one accent zone on the tile and on light backgrounds |
| Ember, dark mode | `#ff7a45` | the accent zone of `krova-mark-dark.svg` on dark surfaces |

No gradients, no opacity, no blue.

## Construction (master, `krova.svg`, 512 x 512)

- Tile: 512 square, corner radius 112, ink.
- Mark: a 336 square at 88..424 (17% margin), cut by 20-unit gaps into
  `120 x 336` (left, tall), `196 x 196` (top right, a true square), `104 x 120` and `72 x 120` (bottom right).
  The left column is the golden section of the width (120 of 316) and the top row the golden section of the height
  (196 of 316); the left width equals the bottom row height (120).
- Corners: the four outer corners of the mark are 52, every inner corner is 8, so the zones read as one rounded
  screen that has been sliced, not as four separate chips. This is not a 2 x 2 grid of rounded squares: four
  zones of four different sizes, one silhouette.
- Ember goes on the smallest zone, bottom right: where a dwindling split ends.

## Size rules

The master has 20-unit gaps, which is 2.5 px at 64 px and a grey smear at 16 px. So below 48 px the icon is a
separate cut with every edge on a whole pixel and 1 px gaps (tested at 16, 20, 24 and 32 px, see `previews/`):

| File | Canvas | Mark | Gap | Used for |
| --- | --- | --- | --- | --- |
| `small/krova-16.svg` | 16 | 12 | 1 | 16 px entry, and 2x for the 32 px entry |
| `small/krova-20.svg` | 20 | 14 | 1 | 20 px entry (125% scaling), and 2x for 40 px |
| `small/krova-24.svg` | 24 | 18 | 1 | 24 px entry (Windows 11 taskbar), and 2x for 48 px |
| `krova.svg` | 512 | 336 | 20 | 64 px and up, `icon.png`, `.icns`, README |

Same topology and ratios in every cut; only gaps, margins and the outer radius are rounded to the pixel grid.
Never scale the master below 48 px or a cut above its 2x size; use the next file in the table.

## Other files

- `krova-mark.svg`: the mark without the tile, ink + ember, for light backgrounds (336 x 336 viewBox).
- `krova-mark-dark.svg`: off-white + `#ff7a45`, for dark backgrounds. Pair them with `<picture>` and `prefers-color-scheme`.
- `krova-favicon.svg`: one file with both cuts. Up to 40 px wide it shows the 16 px cut (media query inside the SVG),
  above that the master. `website/public/favicon.svg` is a copy, kept in sync by `website/scripts/sync-icon.mjs`.
- `build.mjs`: regenerates the rasters (below).
- `previews/`: `sizes-light.png`, `sizes-dark.png` (16/24/32/64/256 from the real `icon.ico`, 1:1, plus a pixel zoom of
  16/20/24/32 and the mark/favicon), `taskbar.png` (Windows 11 style dark and light strips, 3x zoom, button and tray).

## Tray icon

Krova's tray icon is `app.default_window_icon()` (`src-tauri/src/tray.rs`), which on Windows is the exe's icon
resource loaded at the system icon size: 32 px at 100%, 40 at 125%, 48 at 150%. The shell then shrinks it by two
to 16, 20 and 24 px for the notification area. That is why the 32, 40 and 48 px entries of `icon.ico` are exact 2x
renders of the 16, 20 and 24 px cuts: a 2:1 shrink of an even-aligned image keeps the 1 px gaps.

Tauri 2 has no theme-specific tray icon setting (only macOS template images), so there is one version for both
taskbars. The ink tile carries its own background: on a light taskbar it is a dark badge, on a dark taskbar the
off-white zones and the ember zone stay legible while the tile edge disappears into the bar (see `previews/taskbar.png`).
A white/black pair would need code (`TrayIcon::set_icon` on a theme change) and the tile-less marks above are the
starting point for it.

## Regenerating

From the repo root (needs Node and network for `npx @tauri-apps/cli`):

```
node assets/icon/build.mjs
```

This rewrites `src-tauri/icons/{32x32,64x64,128x128,128x128@2x,icon}.png`, `icon.icns`, `icon.ico` and
`website/public/apple-touch-icon.png` (180 px, the master with square corners so iOS applies its own mask).
Edit the SVGs, never the PNGs. After changing `krova.svg` also copy its paths into `krova-favicon.svg`.
