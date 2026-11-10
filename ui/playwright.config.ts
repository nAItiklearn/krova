import { defineConfig } from '@playwright/test';

// End-to-end tests for the Krova UI in demo mode (browser + in-memory mock
// backend). See e2e/README.md.
//
// Browser: by default the Microsoft Edge that ships with Windows is used
// (channel "msedge"), so nothing has to be downloaded. Override with
//   PW_CHANNEL=chrome      installed Google Chrome
//   PW_CHANNEL=chromium    Playwright's bundled Chromium (needs `npx playwright install chromium`)

// vite.config.ts: server.port 1420 with strictPort. PW_PORT starts the demo server on
// another port instead (passed to vite as --port); use it when 1420 is taken, because
// with `reuseExistingServer` a stale server on 1420 would be tested instead of this code.
const PORT = Number(process.env.PW_PORT ?? 1420);
const BASE_URL = `http://localhost:${PORT}`;

// PW_PREVIEW=1: build the demo once (`vite build --mode demo` into e2e/.demo-dist) and
// serve it with `vite preview` instead of the dev server. A static build does not
// recompile on the fly, so it is not slowed down or reloaded when files change (other
// work in the same checkout, a busy machine) - use it for long or repeated runs.
const PREVIEW = !!process.env.PW_PREVIEW;
const DEMO_DIST = 'e2e/.demo-dist';

const requested = process.env.PW_CHANNEL ?? 'msedge';
const channel = requested === 'chromium' ? undefined : requested;

export default defineConfig({
  testDir: './e2e',
  testMatch: '**/*.spec.ts',
  // Generated files stay inside e2e/ (git-ignored there).
  outputDir: './e2e/.results',
  reporter: process.env.CI
    ? [['list'], ['html', { open: 'never', outputFolder: './e2e/.report' }]]
    : [['list']],

  // Every test opens its own browser context and runs against `?nopersist`
  // (or `?reset`), so tests do not share state and can run in parallel.
  fullyParallel: true,
  forbidOnly: !!process.env.CI,
  retries: process.env.CI ? 1 : 0,
  // Each worker is a whole browser; more than 4 only slows everything down on a laptop.
  // PW_WORKERS overrides it: on a machine that is already at full load, fewer
  // workers keep the per-test time (and the trace written at teardown) in bounds.
  workers: process.env.PW_WORKERS ? Number(process.env.PW_WORKERS) : process.env.CI ? 2 : 4,

  // Generous on purpose: on a busy machine a page load alone can take 10+ s, and a
  // too-tight timeout shows up as flakiness, not as a bug.
  timeout: 60_000,
  expect: { timeout: 10_000 },

  use: {
    baseURL: BASE_URL,
    channel,
    viewport: { width: 1280, height: 900 },
    // Tests assert on the app's own rendering; keep Playwright out of its way.
    locale: 'en-US',
    trace: 'retain-on-failure',
    screenshot: 'only-on-failure',
  },

  webServer: {
    // `vite --mode demo` (or its static build): strictPort, always the mock backend.
    command: PREVIEW
      ? `npx vite build --mode demo --outDir ${DEMO_DIST} && npx vite preview --outDir ${DEMO_DIST} --port ${PORT} --strictPort`
      : `npm run demo -- --port ${PORT}`,
    url: BASE_URL,
    reuseExistingServer: !process.env.CI,
    // A first compile (or the demo build) can take well over a minute on a busy machine.
    timeout: 180_000,
    stdout: 'ignore',
    stderr: 'pipe',
  },
});
