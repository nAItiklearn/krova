# Icons

Generated from `assets/icon/krova.svg` and checked in, so a fresh clone builds. To
regenerate after changing the SVG (from the repo root):

```
npx -y @tauri-apps/cli@2 icon assets/icon/krova.svg --output src-tauri/icons
```

`tauri.conf.json` lists `32x32.png`, `128x128.png`, `128x128@2x.png`, `icon.icns` and
`icon.ico`. The tray icon reuses the window icon (`app.default_window_icon()`).

The extra mobile/Store folders and PNGs the generator also writes (`android/`, `ios/`,
`Square*.png`, `StoreLogo.png`) are unused by Krova and can be deleted.
