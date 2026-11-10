// Settings view: every control, autosave with the Saved indicator, colour
// validation, theme switching.

import { expect, openSettings, test } from './fixtures';
import type { Page } from '@playwright/test';

const LIGHT_BG = 'rgb(244, 245, 248)'; // --bg #f4f5f8
const DARK_BG = 'rgb(14, 17, 22)'; // --bg #0e1116

const indicator = (page: Page) => page.getByTestId('save-indicator');

async function expectSaved(page: Page): Promise<void> {
  await expect(indicator(page)).toHaveAttribute('data-state', 'saved');
  await expect(indicator(page)).toContainText('Saved');
}

test.describe('defaults', () => {
  test('every control shows its default value', async ({ page }) => {
    await openSettings(page);

    await expect(page.getByRole('heading', { name: 'Settings', level: 2 })).toBeVisible();
    await expect(page.getByText('Changes are saved automatically.')).toBeVisible();

    await expect(page.getByTestId('setting-enabled')).toBeChecked();
    await expect(page.getByTestId('setting-launch_at_login')).not.toBeChecked();
    await expect(page.getByTestId('setting-modifier-shift')).toBeChecked();
    await expect(page.getByTestId('setting-move_mode-animate')).toBeChecked();
    await expect(page.getByTestId('setting-edge_snapping')).not.toBeChecked();
    await expect(page.getByTestId('setting-edge_size_px')).toBeDisabled();
    await expect(page.getByTestId('setting-edge_size_px')).toHaveValue('8');
    await expect(page.getByTestId('setting-highlight_color')).toHaveValue('#ff5b1f');
    await expect(page.getByTestId('setting-highlight_color-picker')).toHaveValue('#ff5b1f');
    await expect(page.getByTestId('setting-highlight_opacity')).toHaveValue('0.35');
    await expect(page.getByText('35%', { exact: true })).toBeVisible();
    await expect(page.getByTestId('setting-theme-system')).toBeChecked();

    // Nothing has been changed, so nothing is being saved.
    await expect(indicator(page)).toHaveAttribute('data-state', 'idle');
    await expect(indicator(page)).toHaveText('');
  });

  test('switches expose the switch role and a name', async ({ page }) => {
    await openSettings(page);
    await expect(page.getByRole('switch', { name: 'Enable Krova' })).toBeChecked();
    await expect(page.getByRole('switch', { name: 'Launch at login' })).not.toBeChecked();
    await expect(page.getByRole('switch', { name: 'Edge snapping' })).not.toBeChecked();
    await expect(page.getByRole('radiogroup')).toHaveCount(3);
  });
});

test.describe('each setting saves automatically', () => {
  const switches = [
    { testId: 'setting-enabled', name: 'Enable Krova', initial: true },
    { testId: 'setting-launch_at_login', name: 'Launch at login', initial: false },
    { testId: 'setting-edge_snapping', name: 'Edge snapping', initial: false },
  ];
  for (const s of switches) {
    test(`toggling "${s.name}" shows the Saved indicator`, async ({ page }) => {
      await openSettings(page);
      const sw = page.getByRole('switch', { name: s.name });
      await expect(sw).toHaveAttribute('data-testid', s.testId);

      await sw.setChecked(!s.initial);
      await expectSaved(page);
      if (s.initial) await expect(sw).not.toBeChecked();
      else await expect(sw).toBeChecked();

      // And back.
      await sw.setChecked(s.initial);
      await expectSaved(page);
      if (s.initial) await expect(sw).toBeChecked();
      else await expect(sw).not.toBeChecked();
    });
  }

  for (const modifier of ['ctrl', 'alt', 'super', 'shift']) {
    test(`activation modifier: ${modifier}`, async ({ page }) => {
      await openSettings(page);
      // Start from a different value than the one under test.
      const other = modifier === 'ctrl' ? 'alt' : 'ctrl';
      await page.getByTestId(`setting-modifier-${other}`).check();
      await expectSaved(page);

      await page.getByTestId(`setting-modifier-${modifier}`).check();
      await expect(page.getByTestId(`setting-modifier-${modifier}`)).toBeChecked();
      await expect(page.getByTestId(`setting-modifier-${other}`)).not.toBeChecked();
      await expectSaved(page);
    });
  }

  test('move behaviour: instant, then animate again', async ({ page }) => {
    await openSettings(page);

    await page.getByTestId('setting-move_mode-instant').check();
    await expect(page.getByTestId('setting-move_mode-instant')).toBeChecked();
    await expect(page.getByTestId('setting-move_mode-animate')).not.toBeChecked();
    await expectSaved(page);

    await page.getByTestId('setting-move_mode-animate').check();
    await expect(page.getByTestId('setting-move_mode-animate')).toBeChecked();
    await expect(page.getByTestId('setting-move_mode-instant')).not.toBeChecked();
    await expectSaved(page);
  });

  test('the modifier description follows the choice', async ({ page }) => {
    await openSettings(page);
    const description = page.locator('#set-modifier-desc');
    await expect(description).toContainText('Hold Shift while dragging');
    await page.getByTestId('setting-modifier-ctrl').check();
    await expect(description).toContainText(/Hold (Ctrl|Control) while dragging/);
  });

  test('edge size is only available with edge snapping on', async ({ page }) => {
    await openSettings(page);
    const size = page.getByTestId('setting-edge_size_px');
    await expect(size).toBeDisabled();

    await page.getByRole('switch', { name: 'Edge snapping' }).check();
    await expect(size).toBeEnabled();

    await size.fill('24');
    await expect(page.getByText('24 px', { exact: true })).toBeVisible();
    await expectSaved(page);
    await expect(size).toHaveValue('24');

    await page.getByRole('switch', { name: 'Edge snapping' }).uncheck();
    await expect(size).toBeDisabled();
    await expect(size).toHaveValue('24'); // the value is kept
  });

  test('highlight opacity', async ({ page }) => {
    await openSettings(page);
    const opacity = page.getByTestId('setting-highlight_opacity');
    await opacity.fill('0.8');
    await expect(page.getByText('80%', { exact: true })).toBeVisible();
    await expectSaved(page);
    await expect(opacity).toHaveValue('0.8');
  });

  test('highlight colour from the colour picker updates the hex field', async ({ page }) => {
    await openSettings(page);
    await page.getByTestId('setting-highlight_color-picker').fill('#00ff7f');
    await expectSaved(page);
    await expect(page.getByTestId('setting-highlight_color')).toHaveValue('#00ff7f');
    await expect(page.getByTestId('setting-highlight_color')).toHaveAttribute(
      'aria-invalid',
      'false',
    );
  });

  test('highlight colour typed as hex is saved, lower-cased, and "#" is optional', async ({
    page,
  }) => {
    await openSettings(page);
    const hex = page.getByTestId('setting-highlight_color');

    await hex.fill('#12AB9F');
    await expectSaved(page);
    await expect(hex).toHaveValue('#12ab9f');
    await expect(page.getByTestId('setting-highlight_color-picker')).toHaveValue('#12ab9f');

    await hex.fill('ff8800');
    await expect(page.getByTestId('setting-highlight_color-picker')).toHaveValue('#ff8800');
    await expect(hex).toHaveValue('#ff8800');
    await expectSaved(page);
  });

  test('the Saved indicator goes away again after a moment', async ({ page }) => {
    await page.clock.install();
    await openSettings(page);
    await page.getByRole('switch', { name: 'Launch at login' }).check();

    await page.clock.fastForward(400); // past the 300 ms autosave delay
    await expectSaved(page);

    await page.clock.fastForward(3_000); // the badge shows for 2.5 s
    await expect(indicator(page)).toHaveAttribute('data-state', 'idle');
    await expect(indicator(page)).toHaveText('');
  });

  test('rapid changes are saved together with the last value', async ({ page }) => {
    await openSettings(page);
    const opacity = page.getByTestId('setting-highlight_opacity');
    await opacity.fill('0.1');
    await opacity.fill('0.2');
    await opacity.fill('0.9');
    await expectSaved(page);
    await expect(opacity).toHaveValue('0.9');
    await expect(page.getByText('90%', { exact: true })).toBeVisible();
  });
});

test.describe('highlight colour validation', () => {
  const bad = ['', 'zzz', '#12345', '#gggggg', '3b82f', '#12 456'];
  for (const value of bad) {
    test(`flags "${value}" as invalid and does not save it`, async ({ page }) => {
      await openSettings(page);
      const hex = page.getByTestId('setting-highlight_color');
      await hex.fill(value);

      await expect(hex).toHaveAttribute('aria-invalid', 'true');
      await expect(hex).toHaveAttribute('aria-describedby', 'set-color-err');
      await expect(page.getByText('Use a color like #3b82f6')).toBeVisible();
      await expect(hex).toHaveAccessibleDescription('Use a color like #3b82f6');
      // The picker still holds the last good colour.
      await expect(page.getByTestId('setting-highlight_color-picker')).toHaveValue('#ff5b1f');
      await expect(indicator(page)).not.toHaveAttribute('data-state', 'saved');
    });
  }

  test('becomes valid again as soon as a good colour is typed', async ({ page }) => {
    await openSettings(page);
    const hex = page.getByTestId('setting-highlight_color');

    await hex.fill('#12');
    await expect(hex).toHaveAttribute('aria-invalid', 'true');

    await hex.fill('#123456');
    await expect(hex).toHaveAttribute('aria-invalid', 'false');
    await expect(hex).not.toHaveAttribute('aria-describedby', /.+/);
    await expect(page.getByText('Use a color like #3b82f6')).toHaveCount(0);
    await expectSaved(page);
    await expect(page.getByTestId('setting-highlight_color-picker')).toHaveValue('#123456');
  });

  test('an invalid value typed after a good one leaves the saved colour alone', async ({
    page,
  }) => {
    await openSettings(page);
    const hex = page.getByTestId('setting-highlight_color');
    await hex.fill('#00aa00');
    await expectSaved(page);

    await hex.fill('#00aa0');
    await expect(hex).toHaveAttribute('aria-invalid', 'true');
    await expect(page.getByTestId('setting-highlight_color-picker')).toHaveValue('#00aa00');
  });
});

test.describe('persistence', () => {
  test('every setting is stored: all of them are still set after a reload', async ({ page }) => {
    await openSettings(page, '?reset');

    await page.getByRole('switch', { name: 'Enable Krova' }).uncheck();
    await page.getByRole('switch', { name: 'Launch at login' }).check();
    await page.getByTestId('setting-modifier-alt').check();
    await page.getByTestId('setting-move_mode-instant').check();
    await page.getByRole('switch', { name: 'Edge snapping' }).check();
    await page.getByTestId('setting-edge_size_px').fill('30');
    await page.getByTestId('setting-highlight_color').fill('#ff0000');
    await page.getByTestId('setting-highlight_opacity').fill('0.6');
    await page.getByTestId('setting-theme-dark').check();
    await expectSaved(page);

    // A plain visit (no ?reset) reads what the demo persisted.
    await page.goto('/');
    await page.getByTestId('nav-settings').click();
    await expect(page.getByTestId('setting-enabled')).not.toBeChecked();
    await expect(page.getByTestId('setting-launch_at_login')).toBeChecked();
    await expect(page.getByTestId('setting-modifier-alt')).toBeChecked();
    await expect(page.getByTestId('setting-move_mode-instant')).toBeChecked();
    await expect(page.getByTestId('setting-edge_snapping')).toBeChecked();
    await expect(page.getByTestId('setting-edge_size_px')).toHaveValue('30');
    await expect(page.getByTestId('setting-highlight_color')).toHaveValue('#ff0000');
    await expect(page.getByTestId('setting-highlight_opacity')).toHaveValue('0.6');
    await expect(page.getByTestId('setting-theme-dark')).toBeChecked();
    await expect(page.locator('html')).toHaveAttribute('data-theme', 'dark');
  });

  test('settings stay put when switching to the editor and back', async ({ page }) => {
    await openSettings(page);
    await page.getByTestId('setting-modifier-super').check();
    await expectSaved(page);
    await page.getByTestId('nav-editor').click();
    await expect(page.getByTestId('canvas')).toBeVisible();
    await page.getByTestId('nav-settings').click();
    await expect(page.getByTestId('setting-modifier-super')).toBeChecked();
  });
});

test.describe('keyboard', () => {
  test('Space toggles a switch', async ({ page }) => {
    await openSettings(page);
    const sw = page.getByRole('switch', { name: 'Launch at login' });
    await sw.focus();
    await page.keyboard.press('Space');
    await expect(sw).toBeChecked();
    await expectSaved(page);
  });

  test('arrow keys move through a radio group and save', async ({ page }) => {
    await openSettings(page);
    await page.getByTestId('setting-modifier-shift').focus();
    await page.keyboard.press('ArrowRight');
    await expect(page.getByTestId('setting-modifier-ctrl')).toBeChecked();
    await expect(page.getByTestId('setting-modifier-ctrl')).toBeFocused();
    await expectSaved(page);
  });

  test('range sliders respond to the arrow keys', async ({ page }) => {
    await openSettings(page);
    const opacity = page.getByTestId('setting-highlight_opacity');
    await opacity.focus();
    await page.keyboard.press('ArrowRight');
    await expect(opacity).toHaveValue('0.36');
    await expectSaved(page);
    await expect(page.getByText('36%', { exact: true })).toBeVisible();
  });
});

test.describe('theme', () => {
  test('switching to Dark, Light and back to System changes the rendered colours', async ({
    page,
  }) => {
    await page.emulateMedia({ colorScheme: 'light' });
    await openSettings(page);
    const html = page.locator('html');
    const body = page.locator('body');

    await expect(html).not.toHaveAttribute('data-theme', /.+/);
    await expect(body).toHaveCSS('background-color', LIGHT_BG);

    await page.getByTestId('setting-theme-dark').check();
    await expect(html).toHaveAttribute('data-theme', 'dark');
    await expect(html).toHaveCSS('color-scheme', 'dark');
    await expect(body).toHaveCSS('background-color', DARK_BG);
    await expectSaved(page);

    await page.getByTestId('setting-theme-light').check();
    await expect(html).toHaveAttribute('data-theme', 'light');
    await expect(html).toHaveCSS('color-scheme', 'light');
    await expect(body).toHaveCSS('background-color', LIGHT_BG);

    await page.getByTestId('setting-theme-system').check();
    await expect(html).not.toHaveAttribute('data-theme', /.+/);
    await expect(html).toHaveCSS('color-scheme', 'light dark');
    await expect(body).toHaveCSS('background-color', LIGHT_BG);
  });

  for (const os of ['light', 'dark'] as const) {
    for (const choice of ['system', 'light', 'dark'] as const) {
      const effective = choice === 'system' ? os : choice;
      test(`OS prefers ${os}, theme "${choice}" renders ${effective}`, async ({ page }) => {
        await page.emulateMedia({ colorScheme: os });
        await openSettings(page);
        await page.getByTestId(`setting-theme-${choice}`).check();

        await expect(page.locator('body')).toHaveCSS(
          'background-color',
          effective === 'dark' ? DARK_BG : LIGHT_BG,
        );
        if (choice === 'system') {
          await expect(page.locator('html')).not.toHaveAttribute('data-theme', /.+/);
        } else {
          await expect(page.locator('html')).toHaveAttribute('data-theme', choice);
        }
      });
    }
  }

  test('"System" follows the operating system while the app is open', async ({ page }) => {
    await page.emulateMedia({ colorScheme: 'light' });
    await openSettings(page);
    await expect(page.locator('body')).toHaveCSS('background-color', LIGHT_BG);

    await page.emulateMedia({ colorScheme: 'dark' });
    await expect(page.locator('body')).toHaveCSS('background-color', DARK_BG);

    await page.emulateMedia({ colorScheme: 'light' });
    await expect(page.locator('body')).toHaveCSS('background-color', LIGHT_BG);
  });

  test('the theme also applies to the editor', async ({ page }) => {
    await page.emulateMedia({ colorScheme: 'light' });
    await openSettings(page);
    await page.getByTestId('setting-theme-dark').check();
    await page.getByTestId('nav-editor').click();
    await expect(page.getByTestId('canvas')).toBeVisible();
    await expect(page.locator('body')).toHaveCSS('background-color', DARK_BG);
    await expect(page.getByTestId('canvas')).toHaveCSS('background-color', 'rgb(10, 13, 18)'); // --canvas-bg #0a0d12
  });
});
