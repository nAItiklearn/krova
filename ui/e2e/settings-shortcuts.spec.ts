// The keyboard-shortcut settings (Shortcuts section) and the "Windows" group
// (Arrange section).
//
// Demo flags used here: ?shortcuts=failed|off and ?capabilities=none (see ui/README.md).
//
// The v0.1 controls (switch, modifier select, key map, Win-key hint, notes) live in
// the "Keyboard" group at the end of the Shortcuts view, with their v0.1 wording.

import type { Page } from '@playwright/test';
import { demo, expect, focusRingVisible, gotoSection, openArrange, openSection, test, expectSaved } from './fixtures';

const openShortcuts = (page: Page, query?: string) => openSection(page, 'shortcuts', query);

const indicator = (page: Page) => page.getByTestId('save-indicator');

const shortcutsSwitch = (page: Page) => page.getByTestId('setting-keyboard_shortcuts');
const modifierSelect = (page: Page) => page.getByTestId('setting-shortcut_modifier');
const restoreSwitch = (page: Page) =>
  page.getByRole('switch', { name: 'Restore windows on reconnect' });
const linkedSwitch = (page: Page) => page.getByRole('switch', { name: 'Linked dividers' });
const gapSlider = (page: Page) => page.getByRole('slider', { name: 'Space between zones (px)' });

const sheetButton = (page: Page, id: string) => page.getByTestId(`shortcut-${id}`);

test.describe('Shortcut sheet: the backend actions', () => {
  test('shows the default chords from the backend, families as modifiers', async ({ page }) => {
    await openShortcuts(page);
    await expect(sheetButton(page, 'move').locator('kbd')).toHaveText(['Ctrl', 'Alt']);
    await expect(sheetButton(page, 'swap').locator('kbd')).toHaveText(['Ctrl', 'Alt', 'Shift']);
    await expect(sheetButton(page, 'pause').locator('kbd')).toHaveText(['Ctrl', 'Alt', 'T']);
    await expect(sheetButton(page, 'switcher').locator('kbd')).toHaveText(['Ctrl', 'Alt', 'Space']);
    // No backend action runs calm desk yet, so it has no row.
    await expect(sheetButton(page, 'calm_desk')).toHaveCount(0);
    await expect(page.getByTestId('shortcuts-reset')).toBeDisabled();
    // The v0.1 modifier choice is replaced by the sheet.
    await expect(modifierSelect(page)).toHaveCount(0);
    await expect(page.getByTestId('shortcuts-keymap')).toHaveCount(0);
  });

  test('recording a free chord saves it, Reset all brings the default back', async ({ page }) => {
    await openShortcuts(page);
    await sheetButton(page, 'pause').click();
    await page.keyboard.press('Control+Alt+KeyP');
    await expect(sheetButton(page, 'pause').locator('kbd')).toHaveText(['Ctrl', 'Alt', 'P']);
    await expect(page.getByTestId('shortcuts-reset')).toBeEnabled();
    await page.getByTestId('shortcuts-reset').click();
    await expect(sheetButton(page, 'pause').locator('kbd')).toHaveText(['Ctrl', 'Alt', 'T']);
  });

  test('a family is rebound as a whole', async ({ page }) => {
    await openShortcuts(page);
    await sheetButton(page, 'move').click();
    await page.keyboard.press('Control+Shift+ArrowLeft');
    await expect(sheetButton(page, 'move').locator('kbd')).toHaveText(['Ctrl', 'Shift']);
  });

  test('a chord another action has is refused with free suggestions', async ({ page }) => {
    await openShortcuts(page);
    await sheetButton(page, 'pause').click();
    await page.keyboard.press('Control+Alt+KeyF');
    const notice = page.getByTestId('shortcut-conflict-pause');
    await expect(notice).toContainText('Ctrl+Alt+F is already Focus');
    // Still the old chord: nothing was saved.
    await expect(sheetButton(page, 'pause').locator('kbd')).toHaveText(['Ctrl', 'Alt', 'T']);
    const chip = page.getByTestId('view-shortcuts').locator('.conflict .chip').first();
    const suggestion = (await chip.textContent())!.trim();
    await chip.click();
    await expect(notice).toHaveCount(0);
    await expect(sheetButton(page, 'pause').locator('kbd')).toHaveText(suggestion.split('+'));
  });

  test('Backspace turns a shortcut off', async ({ page }) => {
    await openShortcuts(page);
    await sheetButton(page, 'focus').click();
    await page.keyboard.press('Backspace');
    await expect(sheetButton(page, 'focus')).toContainText('Off');
  });
});

test.describe('Keyboard shortcuts group', () => {
  test('shows the switch and no warnings by default', async ({ page }) => {
    await openShortcuts(page);
    await expect(page.getByTestId('settings-shortcuts')).toBeVisible();
    await expect(shortcutsSwitch(page)).toBeChecked();
    await expect(shortcutsSwitch(page)).toBeEnabled();
    await expect(shortcutsSwitch(page)).toHaveRole('switch');
    await expect(shortcutsSwitch(page)).toHaveAccessibleName(/.+/);
    await expect(page.getByTestId('shortcuts-win-hint')).toHaveCount(0);
    await expect(page.getByTestId('shortcuts-failed')).toHaveCount(0);
    await expect(page.getByTestId('shortcuts-inactive')).toHaveCount(0);
    await expect(page.getByTestId('shortcuts-unavailable')).toHaveCount(0);
  });

  test('the switch saves', async ({ page }) => {
    await openShortcuts(page);
    await shortcutsSwitch(page).uncheck();
    await expectSaved(page);
    await expect(shortcutsSwitch(page)).not.toBeChecked();
    await shortcutsSwitch(page).check();
    await expectSaved(page);
  });

  test('Space toggles the switch from the keyboard', async ({ page }) => {
    await openShortcuts(page);
    await shortcutsSwitch(page).focus();
    await page.keyboard.press('Space');
    await expect(shortcutsSwitch(page)).not.toBeChecked();
    await expectSaved(page);
  });
});

test.describe('Keyboard shortcuts group: what the backend reports', () => {
  test('?shortcuts=failed lists the shortcuts that could not be registered', async ({ page }) => {
    await openShortcuts(page, '?nopersist&shortcuts=failed');
    const warning = page.getByTestId('shortcuts-failed');
    await expect(warning).toBeVisible();
    // The backend's own sentence is shown (the wording around it is the view's).
    await expect(warning).toContainText('Ctrl+Alt+Shift+3 is already used by another app');

    // It sits in a polite live region, so a conflict found later is announced.
    const region = page.getByTestId('shortcuts-status');
    await expect(region).toHaveAttribute('role', 'status');
    await expect(region).toHaveAttribute('aria-live', 'polite');
    await expect(region.getByTestId('shortcuts-failed')).toBeVisible();

    // The controls still work: the user can pick other keys.
    await expect(shortcutsSwitch(page)).toBeEnabled();
    await expect(page.getByTestId('shortcut-move')).toBeEnabled();
    await expect(page.getByTestId('shortcuts-inactive')).toHaveCount(0);
  });

  test('turning the shortcuts off clears the warning once it is saved', async ({ page }) => {
    await openShortcuts(page, '?nopersist&shortcuts=failed');
    await expect(page.getByTestId('shortcuts-failed')).toBeVisible();
    await shortcutsSwitch(page).uncheck();
    await expectSaved(page);
    await expect(page.getByTestId('shortcuts-failed')).toHaveCount(0);
    await expect(page.getByTestId('shortcuts-inactive')).toHaveCount(0);
  });

  test('?shortcuts=off says the shortcuts are not active', async ({ page }) => {
    await openShortcuts(page, '?nopersist&shortcuts=off');
    const note = page.getByTestId('shortcuts-inactive');
    await expect(note).toBeVisible();
    await expect(note).toContainText('Keyboard shortcuts are not active right now.');
    await expect(page.getByTestId('shortcuts-failed')).toHaveCount(0);
    await expect(shortcutsSwitch(page)).toBeChecked();
    await expect(shortcutsSwitch(page)).toBeEnabled();
  });

  test('the warning and the note follow the backend live', async ({ page }) => {
    await openShortcuts(page);
    await expect(page.getByTestId('shortcuts-failed')).toHaveCount(0);

    await demo(page, 'setShortcuts', 'failed');
    await expect(page.getByTestId('shortcuts-failed')).toBeVisible();

    await demo(page, 'setShortcuts', 'off');
    await expect(page.getByTestId('shortcuts-failed')).toHaveCount(0);
    await expect(page.getByTestId('shortcuts-inactive')).toBeVisible();

    await demo(page, 'setShortcuts', 'ok');
    await expect(page.getByTestId('shortcuts-failed')).toHaveCount(0);
    await expect(page.getByTestId('shortcuts-inactive')).toHaveCount(0);
  });
});

test.describe('Without platform support (?capabilities=none, the macOS case today)', () => {
  test('keyboard shortcuts are explained and the controls are disabled', async ({ page }) => {
    await openShortcuts(page, '?nopersist&capabilities=none');

    const note = page.getByTestId('shortcuts-unavailable');
    await expect(note).toBeVisible();
    await expect(note).toContainText('Keyboard shortcuts are not available on this system yet');

    await expect(shortcutsSwitch(page)).toBeDisabled();
    // The stored value is still shown.
    await expect(shortcutsSwitch(page)).toBeChecked();

    // A key list for keys that do nothing here would only mislead.
    await expect(page.getByTestId('shortcuts-keymap')).toHaveCount(0);
    await expect(page.getByTestId('shortcuts-failed')).toHaveCount(0);
    await expect(page.getByTestId('shortcuts-inactive')).toHaveCount(0);
  });

  test('the window memory toggles are disabled with an explanation, the gap is not', async ({
    page,
  }) => {
    await openArrange(page, '?nopersist&capabilities=none');
    await expect(page.getByTestId('window-memory-unavailable')).toContainText(
      'Remembering snapped windows is not available on this system yet',
    );
    await expect(restoreSwitch(page)).toBeDisabled();
    await expect(linkedSwitch(page)).toBeDisabled();
    await expect(restoreSwitch(page)).toBeChecked();
    await expect(linkedSwitch(page)).toBeChecked();
    await expect(gapSlider(page)).toBeEnabled();
  });

  test('disabled controls cannot be changed and nothing is saved', async ({ page }) => {
    await openShortcuts(page, '?nopersist&capabilities=none');
    await shortcutsSwitch(page).click({ force: true });
    await expect(shortcutsSwitch(page)).toBeChecked();
    await expect(indicator(page)).toHaveAttribute('data-state', 'idle');
    await gotoSection(page, 'arrange');
    await restoreSwitch(page).click({ force: true });
    await expect(restoreSwitch(page)).toBeChecked();
    await expect(indicator(page)).toHaveAttribute('data-state', 'idle');
  });

  test('the disabled controls are skipped by Tab', async ({ page }) => {
    await openArrange(page, '?nopersist&capabilities=none');
    await page.getByTestId('setting-edge_snapping').focus();
    await page.keyboard.press('Tab');
    // Edge size (edge snapping is off), restore and linked switches are disabled: the gap is next.
    await expect(gapSlider(page)).toBeFocused();
  });

  test('the notes appear and disappear when the platform support changes', async ({ page }) => {
    await openShortcuts(page);
    await expect(page.getByTestId('shortcuts-unavailable')).toHaveCount(0);
    await expect(shortcutsSwitch(page)).toBeEnabled();

    await demo(page, 'setCapabilities', 'none');
    await expect(page.getByTestId('shortcuts-unavailable')).toBeVisible();
    await expect(shortcutsSwitch(page)).toBeDisabled();

    await demo(page, 'setCapabilities', 'all');
    await expect(page.getByTestId('shortcuts-unavailable')).toHaveCount(0);
    await expect(shortcutsSwitch(page)).toBeEnabled();
  });

  test('the window memory note follows the platform support live', async ({ page }) => {
    await openArrange(page);
    await expect(page.getByTestId('window-memory-unavailable')).toHaveCount(0);
    await demo(page, 'setCapabilities', 'none');
    await expect(page.getByTestId('window-memory-unavailable')).toBeVisible();
    await expect(restoreSwitch(page)).toBeDisabled();
    await demo(page, 'setCapabilities', 'all');
    await expect(page.getByTestId('window-memory-unavailable')).toHaveCount(0);
    await expect(restoreSwitch(page)).toBeEnabled();
  });
});

test.describe('Windows group', () => {
  test('shows its heading, defaults and help texts', async ({ page }) => {
    await openArrange(page);
    await expect(page.getByRole('heading', { name: 'Windows', level: 3 })).toBeVisible();

    await expect(restoreSwitch(page)).toBeChecked();
    await expect(restoreSwitch(page)).toHaveAttribute('data-testid', 'setting-restore_on_reconnect');
    await expect(restoreSwitch(page)).toHaveAccessibleDescription(
      'When a monitor comes back, windows you snapped to it return to their zones. Kept in memory only.',
    );

    await expect(linkedSwitch(page)).toBeChecked();
    await expect(linkedSwitch(page)).toHaveAttribute('data-testid', 'setting-linked_dividers');
    await expect(linkedSwitch(page)).toHaveAccessibleDescription(
      'Drag the edge between two snapped windows and both resize.',
    );

    await expect(gapSlider(page)).toHaveAttribute('data-testid', 'setting-zone_gap_px');
    await expect(gapSlider(page)).toHaveValue('0');
    await expect(gapSlider(page)).toHaveAttribute('min', '0');
    await expect(gapSlider(page)).toHaveAttribute('max', '32');
    await expect(gapSlider(page)).toHaveAttribute('step', '1');
    await expect(page.getByLabel('Space between zones (px)')).toHaveCount(1);
    await expect(page.locator('output[for="set-gap"]')).toHaveText('0 px');
    await expect(page.getByTestId('window-memory-unavailable')).toHaveCount(0);
  });

  const switches = [
    { name: 'Restore windows on reconnect', get: restoreSwitch },
    { name: 'Linked dividers', get: linkedSwitch },
  ];
  for (const s of switches) {
    test(`toggling "${s.name}" saves, and back`, async ({ page }) => {
      await openArrange(page);
      await s.get(page).uncheck();
      await expectSaved(page);
      await expect(s.get(page)).not.toBeChecked();
      await s.get(page).check();
      await expectSaved(page);
      await expect(s.get(page)).toBeChecked();
    });
  }

  test('the gap slider shows its value in px and saves', async ({ page }) => {
    await openArrange(page);
    await gapSlider(page).fill('12');
    await expect(page.locator('output[for="set-gap"]')).toHaveText('12 px');
    await expect(gapSlider(page)).toHaveAccessibleName('Space between zones (px)');
    await expect(gapSlider(page)).toHaveAttribute('aria-valuetext', '12 pixels');
    await expectSaved(page);
    await expect(gapSlider(page)).toHaveValue('12');
  });

  test('the gap slider cannot leave 0..32 and moves with the arrow keys', async ({ page }) => {
    await openArrange(page);
    await gapSlider(page).fill('32');
    await expect(page.locator('output[for="set-gap"]')).toHaveText('32 px');
    await expectSaved(page);
    // The browser itself clamps anything beyond the range.
    expect(
      await gapSlider(page).evaluate((el: HTMLInputElement) => {
        el.value = '99';
        return el.value;
      }),
    ).toBe('32');
    expect(
      await gapSlider(page).evaluate((el: HTMLInputElement) => {
        el.value = '-5';
        return el.value;
      }),
    ).toBe('0');

    await gapSlider(page).focus();
    await page.keyboard.press('Home');
    await expect(gapSlider(page)).toHaveValue('0');
    await page.keyboard.press('ArrowRight');
    await expect(gapSlider(page)).toHaveValue('1');
    await expect(page.locator('output[for="set-gap"]')).toHaveText('1 px');
    await page.keyboard.press('End');
    await expect(gapSlider(page)).toHaveValue('32');
    await page.keyboard.press('ArrowRight'); // stays at the end
    await expect(gapSlider(page)).toHaveValue('32');
    await expectSaved(page);
  });
});

test.describe('persistence and keyboard order', () => {
  test('every new setting is stored: all of them are still set after a reload', async ({
    page,
  }) => {
    await openShortcuts(page, '?reset');

    // Not the default (Ctrl + Alt + T), so a stored chord is told apart from a default.
    await page.getByTestId('shortcut-pause').click();
    await page.keyboard.press('Control+Alt+KeyP');
    await expect(page.getByTestId('shortcut-pause').locator('kbd')).toHaveText(['Ctrl', 'Alt', 'P']);
    await shortcutsSwitch(page).uncheck();
    await expectSaved(page);
    await gotoSection(page, 'arrange');
    await restoreSwitch(page).uncheck();
    await linkedSwitch(page).uncheck();
    await gapSlider(page).fill('9');
    await expect(page.locator('output[for="set-gap"]')).toHaveText('9 px');
    await expectSaved(page);

    await page.goto('/');
    await gotoSection(page, 'shortcuts');
    await expect(shortcutsSwitch(page)).not.toBeChecked();
    await expect(page.getByTestId('shortcut-pause').locator('kbd')).toHaveText(['Ctrl', 'Alt', 'P']);
    await gotoSection(page, 'arrange');
    await expect(restoreSwitch(page)).not.toBeChecked();
    await expect(linkedSwitch(page)).not.toBeChecked();
    await expect(gapSlider(page)).toHaveValue('9');
  });

  test('the Windows group is in the Tab order after Snapping, each stop shows a focus ring', async ({
    page,
  }) => {
    await openArrange(page);
    await page.getByTestId('setting-edge_snapping').focus();
    // Edge size is disabled while edge snapping is off.
    const stops = [restoreSwitch(page), linkedSwitch(page), gapSlider(page), page.getByTestId('setting-focus_follows_mouse')];
    for (const stop of stops) {
      await page.keyboard.press('Tab');
      await expect(stop).toBeFocused();
      expect(await focusRingVisible(page)).toBe(true);
    }
  });

  test('Tab goes from the shortcut sheet to the shortcut switch', async ({ page }) => {
    await openShortcuts(page);
    await page.getByTestId('shortcut-focus').focus();
    await page.keyboard.press('Tab');
    await expect(page.getByTestId('shortcut-cheat_sheet')).toBeFocused();
    await shortcutsSwitch(page).focus();
    expect(await focusRingVisible(page)).toBe(true);
  });

  test('the settings stay put when switching to the editor and back', async ({ page }) => {
    await openArrange(page);
    await gapSlider(page).fill('5');
    await expectSaved(page);
    await gotoSection(page, 'layouts');
    await gotoSection(page, 'arrange');
    await expect(gapSlider(page)).toHaveValue('5');
  });
});
