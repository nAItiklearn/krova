# Icons

Generated from `assets/icon/` and checked in, so a fresh clone builds. To regenerate after changing the SVGs
(from the repo root):

```
node assets/icon/build.mjs
```

`icon.ico` is assembled by that script from the pixel-snapped cuts in `assets/icon/small/` (16, 20 and 24 px, and 2x
of each) plus the master (64 and 256 px). `tauri icon` alone would shrink the master and blur the 1 px gaps. See
`assets/icon/README.md` for the construction and size rules.

`tauri.conf.json` lists `32x32.png`, `128x128.png`, `128x128@2x.png`, `icon.icns` and `icon.ico`. The tray icon reuses
the window icon (`app.default_window_icon()`), which on Windows is the `icon.ico` resource at the system icon size.

`tauri icon` also writes mobile and Store folders and PNGs (`android/`, `ios/`, `Square*.png`, `StoreLogo.png`); the
build script keeps only the files above.
