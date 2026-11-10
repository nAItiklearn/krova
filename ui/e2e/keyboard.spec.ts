// Keyboard-only operation: Tab order, visible focus, a complete edit without a mouse.

import type { Locator } from '@playwright/test';
import { demo, editorUi, expect, focusRingVisible, openApp, openHome, tabTo, test } from './fixtures';

test.describe('Tab order and visible focus', () => {
  test('the editor is reached in reading order and every stop shows a focus ring', async ({
    page,
  }) => {
    // Open Layouts with the tray-menu hook so the keyboard focus stays at the page start.
    await openHome(page);
    await demo(page, 'navigate', 'editor');
    const ui = editorUi(page);
    await expect(ui.canvas).toBeVisible();

    // Disabled buttons (Delete zone, Undo, Revert, Save, "Active layout") are skipped, and
    // only the current sidebar button, the selected monitor tab and the first segment of
    // each divider are tab stops.
    const stops: Array<[string, Locator]> = [
      ['Arranging switch', page.locator('#st-toggle')],
      ['Layouts nav (current section)', page.getByTestId('nav-layouts')],
      ['demo Reset', page.getByRole('button', { name: 'Reset' })],
      ['selected monitor tab', ui.tabs.nth(0)],
      ['Select tool', ui.toolSelect],
      ['Split vertically tool', ui.toolSplitVertical],
      ['Split horizontally tool', ui.toolSplitHorizontal],
      ['zone 1', ui.zone(1)],
      ['zone 2', ui.zone(2)],
      ['divider', ui.dividers],
      ['layout name', ui.layoutName],
      ['Delete layout', ui.deleteLayout],
      ['New layout', ui.newLayout],
      ['layout item 1', ui.layoutItems.nth(0)],
      ['layout item 2', ui.layoutItems.nth(1)],
      ['layout item 3', ui.layoutItems.nth(2)],
      ['layout item 4', ui.layoutItems.nth(3)],
      ['template Halves', page.getByTestId('template-halves')],
      ['template Thirds', page.getByTestId('template-thirds')],
      ['template Grid', page.getByTestId('template-grid')],
      ['template Priority', page.getByTestId('template-priority')],
    ];

    for (const [label, locator] of stops) {
      await page.keyboard.press('Tab');
      await expect(locator, `Tab stop: ${label}`).toBeFocused();
      expect(await focusRingVisible(page), `visible focus ring on: ${label}`).toBe(true);
    }
  });

  test('Shift+Tab walks the same path backwards', async ({ page }) => {
    await openApp(page);
    const ui = editorUi(page);
    await page.getByTestId('template-priority').focus();
    await page.keyboard.press('Shift+Tab');
    await expect(page.getByTestId('template-grid')).toBeFocused();
    await tabTo(page, ui.dividers, { backwards: true });
    await page.keyboard.press('Shift+Tab');
    await expect(ui.zone(2)).toBeFocused();
    await page.keyboard.press('Shift+Tab');
    await expect(ui.zone(1)).toBeFocused();
  });

  test('buttons that cannot act yet are skipped, then join the Tab order', async ({ page }) => {
    await openApp(page);
    const ui = editorUi(page);
    await page.getByTestId('template-grid').click(); // now dirty: Undo, Revert and Save are enabled
    await ui.toolSplitHorizontal.focus();
    await page.keyboard.press('Tab');
    // Delete zone still needs a selected zone, so the next stop is Undo.
    await expect(ui.undo).toBeFocused();
    await page.keyboard.press('Tab');
    await expect(ui.revert).toBeFocused();
    await page.keyboard.press('Tab');
    await expect(ui.save).toBeFocused();
    expect(await focusRingVisible(page)).toBe(true);
  });

  test('a selected zone enables Delete zone and it becomes a Tab stop', async ({ page }) => {
    await openApp(page);
    const ui = editorUi(page);
    await ui.zone(1).focus();
    await page.keyboard.press('Enter');
    await expect(ui.deleteZone).toBeEnabled();
    await ui.toolSplitHorizontal.focus();
    await page.keyboard.press('Tab');
    await expect(ui.deleteZone).toBeFocused();
  });

  test('the Arrange view is reached in order and every control shows a focus ring', async ({
    page,
  }) => {
    // Switch section with the navigate hook so the keyboard focus stays at the page start.
    await openHome(page);
    await demo(page, 'navigate', 'arrange');
    await expect(page.getByTestId('setting-enabled')).toBeVisible();

    const stops: Array<[string, string]> = [
      ['Arrange new windows', 'arrange-enabled'],
      ['Which zone (checked radio)', 'arrange-policy-next_free'],
      ['Fill gaps', 'arrange-fill_gaps'],
      ['Stack when full', 'arrange-stack_when_full'],
      ['Enable Krova', 'setting-enabled'],
      ['Activation modifier (checked radio)', 'setting-modifier-shift'],
      ['Move behavior (checked radio)', 'setting-move_mode-animate'],
      ['Edge snapping', 'setting-edge_snapping'],
      // The disabled edge-size slider is skipped.
      ['Restore windows on reconnect', 'setting-restore_on_reconnect'],
      ['Linked dividers', 'setting-linked_dividers'],
      ['Space between zones', 'setting-zone_gap_px'],
      ['Focus follows mouse', 'setting-focus_follows_mouse'],
    ];

    // The sidebar comes first (the Arranging switch, the current section), then the demo
    // "Reset" button.
    await page.keyboard.press('Tab');
    await expect(page.locator('#st-toggle')).toBeFocused();
    await page.keyboard.press('Tab');
    await expect(page.getByTestId('nav-arrange')).toBeFocused();
    expect(await focusRingVisible(page)).toBe(true);
    await page.keyboard.press('Tab');
    await expect(page.getByRole('button', { name: 'Reset' })).toBeFocused();

    for (const [label, testId] of stops) {
      await page.keyboard.press('Tab');
      await expect(page.getByTestId(testId), `Tab stop: ${label}`).toBeFocused();
      expect(await focusRingVisible(page), `visible focus ring on: ${label}`).toBe(true);
    }
  });

  test('the focus ring does not rely on colour alone: it is an outline at least 2px wide', async ({
    page,
  }) => {
    await openApp(page);
    await page.keyboard.press('Tab');
    const outline = await page.evaluate(() => {
      const style = getComputedStyle(document.activeElement as Element);
      return { style: style.outlineStyle, width: parseFloat(style.outlineWidth) };
    });
    expect(outline.style).toBe('solid');
    expect(outline.width).toBeGreaterThanOrEqual(2);
  });
});

test.describe('operating the editor without a mouse', () => {
  test('apply Grid, split a zone, save: only the keyboard is used', async ({ page }) => {
    await openApp(page);
    const ui = editorUi(page);

    // Apply the Grid template.
    const grid = page.getByTestId('template-grid');
    await tabTo(page, grid);
    await page.keyboard.press('Enter');
    await expect(ui.zones).toHaveCount(4);
    await expect(ui.status).toHaveText('Applied the Grid 2x2 template. Save to keep it.');

    // Switch to the vertical split tool with V and go back to zone 1.
    await page.keyboard.press('v');
    await expect(ui.toolSplitVertical).toHaveAttribute('aria-pressed', 'true');
    await tabTo(page, ui.zone(1), { backwards: true });

    // Zone 1 is the top-left quarter (0..50 % wide): the guide starts at its centre, 25 %.
    await expect(ui.preview).toContainText('25%');
    await page.keyboard.press('Shift+ArrowRight');
    await expect(ui.preview).toContainText('30%');
    await page.keyboard.press('Enter');
    await expect(ui.status).toHaveText('Split zone 1 vertically at 30%; new zone 5.');
    await expect(ui.zones).toHaveCount(5);

    // Leave the tool with Escape and save with Ctrl+S.
    await page.keyboard.press('Escape');
    await expect(ui.toolSelect).toHaveAttribute('aria-pressed', 'true');
    await page.keyboard.press('ControlOrMeta+s');
    await expect(ui.status).toHaveText('Saved "Halves".');
    await expect(ui.save).toBeDisabled();
    await expect(ui.zones).toHaveCount(5);
  });

  test('select, delete and undo a zone using only the keyboard', async ({ page }) => {
    await openApp(page);
    const ui = editorUi(page);

    await tabTo(page, ui.zone(2));
    await page.keyboard.press('Enter'); // select
    await expect(ui.zone(2)).toHaveAttribute('aria-pressed', 'true');
    await page.keyboard.press('Delete');
    await expect(ui.status).toHaveText('Deleted zone 2; zone 1 took its space.');
    await expect(ui.zones).toHaveCount(1);

    await page.keyboard.press('ControlOrMeta+z');
    await expect(ui.status).toHaveText('Undid the last change.');
    await expect(ui.zones).toHaveCount(2);
  });

  test('a divider can be moved and the move checked through its accessible value', async ({
    page,
  }) => {
    await openApp(page);
    const ui = editorUi(page);
    await tabTo(page, ui.dividers);
    expect(await focusRingVisible(page)).toBe(true);

    await expect(ui.dividers).toHaveAttribute('aria-valuemin', '5');
    await expect(ui.dividers).toHaveAttribute('aria-valuemax', '95');
    await expect(ui.dividers).toHaveAttribute('aria-describedby', 'divider-help');
    await expect(page.locator('#divider-help')).toContainText('arrow keys');

    await page.keyboard.press('Shift+ArrowRight');
    await expect(ui.dividers).toHaveAttribute('aria-valuenow', '55');
    await expect(ui.dividers).toHaveAttribute('aria-valuetext', '55%');
    await expect(ui.dividers).toBeFocused();
  });

  test('errors are announced in a polite live region', async ({ page }) => {
    await openApp(page);
    const ui = editorUi(page);
    await expect(ui.status).toHaveAttribute('aria-live', 'polite');
    await expect(ui.status).toHaveAttribute('aria-atomic', 'true');

    await page.keyboard.press('v');
    await tabTo(page, ui.zone(1));
    for (let i = 0; i < 5; i++) await page.keyboard.press('Shift+ArrowLeft');
    await page.keyboard.press('Enter');
    // The same live region is found by its ARIA role and carries the backend's message.
    await expect(
      page.getByRole('status').filter({ hasText: 'smaller than the minimum size' }),
    ).toHaveCount(1);
  });
});
