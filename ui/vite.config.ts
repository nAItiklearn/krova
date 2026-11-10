import { svelte } from '@sveltejs/vite-plugin-svelte';
import { defineConfig } from 'vitest/config';

// `npm run demo` runs `vite --mode demo`; src/lib/backend.ts reads
// import.meta.env.MODE and forces the in-browser mock backend in that mode.
export default defineConfig({
  base: './',
  plugins: [svelte()],
  // Keep Rust/Tauri compiler output visible when launched via `tauri dev`.
  clearScreen: false,
  envPrefix: ['VITE_', 'TAURI_ENV_'],
  server: {
    port: 1420,
    strictPort: true,
  },
  preview: {
    port: 1420,
    strictPort: true,
  },
  build: {
    outDir: 'dist',
    emptyOutDir: true,
    // WebView2 (Chromium >= 105) on Windows, WKWebView on macOS 13+ (Safari 16).
    target: ['chrome105', 'safari15'],
    sourcemap: false,
    // No fetch()-based modulepreload polyfill (nothing in the app calls fetch itself);
    // the shared chunk is still linked with <link rel="modulepreload">.
    modulePreload: { polyfill: false },
    rollupOptions: {
      // One page per window: the editor/settings app (index.html), the layout
      // switcher (switcher.html) and the quick panel the notch opens (panel.html), and the
      // hover zone picker (picker.html).
      // Each loads only its own entry; shared code lands in common chunks.
      // Paths are relative to the project root (this folder; npm scripts run here).
      input: {
        main: 'index.html',
        switcher: 'switcher.html',
        panel: 'panel.html',
        picker: 'picker.html',
      },
    },
  },
  test: {
    environment: 'node',
    include: ['src/**/*.test.ts'],
  },
});
