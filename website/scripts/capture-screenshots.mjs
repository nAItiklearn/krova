// Captures the real Krova UI (../ui, demo mode, in-memory mock backend) into website/public/screenshots/.
// The home page shows these files, so to refresh them after the desktop UI changes, run this one script:
//
//   npm run screenshots                 (from website/)
//   npm run screenshots -- --only editor,crop-keyboard
//
// What it does
//   1. Starts `vite --mode demo` from ../ui on a free local port (or uses KROVA_SHOT_URL if you already run one).
//   2. Opens it in Chromium at 2x device pixel ratio, once in the light theme and once in the dark theme.
//   3. Saves a full window shot of the Editor and of Settings, plus small crops of single cards and rows.
//   4. Writes PNGs named <shot>-light.png and <shot>-dark.png, quantised to a small palette with `sharp`
//      (a dependency of Astro; if it is missing the raw PNG is kept).
//
// The page never leaves localhost. The only change made to the UI is hiding the "Demo mode" badge, which
// exists in the browser demo only; the real app has no such badge. The monitors are the mock backend's two
// sample monitors, so captions on the site say "sample monitors".
//
// Browser: Microsoft Edge by default (ships with Windows, nothing to download). Override with
//   PW_CHANNEL=chrome        installed Google Chrome
//   PW_CHANNEL=chromium      Playwright's own Chromium (needs `npx playwright install chromium` in ../ui)
//
// The crops locate things by role, visible heading text and data-testid (the same ids the UI's e2e tests use),
// so a restyle of the UI does not break them. A shot that cannot be found is skipped with a warning and the
// previous file stays in place.
import { spawn } from 'node:child_process';
import { existsSync, mkdirSync, writeFileSync } from 'node:fs';
import { createRequire } from 'node:module';
import { dirname, join, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';

const here = dirname(fileURLToPath(import.meta.url));
const site = resolve(here, '..');
const ui = resolve(site, '../ui');
const outDir = join(site, 'public', 'screenshots');
const tag = '[screenshots]';

const args = process.argv.slice(2);
const onlyArg = args.find((a) => a.startsWith('--only='))?.slice(7) ?? (args.includes('--only') ? args[args.indexOf('--only') + 1] : '');
const only = new Set(onlyArg.split(',').map((s) => s.trim()).filter(Boolean));
const want = (name) => only.size === 0 || only.has(name);

const DPR = 2;
const THEMES = ['light', 'dark'];
const EDITOR_WIDTH = 1280; // the editor window; its height is the page height, so nothing is cut off
const NARROW_WIDTH = 400; // a narrow window: settings rows wrap, so crops of single rows stay legible at card size
const SETTINGS_WIDTH = 1040; // settings is a centred column, so a narrower window
const SETTINGS_SHOT_HEIGHT = 860; // fallback height of the settings shot, if the end card below is not found

function fail(msg) {
  console.error(`\n${tag} ERROR: ${msg}\n`);
  process.exit(1);
}

if (!existsSync(join(ui, 'node_modules', '@playwright', 'test'))) {
  fail('Playwright is not installed in ../ui. Run: npm ci --prefix ../ui');
}
const require = createRequire(join(ui, 'package.json'));
const { chromium } = require('@playwright/test');

let sharp = null;
try {
  sharp = createRequire(join(site, 'package.json'))('sharp');
} catch {
  console.warn(`${tag} sharp not found, keeping raw PNGs`);
}

/* ------------------------------------------------------------------ dev server */

let server = null;

async function waitFor(url, ms) {
  const end = Date.now() + ms;
  while (Date.now() < end) {
    try {
      const res = await fetch(url);
      if (res.ok) return true;
    } catch {
      /* not up yet */
    }
    await new Promise((r) => setTimeout(r, 250));
  }
  return false;
}

async function startUi() {
  if (process.env.KROVA_SHOT_URL) return process.env.KROVA_SHOT_URL.replace(/\/+$/, '');
  const port = Number(process.env.KROVA_SHOT_PORT ?? 5199);
  const base = `http://127.0.0.1:${port}`;
  const vite = join(ui, 'node_modules', 'vite', 'bin', 'vite.js');
  if (!existsSync(vite)) fail('Vite is not installed in ../ui. Run: npm ci --prefix ../ui');
  server = spawn(process.execPath, [vite, '--mode', 'demo', '--host', '127.0.0.1', '--port', String(port), '--strictPort'], {
    cwd: ui,
    stdio: ['ignore', 'ignore', 'inherit'],
  });
  server.on('exit', (code) => {
    if (code) console.error(`${tag} the UI dev server exited with code ${code}`);
  });
  if (!(await waitFor(base + '/', 60_000))) fail(`the UI dev server did not answer on ${base} (is the port in use? set KROVA_SHOT_PORT)`);
  return base;
}

/* ------------------------------------------------------------------ saving */

mkdirSync(outDir, { recursive: true });

async function save(name, theme, buffer) {
  const file = join(outDir, `${name}-${theme}.png`);
  let out = buffer;
  if (sharp) {
    try {
      out = await sharp(buffer).png({ palette: true, quality: 96, effort: 10, colours: 256, dither: 0.6 }).toBuffer();
    } catch (e) {
      console.warn(`${tag} could not quantise ${name}-${theme}: ${e.message}`);
    }
  }
  writeFileSync(file, out);
  console.log(`${tag} ${name}-${theme}.png  ${(out.length / 1024).toFixed(0)} KB`);
}

/* ------------------------------------------------------------------ helpers */

/**
 * Make the viewport as tall as the page, so nothing scrolls (no sticky header in the middle of a crop) and
 * the page is shown whole, like a window big enough for its content.
 */
async function fit(page) {
  const width = page.viewportSize().width;
  for (let i = 0; i < 2; i++) {
    await page.setViewportSize({ width, height: 400 });
    const height = await page.evaluate(() => Math.ceil(document.documentElement.scrollHeight));
    await page.setViewportSize({ width, height });
    await page.waitForTimeout(150);
  }
}

/** Screenshot of an element with some room around it. Needs fit() first: coordinates are page coordinates. */
async function shotAround(page, locator, pad = 10) {
  const box = await locator.first().boundingBox();
  if (!box) throw new Error('element has no box');
  const vp = page.viewportSize();
  const x = Math.max(0, box.x - pad);
  const y = Math.max(0, box.y - pad);
  const width = Math.min(vp.width - x, box.width + 2 * pad);
  const height = Math.min(vp.height - y, box.height + 2 * pad);
  return page.screenshot({ clip: { x, y, width, height } });
}

/** The card (a <section>) that holds a heading. */
const sectionWithHeading = (page, name) =>
  page.getByRole('heading', { name, exact: true }).locator('xpath=ancestor::section[1]');

/** The element that directly holds a heading (a settings card). */
const parentOfHeading = (page, name) => page.getByRole('heading', { name, exact: true }).locator('xpath=..');

/** The closest ancestor <div> of a control that also holds a label: one settings row. */
const rowOf = (page, testId) => page.getByTestId(testId).locator('xpath=ancestor::div[.//label][1]');

async function go(page, base, view) {
  await page.goto(`${base}/?nopersist`);
  await page.getByTestId('canvas').waitFor();
  await page.addStyleTag({ content: '[data-testid="demo-badge"] { display: none !important; }' });
  if (view === 'settings') {
    await page.getByTestId('nav-settings').click();
    await page.getByTestId('settings').waitFor();
  }
  // Let fonts, focus rings and layout settle.
  await page.mouse.move(2, 2);
  await page.waitForTimeout(300);
}

async function attempt(name, theme, fn) {
  try {
    await fn();
  } catch (e) {
    console.warn(`${tag} skipped ${name}-${theme}: ${String(e.message).split('\n')[0]}`);
  }
}

/* ------------------------------------------------------------------ the shots */

async function captureTheme(browser, base, theme) {
  const ctxOptions = { deviceScaleFactor: DPR, colorScheme: theme, locale: 'en-US', reducedMotion: 'reduce' };

  // ---- Editor ---------------------------------------------------------------------------------------
  {
    const ctx = await browser.newContext({ ...ctxOptions, viewport: { width: EDITOR_WIDTH, height: 400 } });
    const page = await ctx.newPage();
    await go(page, base, 'editor');

    // The Priority layout has three zones of different widths, which shows more than the default halves.
    await page.getByTestId('layout-item').filter({ hasText: 'Priority' }).click();
    await page.getByTestId('zone-2').click();
    await page.mouse.move(2, 2);
    await fit(page);

    if (want('editor')) await attempt('editor', theme, async () => save('editor', theme, await page.screenshot()));

    if (want('crop-layouts')) {
      await attempt('crop-layouts', theme, async () =>
        save('crop-layouts', theme, await shotAround(page, sectionWithHeading(page, 'Layouts on this monitor'), 1)),
      );
    }

    if (want('crop-zones')) {
      await attempt('crop-zones', theme, async () => {
        // Split tool with the guide over the middle of the wide zone: an edit in progress.
        await page.getByTestId('tool-split-vertical').click();
        const z = await page.getByTestId('zone-2').boundingBox();
        if (!z) throw new Error('zone-2 has no box');
        await page.mouse.move(z.x + z.width * 0.5, z.y + z.height * 0.5);
        await page.waitForTimeout(250);
        const canvas = page.getByTestId('canvas');
        await save('crop-zones', theme, await shotAround(page, canvas, 0));
        await page.getByTestId('tool-select').click();
      });
    }

    if (want('crop-export')) {
      await attempt('crop-export', theme, async () => {
        await page.getByTestId('export-layout').click();
        await page.getByTestId('export-panel').waitFor();
        await page.mouse.move(2, 2);
        await fit(page);
        await save('crop-export', theme, await shotAround(page, page.getByTestId('layout-transfer'), 1));
      });
    }

    if (want('crop-footer')) {
      await attempt('crop-footer', theme, async () =>
        save('crop-footer', theme, await shotAround(page, page.getByTestId('privacy'), 6)),
      );
    }

    await ctx.close();
  }

  // ---- Settings -------------------------------------------------------------------------------------
  {
    const ctx = await browser.newContext({ ...ctxOptions, viewport: { width: SETTINGS_WIDTH, height: 400 } });
    const page = await ctx.newPage();
    await go(page, base, 'settings');

    await fit(page);

    if (want('settings')) {
      await attempt('settings', theme, async () => {
        // The top of the page down to the bottom of the Appearance card, so the shot ends between two cards
        // (a window scrolled to the top, big enough for its first three cards).
        let height = SETTINGS_SHOT_HEIGHT;
        const card = await parentOfHeading(page, 'Appearance').boundingBox();
        if (card) height = Math.min(page.viewportSize().height, Math.ceil(card.y + card.height + 16));
        await save('settings', theme, await page.screenshot({ clip: { x: 0, y: 0, width: SETTINGS_WIDTH, height } }));
      });
    }

    await ctx.close();
  }

  // ---- Settings in a narrow window: crops of single controls ------------------------------------------
  {
    const ctx = await browser.newContext({ ...ctxOptions, viewport: { width: NARROW_WIDTH, height: 400 } });
    const page = await ctx.newPage();
    await go(page, base, 'settings');
    await fit(page);

    if (want('crop-keyboard')) {
      await attempt('crop-keyboard', theme, async () =>
        save('crop-keyboard', theme, await shotAround(page, page.getByTestId('shortcuts-keymap').locator('xpath=..'), 12)),
      );
    }
    if (want('crop-memory')) {
      await attempt('crop-memory', theme, async () =>
        save('crop-memory', theme, await shotAround(page, rowOf(page, 'setting-restore_on_reconnect'), 12)),
      );
    }

    await ctx.close();
  }
}

const base = await startUi();
console.log(`${tag} UI at ${base}; output ${outDir}`);

const requested = process.env.PW_CHANNEL ?? 'msedge';
const browser = await chromium.launch({ channel: requested === 'chromium' ? undefined : requested });

try {
  for (const theme of THEMES) await captureTheme(browser, base, theme);
} finally {
  await browser.close();
  if (server) server.kill();
}
console.log(`${tag} done`);
