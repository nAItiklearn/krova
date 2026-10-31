// Privacy promise: "no accounts, no telemetry, nothing leaves your computer".
//
// Every test in this suite already fails when the page contacts anything but the
// local dev server (see the `guard` fixture in fixtures.ts). The tests here make
// that explicit, exercise the whole app while watching, and check the guard itself.

import { demo, editorUi, expect, isAllowedUrl, openApp, test } from './fixtures';

const BASE = 'http://localhost:1420';

test.describe('network traffic', () => {
  test('a full tour of the app only ever talks to the local dev server', async ({
    page,
    guard,
    baseURL,
  }) => {
    // The port is 1420 unless PW_PORT says otherwise (playwright.config.ts).
    const base = baseURL ?? BASE;
    await openApp(page, '?nopersist&permission=denied');
    const ui = editorUi(page);

    // Editor: templates, a split, a divider move, a delete, undo, save, layouts, monitors.
    await page.getByTestId('template-grid').click();
    await ui.toolSplitVertical.click();
    await ui.clickAt(0.3, 0.25);
    await page.keyboard.press('Escape');
    await ui.verticalDividers.first().focus();
    await page.keyboard.press('ArrowRight');
    await ui.zone(2).click();
    await ui.deleteZone.click();
    await ui.undo.click();
    await ui.save.click();
    await ui.newLayout.click();
    await ui.tabs.nth(1).click();
    await ui.layoutItem('Priority').click();
    await ui.setActive.click();
    await expect(ui.status).toContainText('is now the active layout');

    // Settings: change every kind of control, wait until it is saved.
    await page.getByTestId('nav-settings').click();
    await page.getByRole('switch', { name: 'Launch at login' }).check();
    await page.getByTestId('setting-modifier-ctrl').check();
    await page.getByTestId('setting-highlight_color').fill('#ff00aa');
    await page.getByTestId('setting-theme-dark').check();
    await expect(page.getByTestId('save-indicator')).toContainText('Saved');

    // Permission banner and demo hooks.
    await page.getByTestId('permission-recheck').click();
    await demo(page, 'setPermission', 'granted');
    await demo(page, 'toggleEnabled');
    await demo(page, 'navigate', 'editor');
    await expect(ui.canvas).toBeVisible();

    // The listener really works: the page document itself was seen.
    expect(guard.urls.length).toBeGreaterThan(0);
    expect(guard.urls).toContain(`${base}/?nopersist&permission=denied`);

    // Exactly one host was contacted, and nothing foreign.
    const hosts = new Set(
      guard.urls
        .map((u) => new URL(u))
        .filter((u) => !['data:', 'blob:', 'about:'].includes(u.protocol))
        .map((u) => u.host),
    );
    expect([...hosts]).toEqual([new URL(base).host]);
    expect(guard.foreign()).toEqual([]);
  });

  test('the page makes no request of its own to another origin on load', async ({
    page,
    guard,
    baseURL,
  }) => {
    await openApp(page);
    await page.waitForLoadState('networkidle');
    expect(guard.foreign()).toEqual([]);
    for (const url of guard.urls) {
      expect(isAllowedUrl(url, baseURL ?? BASE), url).toBe(true);
    }
  });

  test('the document references no external script, stylesheet, image or link', async ({
    page,
  }) => {
    await openApp(page);
    const external = await page.evaluate(() => {
      const refs: string[] = [];
      for (const el of document.querySelectorAll('[src], [href], [action], [poster], [srcset]')) {
        for (const attr of ['src', 'href', 'action', 'poster']) {
          const value = el.getAttribute(attr);
          if (value) refs.push(new URL(value, document.baseURI).href);
        }
      }
      return refs.filter((href) => {
        const url = new URL(href);
        return !['data:', 'blob:', 'about:'].includes(url.protocol) && url.origin !== location.origin;
      });
    });
    expect(external).toEqual([]);
  });

  test('the Content-Security-Policy only allows the page itself (and Tauri IPC)', async ({
    page,
  }) => {
    await openApp(page);
    const csp = await page
      .locator('meta[http-equiv="Content-Security-Policy"]')
      .getAttribute('content');
    expect(csp).not.toBeNull();
    expect(csp).toContain("default-src 'self'");
    expect(csp).toContain("connect-src 'self' ipc: http://ipc.localhost");
    // No wildcard and no remote host anywhere in the policy.
    expect(csp).not.toMatch(/\*|https?:\/\/(?!ipc\.localhost)/);
  });

});

test.describe('Content-Security-Policy as a second line of defence', () => {
  // The browser logs the violation on purpose, so the console guard is off here.
  test.use({ strictGuards: false });

  test('the browser itself refuses a script-initiated request to another origin', async ({
    page,
  }) => {
    await openApp(page);
    const outcome = await page.evaluate(
      () =>
        new Promise<{ result: string; directive: string }>((resolve) => {
          // Resolve once both the fetch outcome and the CSP report are known.
          let result = '';
          let directive = '';
          const done = (): void => {
            if (result && directive) resolve({ result, directive });
          };
          document.addEventListener('securitypolicyviolation', (e) => {
            directive = e.effectiveDirective;
            done();
          });
          fetch('https://example.com/').then(
            () => {
              result = 'allowed';
              done();
            },
            () => {
              result = 'blocked';
              done();
            },
          );
        }),
    );
    expect(outcome).toEqual({ result: 'blocked', directive: 'connect-src' });
  });
});

test.describe('the guard itself', () => {
  test.use({ strictGuards: false });

  test('flags a request to another origin from any page of the context', async ({
    context,
    page,
    guard,
  }) => {
    await openApp(page);
    expect(guard.foreign()).toEqual([]);

    // A second page in the same browser context, with no CSP of its own.
    const other = await context.newPage();
    await other.route('http://example.invalid/**', (route) =>
      route.fulfill({ status: 200, body: 'x', headers: { 'access-control-allow-origin': '*' } }),
    );
    await other.goto('about:blank');
    await other.evaluate(() => fetch('http://example.invalid/leak').then((r) => r.text()));

    expect(guard.foreign()).toEqual(['http://example.invalid/leak']);
  });

  test('records console errors and uncaught exceptions', async ({ page, guard }) => {
    await openApp(page);
    expect(guard.consoleErrors).toEqual([]);
    expect(guard.pageErrors).toEqual([]);

    await page.evaluate(() => {
      console.error('console boom');
      setTimeout(() => {
        throw new Error('uncaught boom');
      }, 0);
    });

    await expect.poll(() => guard.pageErrors).toEqual(['uncaught boom']);
    expect(guard.consoleErrors.join('\n')).toContain('console boom');
  });

  test('isAllowedUrl accepts only the dev server origin, its socket, and non-network URLs', () => {
    expect(isAllowedUrl(`${BASE}/src/main.ts`, BASE)).toBe(true);
    expect(isAllowedUrl(`${BASE}/?nopersist`, BASE)).toBe(true);
    expect(isAllowedUrl('ws://localhost:1420/?token=abc', BASE)).toBe(true);
    expect(isAllowedUrl('data:,', BASE)).toBe(true);
    expect(isAllowedUrl('blob:http://localhost:1420/1234', BASE)).toBe(true);
    expect(isAllowedUrl('about:blank', BASE)).toBe(true);

    expect(isAllowedUrl('https://example.com/', BASE)).toBe(false);
    expect(isAllowedUrl('http://localhost:1421/', BASE)).toBe(false);
    expect(isAllowedUrl('http://127.0.0.1:1420/', BASE)).toBe(false);
    expect(isAllowedUrl('https://localhost:1420/', BASE)).toBe(false);
    expect(isAllowedUrl('wss://localhost:1420/', BASE)).toBe(false);
    expect(isAllowedUrl('ws://evil.example/', BASE)).toBe(false);
    expect(isAllowedUrl('not a url', BASE)).toBe(false);
  });
});
