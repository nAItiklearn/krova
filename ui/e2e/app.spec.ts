// App shell: first load, monitor tabs, navigation, permission banner, demo hooks.

import {
  SECTIONS,
  demo,
  editorUi,
  expect,
  gotoSection,
  openApp,
  openArrange,
  openHome,
  sectionContent,
  test,
} from './fixtures';

test.describe('first load', () => {
  test('shows the demo badge, both monitors and logs no console errors', async ({
    page,
    guard,
  }) => {
    await openApp(page);

    await expect(page).toHaveTitle('Krova');
    await expect(page.getByTestId('demo-badge')).toContainText(
      'Demo mode - changes are not saved to a real config',
    );
    await expect(page.getByRole('note')).toBeVisible();

    const { tabs } = editorUi(page);
    await expect(tabs).toHaveCount(2);
    await expect(tabs.nth(0)).toContainText('Display 1 (primary)');
    await expect(tabs.nth(0)).toContainText('2560 × 1440');
    await expect(tabs.nth(1)).toContainText('Display 2');
    await expect(tabs.nth(1)).toContainText('1920 × 1080');
    await expect(tabs.nth(0)).toHaveAttribute('aria-selected', 'true');
    await expect(tabs.nth(1)).toHaveAttribute('aria-selected', 'false');

    await expect(page.getByTestId('version')).toHaveText(/^Krova v\d+\.\d+\.\d+/);
    await expect(page.getByTestId('privacy')).toContainText(/no telemetry/i);

    // No banner when the (fake) permission is not required.
    await expect(page.getByTestId('permission-banner')).toHaveCount(0);

    // Visit every section before judging the console.
    for (const section of SECTIONS) await gotoSection(page, section);
    await gotoSection(page, 'layouts');

    expect(guard.consoleErrors, 'console errors').toEqual([]);
    expect(guard.pageErrors, 'uncaught page errors').toEqual([]);
  });

  test('starts on Display 1 with its active layout (Halves)', async ({ page }) => {
    await openApp(page);
    const ui = editorUi(page);

    await expect(ui.canvas).toHaveAccessibleName('Layout Halves on Display 1 (primary)');
    await expect(ui.zones).toHaveCount(2);
    await expect(ui.zone(1)).toHaveAccessibleName('Zone 1, 1280 × 1400, 50% × 100%');
    await expect(ui.zone(1)).toHaveAccessibleDescription(
      '1280 by 1400 pixels, 50% wide by 100% tall.',
    );
    await expect(ui.zone(2)).toHaveAccessibleName('Zone 2, 1280 × 1400, 50% × 100%');
    await expect(ui.caption).toHaveText(
      'Display 1 (primary): work area 2560 × 1400 px (screen 2560 × 1440). 2 zones.',
    );

    // One divider between the halves, nothing to save yet.
    await expect(ui.dividers).toHaveCount(1);
    await expect(ui.dividers).toHaveAttribute('data-axis', 'vertical');
    await expect(ui.dividers).toHaveAttribute('data-pos', '0.5');
    await expect(ui.save).toBeDisabled();
    await expect(ui.undo).toBeDisabled();
    await expect(ui.revert).toBeDisabled();
    await expect(ui.deleteZone).toBeDisabled();
    await expect(ui.toolSelect).toHaveAttribute('aria-pressed', 'true');
  });
});

test.describe('monitor tabs', () => {
  test('selecting the second monitor shows its layout and aspect ratio', async ({ page }) => {
    await openApp(page);
    const ui = editorUi(page);

    // Display 1 work area is 2560 x 1400, Display 2 is 1920 x 1080.
    await expect.poll(() => ui.ratio()).toBeCloseTo(2560 / 1400, 2);

    await ui.tabs.nth(1).click();

    await expect(ui.tabs.nth(1)).toHaveAttribute('aria-selected', 'true');
    await expect(ui.tabs.nth(0)).toHaveAttribute('aria-selected', 'false');
    await expect(ui.canvas).toHaveAccessibleName('Layout Thirds on Display 2');
    await expect.poll(() => ui.ratio()).toBeCloseTo(1920 / 1080, 2);
    await expect(ui.zones).toHaveCount(3);
    await expect(ui.caption).toHaveText(
      'Display 2: work area 1920 × 1080 px (screen 1920 × 1080). 3 zones.',
    );
    await expect(ui.dividers).toHaveCount(2);

    await ui.tabs.nth(0).click();
    await expect(ui.canvas).toHaveAccessibleName('Layout Halves on Display 1 (primary)');
    await expect.poll(() => ui.ratio()).toBeCloseTo(2560 / 1400, 2);
  });

  test('each monitor keeps its own layouts and active layout', async ({ page }) => {
    await openApp(page);
    const ui = editorUi(page);

    await expect(ui.layoutItems).toHaveCount(4);
    await expect(ui.layoutItem('Halves')).toContainText('Active');
    await expect(ui.layoutItem('Halves')).toHaveAttribute('aria-current', 'true');

    await ui.tabs.nth(1).click();
    await expect(ui.layoutItems).toHaveCount(4);
    await expect(ui.layoutItem('Thirds')).toContainText('Active');
    await expect(ui.layoutItem('Thirds')).toHaveAttribute('aria-current', 'true');
    await expect(ui.layoutItem('Halves')).not.toContainText('Active');
  });

  test('arrow keys, Home and End move between the tabs', async ({ page }) => {
    await openApp(page);
    const { tabs, canvas } = editorUi(page);

    await tabs.nth(0).focus();
    await page.keyboard.press('ArrowRight');
    await expect(tabs.nth(1)).toHaveAttribute('aria-selected', 'true');
    await expect(tabs.nth(1)).toBeFocused();
    await expect(canvas).toHaveAccessibleName('Layout Thirds on Display 2');

    await page.keyboard.press('ArrowRight'); // wraps around
    await expect(tabs.nth(0)).toHaveAttribute('aria-selected', 'true');
    await expect(tabs.nth(0)).toBeFocused();

    await page.keyboard.press('End');
    await expect(tabs.nth(1)).toHaveAttribute('aria-selected', 'true');
    await expect(tabs.nth(1)).toBeFocused();

    await page.keyboard.press('Home');
    await expect(tabs.nth(0)).toHaveAttribute('aria-selected', 'true');
    await expect(tabs.nth(0)).toBeFocused();

    await page.keyboard.press('ArrowLeft'); // wraps backwards
    await expect(tabs.nth(1)).toHaveAttribute('aria-selected', 'true');
  });

  test('only the selected tab is in the Tab order (roving tabindex)', async ({ page }) => {
    await openApp(page);
    const { tabs } = editorUi(page);
    await expect(tabs.nth(0)).toHaveAttribute('tabindex', '0');
    await expect(tabs.nth(1)).toHaveAttribute('tabindex', '-1');
    await tabs.nth(1).click();
    await expect(tabs.nth(1)).toHaveAttribute('tabindex', '0');
    await expect(tabs.nth(0)).toHaveAttribute('tabindex', '-1');
  });
});

test.describe('navigation', () => {
  test('the app opens on the Overview', async ({ page }) => {
    await openHome(page);
    await expect(page.getByTestId('nav-overview')).toHaveAttribute('aria-current', 'page');
    await expect(page.getByTestId('canvas')).toHaveCount(0);
  });

  test('each sidebar button opens its section and only that one', async ({ page }) => {
    await openHome(page);
    for (const section of SECTIONS) {
      const nav = page.getByTestId(`nav-${section}`);
      await nav.click();
      await expect(sectionContent(page, section)).toBeVisible();
      await expect(nav).toHaveAttribute('aria-current', 'page');
      await expect(page.locator('nav [aria-current="page"]')).toHaveCount(1);
      for (const other of SECTIONS) {
        if (other !== section) await expect(sectionContent(page, other)).toHaveCount(0);
      }
    }
  });

  test('each section has its page heading', async ({ page }) => {
    await openArrange(page);
    await expect(page.getByRole('heading', { name: 'Arrange', level: 2 })).toBeVisible();
    await gotoSection(page, 'privacy');
    await expect(page.getByRole('heading', { name: 'Privacy & about', level: 2 })).toBeVisible();
  });

  test('Up and Down arrows, Home and End move between the sidebar buttons', async ({ page }) => {
    await openHome(page);
    const nav = (section: string) => page.getByTestId(`nav-${section}`);

    await nav('overview').focus();
    await page.keyboard.press('ArrowDown');
    await expect(nav(SECTIONS[1])).toBeFocused();
    await expect(sectionContent(page, SECTIONS[1])).toBeVisible();

    await page.keyboard.press('ArrowUp');
    await expect(nav('overview')).toBeFocused();
    await expect(sectionContent(page, 'overview')).toBeVisible();

    await page.keyboard.press('ArrowUp'); // wraps around
    await expect(nav(SECTIONS[SECTIONS.length - 1])).toBeFocused();

    await page.keyboard.press('Home');
    await expect(nav(SECTIONS[0])).toBeFocused();
    await page.keyboard.press('End');
    await expect(nav(SECTIONS[SECTIONS.length - 1])).toBeFocused();
    await expect(sectionContent(page, SECTIONS[SECTIONS.length - 1])).toBeVisible();
  });

  test('only the current sidebar button is in the Tab order (roving tabindex)', async ({ page }) => {
    await openHome(page);
    await expect(page.getByTestId('nav-overview')).toHaveAttribute('tabindex', '0');
    await expect(page.getByTestId('nav-arrange')).toHaveAttribute('tabindex', '-1');
    await gotoSection(page, 'arrange');
    await expect(page.getByTestId('nav-arrange')).toHaveAttribute('tabindex', '0');
    await expect(page.getByTestId('nav-overview')).toHaveAttribute('tabindex', '-1');
  });

  test('the "navigate" demo hook switches views like the tray menu', async ({ page }) => {
    await openApp(page);

    // "settings" from the tray opens the Overview, "editor" opens Layouts.
    await demo(page, 'navigate', 'settings');
    await expect(page.getByTestId('view-overview')).toBeVisible();
    await expect(page.getByTestId('nav-overview')).toHaveAttribute('aria-current', 'page');

    await demo(page, 'navigate', 'editor');
    await expect(page.getByTestId('canvas')).toBeVisible();
    await expect(page.getByTestId('nav-layouts')).toHaveAttribute('aria-current', 'page');

    // An unknown view is ignored.
    await demo(page, 'navigate', 'bogus');
    await expect(page.getByTestId('canvas')).toBeVisible();
    await expect(page.getByTestId('nav-layouts')).toHaveAttribute('aria-current', 'page');
  });
});

test.describe('Accessibility permission banner', () => {
  test('is not shown by default', async ({ page }) => {
    await openApp(page);
    await expect(page.getByTestId('permission-banner')).toHaveCount(0);
    await expect(page.getByRole('alert')).toHaveCount(0);
  });

  test('?permission=denied shows the banner with instructions', async ({ page }) => {
    await openApp(page, '?nopersist&permission=denied');

    const banner = page.getByTestId('permission-banner');
    await expect(banner).toBeVisible();
    await expect(banner).toHaveAttribute('role', 'alert');
    await expect(banner).toContainText('Krova needs Accessibility access to move windows.');
    await expect(banner).toContainText('Privacy & Security');
    await expect(page.getByRole('alert')).toBeVisible();

    // The banner stays on every view.
    await gotoSection(page, 'arrange');
    await expect(banner).toBeVisible();
    await gotoSection(page, 'overview');
    await expect(banner).toBeVisible();
  });

  test('"Check again" re-reads the state and keeps the banner while access is denied', async ({
    page,
  }) => {
    await openApp(page, '?nopersist&permission=denied');
    const recheck = page.getByTestId('permission-recheck');
    await expect(recheck).toBeEnabled();
    await recheck.click();
    await expect(recheck).toBeEnabled();
    await expect(page.getByTestId('permission-banner')).toBeVisible();
  });

  for (const permission of ['granted', 'not_required']) {
    test(`?permission=${permission} shows no banner`, async ({ page }) => {
      await openApp(page, `?nopersist&permission=${permission}`);
      await expect(page.getByTestId('permission-banner')).toHaveCount(0);
    });
  }

  test('the setPermission hook shows and hides the banner live', async ({ page }) => {
    await openApp(page);
    const banner = page.getByTestId('permission-banner');

    await demo(page, 'setPermission', 'denied');
    await expect(banner).toBeVisible();

    await demo(page, 'setPermission', 'granted');
    await expect(banner).toHaveCount(0);

    await demo(page, 'setPermission', 'denied');
    await expect(banner).toBeVisible();

    await demo(page, 'setPermission', 'not_required');
    await expect(banner).toHaveCount(0);
  });
});

test.describe('demo hooks and persistence', () => {
  test('toggleEnabled behaves like the tray item and the Arrange switch follows', async ({
    page,
  }) => {
    await openArrange(page);
    const enabled = page.getByTestId('setting-enabled');
    await expect(enabled).toBeChecked();

    await demo(page, 'toggleEnabled');
    await expect(enabled).not.toBeChecked();

    await demo(page, 'toggleEnabled');
    await expect(enabled).toBeChecked();
  });

  test('saved edits survive a reload; the demo "Reset" button forgets them', async ({ page }) => {
    // ?reset starts from a clean slate; the later plain visits read localStorage.
    await openApp(page, '?reset');
    const ui = editorUi(page);

    await page.getByTestId('template-grid').click();
    await expect(ui.zones).toHaveCount(4);
    await ui.save.click();
    await expect(ui.status).toHaveText('Saved "Halves".');

    await page.goto('/');
    await gotoSection(page, 'layouts');
    await expect(ui.zones).toHaveCount(4);
    await expect(ui.caption).toContainText('4 zones');

    const reloaded = page.waitForEvent('load');
    await page.getByRole('button', { name: 'Reset' }).click();
    await reloaded;
    await gotoSection(page, 'layouts');
    await expect(ui.zones).toHaveCount(2);
    expect(await page.evaluate(() => localStorage.getItem('krova-demo'))).toBeNull();
  });

  test('?nopersist keeps edits in memory only', async ({ page }) => {
    await openApp(page, '?nopersist');
    const ui = editorUi(page);
    await page.getByTestId('template-grid').click();
    await ui.save.click();
    await expect(ui.status).toHaveText('Saved "Halves".');
    expect(await page.evaluate(() => localStorage.getItem('krova-demo'))).toBeNull();

    await page.goto('/?nopersist');
    await gotoSection(page, 'layouts');
    await expect(ui.zones).toHaveCount(2);
  });

  test('the reset hook returns the saved state to the start', async ({ page }) => {
    await openApp(page);
    const ui = editorUi(page);
    await page.getByTestId('template-grid').click();
    await ui.save.click();
    await expect(ui.zones).toHaveCount(4);
    await expect(ui.save).toBeDisabled();

    await demo(page, 'reset');
    await expect(ui.zones).toHaveCount(2);
  });
});
