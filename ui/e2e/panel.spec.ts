// The quick panel (panel.html) in demo mode: the mock "clicks the notch" as soon as
// the panel listens, so the panel opens by itself. Window calls (place, hide, open
// the main window) are recorded on window.__krovaPanel.

import AxeBuilder from '@axe-core/playwright';
import type { Page } from '@playwright/test';
import { expect, NOPERSIST, test } from './fixtures';

declare global {
  interface Window {
    __krovaPanel?: {
      placed: { side: 'top' | 'bottom'; monitorId: string | null; width: number } | null;
      hidden: number;
      opened: string[];
    } | null;
  }
}

const WCAG_TAGS = ['wcag2a', 'wcag2aa', 'wcag21a', 'wcag21aa', 'wcag22aa'];
const SHOTS = 'e2e/.screenshots';

async function openPanel(page: Page): Promise<void> {
  await page.goto(`/panel.html${NOPERSIST}`);
  const panel = page.getByTestId('panel');
  await expect(panel).toHaveAttribute('data-state', 'open');
  await expect(panel).toBeVisible();
  // Settled: the spring ends exactly at opacity 1.
  await expect(panel).toHaveCSS('opacity', '1');
}

const log = (page: Page) => page.evaluate(() => window.__krovaPanel!);

/** Click the (demo) notch at a place: sends "panel-anchor". */
const TOP: Parameters<typeof clickNotch>[1] = { x: 1220, y: 6, w: 120, h: 28, corner: 'top_center', monitor_id: 'monitor-1' };
function clickNotch(
  page: Page,
  anchor: { x: number; y: number; w: number; h: number; corner: string; monitor_id: string },
): Promise<void> {
  return page.evaluate((a) => window.__krovaDemo!.panelAnchor(a), anchor);
}

test.describe('quick panel', () => {
  test('opens from the demo notch on Display 1 and focuses the switch', async ({ page }) => {
    await openPanel(page);
    await expect(page.getByTestId('panel-monitor')).toHaveText('Display 1 (primary)');
    await expect(page.getByTestId('panel-pause')).toBeFocused();
    const placed = (await log(page)).placed;
    expect(placed?.side).toBe('top');
    expect(placed?.monitorId).toBe('monitor-1');
    expect(placed?.width).toBe(372);
  });

  test('pause switch flips the status and the dot', async ({ page }) => {
    await openPanel(page);
    const sw = page.getByTestId('panel-pause');
    const status = page.getByTestId('panel-status');
    await expect(status).toHaveText('Arranging');
    await expect(sw).toHaveAttribute('aria-checked', 'true');
    await sw.click();
    await expect(status).toHaveText('Paused');
    await expect(sw).toHaveAttribute('aria-checked', 'false');
    await page.keyboard.press('Space');
    await expect(status).toHaveText('Arranging');
  });

  test('applying a layout moves the accent ring', async ({ page }) => {
    await openPanel(page);
    const chips = page.getByTestId('panel-layouts').getByRole('button');
    expect(await chips.count()).toBeGreaterThan(1);
    const active = page.getByTestId('panel-layouts').locator('[aria-pressed="true"]');
    await expect(active).toHaveCount(1);
    const before = await active.getAttribute('data-testid');
    const other = page.getByTestId('panel-layouts').locator('[aria-pressed="false"]').first();
    const id = await other.getAttribute('data-testid');
    await other.click();
    await expect(page.getByTestId(id!)).toHaveAttribute('aria-pressed', 'true');
    await expect(page.getByTestId(before!)).toHaveAttribute('aria-pressed', 'false');
    await expect(active).toHaveCount(1);
  });

  test('a bottom notch on Display 2 opens upwards with that monitor\'s layouts', async ({ page }) => {
    await openPanel(page);
    await clickNotch(page, { x: 2700, y: 1040, w: 120, h: 28, corner: 'bottom_left', monitor_id: 'monitor-2' });
    await expect(page.getByTestId('panel-monitor')).toHaveText('Display 2');
    await expect.poll(async () => (await log(page)).placed?.side).toBe('bottom');
    await expect(page.getByTestId('panel')).toHaveAttribute('data-side', 'bottom');
  });

  test('keyboard order: switch, layouts, toggles, actions, footer link', async ({ page }) => {
    await openPanel(page);
    const chips = await page.getByTestId('panel-layouts').getByRole('button').count();
    const expected = [
      'panel-pause',
      ...Array.from({ length: chips }, () => 'chip'),
      'panel-toggle-focus',
      'panel-toggle-calm',
      'panel-toggle-share',
      'panel-toggle-zones',
      'panel-act-switcher',
      'panel-act-cheat',
      'panel-act-open',
      'panel-customise',
    ];
    const seen: string[] = [];
    for (let i = 0; i < expected.length; i++) {
      const id = await page.evaluate(() => document.activeElement?.getAttribute('data-testid') ?? '');
      seen.push(id.startsWith('panel-layout-') ? 'chip' : id);
      await page.keyboard.press('Tab');
    }
    expect(seen).toEqual(expected);
  });

  test('quick toggles are pressable and say they are coming soon', async ({ page }) => {
    await openPanel(page);
    const focus = page.getByTestId('panel-toggle-focus');
    await expect(focus).toHaveAttribute('aria-pressed', 'false');
    await focus.click();
    await expect(focus).toHaveAttribute('aria-pressed', 'true');
    await expect(focus).toHaveAccessibleDescription(/Coming soon/);
  });

  test('actions hand off to the main window and close the panel', async ({ page }) => {
    await openPanel(page);
    await page.getByTestId('panel-act-switcher').click();
    await expect(page.getByTestId('panel')).toBeHidden();
    expect((await log(page)).opened).toEqual(['switcher']);
    expect((await log(page)).hidden).toBe(1);

    // Clicking the notch again re-opens it.
    await clickNotch(page, TOP);
    await expect(page.getByTestId('panel')).toBeVisible();
    await page.getByTestId('panel-customise').click();
    await expect(page.getByTestId('panel')).toBeHidden();
    expect((await log(page)).opened).toEqual(['switcher', 'corners']);
  });

  test('Esc closes', async ({ page }) => {
    await openPanel(page);
    await page.keyboard.press('Escape');
    await expect(page.getByTestId('panel')).toBeHidden();
    expect((await log(page)).hidden).toBe(1);
  });

  test('clicking outside or losing focus closes', async ({ page }) => {
    await openPanel(page);
    await page.mouse.click(5, 600);
    await expect(page.getByTestId('panel')).toBeHidden();

    await clickNotch(page, TOP);
    await expect(page.getByTestId('panel')).toHaveCSS('opacity', '1');
    await page.evaluate(() => window.dispatchEvent(new Event('blur')));
    await expect(page.getByTestId('panel')).toBeHidden();
    expect((await log(page)).hidden).toBe(2);
  });
});

for (const scheme of ['light', 'dark'] as const) {
  test.describe(`quick panel, ${scheme}`, () => {
    test.use({ colorScheme: scheme, reducedMotion: 'reduce' });

    test('no WCAG A/AA violations', async ({ page }) => {
      await openPanel(page);
      await page.getByTestId('panel-toggle-calm').click();
      const results = await new AxeBuilder({ page }).withTags(WCAG_TAGS).analyze();
      expect(results.violations.map((v) => `${v.id}: ${v.nodes.map((n) => n.target.join(' ')).join(', ')}`)).toEqual([]);
      await page.mouse.move(0, 0);
      await page.getByTestId('panel').screenshot({ path: `${SHOTS}/panel-${scheme}.png`, omitBackground: true });
      await page.screenshot({ path: `${SHOTS}/panel-${scheme}-desk.png`, clip: { x: 0, y: 0, width: 1280, height: 520 } });
    });
  });
}
