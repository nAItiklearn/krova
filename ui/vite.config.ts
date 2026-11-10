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
    // Two pages: the main window (index.html) and the quick panel the notch opens
    // (panel.html). Their shared code lands in one chunk; Vite's fetch()-based
    // modulepreload polyfill stays out (nothing in the app calls fetch itself).
    modulePreload: { polyfill: false },
    rollupOptions: {
      input: {
        // Relative to the project root (this folder; npm scripts run here).
        main: 'index.html',
        panel: 'panel.html',
      },
    },
  },
  test: {
    environment: 'node',
    include: ['src/**/*.test.ts'],
  },
});
