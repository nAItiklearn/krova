import { defineConfig } from '@playwright/test';

// End-to-end tests for the Krova UI in demo mode (browser + in-memory mock
// backend). See e2e/README.md.
//
// Browser: by default the Microsoft Edge that ships with Windows is used
// (channel "msedge"), so nothing has to be downloaded. Override with
//   PW_CHANNEL=chrome      installed Google Chrome
//   PW_CHANNEL=chromium    Playwright's bundled Chromium (needs `npx playwright install chromium`)

const PORT = 1420; // vite.config.ts: server.port with strictPort
const BASE_URL = `http://localhost:${PORT}`;

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
  workers: process.env.CI ? 2 : 4,

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
    // `vite --mode demo`: strictPort 1420, always the mock backend.
    command: 'npm run demo',
    url: BASE_URL,
    reuseExistingServer: !process.env.CI,
    timeout: 60_000,
    stdout: 'ignore',
    stderr: 'pipe',
  },
});
