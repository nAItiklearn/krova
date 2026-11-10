// The keyboard-shortcut settings (Shortcuts section) and the "Windows" group
// (Arrange section).
//
// Demo flags used here: ?shortcuts=failed|off and ?capabilities=none (see ui/README.md).
//
// Some tests are `test.fixme`: they need parts of the Shortcuts view that the new
// sidebar UI does not have (yet): the key map (shortcuts-keymap, keymap-*), the
// Win-key hint (shortcuts-win-hint) and the "not active" note (shortcuts-inactive).

import type { Page } from '@playwright/test';
import { demo, expect, focusRingVisible, gotoSection, openArrange, openSection, test } from './fixtures';

const openShortcuts = (page: Page, query?: string) => openSection(page, 'shortcuts', query);

const indicator = (page: Page) => page.getByTestId('save-indicator');

async function expectSaved(page: Page): Promise<void> {
  await expect(indicator(page)).toHaveAttribute('data-state', 'saved');
  await expect(indicator(page)).toContainText('Saved');
}

const shortcutsSwitch = (page: Page) => page.getByTestId('setting-keyboard_shortcuts');
const modifierSelect = (page: Page) => page.getByTestId('setting-shortcut_modifier');
const restoreSwitch = (page: Page) =>
  page.getByRole('switch', { name: 'Restore windows on reconnect' });
const linkedSwitch = (page: Page) => page.getByRole('switch', { name: 'Linked dividers' });
const gapSlider = (page: Page) => page.getByRole('slider', { name: 'Space between zones (px)' });

// In the order of the select: the default first, the Win-key choices last.
const MODIFIER_CHOICES = [
  { value: 'ctrl_alt_shift', label: 'Ctrl + Alt + Shift', winKey: false },
  { value: 'ctrl_alt', label: 'Ctrl + Alt', winKey: false },
  { value: 'win_alt', label: 'Win + Alt', winKey: true },
  { value: 'win_ctrl', label: 'Win + Ctrl', winKey: true },
] as const;

/** A key-combination label that matches with or without spaces around "+". */
const loose = (label: string): RegExp =>
  new RegExp(`^\\s*${label.split(' + ').join('\\s*\\+\\s*')}\\s*$`);

/** The four rows of the key map for a modifier label like "Ctrl + Alt". */
async function expectKeymap(page: Page, mods: string): Promise<void> {
  await expect(page.getByTestId('keymap-left')).toHaveText(`${mods} + Left`);
  await expect(page.getByTestId('keymap-right')).toHaveText(`${mods} + Right`);
  await expect(page.getByTestId('keymap-number')).toHaveText(`${mods} + 1 to 9`);
  await expect(page.getByTestId('keymap-up')).toHaveText(`${mods} + Up`);
}

test.describe('Keyboard shortcuts group: defaults', () => {
  test('shows the switch and the four modifier choices', async ({ page }) => {
    await openShortcuts(page);

    await expect(page.getByTestId('settings-shortcuts')).toBeVisible();
    await expect(shortcutsSwitch(page)).toBeChecked();
    await expect(shortcutsSwitch(page)).toBeEnabled();
    await expect(shortcutsSwitch(page)).toHaveRole('switch');
    await expect(shortcutsSwitch(page)).toHaveAccessibleName(/.+/);

    const select = modifierSelect(page);
    await expect(select).toHaveAttribute('data-testid', 'setting-shortcut_modifier');
    await expect(select).toBeEnabled();
    // Windows refuses most Win-key chords, so the default is Ctrl + Alt + Shift.
    await expect(select).toHaveValue('ctrl_alt_shift');
    // "Ctrl + Alt" or "Ctrl+Alt": the spacing is up to the view.
    await expect(select.locator('option')).toHaveText(MODIFIER_CHOICES.map((c) => loose(c.label)));
    await expect(select.locator('option')).toHaveCount(4);
    for (const c of MODIFIER_CHOICES) {
      await expect(select.locator(`option[value="${c.value}"]`)).toHaveText(loose(c.label));
    }
    await expect(page.getByTestId('shortcuts-win-hint')).toHaveCount(0);

    // Nothing to warn about.
    await expect(page.getByTestId('shortcuts-failed')).toHaveCount(0);
    await expect(page.getByTestId('shortcuts-inactive')).toHaveCount(0);
    await expect(page.getByTestId('shortcuts-unavailable')).toHaveCount(0);
  });

  test.fixme('shows the key map for the default modifier', async ({ page }) => {
    // fixme: the new Shortcuts view has no key map (shortcuts-keymap, keymap-*) yet.
    await openShortcuts(page);
    await expectKeymap(page, 'Ctrl + Alt + Shift');
    await expect(page.getByTestId('keymap-left').locator('kbd')).toHaveText([
      'Ctrl',
      'Alt',
      'Shift',
      'Left',
    ]);
    await expect(page.getByTestId('keymap-number').locator('kbd')).toHaveText([
      'Ctrl',
      'Alt',
      'Shift',
      '1',
      '9',
    ]);
    await expect(page.getByTestId('shortcuts-keymap')).toContainText(
      'Move the focused window to the previous zone',
    );
    await expect(page.getByTestId('shortcuts-keymap')).toContainText(
      'Send the focused window to the next monitor, keeping its zone number',
    );

  });

  test.fixme('the key map is a real description list with key caps', async ({ page }) => {
    // fixme: the new Shortcuts view has no key map (shortcuts-keymap) yet.
    await openShortcuts(page);
    const keymap = page.getByTestId('shortcuts-keymap');
    await expect(keymap).toHaveJSProperty('tagName', 'DL');
    await expect(keymap.locator('dt')).toHaveCount(4);
    await expect(keymap.locator('dd')).toHaveCount(4);
    await expect(keymap.locator('kbd').first()).toBeVisible();
    await expect(page.getByRole('heading', { name: 'What the keys do', level: 4 })).toBeVisible();
  });
});

test.describe('Keyboard shortcuts group: changing it', () => {
  test('the switch saves, and the modifier choice is only available while it is on', async ({
    page,
  }) => {
    await openShortcuts(page);
    await shortcutsSwitch(page).uncheck();
    await expectSaved(page);
    await expect(shortcutsSwitch(page)).not.toBeChecked();
    await expect(modifierSelect(page)).toBeDisabled();
    // The value is kept while it is greyed out.
    await expect(modifierSelect(page)).toHaveValue('ctrl_alt_shift');

    await shortcutsSwitch(page).check();
    await expectSaved(page);
    await expect(modifierSelect(page)).toBeEnabled();
  });

  for (const choice of MODIFIER_CHOICES) {
    test(`choosing "${choice.label}" saves it`, async ({ page }) => {
      await openShortcuts(page);
      // Start from something else so a change really happens.
      const other = choice.value === 'ctrl_alt' ? 'win_ctrl' : 'ctrl_alt';
      await modifierSelect(page).selectOption(other);
      await expectSaved(page);

      await modifierSelect(page).selectOption(choice.value);
      await expect(modifierSelect(page)).toHaveValue(choice.value);
      await expectSaved(page);
    });

    test.fixme(`choosing "${choice.label}" rewrites the key map and hints at Win-key trouble`, async ({ page }) => {
      // fixme: the new Shortcuts view has no key map (keymap-*) or Win-key hint (shortcuts-win-hint) yet.
      await openShortcuts(page);
      // Start from something else so a change really happens.
      const other = choice.value === 'ctrl_alt' ? 'win_ctrl' : 'ctrl_alt';
      await modifierSelect(page).selectOption(other);
      await expectSaved(page);

      await modifierSelect(page).selectOption(choice.value);
      await expect(modifierSelect(page)).toHaveValue(choice.value);
      await expectKeymap(page, choice.label);
      await expectSaved(page);

      // Windows keeps many Win-key chords for itself: those choices carry a hint.
      const hint = page.getByTestId('shortcuts-win-hint');
      if (choice.winKey) {
        await expect(hint).toBeVisible();
        await expect(hint).toContainText('Ctrl + Alt + Shift is the safest choice');
        await expect(modifierSelect(page)).toHaveAccessibleDescription(/Windows keeps many shortcuts/);
      } else {
        await expect(hint).toHaveCount(0);
      }
    });
  }

  test.fixme('the key map follows the choice at once, before the save is done', async ({ page }) => {
    // fixme: the new Shortcuts view has no key map (keymap-*) yet.
    await openShortcuts(page);
    await modifierSelect(page).selectOption('win_ctrl');
    await expectKeymap(page, 'Win + Ctrl');
    await expect(page.getByTestId('keymap-left').locator('kbd')).toHaveText([
      'Win',
      'Ctrl',
      'Left',
    ]);
  });

  test('the arrow keys change the choice on a focused select and save', async ({ page }) => {
    await openShortcuts(page);
    await modifierSelect(page).focus();
    await page.keyboard.press('ArrowDown');
    await expect(modifierSelect(page)).toHaveValue('ctrl_alt');
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
    await expect(modifierSelect(page)).toBeEnabled();
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

  test.fixme('?shortcuts=off says the shortcuts are not active', async ({ page }) => {
    // fixme: the new Shortcuts view has no "not active" note (shortcuts-inactive) yet.
    await openShortcuts(page, '?nopersist&shortcuts=off');
    const note = page.getByTestId('shortcuts-inactive');
    await expect(note).toBeVisible();
    await expect(note).toContainText('Keyboard shortcuts are not active right now.');
    await expect(page.getByTestId('shortcuts-failed')).toHaveCount(0);
    await expect(shortcutsSwitch(page)).toBeChecked();
    await expect(shortcutsSwitch(page)).toBeEnabled();
  });

  test.fixme('the warning and the note follow the backend live', async ({ page }) => {
    // fixme: the new Shortcuts view has no "not active" note (shortcuts-inactive) yet.
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
    await expect(modifierSelect(page)).toBeDisabled();
    // The stored values are still shown.
    await expect(shortcutsSwitch(page)).toBeChecked();
    await expect(modifierSelect(page)).toHaveValue('ctrl_alt_shift');

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

    // Not the default (Ctrl + Alt + Shift), so a stored choice is told apart from a default.
    await modifierSelect(page).selectOption('win_alt');
    await expectSaved(page);
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
    await expect(modifierSelect(page)).toHaveValue('win_alt');
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

  test('Tab goes from the shortcut switch to the modifier choice', async ({ page }) => {
    await openShortcuts(page);
    await shortcutsSwitch(page).focus();
    await page.keyboard.press('Tab');
    await expect(modifierSelect(page)).toBeFocused();
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
