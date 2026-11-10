// The Arrange, Displays and Privacy sections of the sidebar UI.

import { expect, gotoSection, openArrange, openSection, test } from './fixtures';

test.describe('Arrange: auto-arrange', () => {
  test('defaults, and the options follow the main switch', async ({ page }) => {
    await openArrange(page);
    const enabled = page.getByTestId('arrange-enabled');
    await expect(enabled).toBeChecked();
    await expect(page.getByTestId('arrange-policy-next_free')).toBeChecked();
    await expect(page.getByTestId('arrange-fill_gaps')).toBeChecked();
    await expect(page.getByTestId('arrange-stack_when_full')).toBeChecked();

    await enabled.uncheck();
    await expect(page.getByTestId('arrange-fill_gaps')).toBeDisabled();
    await expect(page.getByTestId('arrange-stack_when_full')).toBeDisabled();
    await expect(page.getByTestId('arrange-policy-main_first')).toBeDisabled();

    await enabled.check();
    await page.getByTestId('arrange-policy-main_first').check();
    await expect(page.getByTestId('arrange-policy-main_first')).toBeChecked();
    await page.getByTestId('arrange-fill_gaps').uncheck();
    await expect(page.getByTestId('arrange-fill_gaps')).not.toBeChecked();
  });

  test('the choices are kept when coming back to Arrange', async ({ page }) => {
    await openArrange(page);
    await page.getByTestId('arrange-policy-main_first').check();
    await page.getByTestId('arrange-stack_when_full').uncheck();
    await page.getByTestId('setting-focus_follows_mouse').check();
    await gotoSection(page, 'overview');
    await gotoSection(page, 'arrange');
    await expect(page.getByTestId('arrange-policy-main_first')).toBeChecked();
    await expect(page.getByTestId('arrange-stack_when_full')).not.toBeChecked();
    await expect(page.getByTestId('setting-focus_follows_mouse')).toBeChecked();
  });

  test('the sidebar status follows the auto-arrange switch', async ({ page }) => {
    await openArrange(page);
    await expect(page.locator('#st-label')).toHaveText('Arranging');
    await page.getByTestId('arrange-enabled').uncheck();
    await expect(page.locator('#st-label')).toHaveText('Paused');
  });
});

test.describe('Arrange: app rules', () => {
  test('lists the rules with their action, and a zone number for "Zone"', async ({ page }) => {
    await openArrange(page);
    const rules = page.getByTestId('app-rule');
    await expect(rules).toHaveCount(2);
    await expect(rules.nth(0).getByTestId('app-rule-exe')).toHaveText('Calculator.exe');
    await expect(rules.nth(0).getByTestId('app-rule-action-float')).toBeChecked();
    await expect(rules.nth(0).getByTestId('app-rule-zone')).toHaveCount(0);
    await expect(rules.nth(1).getByTestId('app-rule-action-preferred_zone')).toBeChecked();
    await expect(rules.nth(1).getByTestId('app-rule-zone')).toHaveValue('3');
  });

  test('changing the action and the zone number', async ({ page }) => {
    await openArrange(page);
    const first = page.getByTestId('app-rule').nth(0);
    await first.getByTestId('app-rule-action-preferred_zone').check();
    await expect(first.getByTestId('app-rule-zone')).toHaveValue('1');
    await first.getByTestId('app-rule-zone').fill('4');
    await first.getByTestId('app-rule-zone').press('Tab');
    await expect(first.getByTestId('app-rule-zone')).toHaveValue('4');
    await first.getByTestId('app-rule-action-never_touch').check();
    await expect(first.getByTestId('app-rule-zone')).toHaveCount(0);
  });

  test('adding a valid program name, then removing every rule shows the empty state', async ({
    page,
  }) => {
    await openArrange(page);
    await page.getByTestId('app-rule-input').fill('  Notepad.exe ');
    await page.getByTestId('app-rule-input').press('Enter');
    await expect(page.getByTestId('app-rule')).toHaveCount(3);
    await expect(page.getByTestId('app-rule').nth(2).getByTestId('app-rule-exe')).toHaveText('Notepad.exe');
    await expect(page.getByTestId('app-rule-input')).toHaveValue('');

    await page.getByRole('button', { name: 'Remove the rule for Calculator.exe' }).click();
    await expect(page.getByTestId('app-rule')).toHaveCount(2);
    while ((await page.getByTestId('app-rule-remove').count()) > 0) {
      await page.getByTestId('app-rule-remove').first().click();
    }
    await expect(page.getByTestId('app-rules-empty')).toBeVisible();
  });

  const bad = [
    ['', 'empty'],
    ['notepad', 'no .exe'],
    ['C:\\Windows\\notepad.exe', 'a path'],
    ['tools/app.exe', 'a forward-slash path'],
    ['note\u202Epad.exe', 'a bidi override'],
    ['note\u200Bpad.exe', 'a zero-width space'],
    ['a'.repeat(130) + '.exe', 'too long'],
    ['calculator.EXE', 'a duplicate'],
  ] as const;
  for (const [value, why] of bad) {
    test(`refuses ${why}`, async ({ page }) => {
      await openArrange(page);
      const input = page.getByTestId('app-rule-input');
      await input.fill(value);
      await page.getByTestId('app-rule-add').click();
      await expect(page.getByTestId('app-rule-error')).toBeVisible();
      await expect(input).toHaveAttribute('aria-invalid', 'true');
      await expect(input).toHaveAccessibleDescription(/.+/);
      await expect(page.getByTestId('app-rule')).toHaveCount(2);
      // Typing again clears the message.
      await input.press('End');
      await input.press('x');
      await expect(page.getByTestId('app-rule-error')).toHaveCount(0);
    });
  }
});

test.describe('Displays', () => {
  test('one card per connected monitor with its facts and active layout', async ({ page }) => {
    await openSection(page, 'displays');
    const cards = page.getByTestId('display-card');
    await expect(cards).toHaveCount(2);
    await expect(cards.nth(0)).toContainText('Display 1');
    await expect(cards.nth(0).getByTestId('display-resolution')).toHaveText('2560 × 1440');
    await expect(cards.nth(0).getByTestId('display-layout')).toHaveText('Halves');
    await expect(cards.nth(0).getByTestId('display-primary')).toBeVisible();
    await expect(cards.nth(1).getByTestId('display-resolution')).toHaveText('1920 × 1080');
    await expect(cards.nth(1).getByTestId('display-layout')).toHaveText('Thirds');
    await expect(cards.nth(1).getByTestId('display-primary')).toHaveCount(0);
    await expect(cards.nth(0).getByTestId('display-scale')).toHaveText(/^\d+%$/);
  });

  test('reserved edges are a disabled preview for now', async ({ page }) => {
    await openSection(page, 'displays');
    const reserved = page.getByTestId('display-reserved').first();
    await expect(reserved.getByRole('switch')).toHaveCount(4);
    for (const sw of await reserved.getByRole('switch').all()) await expect(sw).toBeDisabled();
    await expect(page.getByTestId('display-card').first()).toContainText('Coming soon');
  });

  test('a new active layout shows up on its card', async ({ page }) => {
    await openSection(page, 'layouts');
    await page.getByTestId('layout-item').filter({ hasText: 'Grid' }).click();
    await page.getByTestId('set-active').click();
    await gotoSection(page, 'displays');
    await expect(page.getByTestId('display-card').nth(0).getByTestId('display-layout')).toHaveText(/Grid/);
  });
});

test.describe('Privacy & about', () => {
  test('states the privacy promise and the version', async ({ page }) => {
    await openSection(page, 'privacy');
    const promise = page.getByTestId('privacy-promise');
    for (const text of [
      'No account',
      'No telemetry',
      'No network calls',
      'Window titles are never logged or stored',
      'one local file',
    ]) {
      await expect(promise).toContainText(text);
    }
    await expect(page.getByTestId('about-version')).toHaveText(/^Krova v\d+\.\d+\.\d+/);
    await expect(page.getByTestId('about-licence')).toContainText('MIT');
    // Links are plain text: nothing on the page can open a website.
    await expect(page.getByTestId('view-privacy').locator('a[href]')).toHaveCount(0);
  });

  test('"Open Layouts" goes to the layout editor for import and export', async ({ page }) => {
    await openSection(page, 'privacy');
    await page.getByTestId('privacy-open-layouts').click();
    await expect(page.getByTestId('canvas')).toBeVisible();
    await expect(page.getByTestId('export-layout')).toBeVisible();
    await expect(page.getByTestId('nav-layouts')).toHaveAttribute('aria-current', 'page');
  });
});
