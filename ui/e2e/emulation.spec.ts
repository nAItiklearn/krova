// Media-feature emulation: prefers-reduced-motion, prefers-color-scheme, forced-colors.

import type { Page } from '@playwright/test';
import { editorUi, expect, focusRingVisible, openApp, openSettings, test } from './fixtures';

const LIGHT = {
  bg: 'rgb(244, 245, 248)', // --bg #f4f5f8
  surface: 'rgb(255, 255, 255)', // --surface #ffffff
  text: 'rgb(23, 26, 33)', // --text #171a21
  canvas: 'rgb(217, 221, 230)', // --canvas-bg #d9dde6
  focus: 'rgb(37, 86, 217)', // --focus #2556d9
};
const DARK = {
  bg: 'rgb(14, 17, 22)', // #0e1116
  surface: 'rgb(22, 26, 34)', // #161a22
  text: 'rgb(231, 234, 240)', // #e7eaf0
  canvas: 'rgb(10, 13, 18)', // #0a0d12
  focus: 'rgb(157, 185, 255)', // #9db9ff
};

/** Computed transition-duration list of an element (or pseudo-element), in seconds. */
async function transitionSeconds(page: Page, testId: string, pseudo?: string): Promise<number[]> {
  return page.getByTestId(testId).evaluate((el, p) => {
    const value = getComputedStyle(el, p ?? null).transitionDuration;
    return value.split(',').map((part) => parseFloat(part));
  }, pseudo);
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
    await page.getByTestId('nav-settings').click();
    await expect(page.getByTestId('setting-enabled')).toBeVisible();
    for (const d of await transitionSeconds(page, 'setting-enabled')) expect(d).toBeLessThan(0.001);
    for (const d of await transitionSeconds(page, 'setting-enabled', '::after')) {
      expect(d).toBeLessThan(0.001);
    }
  });

  test('"no-preference" keeps the short transitions', async ({ page }) => {
    await page.emulateMedia({ reducedMotion: 'no-preference' });
    await openApp(page);
    for (const d of await transitionSeconds(page, 'tool-select')) {
      expect(d, 'button transition-duration').toBeGreaterThanOrEqual(0.1);
      expect(d).toBeLessThanOrEqual(0.2);
    }
    await page.getByTestId('nav-settings').click();
    await expect(page.getByTestId('setting-enabled')).toBeVisible();
    for (const d of await transitionSeconds(page, 'setting-enabled', '::after')) {
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
    await openSettings(page);
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
    await page.keyboard.press('Tab');
    expect(await focusRingVisible(page)).toBe(true);
    await expect(page.getByTestId('nav-editor')).toHaveCSS('outline-color', DARK.focus);
  });

  test('the Settings view renders with the dark palette', async ({ page }) => {
    await openSettings(page);
    await expect(page.locator('body')).toHaveCSS('background-color', DARK.bg);
    await expect(page.getByTestId('settings')).toBeVisible();
  });

  test('choosing the Light theme overrides the OS preference', async ({ page }) => {
    await openSettings(page);
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
    await page.keyboard.press('Tab');
    await expect(page.getByTestId('nav-editor')).toHaveCSS('outline-color', LIGHT.focus);
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
