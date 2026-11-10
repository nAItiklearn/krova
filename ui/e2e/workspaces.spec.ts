// Workspaces, Focus, Screen edges and the hover zone picker (demo mode).
// Screenshots go to e2e/.results/screens/ (git-ignored).

import AxeBuilder from '@axe-core/playwright';
import type { Page } from '@playwright/test';
import { expect, openHome, test } from './fixtures';

const SHOTS = 'e2e/.results/screens';
const WCAG_TAGS = ['wcag2a', 'wcag2aa', 'wcag21a', 'wcag21aa', 'wcag22aa'];

async function open(page: Page, section: 'workspaces' | 'focus' | 'screen'): Promise<void> {
  await openHome(page);
  await page.getByTestId(`nav-${section}`).click();
  await expect(page.getByTestId(`nav-${section}`)).toHaveAttribute('aria-current', 'page');
  await expect(page.getByTestId(`view-${section}`)).toBeVisible();
}

/** Save one workspace through the empty state, accepting the first-save notice. */
async function captureFirst(page: Page): Promise<void> {
  await page.getByTestId('ws-empty').getByTestId('ws-capture').click();
  await expect(page.getByTestId('ws-privacy-ask')).toContainText('full path of each app');
  await page.getByTestId('ws-privacy-ok').click();
  await expect(page.getByTestId('ws-card')).toHaveCount(1);
}

async function axe(page: Page): Promise<string[]> {
  const r = await new AxeBuilder({ page }).withTags(WCAG_TAGS).analyze();
  return r.violations.map((v) => `${v.id}: ${v.nodes.map((n) => n.target.join(' ')).join(' | ')}`);
}

test.describe('Workspaces', () => {
  test('empty state, privacy notice, then capture draws the zones with exe names', async ({ page }) => {
    await open(page, 'workspaces');
    await expect(page.getByTestId('ws-empty')).toContainText('No workspaces yet');
    await expect(page.getByTestId('ws-privacy')).toContainText('Nothing leaves your computer');

    // Cancel the first-save notice: nothing is saved.
    await page.getByTestId('ws-capture').click();
    await page.getByTestId('ws-privacy-ask').getByRole('button', { name: 'Cancel' }).click();
    await expect(page.getByTestId('ws-card')).toHaveCount(0);

    await captureFirst(page);
    const card = page.getByTestId('ws-card');
    await expect(card.getByTestId('ws-name')).toHaveText('Workspace 1');
    await expect(card.getByTestId('ws-count')).toHaveText('2 apps');
    await expect(card.getByTestId('ws-zone-app')).toHaveText(['Code.exe', 'mail.exe']);
    // Never a folder in the drawing.
    await expect(card.getByTestId('ws-zone-app').first()).not.toContainText('Program Files');

    // The notice is shown only once.
    await page.getByTestId('ws-capture').click();
    await expect(page.getByTestId('ws-card')).toHaveCount(2);
    await expect(page.getByTestId('ws-privacy-ask')).toHaveCount(0);
    await page.screenshot({ path: `${SHOTS}/workspaces.png` });
  });

  test('restore shows a per-entry report, one by one', async ({ page }) => {
    await open(page, 'workspaces');
    await captureFirst(page);
    await page.getByTestId('ws-restore').click();
    const entries = page.getByTestId('ws-report-entry');
    await expect(entries).toHaveCount(2);
    await expect(entries.nth(0)).toContainText('Code.exe');
    await expect(entries.nth(0)).toContainText('Placed');
    await expect(entries.nth(0).getByTestId('ws-report-path')).toHaveCount(0);
    await expect(page.getByTestId('ws-report-summary')).toContainText('2 placed');
    await expect(page.getByTestId('ws-report-summary')).toContainText('minimised');
    await page.screenshot({ path: `${SHOTS}/workspaces-restore.png` });
  });

  test('"Open missing apps" asks first and lists the full paths', async ({ page }) => {
    await open(page, 'workspaces');
    await captureFirst(page);
    await page.getByTestId('ws-launch').check();
    await page.getByTestId('ws-restore').click();
    const confirm = page.getByTestId('ws-confirm');
    await expect(confirm.getByTestId('ws-confirm-path')).toHaveText([
      'C:\\Program Files\\Code\\Code.exe',
      'C:\\Program Files\\Mail\\mail.exe',
    ]);
    await expect(page.getByTestId('ws-report')).toHaveCount(0);
    await page.screenshot({ path: `${SHOTS}/workspaces-confirm.png` });
    await page.getByTestId('ws-confirm-ok').click();
    await expect(page.getByTestId('ws-report-entry')).toHaveCount(2);
  });

  test('rename inline: Enter saves, Escape cancels, a bad name shows the error', async ({ page }) => {
    await open(page, 'workspaces');
    await captureFirst(page);
    const card = page.getByTestId('ws-card');
    await card.getByTestId('ws-rename').click();
    const input = card.getByTestId('ws-name-input');
    await expect(input).toBeFocused();
    await input.fill('Coding');
    await input.press('Enter');
    await expect(card.getByTestId('ws-name')).toHaveText('Coding');

    await card.getByTestId('ws-rename').click();
    await card.getByTestId('ws-name-input').fill('Something else');
    await card.getByTestId('ws-name-input').press('Escape');
    await expect(card.getByTestId('ws-name')).toHaveText('Coding');
    await expect(card.getByTestId('ws-rename')).toBeFocused();

    await card.getByTestId('ws-rename').click();
    await card.getByTestId('ws-name-input').fill('   ');
    await card.getByTestId('ws-name-input').press('Enter');
    await expect(card.getByTestId('ws-rename-error')).toHaveText('give the workspace a name');
  });

  test('delete hides the card with an Undo toast; Undo brings it back', async ({ page }) => {
    await open(page, 'workspaces');
    await captureFirst(page);
    await page.getByTestId('ws-delete').click();
    await expect(page.getByTestId('ws-toast')).toContainText('Deleted “Workspace 1”');
    await expect(page.getByTestId('ws-empty')).toBeVisible();
    await page.screenshot({ path: `${SHOTS}/workspaces-undo.png` });
    await page.getByTestId('ws-undo').click();
    await expect(page.getByTestId('ws-card')).toHaveCount(1);
    await expect(page.getByTestId('ws-toast')).toHaveCount(0);

    // Without Undo, the workspace is really deleted (also when leaving the view).
    await page.getByTestId('ws-delete').click();
    await page.getByTestId('nav-overview').click();
    await page.getByTestId('nav-workspaces').click();
    await expect(page.getByTestId('ws-empty')).toBeVisible();
  });

  test('a bound shortcut shows as a chip', async ({ page }) => {
    await page.addInitScript(() =>
      localStorage.setItem('krova.ext.v1', JSON.stringify({ shortcuts: { 'workspace:ws-1': 'Ctrl+Alt+1' } })),
    );
    await open(page, 'workspaces');
    await captureFirst(page);
    await expect(page.getByTestId('ws-shortcut')).toHaveText('Ctrl+Alt+1');
  });
});

test.describe('Focus', () => {
  test('focus dim, strength, calm desk, share-safe and focus follows mouse', async ({ page }) => {
    await open(page, 'focus');
    const dim = page.getByTestId('focus-dim-enabled');
    await expect(dim).not.toBeChecked();
    await dim.check();
    await expect(dim).toBeChecked();
    await page.getByTestId('focus-dim-strength').fill('70');
    await expect(page.getByTestId('focus-dim-value')).toHaveText('70%');

    await page.getByTestId('focus-calm-toggle').check();
    await expect(page.getByTestId('focus-calm-status')).toHaveText('2 windows are tucked away.');
    await page.getByTestId('focus-calm-toggle').uncheck();
    await expect(page.getByTestId('focus-calm-status')).toHaveCount(0);

    await page.getByTestId('focus-share-safe').check();
    await expect(page.getByTestId('focus-share-safe')).toBeChecked();

    const delay = page.getByTestId('focus-ffm-delay');
    await expect(delay).toBeDisabled();
    await page.getByTestId('focus-ffm').check();
    await expect(delay).toBeEnabled();
    await delay.fill('500');
    await expect(page.getByTestId('focus-ffm-delay-value')).toHaveText('500 ms');
    await page.screenshot({ path: `${SHOTS}/focus.png`, fullPage: true });

    // Saved: coming back shows the same values.
    await page.waitForTimeout(500);
    await page.getByTestId('nav-overview').click();
    await page.getByTestId('nav-focus').click();
    await expect(page.getByTestId('focus-dim-value')).toHaveText('70%');
    await expect(page.getByTestId('focus-ffm-delay-value')).toHaveText('500 ms');
    await expect(page.getByTestId('focus-share-safe')).toBeChecked();
  });

  test('share-safe hides app names in the workspace drawings', async ({ page }) => {
    await open(page, 'focus');
    await page.getByTestId('focus-share-safe').check();
    await page.waitForTimeout(450);
    await page.getByTestId('nav-workspaces').click();
    await captureFirst(page);
    await expect(page.getByTestId('ws-zone-app')).toHaveCount(0);
  });
});

test.describe('Screen edges', () => {
  test('typing a reserved edge draws the band and is kept', async ({ page }) => {
    await open(page, 'screen');
    const mon = page.getByTestId('screen-monitor').first();
    await expect(mon.getByTestId('screen-clear')).toBeDisabled();
    const band = mon.getByTestId('screen-band-left');
    expect((await band.boundingBox())?.width ?? 0).toBeLessThan(1);
    await mon.getByTestId('screen-left').fill('256');
    await expect.poll(async () => (await band.boundingBox())?.width ?? 0).toBeGreaterThan(20);
    await mon.getByTestId('screen-top').fill('999');
    await expect(mon.getByTestId('screen-top')).toHaveValue('400');
    await page.screenshot({ path: `${SHOTS}/screen-edges.png`, fullPage: true });
    // Saved to the backend: survives leaving the view.
    await page.waitForTimeout(600);
    await page.getByTestId('nav-overview').click();
    await page.getByTestId('nav-screen').click();
    await expect(page.getByTestId('screen-monitor').first().getByTestId('screen-left')).toHaveValue('256');
    await page.getByTestId('screen-monitor').first().getByTestId('screen-clear').click();
    await expect(page.getByTestId('screen-monitor').first().getByTestId('screen-left')).toHaveValue('0');
  });

  test('language picker', async ({ page }) => {
    await open(page, 'screen');
    const sel = page.getByTestId('screen-language-select');
    await expect(sel).toHaveValue('system');
    await expect(page.getByTestId('screen-language-in-use')).toContainText('English');
    await expect(sel.locator('option')).toHaveText(['System', 'English', 'Español', 'Deutsch', '日本語', '简体中文']);
    await sel.selectOption('de');
    await expect(page.getByTestId('screen-language-in-use')).toHaveCount(0);
    await expect(page.getByTestId('view-screen')).toContainText('Translations are in progress');
  });
});

test.describe('Hover zone picker page', () => {
  test('simulate, hover, click to place; Esc and the end event close it', async ({ page }) => {
    await page.goto('/picker.html?nopersist');
    await expect(page.getByTestId('picker-demo')).toBeVisible();
    await expect(page.getByTestId('picker-card')).toHaveCount(0);

    await page.getByTestId('picker-simulate').click();
    const card = page.getByTestId('picker-card');
    await expect(card).toBeVisible();
    await expect(card.getByRole('button', { name: /^Zone \d$/ })).toHaveCount(2);
    await page.getByTestId('picker-zone-2').hover();
    await expect(page.getByTestId('picker-zone-2')).toHaveClass(/hot/);
    await page.screenshot({ path: `${SHOTS}/picker.png` });
    await page.getByTestId('picker-zone-2').click();
    await expect(card).toHaveCount(0);
    await expect(page.getByTestId('picker-placed')).toHaveText('Placed in zone 2.');

    await page.getByTestId('picker-simulate-2').click();
    await expect(card.getByRole('button')).toHaveCount(3);
    await page.keyboard.press('Escape');
    await expect(card).toHaveCount(0);

    await page.getByTestId('picker-simulate').click();
    await page.getByTestId('picker-simulate-end').click();
    await expect(card).toHaveCount(0);

    await page.getByTestId('picker-simulate-2').click();
    await page.keyboard.press('3');
    await expect(page.getByTestId('picker-placed')).toHaveText('Placed in zone 3.');
  });
});

for (const scheme of ['light', 'dark'] as const) {
  test.describe(`axe AA, ${scheme}`, () => {
    test.use({ colorScheme: scheme, reducedMotion: 'reduce' });

    test('Workspaces: empty, notice, cards, report, confirm, toast', async ({ page }) => {
      await open(page, 'workspaces');
      expect(await axe(page)).toEqual([]);
      await page.getByTestId('ws-capture').click();
      expect(await axe(page)).toEqual([]);
      await page.getByTestId('ws-privacy-ok').click();
      await page.getByTestId('ws-launch').check();
      await page.getByTestId('ws-restore').click();
      expect(await axe(page)).toEqual([]);
      await page.getByTestId('ws-confirm-ok').click();
      await expect(page.getByTestId('ws-report-summary')).toContainText('placed');
      await page.getByTestId('ws-capture').click();
      await page.getByTestId('ws-card').nth(1).getByTestId('ws-rename').click();
      expect(await axe(page)).toEqual([]);
      await page.keyboard.press('Escape');
      await page.getByTestId('ws-delete').first().click();
      expect(await axe(page)).toEqual([]);
      await page.screenshot({ path: `${SHOTS}/workspaces-${scheme}.png` });
    });

    test('Focus', async ({ page }) => {
      await open(page, 'focus');
      await page.getByTestId('focus-dim-enabled').check();
      await page.getByTestId('focus-calm-toggle').check();
      await page.getByTestId('focus-ffm').check();
      await expect(page.getByTestId('focus-calm-status')).toBeVisible();
      expect(await axe(page)).toEqual([]);
      await page.screenshot({ path: `${SHOTS}/focus-${scheme}.png`, fullPage: true });
    });

    test('Screen edges', async ({ page }) => {
      await open(page, 'screen');
      await page.getByTestId('screen-monitor').nth(1).getByTestId('screen-top').fill('120');
      await page.getByTestId('screen-monitor').nth(1).getByTestId('screen-right').fill('64');
      await expect(page.getByTestId('screen-monitor').nth(1).getByTestId('screen-clear')).toBeEnabled();
      expect(await axe(page)).toEqual([]);
      await page.screenshot({ path: `${SHOTS}/screen-${scheme}.png`, fullPage: true });
    });

    test('picker page', async ({ page }) => {
      await page.goto('/picker.html?nopersist');
      expect(await axe(page)).toEqual([]);
      await page.getByTestId('picker-simulate').click();
      await page.getByTestId('picker-zone-1').hover();
      expect(await axe(page)).toEqual([]);
      await page.screenshot({ path: `${SHOTS}/picker-${scheme}.png` });
    });
  });
}
