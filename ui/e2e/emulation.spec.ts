// Media-feature emulation: prefers-reduced-motion, prefers-color-scheme, forced-colors.

import type { Locator, Page } from '@playwright/test';
import { editorUi, expect, focusRingVisible, openApp, openArrange, openSection, test } from './fixtures';

const LIGHT = {
  bg: 'rgb(246, 245, 242)', // --bg #f6f5f2
  surface: 'rgb(255, 255, 255)', // --surface #ffffff
  text: 'rgb(20, 20, 19)', // --text #141413
  canvas: 'rgb(235, 232, 226)', // --canvas-bg #ebe8e2
  focus: 'rgb(217, 72, 15)', // --focus #d9480f
};
const DARK = {
  bg: 'rgb(17, 17, 16)', // #111110
  surface: 'rgb(28, 28, 27)', // #1c1c1b
  text: 'rgb(242, 241, 238)', // #f2f1ee
  canvas: 'rgb(13, 13, 12)', // #0d0d0c
  focus: 'rgb(255, 138, 92)', // #ff8a5c
};

/** Computed transition-duration list of an element (or pseudo-element), in seconds. */
async function transitionSeconds(
  page: Page,
  target: string | Locator,
  pseudo?: string,
): Promise<number[]> {
  const locator = typeof target === 'string' ? page.getByTestId(target) : target;
  return locator.evaluate((el, p) => {
    const value = getComputedStyle(el, p ?? null).transitionDuration;
    return value.split(',').map((part) => parseFloat(part));
  }, pseudo);
}

/** The sliding knob drawn next to a switch's (invisible) checkbox. */
const knob = (page: Page, testId: string): Locator =>
  page.getByTestId(testId).locator('xpath=following-sibling::*[1]').locator('.knob');

/** Focus the current sidebar button from the keyboard, so :focus-visible applies. */
async function keyboardFocusNav(page: Page): Promise<void> {
  await page.keyboard.press('Tab'); // from the clicked Layouts button into the editor
  await page.keyboard.press('Shift+Tab');
  await expect(page.getByTestId('nav-layouts')).toBeFocused();
}

test.describe('prefers-reduced-motion', () => {
  test('"reduce" turns the transitions off', async ({ page }) => {
    await page.emulateMedia({ reducedMotion: 'reduce' });
    await openApp(page);
    expect(await page.evaluate(() => matchMedia('(prefers-reduced-motion: reduce)').matches)).toBe(
      true,
    );

    // Buttons normally fade their background over 0.12 s.
    for (const id of ['tool-select', 'template-grid', 'new-layout']) {
      const durations = await transitionSeconds(page, id);
      expect(durations.length, id).toBeGreaterThan(0);
      for (const d of durations) expect(d, `${id} transition-duration`).toBeLessThan(0.001);
    }

    // So does the sliding knob of a switch.
    await page.getByTestId('nav-arrange').click();
    await expect(page.getByTestId('setting-enabled')).toBeVisible();
    const durations = await transitionSeconds(page, knob(page, 'setting-enabled'));
    expect(durations.length).toBeGreaterThan(0);
    for (const d of durations) expect(d).toBeLessThan(0.001);
  });

  test('"no-preference" keeps the short transitions', async ({ page }) => {
    await page.emulateMedia({ reducedMotion: 'no-preference' });
    await openApp(page);
    for (const d of await transitionSeconds(page, 'tool-select')) {
      expect(d, 'button transition-duration').toBeGreaterThanOrEqual(0.1);
      expect(d).toBeLessThanOrEqual(0.2);
    }
    await page.getByTestId('nav-arrange').click();
    await expect(page.getByTestId('setting-enabled')).toBeVisible();
    for (const d of await transitionSeconds(page, knob(page, 'setting-enabled'))) {
      expect(d, 'switch knob transition-duration').toBeGreaterThanOrEqual(0.1);
    }
  });

  test('the editor still works with reduced motion', async ({ page }) => {
    await page.emulateMedia({ reducedMotion: 'reduce' });
    await openApp(page);
    const ui = editorUi(page);
    await page.getByTestId('template-grid').click();
    await expect(ui.zones).toHaveCount(4);
    await ui.save.click();
    await expect(ui.status).toHaveText('Saved "Halves".');
  });

  test('the Saved indicator and the status line need no animation to be seen', async ({
    page,
  }) => {
    await page.emulateMedia({ reducedMotion: 'reduce' });
    await openSection(page, 'privacy');
    await page.getByRole('switch', { name: 'Launch at login' }).check();
    await expect(page.getByTestId('save-indicator')).toContainText('Saved');
  });
});

test.describe('prefers-color-scheme: dark', () => {
  test.use({ colorScheme: 'dark' });

  test('the editor renders with the dark palette', async ({ page }) => {
    await openApp(page);
    const ui = editorUi(page);
    await expect(page.locator('html')).toHaveCSS('color-scheme', 'light dark');
    await expect(page.locator('body')).toHaveCSS('background-color', DARK.bg);
    await expect(page.locator('body')).toHaveCSS('color', DARK.text);
    await expect(ui.canvas).toHaveCSS('background-color', DARK.canvas);
    await expect(page.getByTestId('tool-split-vertical')).toHaveCSS(
      'background-color',
      DARK.surface,
    );
    expect(await page.evaluate(() => matchMedia('(prefers-color-scheme: dark)').matches)).toBe(
      true,
    );
  });

  test('the focus ring uses the lighter dark-theme colour', async ({ page }) => {
    await openApp(page);
    await keyboardFocusNav(page);
    expect(await focusRingVisible(page)).toBe(true);
    await expect(page.getByTestId('nav-layouts')).toHaveCSS('outline-color', DARK.focus);
  });

  test('the Arrange view renders with the dark palette', async ({ page }) => {
    await openArrange(page);
    await expect(page.locator('body')).toHaveCSS('background-color', DARK.bg);
    await expect(page.getByTestId('view-arrange')).toBeVisible();
  });

  test('choosing the Light theme overrides the OS preference', async ({ page }) => {
    await openSection(page, 'appearance');
    await page.getByTestId('setting-theme-light').check();
    await expect(page.locator('body')).toHaveCSS('background-color', LIGHT.bg);
    await expect(page.locator('html')).toHaveCSS('color-scheme', 'light');
  });
});

test.describe('prefers-color-scheme: light', () => {
  test.use({ colorScheme: 'light' });

  test('the editor renders with the light palette', async ({ page }) => {
    await openApp(page);
    const ui = editorUi(page);
    await expect(page.locator('body')).toHaveCSS('background-color', LIGHT.bg);
    await expect(page.locator('body')).toHaveCSS('color', LIGHT.text);
    await expect(ui.canvas).toHaveCSS('background-color', LIGHT.canvas);
    await expect(page.getByTestId('tool-split-vertical')).toHaveCSS(
      'background-color',
      LIGHT.surface,
    );
    await keyboardFocusNav(page);
    await expect(page.getByTestId('nav-layouts')).toHaveCSS('outline-color', LIGHT.focus);
  });
});

test.describe('forced colors (Windows high contrast)', () => {
  test.use({ forcedColors: 'active' });

  test('the app stays usable and the pressed tool keeps an outline', async ({ page }) => {
    await openApp(page);
    const ui = editorUi(page);
    expect(await page.evaluate(() => matchMedia('(forced-colors: active)').matches)).toBe(true);

    await expect(ui.toolSelect).toHaveAttribute('aria-pressed', 'true');
    await expect(ui.toolSelect).toHaveCSS('outline-style', 'solid');

    await page.getByTestId('template-grid').click();
    await expect(ui.zones).toHaveCount(4);
    await page.keyboard.press('Tab');
    expect(await focusRingVisible(page)).toBe(true);
  });
});
