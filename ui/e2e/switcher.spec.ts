// The layout switcher window (switcher.html) in demo mode: keyboard flow,
// swipe-to-delete and grip reorder with real pointer events, and an axe audit
// in the light and the dark theme. Screenshots for design review go to
// e2e/.shots/ (git-ignored).

import AxeBuilder from '@axe-core/playwright';
import type { Locator, Page } from '@playwright/test';
import { expect, test } from './fixtures';

const URL = '/switcher.html?nopersist';
const SHOTS = 'e2e/.shots';

const dialog = (page: Page): Locator => page.getByTestId('switcher');
const options = (page: Page): Locator => page.getByRole('option');
const selected = (page: Page): Locator => page.locator('[role="option"][aria-selected="true"]');
const live = (page: Page): Locator => page.getByTestId('switcher-live');

async function names(page: Page): Promise<string[]> {
  return options(page).evaluateAll((els) => els.map((e) => e.querySelector('.name')?.textContent ?? ''));
}

async function openSwitcher(page: Page, query = URL): Promise<void> {
  await page.goto(query);
  await expect(dialog(page)).toHaveAttribute('data-open', 'true');
  await expect(options(page)).toHaveCount(4);
  await expect(selected(page)).toBeFocused();
}

/** Wait until the springs have come to rest (the deck stops writing styles). */
async function settle(page: Page): Promise<void> {
  await page.waitForFunction(
    () =>
      new Promise<boolean>((resolve) => {
        const read = (): string =>
          [...document.querySelectorAll<HTMLElement>('[data-card], [data-testid="switcher"]')]
            .map((e) => e.style.transform + e.style.opacity)
            .join('|');
        const a = read();
        setTimeout(() => resolve(read() === a), 150);
      }),
  );
}

async function centerOf(loc: Locator): Promise<{ x: number; y: number }> {
  const b = await loc.boundingBox();
  if (!b) throw new Error('no box');
  return { x: b.x + b.width / 2, y: b.y + b.height / 2 };
}

test.describe('layout switcher', () => {
  test('opens on the monitor it is for, with the active layout in front', async ({ page }) => {
    await openSwitcher(page);
    await expect(page.getByRole('heading', { name: 'Layouts' })).toBeVisible();
    await expect(dialog(page)).toContainText('Display 1 (primary) · 2560 × 1440');
    await expect(selected(page)).toHaveAccessibleName('Halves Active 2 zones');
    await expect(page.getByTestId('switcher-active-badge')).toHaveCount(1);
    await expect(page.getByRole('listbox', { name: 'Layouts' })).toBeVisible();
    expect(await names(page)).toEqual(['Halves', 'Thirds', 'Grid 2x2', 'Priority']);
  });

  test('keyboard: flip, reorder, delete, undo, apply, close', async ({ page }) => {
    await openSwitcher(page);

    await page.keyboard.press('ArrowDown');
    await expect(selected(page)).toHaveAccessibleName(/^Thirds/);
    await expect(selected(page)).toBeFocused();
    await page.keyboard.press('ArrowUp');
    await expect(selected(page)).toHaveAccessibleName(/^Halves/);
    await page.keyboard.press('End');
    await expect(selected(page)).toHaveAccessibleName(/^Priority/);
    await page.keyboard.press('Home');

    // Alt+Down moves the focused layout one place down and keeps it focused.
    await page.keyboard.press('Alt+ArrowDown');
    expect(await names(page)).toEqual(['Thirds', 'Halves', 'Grid 2x2', 'Priority']);
    await expect(selected(page)).toHaveAccessibleName(/^Halves/);
    await expect(live(page)).toHaveText('Halves moved to position 2 of 4.');

    // Delete, then undo with Ctrl+Z: back at its place and active again.
    await page.keyboard.press('Delete');
    await expect(options(page)).toHaveCount(3);
    await expect(page.getByTestId('switcher-toast')).toContainText('Deleted “Halves”');
    await expect(live(page)).toHaveText('Deleted Halves. Press Control Z to undo.');
    await page.keyboard.press('Control+z');
    await expect(options(page)).toHaveCount(4);
    expect(await names(page)).toEqual(['Thirds', 'Halves', 'Grid 2x2', 'Priority']);
    await expect(selected(page)).toHaveAccessibleName('Halves Active 2 zones');

    // Enter applies the focused layout and closes the switcher.
    await page.keyboard.press('ArrowDown');
    await expect(selected(page)).toHaveAccessibleName(/^Grid 2x2/);
    await page.keyboard.press('Enter');
    await expect(dialog(page)).toHaveAttribute('data-open', 'false');
    await expect(page.getByTestId('switcher-toast')).toContainText('“Grid 2x2” is now active on Display 1');
    await expect(page.getByTestId('switcher-reopen')).toBeFocused();

    // Reopening reads the backend again: the reorder and the new active layout stuck.
    await page.getByTestId('switcher-reopen').click();
    await expect(dialog(page)).toHaveAttribute('data-open', 'true');
    expect(await names(page)).toEqual(['Thirds', 'Halves', 'Grid 2x2', 'Priority']);
    await expect(selected(page)).toHaveAccessibleName('Grid 2x2 Active 4 zones');

    await page.keyboard.press('Escape');
    await expect(dialog(page)).toHaveAttribute('data-open', 'false');
    await expect(dialog(page)).toHaveAttribute('inert', '');
  });

  test('the Undo button in the toast restores a deleted layout', async ({ page }) => {
    await openSwitcher(page);
    await page.keyboard.press('ArrowDown');
    await page.keyboard.press('Delete');
    await expect(options(page)).toHaveCount(3);
    await page.getByRole('button', { name: /Undo/ }).click();
    await expect(options(page)).toHaveCount(4);
    expect(await names(page)).toEqual(['Halves', 'Thirds', 'Grid 2x2', 'Priority']);
  });

  test('swipe sideways deletes, a short swipe springs back', async ({ page }) => {
    await openSwitcher(page);
    await settle(page);
    const card = selected(page);
    const c = await centerOf(card);

    // Short swipe: not far or fast enough, the card returns.
    await page.mouse.move(c.x, c.y);
    await page.mouse.down();
    for (let i = 1; i <= 6; i++) await page.mouse.move(c.x + i * 10, c.y, { steps: 2 });
    await page.waitForTimeout(120); // let the velocity window go quiet
    await page.mouse.up();
    await expect(options(page)).toHaveCount(4);

    // Long swipe: deleted, with the Undo toast; mid-swipe frame for review.
    await settle(page);
    await page.mouse.move(c.x, c.y);
    await page.mouse.down();
    for (let i = 1; i <= 8; i++) await page.mouse.move(c.x + i * 15, c.y + i, { steps: 2 });
    await page.screenshot({ path: `${SHOTS}/switcher-mid-swipe.png` });
    for (let i = 9; i <= 14; i++) await page.mouse.move(c.x + i * 15, c.y + i, { steps: 2 });
    await page.mouse.up();
    await expect(options(page)).toHaveCount(3);
    expect(await names(page)).toEqual(['Thirds', 'Grid 2x2', 'Priority']);
    await expect(page.getByTestId('switcher-toast')).toContainText('Deleted “Halves”');

    await page.keyboard.press('Control+z');
    await expect(options(page)).toHaveCount(4);
    await expect(selected(page)).toHaveAccessibleName('Halves Active 2 zones');
  });

  test('a tap applies, a tap on the backdrop closes', async ({ page }) => {
    await openSwitcher(page);
    await settle(page);
    // The title strip of the next card shows above the focused one.
    const thirds = options(page).nth(1);
    const box = (await thirds.boundingBox())!;
    await page.mouse.click(box.x + box.width / 2, box.y + 12);
    await expect(dialog(page)).toHaveAttribute('data-open', 'false');
    await expect(page.getByTestId('switcher-toast')).toContainText('“Thirds” is now active');

    await page.getByTestId('switcher-reopen').click();
    await expect(selected(page)).toHaveAccessibleName('Thirds Active 3 zones');
    await settle(page);
    await page.mouse.click(20, 300);
    await expect(dialog(page)).toHaveAttribute('data-open', 'false');
  });

  test('dragging the grip reorders', async ({ page }) => {
    await openSwitcher(page);
    await settle(page);
    const grip = selected(page).locator('[data-grip]');
    const g = await centerOf(grip);
    await page.mouse.move(g.x, g.y);
    await page.mouse.down();
    // Up by two title strips: over the slot two behind.
    for (let i = 1; i <= 10; i++) await page.mouse.move(g.x, g.y - i * 10, { steps: 2 });
    await page.mouse.up();
    expect(await names(page)).toEqual(['Thirds', 'Grid 2x2', 'Halves', 'Priority']);
    await expect(live(page)).toHaveText('Halves moved to position 3 of 4.');

    await page.keyboard.press('Escape');
    await page.getByTestId('switcher-reopen').click();
    expect(await names(page)).toEqual(['Thirds', 'Grid 2x2', 'Halves', 'Priority']);
  });

  test('vertical drag flips through with a rubber band at the ends', async ({ page }) => {
    await openSwitcher(page);
    await settle(page);
    const c = await centerOf(selected(page));
    await page.mouse.move(c.x - 150, c.y);
    await page.mouse.down();
    for (let i = 1; i <= 12; i++) await page.mouse.move(c.x - 150, c.y + i * 10, { steps: 2 });
    await page.waitForTimeout(120);
    await page.mouse.up();
    await expect(selected(page)).toHaveAccessibleName(/^Thirds/);
    // Pulling past the first layout bounces back to it.
    const d = await centerOf(selected(page));
    await page.mouse.move(d.x - 150, d.y);
    await page.mouse.down();
    for (let i = 1; i <= 20; i++) await page.mouse.move(d.x - 150, d.y - i * 30, { steps: 2 });
    await page.waitForTimeout(120);
    await page.mouse.up();
    await expect(selected(page)).toHaveAccessibleName(/^Halves/);
  });

  for (const scheme of ['light', 'dark'] as const) {
    test(`axe: no WCAG A/AA violations (${scheme})`, async ({ page }) => {
      await page.emulateMedia({ colorScheme: scheme, reducedMotion: 'reduce' });
      await openSwitcher(page);
      await settle(page);
      await page.screenshot({ path: `${SHOTS}/switcher-${scheme}.png` });
      const results = await new AxeBuilder({ page })
        .withTags(['wcag2a', 'wcag2aa', 'wcag21a', 'wcag21aa', 'wcag22aa'])
        .analyze();
      expect(results.violations.map((v) => `${v.id}: ${v.nodes.map((n) => n.target.join(' ')).join(', ')}`)).toEqual([]);

      // And with the Undo toast showing.
      await page.keyboard.press('Delete');
      await expect(page.getByRole('button', { name: /Undo/ })).toBeVisible();
      await settle(page);
      const again = await new AxeBuilder({ page })
        .withTags(['wcag2a', 'wcag2aa', 'wcag21a', 'wcag21aa', 'wcag22aa'])
        .analyze();
      expect(again.violations.map((v) => `${v.id}: ${v.nodes.map((n) => n.target.join(' ')).join(', ')}`)).toEqual([]);
    });
  }

  test('reduced motion: everything lands at once', async ({ page }) => {
    await page.emulateMedia({ reducedMotion: 'reduce' });
    await openSwitcher(page);
    const before = await selected(page).evaluate((e) => e.style.transform);
    await page.waitForTimeout(100);
    expect(await selected(page).evaluate((e) => e.style.transform)).toBe(before);
    await page.keyboard.press('ArrowDown');
    await expect(selected(page)).toHaveAccessibleName(/^Thirds/);
  });
});

// Ctrl+Alt+/ (and the notch menu) show the switcher window with
// `navigate: "cheat_sheet"`; the demo page takes ?view=cheat_sheet.
test.describe('cheat sheet in the switcher window', () => {
  const SHEET = '/switcher.html?nopersist&view=cheat_sheet';

  test('shows the shortcuts grouped, with families collapsed, and closes on any key', async ({ page }) => {
    await page.goto(SHEET);
    const sheet = page.getByTestId('cheat-sheet');
    await expect(sheet).toBeVisible();
    await expect(sheet).toBeFocused();
    await expect(page.getByRole('heading', { name: 'Krova shortcuts' })).toBeVisible();
    await expect(dialog(page)).not.toHaveAttribute('data-open', 'true');
    for (const g of ['Windows', 'Layouts', 'Krova']) {
      await expect(sheet.getByRole('region', { name: g })).toBeVisible();
    }
    await expect(sheet).toContainText('Move to zone 1–9');
    await expect(sheet).toContainText('Apply layout 1–9');
    await page.keyboard.press('a');
    await expect(sheet).toHaveCount(0);
  });

  test('a click outside closes it', async ({ page }) => {
    await page.goto(SHEET);
    await expect(page.getByTestId('cheat-sheet')).toBeVisible();
    await page.mouse.click(5, 5);
    await expect(page.getByTestId('cheat-sheet')).toHaveCount(0);
  });

  for (const scheme of ['light', 'dark'] as const) {
    test(`axe AA (${scheme})`, async ({ page }) => {
      await page.emulateMedia({ colorScheme: scheme, reducedMotion: 'reduce' });
      await page.goto(SHEET);
      await expect(page.getByTestId('cheat-sheet')).toBeVisible();
      const results = await new AxeBuilder({ page })
        .withTags(['wcag2a', 'wcag2aa', 'wcag21a', 'wcag21aa', 'wcag22aa'])
        .analyze();
      expect(results.violations.map((v) => `${v.id}: ${v.nodes.map((n) => n.target.join(' ')).join(', ')}`)).toEqual([]);
    });
  }
});
