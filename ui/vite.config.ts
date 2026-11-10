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
    // Single chunk, so no preload links; this also keeps the bundle free of Vite's
    // fetch()-based modulepreload polyfill (nothing in the app calls fetch itself).
    modulePreload: { polyfill: false },
    // Two pages: the main window and the hover zone picker window (paths are
    // relative to ui/, where every npm script runs).
    rollupOptions: {
      input: {
        main: 'index.html',
        picker: 'picker.html',
      },
    },
  },
  test: {
    environment: 'node',
    include: ['src/**/*.test.ts'],
  },
});
