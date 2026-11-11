// Captures the real Krova UI (../ui, demo mode, in-memory mock backend) into website/public/screenshots/.
// The home page shows these files, so to refresh them after the desktop UI changes, run this one script:
//
//   npm run screenshots                       (from website/)
//   npm run screenshots -- --only app-panel,app-switcher
//   npm run screenshots -- --static           (build the demo once and serve it; steadier on a busy PC)
//
// What it does
//   1. Starts `vite --mode demo` from ../ui on a free local port (or uses KROVA_SHOT_URL if you already run one).
//      With --static (or KROVA_SHOT_STATIC=1) it runs `vite build --mode demo` into a temporary folder outside
//      ../ui and serves that with `vite preview` instead: no on-demand compiling, no reloads.
//   2. Opens it in Chromium at 2x device pixel ratio, once in the light theme and once in the dark theme.
//   3. Saves the main window (sidebar + Overview), crops of single sidebar views (Arrange, Workspaces, Focus,
//      Shortcuts, Corners & notch, Appearance), the cheat sheet, and the three popup pages: the quick panel
//      (panel.html), the layout switcher (switcher.html) and the hover zone picker (picker.html).
//   4. Writes PNGs named <shot>-light.png and <shot>-dark.png, quantised to a small palette with `sharp`
//      (a dependency of Astro; if it is missing the raw PNG is kept).
//
// The page never leaves localhost. The only change made to the UI is hiding the "Demo mode" note, which exists
// in the browser demo only; the real app has no such note. The monitors, layouts, app rules and workspace apps
// are the mock backend's samples, so captions on the site say "sample".
//
// What cannot be captured here: everything Krova draws natively on the desktop (the notch pill, the drag zone
// overlay, stack tab strips, focus dim, highlight rings, the hot-corner ripple). The site describes those in
// words or with its own diagrams, never with a made-up screenshot.
//
// Browser: Microsoft Edge by default (ships with Windows, nothing to download). Override with
//   PW_CHANNEL=chrome        installed Google Chrome
//   PW_CHANNEL=chromium      Playwright's own Chromium (needs `npx playwright install chromium` in ../ui)
//
// The crops locate things by role, visible heading text and data-testid (the same ids the UI's e2e tests use),
// so a restyle of the UI does not break them. A shot that cannot be found is skipped with a warning and the
// previous file stays in place.
import { spawn } from 'node:child_process';
import { existsSync, mkdirSync, mkdtempSync, rmSync, writeFileSync } from 'node:fs';
import { createRequire } from 'node:module';
import { tmpdir } from 'node:os';
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
const useStatic = args.includes('--static') || process.env.KROVA_SHOT_STATIC === '1';

const DPR = 2;
const THEMES = ['light', 'dark'];
const WINDOW = { width: 1200, height: 780 }; // the main window as a user would size it
const TALL = 2600; // for crops: a viewport taller than any view, so nothing scrolls inside the window
const SWITCHER = { width: 1000, height: 640 };

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

/* ------------------------------------------------------------------ UI server */

let server = null;
let staticDir = null;

async function waitFor(url, ms) {
  const end = Date.now() + ms;
  while (Date.now() < end) {
    try {
      const res = await fetch(url, { signal: AbortSignal.timeout(5000) });
      if (res.ok) return true;
    } catch {
      /* not up yet */
    }
    await new Promise((r) => setTimeout(r, 250));
  }
  return false;
}

function run(cmdArgs) {
  return new Promise((ok, no) => {
    const p = spawn(process.execPath, cmdArgs, { cwd: ui, stdio: ['ignore', 'ignore', 'inherit'] });
    p.on('exit', (code) => (code ? no(new Error(`exit code ${code}`)) : ok()));
  });
}

async function startUi() {
  if (process.env.KROVA_SHOT_URL) return process.env.KROVA_SHOT_URL.replace(/\/+$/, '');
  const port = Number(process.env.KROVA_SHOT_PORT ?? 5287);
  const base = `http://127.0.0.1:${port}`;
  const vite = join(ui, 'node_modules', 'vite', 'bin', 'vite.js');
  if (!existsSync(vite)) fail('Vite is not installed in ../ui. Run: npm ci --prefix ../ui');
  const common = ['--host', '127.0.0.1', '--port', String(port), '--strictPort'];
  if (useStatic) {
    staticDir = mkdtempSync(join(tmpdir(), 'krova-demo-'));
    console.log(`${tag} building the demo into ${staticDir}`);
    try {
      await run([vite, 'build', '--mode', 'demo', '--outDir', staticDir, '--emptyOutDir']);
    } catch (e) {
      fail(`the demo build failed (${e.message})`);
    }
    server = spawn(process.execPath, [vite, 'preview', '--outDir', staticDir, ...common], {
      cwd: ui,
      stdio: ['ignore', 'ignore', 'inherit'],
    });
  } else {
    server = spawn(process.execPath, [vite, '--mode', 'demo', ...common], { cwd: ui, stdio: ['ignore', 'ignore', 'inherit'] });
  }
  server.on('exit', (code) => {
    if (code) console.error(`${tag} the UI server exited with code ${code}`);
  });
  if (!(await waitFor(base + '/', 120_000))) fail(`the UI server did not answer on ${base} (is the port in use? set KROVA_SHOT_PORT)`);
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

/** Screenshot of the union of some elements' boxes, with room around it. Page coordinates (nothing scrolls). */
async function shotAround(page, locators, pad = 12) {
  const list = Array.isArray(locators) ? locators : [locators];
  const boxes = [];
  for (const l of list) {
    const b = await l.first().boundingBox();
    if (!b) throw new Error('element has no box');
    boxes.push(b);
  }
  const vp = page.viewportSize();
  const x0 = Math.max(0, Math.min(...boxes.map((b) => b.x)) - pad);
  const y0 = Math.max(0, Math.min(...boxes.map((b) => b.y)) - pad);
  const x1 = Math.min(vp.width, Math.max(...boxes.map((b) => b.x + b.width)) + pad);
  const y1 = Math.min(vp.height, Math.max(...boxes.map((b) => b.y + b.height)) + pad);
  return page.screenshot({ clip: { x: x0, y: y0, width: x1 - x0, height: y1 - y0 } });
}

const HIDE_DEMO = '[data-testid="demo-badge"] { display: none !important; }';

/** Settle: fonts, springs and focus rings. Reduced motion is on, so this is short. */
async function settle(page, ms = 400) {
  await page.evaluate(() => document.fonts?.ready);
  await page.mouse.move(2, 2);
  await page.waitForTimeout(ms);
}

async function openMain(page, base, section) {
  await page.goto(`${base}/?nopersist`);
  await page.getByTestId('view-overview').waitFor();
  await page.addStyleTag({ content: HIDE_DEMO });
  if (section !== 'overview') {
    await page.getByTestId(`nav-${section}`).click();
    await page.getByTestId(section === 'layouts' ? 'canvas' : `view-${section}`).waitFor();
  }
  await settle(page);
}

async function attempt(name, theme, fn) {
  if (!want(name)) return;
  try {
    await fn();
  } catch (e) {
    console.warn(`${tag} skipped ${name}-${theme}: ${String(e.message).split('\n')[0]}`);
  }
}

/** The <section class="group"> that holds a group heading. */
const group = (page, title) => page.getByRole('heading', { name: title, exact: true }).locator('xpath=ancestor::section[1]');

/* ------------------------------------------------------------------ the shots */

async function captureTheme(browser, base, theme) {
  const ctxOptions = { deviceScaleFactor: DPR, colorScheme: theme, locale: 'en-US', reducedMotion: 'reduce' };

  // ---- The main window at a normal size: sidebar + Overview ------------------------------------------
  {
    const ctx = await browser.newContext({ ...ctxOptions, viewport: WINDOW });
    const page = await ctx.newPage();
    await openMain(page, base, 'overview');
    await attempt('app-overview', theme, async () => save('app-overview', theme, await page.screenshot()));
    await ctx.close();
  }

  // ---- Single views, cropped (tall window: nothing scrolls) -------------------------------------------
  {
    const ctx = await browser.newContext({ ...ctxOptions, viewport: { width: WINDOW.width, height: TALL } });
    const page = await ctx.newPage();

    await openMain(page, base, 'arrange');
    await attempt('app-arrange', theme, async () =>
      save('app-arrange', theme, await shotAround(page, [group(page, 'Auto-arrange')], 14)),
    );
    await attempt('app-rules', theme, async () =>
      save('app-rules', theme, await shotAround(page, page.getByTestId('app-rules'), 14)),
    );

    await openMain(page, base, 'workspaces');
    await attempt('app-workspaces', theme, async () => {
      // Save one workspace from the sample windows, accepting the one-time path notice.
      await page.getByTestId('ws-empty').getByTestId('ws-capture').click();
      await page.getByTestId('ws-privacy-ok').click();
      await page.getByTestId('ws-card').first().waitFor();
      await settle(page);
      await save('app-workspaces', theme, await shotAround(page, [page.getByTestId('ws-card'), page.getByTestId('ws-privacy')], 14));
    });

    await openMain(page, base, 'focus');
    await attempt('app-focus', theme, async () =>
      save('app-focus', theme, await shotAround(page, [page.getByTestId('focus-dim'), page.getByTestId('focus-calm'), page.getByTestId('focus-share')], 14)),
    );

    await openMain(page, base, 'shortcuts');
    await attempt('app-shortcuts', theme, async () =>
      // The keyboard picture and the first two groups of the sheet (the rest is the same pattern).
      save('app-shortcuts', theme, await shotAround(page, [page.locator('.kb-wrap'), group(page, 'Windows'), group(page, 'Layouts')], 14)),
    );
    await attempt('app-cheatsheet', theme, async () => {
      await page.getByTestId('open-cheat-sheet').click();
      const sheet = page.getByRole('dialog', { name: 'Krova shortcuts' });
      await sheet.waitFor();
      await settle(page);
      await save('app-cheatsheet', theme, await shotAround(page, sheet, 16));
      await page.keyboard.press('Escape');
    });

    await openMain(page, base, 'corners');
    await attempt('app-corners', theme, async () => {
      const preview = page.getByRole('complementary', { name: 'Preview of the panel the notch opens' }).locator('xpath=..');
      await save('app-corners', theme, await shotAround(page, preview, 14));
    });

    await openMain(page, base, 'appearance');
    await attempt('app-appearance', theme, async () => {
      const desk = page.getByTestId('view-appearance').locator('.desk').first();
      await save('app-appearance', theme, await shotAround(page, [desk, group(page, 'Accent'), group(page, 'Windows')], 14));
    });

    await ctx.close();
  }

  // ---- The quick panel (panel.html): the demo "clicks the notch" as soon as it listens -----------------
  {
    const ctx = await browser.newContext({ ...ctxOptions, viewport: { width: 760, height: 620 } });
    const page = await ctx.newPage();
    await attempt('app-panel', theme, async () => {
      await page.goto(`${base}/panel.html?nopersist`);
      const panel = page.getByTestId('panel');
      await panel.waitFor();
      await page.waitForFunction(() => document.querySelector('[data-testid="panel"]')?.getAttribute('data-state') === 'open');
      await settle(page, 700);
      // No focus ring on the first switch in a still picture.
      await page.evaluate(() => (document.activeElement instanceof HTMLElement ? document.activeElement.blur() : null));
      await page.waitForTimeout(150);
      await save('app-panel', theme, await shotAround(page, panel, 28));
    });
    await ctx.close();
  }

  // ---- The layout switcher (switcher.html), whole window ----------------------------------------------
  {
    const ctx = await browser.newContext({ ...ctxOptions, viewport: SWITCHER });
    const page = await ctx.newPage();
    await attempt('app-switcher', theme, async () => {
      await page.goto(`${base}/switcher.html?nopersist`);
      await page.waitForFunction(() => document.querySelector('[data-testid="switcher"]')?.getAttribute('data-open') === 'true');
      await settle(page, 900);
      await save('app-switcher', theme, await page.screenshot());
    });
    await ctx.close();
  }

  // ---- The hover zone picker (picker.html): its demo button shows it as the backend would ---------------
  {
    const ctx = await browser.newContext({ ...ctxOptions, viewport: { width: 900, height: 620 } });
    const page = await ctx.newPage();
    await attempt('app-picker', theme, async () => {
      await page.goto(`${base}/picker.html?nopersist`);
      await page.getByTestId('picker-simulate').click();
      const card = page.getByTestId('picker-card');
      await card.waitFor();
      await settle(page, 500);
      await page.getByTestId('picker-zone-2').hover();
      await page.waitForTimeout(250);
      await save('app-picker', theme, await shotAround(page, card, 40));
    });
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
  if (staticDir) rmSync(staticDir, { recursive: true, force: true });
}
console.log(`${tag} done`);
