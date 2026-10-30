// Zone editor: templates, saving, splitting, dividers, deleting zones, undo.
//
// Pointer positions are given as fractions of the canvas box and converted to
// page coordinates with editorUi().pointAt(). Positions are chosen away from the
// 0.5 % rounding steps and the 1 % centre snap so one pixel of rounding never
// changes the outcome.

import { editorUi, expect, openApp, tabTo, test } from './fixtures';

const TOO_SMALL = 'the change would leave a zone smaller than the minimum size';

test.describe('templates and saving', () => {
  test('lists the four templates', async ({ page }) => {
    await openApp(page);
    for (const [id, name] of [
      ['halves', 'Halves'],
      ['thirds', 'Thirds'],
      ['grid', 'Grid 2x2'],
      ['priority', 'Priority'],
    ]) {
      await expect(page.getByTestId(`template-${id}`)).toContainText(name);
    }
  });

  test('applying the Grid template gives 4 zones, then Save keeps them', async ({ page }) => {
    await openApp(page);
    const ui = editorUi(page);

    await page.getByTestId('template-grid').click();

    await expect(ui.status).toHaveText('Applied the Grid 2x2 template. Save to keep it.');
    await expect(ui.status).toHaveAttribute('data-kind', 'success');
    await expect(ui.zones).toHaveCount(4);
    await expect(ui.caption).toContainText('4 zones.');
    await expect(ui.verticalDividers).toHaveCount(1);
    await expect(ui.horizontalDividers).toHaveCount(1);
    await expect(ui.zone(4)).toHaveAccessibleName('Zone 4, 1280 × 700, 50% × 50%');

    // Unsaved: the toolbar and the layout list say so, Save is enabled.
    await expect(ui.unsavedNote).toHaveText('Unsaved changes');
    await expect(ui.layoutItem('Halves')).toContainText('Unsaved');
    await expect(ui.tabs.nth(0)).toContainText('(unsaved changes)');
    await expect(ui.save).toBeEnabled();
    await expect(ui.revert).toBeEnabled();
    await expect(ui.undo).toBeEnabled();

    await ui.save.click();

    await expect(ui.status).toHaveText('Saved "Halves".');
    await expect(ui.status).toHaveAttribute('data-kind', 'success');
    await expect(ui.unsavedNote).toHaveCount(0);
    await expect(ui.layoutItem('Halves')).not.toContainText('Unsaved');
    await expect(ui.tabs.nth(0)).not.toContainText('(unsaved changes)');
    await expect(ui.save).toBeDisabled();
    await expect(ui.zones).toHaveCount(4);
  });

  test('a template replaces only the zones, the layout keeps its name', async ({ page }) => {
    await openApp(page);
    const ui = editorUi(page);
    await page.getByTestId('template-priority').click();
    await expect(ui.zones).toHaveCount(3);
    await expect(ui.layoutName).toHaveValue('Halves');
    await expect(ui.zone(1)).toHaveAccessibleName(/^Zone 1, \d+ × \d+, 25% × 100%$/);
    await expect(ui.zone(2)).toHaveAccessibleName(/^Zone 2, \d+ × \d+, 50% × 100%$/);
  });

  test('Undo and Revert both take back an applied template', async ({ page }) => {
    await openApp(page);
    const ui = editorUi(page);

    await page.getByTestId('template-grid').click();
    await expect(ui.zones).toHaveCount(4);
    await ui.undo.click();
    await expect(ui.status).toHaveText('Undid the last change.');
    await expect(ui.zones).toHaveCount(2);
    await expect(ui.unsavedNote).toHaveCount(0);
    await expect(ui.save).toBeDisabled();

    await page.getByTestId('template-thirds').click();
    await expect(ui.zones).toHaveCount(3);
    await ui.revert.click();
    await expect(ui.status).toHaveText('Discarded unsaved changes.');
    await expect(ui.zones).toHaveCount(2);
    await expect(ui.unsavedNote).toHaveCount(0);
  });

  test('unsaved edits survive switching monitor and view', async ({ page }) => {
    await openApp(page);
    const ui = editorUi(page);

    await page.getByTestId('template-grid').click();
    await expect(ui.zones).toHaveCount(4);

    await ui.tabs.nth(1).click();
    await expect(ui.canvas).toHaveAccessibleName('Layout Thirds on Display 2');
    await expect(ui.tabs.nth(0)).toContainText('(unsaved changes)');
    await expect(ui.unsavedNote).toHaveCount(0);

    await ui.tabs.nth(0).click();
    await expect(ui.zones).toHaveCount(4);
    await expect(ui.unsavedNote).toBeVisible();

    await page.getByTestId('nav-settings').click();
    await expect(page.getByTestId('settings')).toBeVisible();
    await page.getByTestId('nav-editor').click();
    await expect(ui.zones).toHaveCount(4);
    await expect(ui.unsavedNote).toBeVisible();
  });
});

test.describe('splitting zones with the pointer', () => {
  test('vertical split at a pointer position', async ({ page }) => {
    await openApp(page);
    const ui = editorUi(page);

    await ui.toolSplitVertical.click();
    await expect(ui.toolSplitVertical).toHaveAttribute('aria-pressed', 'true');
    await expect(ui.toolSelect).toHaveAttribute('aria-pressed', 'false');
    await expect(ui.canvas).toHaveAttribute('data-tool', 'split-vertical');

    // Zone 1 spans 0..50 % of the width; split it at 30 %.
    const at = await ui.pointAt(0.3, 0.5);
    await page.mouse.move(at.x, at.y);
    await expect(ui.preview).toBeVisible();
    await expect(ui.preview).toContainText('30%');
    await expect(ui.preview).not.toContainText('too close');

    await page.mouse.click(at.x, at.y);

    await expect(ui.status).toHaveText('Split zone 1 vertically at 30%; new zone 3.');
    await expect(ui.status).toHaveAttribute('data-kind', 'success');
    await expect(ui.zones).toHaveCount(3);
    await expect(ui.caption).toContainText('3 zones.');
    await expect(ui.zone(1)).toHaveAccessibleName('Zone 1, 768 × 1400, 30% × 100%');
    await expect(ui.zone(3)).toHaveAccessibleName(/^Zone 3, \d+ × \d+, 20% × 100%$/);
    await expect(ui.zone(2)).toHaveAccessibleName(/^Zone 2, \d+ × \d+, 50% × 100%$/);
    await expect(ui.unsavedNote).toBeVisible();
    await expect(ui.preview).toHaveCount(0);

    // Back in the Select tool the new zone is the selected one.
    await page.keyboard.press('Escape');
    await expect(ui.toolSelect).toHaveAttribute('aria-pressed', 'true');
    await expect(ui.zone(3)).toHaveAttribute('aria-pressed', 'true');
  });

  test('horizontal split at a pointer position', async ({ page }) => {
    await openApp(page);
    const ui = editorUi(page);

    await ui.toolSplitHorizontal.click();
    await expect(ui.toolSplitHorizontal).toHaveAttribute('aria-pressed', 'true');

    // Zone 2 is the right half; split it 30 % from the top.
    await ui.clickAt(0.75, 0.3);

    await expect(ui.status).toHaveText('Split zone 2 horizontally at 30%; new zone 3.');
    await expect(ui.zones).toHaveCount(3);
    await expect(ui.zone(2)).toHaveAccessibleName('Zone 2, 1280 × 420, 50% × 30%');
    await expect(ui.zone(3)).toHaveAccessibleName('Zone 3, 1280 × 980, 50% × 70%');
    await expect(ui.horizontalDividers).toHaveCount(1);
    await expect(ui.horizontalDividers).toHaveAttribute('data-pos', '0.3');
  });

  test('a split close to a zone centre snaps to the centre', async ({ page }) => {
    await openApp(page);
    const ui = editorUi(page);
    await ui.toolSplitVertical.click();
    // The centre of zone 1 is at 25 %; 0.4 % off the centre snaps to it.
    await ui.clickAt(0.254, 0.5);
    await expect(ui.status).toHaveText('Split zone 1 vertically at 25%; new zone 3.');
  });

  test('a split just inside the 5 % limit is accepted', async ({ page }) => {
    await openApp(page);
    const ui = editorUi(page);
    await ui.toolSplitVertical.click();
    await ui.clickAt(0.07, 0.5);
    await expect(ui.status).toHaveAttribute('data-kind', 'success');
    await expect(ui.status).toHaveText('Split zone 1 vertically at 7%; new zone 3.');
    await expect(ui.zones).toHaveCount(3);
  });

  const tooClose = [
    { name: 'vertical, 2% from the left screen edge', tool: 'v', fx: 0.02, fy: 0.5 },
    { name: "vertical, 3% from the zone's right edge", tool: 'v', fx: 0.47, fy: 0.5 },
    { name: 'horizontal, 3% from the top edge', tool: 'h', fx: 0.25, fy: 0.03 },
    { name: 'horizontal, 3% from the bottom edge', tool: 'h', fx: 0.75, fy: 0.97 },
  ];
  for (const c of tooClose) {
    test(`a split closer than 5% to an edge is refused with the backend's message (${c.name})`, async ({
      page,
    }) => {
      await openApp(page);
      const ui = editorUi(page);

      await page.keyboard.press(c.tool); // V / H shortcut
      await expect(c.tool === 'v' ? ui.toolSplitVertical : ui.toolSplitHorizontal).toHaveAttribute(
        'aria-pressed',
        'true',
      );

      const at = await ui.pointAt(c.fx, c.fy);
      await page.mouse.move(at.x, at.y);
      // The guide already warns before the click.
      await expect(ui.preview).toContainText('too close to the edge');

      await page.mouse.click(at.x, at.y);

      // The error text is the backend's, verbatim, in the aria-live status line.
      await expect(ui.status).toHaveText(TOO_SMALL);
      await expect(ui.status).toHaveAttribute('data-kind', 'error');
      await expect(ui.status).toHaveAttribute('role', 'status');
      await expect(ui.status).toHaveAttribute('aria-live', 'polite');

      // ...and the layout is exactly as before.
      await expect(ui.zones).toHaveCount(2);
      await expect(ui.zone(3)).toHaveCount(0);
      await expect(ui.caption).toContainText('2 zones.');
      await expect(ui.zone(1)).toHaveAccessibleName('Zone 1, 1280 × 1400, 50% × 100%');
      await expect(ui.zone(2)).toHaveAccessibleName('Zone 2, 1280 × 1400, 50% × 100%');
      await expect(ui.unsavedNote).toHaveCount(0);
      await expect(ui.save).toBeDisabled();
      await expect(ui.undo).toBeDisabled();
    });
  }

  test('a successful edit after an error replaces the error message', async ({ page }) => {
    await openApp(page);
    const ui = editorUi(page);
    await ui.toolSplitVertical.click();

    await ui.clickAt(0.02, 0.5);
    await expect(ui.status).toHaveAttribute('data-kind', 'error');

    await ui.clickAt(0.3, 0.5);
    await expect(ui.status).toHaveAttribute('data-kind', 'success');
    await expect(ui.status).toHaveText('Split zone 1 vertically at 30%; new zone 3.');
  });

  test('Escape leaves the split tool and removes the guide', async ({ page }) => {
    await openApp(page);
    const ui = editorUi(page);
    await ui.toolSplitVertical.click();
    const at = await ui.pointAt(0.3, 0.5);
    await page.mouse.move(at.x, at.y);
    await expect(ui.preview).toBeVisible();

    await page.keyboard.press('Escape');
    await expect(ui.toolSelect).toHaveAttribute('aria-pressed', 'true');
    await expect(ui.preview).toHaveCount(0);
    await expect(ui.canvas).toHaveAttribute('data-tool', 'select');
  });
});

test.describe('dragging dividers', () => {
  test('dragging a divider previews live and commits on release', async ({ page }) => {
    await openApp(page);
    const ui = editorUi(page);

    const box = await ui.dividers.boundingBox();
    const canvas = await ui.canvas.boundingBox();
    if (!box || !canvas) throw new Error('missing bounding boxes');
    const x = box.x + box.width / 2;
    const y = box.y + box.height / 2;

    await page.mouse.move(x, y);
    await page.mouse.down();
    await page.mouse.move(x + canvas.width * 0.1, y, { steps: 6 });

    // Still dragging: the zones follow the pointer, nothing has been committed.
    await expect(ui.zone(1)).toHaveAccessibleName(/^Zone 1, \d+ × \d+, 60% × /);
    await expect(ui.zone(2)).toHaveAccessibleName(/^Zone 2, \d+ × \d+, 40% × /);
    await expect(ui.canvas).toHaveClass(/dragging/);
    await expect(ui.save).toBeDisabled();
    await expect(ui.status).toHaveText('');

    await page.mouse.up();

    await expect(ui.status).toHaveText('Moved the vertical divider from 50% to 60%.');
    await expect(ui.status).toHaveAttribute('data-kind', 'success');
    await expect(ui.dividers).toHaveAttribute('data-pos', '0.6');
    await expect(ui.zone(1)).toHaveAccessibleName(/^Zone 1, \d+ × \d+, 60% × /);
    await expect(ui.zone(2)).toHaveAccessibleName(/^Zone 2, \d+ × \d+, 40% × /);
    await expect(ui.save).toBeEnabled();
    await expect(ui.canvas).not.toHaveClass(/dragging/);
  });

  test('Escape during a drag cancels it', async ({ page }) => {
    await openApp(page);
    const ui = editorUi(page);

    const box = await ui.dividers.boundingBox();
    const canvas = await ui.canvas.boundingBox();
    if (!box || !canvas) throw new Error('missing bounding boxes');
    const x = box.x + box.width / 2;
    const y = box.y + box.height / 2;

    await page.mouse.move(x, y);
    await page.mouse.down();
    await page.mouse.move(x + canvas.width * 0.2, y, { steps: 6 });
    await expect(ui.zone(1)).toHaveAccessibleName(/^Zone 1, \d+ × \d+, 70% × /);

    await page.keyboard.press('Escape');
    await expect(ui.zone(1)).toHaveAccessibleName(/^Zone 1, \d+ × \d+, 50% × /);
    await expect(ui.canvas).not.toHaveClass(/dragging/);

    await page.mouse.up();

    // Nothing was committed.
    await expect(ui.status).toHaveText('');
    await expect(ui.dividers).toHaveAttribute('data-pos', '0.5');
    await expect(ui.zone(1)).toHaveAccessibleName(/^Zone 1, \d+ × \d+, 50% × /);
    await expect(ui.save).toBeDisabled();
    await expect(ui.undo).toBeDisabled();
  });

  test('releasing without moving is not an edit', async ({ page }) => {
    await openApp(page);
    const ui = editorUi(page);
    const box = await ui.dividers.boundingBox();
    if (!box) throw new Error('missing bounding box');
    await page.mouse.move(box.x + box.width / 2, box.y + box.height / 2);
    await page.mouse.down();
    await page.mouse.up();
    await expect(ui.status).toHaveText('');
    await expect(ui.save).toBeDisabled();
  });

  for (const target of [
    { name: 'far right is clamped to 95%', to: 'right', pos: '0.95', text: '50% to 95%' },
    { name: 'far left is clamped to 5%', to: 'left', pos: '0.05', text: '50% to 5%' },
  ]) {
    test(`a drag ${target.name}`, async ({ page }) => {
      await openApp(page);
      const ui = editorUi(page);
      const box = await ui.dividers.boundingBox();
      const canvas = await ui.canvas.boundingBox();
      if (!box || !canvas) throw new Error('missing bounding boxes');
      const x = box.x + box.width / 2;
      const y = box.y + box.height / 2;
      const farX = target.to === 'right' ? canvas.x + canvas.width + 60 : 2;

      await page.mouse.move(x, y);
      await page.mouse.down();
      await page.mouse.move(farX, y, { steps: 8 });
      await page.mouse.up();

      await expect(ui.status).toHaveText(`Moved the vertical divider from ${target.text}.`);
      await expect(ui.dividers).toHaveAttribute('data-pos', target.pos);
    });
  }

  test('a horizontal divider drags vertically', async ({ page }) => {
    await openApp(page);
    const ui = editorUi(page);
    await page.getByTestId('template-grid').click();
    await expect(ui.horizontalDividers).toHaveCount(1);

    const box = await ui.horizontalDividers.boundingBox();
    const canvas = await ui.canvas.boundingBox();
    if (!box || !canvas) throw new Error('missing bounding boxes');
    const x = box.x + box.width / 2;
    const y = box.y + box.height / 2;

    await page.mouse.move(x, y);
    await page.mouse.down();
    await page.mouse.move(x, y + canvas.height * 0.2, { steps: 6 });
    await page.mouse.up();

    await expect(ui.status).toHaveText('Moved the horizontal divider from 50% to 70%.');
    await expect(ui.horizontalDividers).toHaveAttribute('data-pos', '0.7');
    await expect(ui.verticalDividers).toHaveAttribute('data-pos', '0.5');
    await expect(ui.zone(1)).toHaveAccessibleName(/^Zone 1, \d+ × \d+, 50% × 70%$/);
    await expect(ui.zone(3)).toHaveAccessibleName(/^Zone 3, \d+ × \d+, 50% × 30%$/);
  });

  test('dividers are not draggable while a split tool is active', async ({ page }) => {
    await openApp(page);
    const ui = editorUi(page);
    await ui.toolSplitVertical.click();
    await expect(ui.dividers).toHaveCSS('pointer-events', 'none');
    await expect(ui.dividers).toHaveAttribute('tabindex', '-1');
  });
});

test.describe('moving dividers with the keyboard', () => {
  test('arrow keys move by 1%, Shift+arrow by 5%, and focus stays on the divider', async ({
    page,
  }) => {
    await openApp(page);
    const ui = editorUi(page);
    const divider = ui.dividers;

    await expect(divider).toHaveAttribute('role', 'separator');
    await expect(divider).toHaveAttribute('aria-orientation', 'vertical');
    await expect(divider).toHaveAttribute('aria-valuenow', '50');
    await expect(divider).toHaveAccessibleName('Vertical divider at 50%');
    await expect(divider).toHaveAttribute('tabindex', '0');

    await divider.focus();
    await page.keyboard.press('ArrowRight');
    await expect(ui.status).toHaveText('Moved the vertical divider from 50% to 51%.');
    await expect(divider).toHaveAttribute('data-pos', '0.51');
    await expect(divider).toHaveAttribute('aria-valuenow', '51');
    await expect(divider).toHaveAccessibleName('Vertical divider at 51%');
    await expect(divider).toBeFocused();

    await page.keyboard.press('Shift+ArrowLeft');
    await expect(ui.status).toHaveText('Moved the vertical divider from 51% to 46%.');
    await expect(divider).toHaveAttribute('data-pos', '0.46');
    await expect(divider).toBeFocused();

    await expect(ui.zone(1)).toHaveAccessibleName(/^Zone 1, \d+ × \d+, 46% × /);
    await expect(ui.zone(2)).toHaveAccessibleName(/^Zone 2, \d+ × \d+, 54% × /);
    await expect(ui.save).toBeEnabled();
  });

  test('a horizontal divider uses Up and Down', async ({ page }) => {
    await openApp(page);
    const ui = editorUi(page);
    await page.getByTestId('template-grid').click();
    const divider = ui.horizontalDividers;
    await expect(divider).toHaveAttribute('aria-orientation', 'horizontal');

    await divider.focus();
    await page.keyboard.press('ArrowDown');
    await expect(ui.status).toHaveText('Moved the horizontal divider from 50% to 51%.');
    await expect(divider).toHaveAttribute('data-pos', '0.51');

    await page.keyboard.press('Shift+ArrowUp');
    await expect(divider).toHaveAttribute('data-pos', '0.46');

    // Left/Right do nothing on a horizontal divider.
    await page.keyboard.press('ArrowRight');
    await expect(divider).toHaveAttribute('data-pos', '0.46');
  });

  test('pushing a divider into the 5% limit shows the backend error and keeps the position', async ({
    page,
  }) => {
    await openApp(page);
    const ui = editorUi(page);
    await ui.dividers.focus();

    // 50 % -> 95 % in nine 5 % steps; wait for each so no key press races the edit.
    for (let pos = 55; pos <= 95; pos += 5) {
      await page.keyboard.press('Shift+ArrowRight');
      await expect(ui.dividers).toHaveAttribute('data-pos', String(pos / 100));
    }
    await expect(ui.status).toHaveText('Moved the vertical divider from 90% to 95%.');

    await page.keyboard.press('Shift+ArrowRight'); // would be 100 %
    await expect(ui.status).toHaveText(TOO_SMALL);
    await expect(ui.status).toHaveAttribute('data-kind', 'error');
    await expect(ui.dividers).toHaveAttribute('data-pos', '0.95');
    await expect(ui.zone(2)).toHaveAccessibleName(/^Zone 2, \d+ × \d+, 5% × /);
  });
});

test.describe('deleting zones', () => {
  test('deleting a zone merges it into its neighbour, and Undo brings it back', async ({
    page,
  }) => {
    await openApp(page);
    const ui = editorUi(page);

    await expect(ui.deleteZone).toBeDisabled();
    await ui.zone(2).click();
    await expect(ui.zone(2)).toHaveAttribute('aria-pressed', 'true');
    await expect(ui.zone(1)).toHaveAttribute('aria-pressed', 'false');
    await expect(ui.deleteZone).toBeEnabled();

    await ui.deleteZone.click();

    await expect(ui.status).toHaveText('Deleted zone 2; zone 1 took its space.');
    await expect(ui.status).toHaveAttribute('data-kind', 'success');
    await expect(ui.zone(2)).toHaveCount(0);
    await expect(ui.zones).toHaveCount(1);
    await expect(ui.caption).toContainText('1 zone.');
    await expect(ui.zone(1)).toHaveAccessibleName(/^Zone 1, \d+ × \d+, 100% × 100%$/);
    await expect(ui.dividers).toHaveCount(0);
    await expect(ui.unsavedNote).toBeVisible();

    await ui.undo.click();

    await expect(ui.status).toHaveText('Undid the last change.');
    await expect(ui.zones).toHaveCount(2);
    await expect(ui.zone(2)).toBeVisible();
    await expect(ui.zone(1)).toHaveAccessibleName(/^Zone 1, \d+ × \d+, 50% × 100%$/);
    await expect(ui.dividers).toHaveCount(1);
    // Back to the saved layout: nothing left to save.
    await expect(ui.unsavedNote).toHaveCount(0);
    await expect(ui.save).toBeDisabled();
    await expect(ui.undo).toBeDisabled();
  });

  test('the Delete key and Ctrl+Z work from the keyboard', async ({ page }) => {
    await openApp(page);
    const ui = editorUi(page);

    await ui.zone(1).focus();
    await page.keyboard.press('Delete');
    await expect(ui.status).toHaveText('Deleted zone 1; zone 2 took its space.');
    await expect(ui.zones).toHaveCount(1);

    await page.keyboard.press('ControlOrMeta+z');
    await expect(ui.status).toHaveText('Undid the last change.');
    await expect(ui.zones).toHaveCount(2);
  });

  test('Enter and Space select a zone from the keyboard', async ({ page }) => {
    await openApp(page);
    const ui = editorUi(page);
    await ui.zone(2).focus();
    await page.keyboard.press('Enter');
    await expect(ui.zone(2)).toHaveAttribute('aria-pressed', 'true');
    await ui.zone(1).focus();
    await page.keyboard.press('Space');
    await expect(ui.zone(1)).toHaveAttribute('aria-pressed', 'true');
    await expect(ui.zone(2)).toHaveAttribute('aria-pressed', 'false');
  });

  test('the last zone of a layout cannot be deleted', async ({ page }) => {
    await openApp(page);
    const ui = editorUi(page);

    // A new layout is a single full-screen zone.
    await ui.newLayout.click();
    await expect(ui.status).toContainText('Created "Layout');
    await expect(ui.zones).toHaveCount(1);

    await ui.zone(1).click();
    await expect(ui.deleteZone).toBeEnabled();
    await ui.deleteZone.click();

    await expect(ui.status).toHaveText('a layout needs at least one zone');
    await expect(ui.status).toHaveAttribute('data-kind', 'error');
    await expect(ui.zones).toHaveCount(1);
    await expect(ui.zone(1)).toBeVisible();
    await expect(ui.unsavedNote).toHaveCount(0);

    // Same through the keyboard.
    await ui.zone(1).focus();
    await page.keyboard.press('Delete');
    await expect(ui.status).toHaveText('a layout needs at least one zone');
    await expect(ui.zones).toHaveCount(1);
  });

  test('a zone without a neighbour that shares a full edge cannot be merged', async ({
    page,
  }) => {
    await openApp(page);
    const ui = editorUi(page);
    await page.getByTestId('template-grid').click();
    await expect(ui.zones).toHaveCount(4);

    // Cut the two neighbours of zone 1 so that neither shares a full edge with it.
    await page.keyboard.press('h');
    await ui.clickAt(0.75, 0.25); // zone 2 -> top/bottom halves
    await expect(ui.status).toHaveText('Split zone 2 horizontally at 25%; new zone 5.');
    await page.keyboard.press('v');
    await ui.clickAt(0.25, 0.75); // zone 3 -> left/right halves
    await expect(ui.status).toHaveText('Split zone 3 vertically at 25%; new zone 6.');
    await page.keyboard.press('Escape');
    await expect(ui.zones).toHaveCount(6);

    await ui.zone(1).click();
    await ui.deleteZone.click();

    await expect(ui.status).toHaveText('zone 1 has no neighbour it can merge into');
    await expect(ui.status).toHaveAttribute('data-kind', 'error');
    await expect(ui.zones).toHaveCount(6);
    await expect(ui.zone(1)).toBeVisible();
  });
});

test.describe('layouts', () => {
  test('New layout creates a one-zone layout; rename, save and set it active', async ({
    page,
  }) => {
    await openApp(page);
    const ui = editorUi(page);

    await expect(ui.layoutItems).toHaveCount(4);
    await ui.newLayout.click();

    await expect(ui.status).toContainText('Created "Layout');
    await expect(ui.status).toHaveAttribute('data-kind', 'success');
    await expect(ui.layoutItems).toHaveCount(5);
    await expect(ui.layoutName).toHaveValue(/^Layout \d+$/);
    await expect(ui.zones).toHaveCount(1);
    await expect(ui.caption).toContainText('1 zone.');
    await expect(ui.setActive).toBeEnabled();
    await expect(ui.setActive).toContainText('Set as active');
    await expect(ui.layoutItems.nth(4)).toHaveAttribute('aria-current', 'true');
    await expect(ui.canvas).toHaveAccessibleName(/^Layout Layout \d+ on Display 1/);

    // Rename: part of the unsaved draft until Save.
    await ui.layoutName.fill('Focus mode');
    await expect(ui.layoutItem('Focus mode')).toContainText('Unsaved');
    await expect(ui.canvas).toHaveAccessibleName('Layout Focus mode on Display 1 (primary)');
    await ui.save.click();
    await expect(ui.status).toHaveText('Saved "Focus mode".');
    await expect(ui.layoutItem('Focus mode')).not.toContainText('Unsaved');

    // Set as active.
    await ui.setActive.click();
    await expect(ui.status).toHaveText('"Focus mode" is now the active layout on Display 1 (primary).');
    await expect(ui.setActive).toBeDisabled();
    await expect(ui.setActive).toContainText('Active layout');
    await expect(ui.layoutItem('Focus mode')).toContainText('Active');
    await expect(ui.layoutItem('Halves')).not.toContainText('Active');
  });

  test('"Set as active" saves unsaved edits of that layout first', async ({ page }) => {
    await openApp(page);
    const ui = editorUi(page);

    await ui.layoutItem('Thirds').click();
    await page.getByTestId('template-grid').click();
    await expect(ui.layoutItem('Thirds')).toContainText('Unsaved');

    await ui.setActive.click();
    await expect(ui.status).toHaveText('"Thirds" is now the active layout on Display 1 (primary).');
    await expect(ui.layoutItem('Thirds')).toContainText('Active');
    await expect(ui.layoutItem('Thirds')).not.toContainText('Unsaved');
    await expect(ui.zones).toHaveCount(4);
    await expect(ui.save).toBeDisabled();
  });

  test('a blank name is saved as "Untitled layout"', async ({ page }) => {
    await openApp(page);
    const ui = editorUi(page);
    await ui.layoutName.fill('   ');
    await ui.save.click();
    await expect(ui.status).toHaveText('Saved "Untitled layout".');
    await expect(ui.layoutItem('Untitled layout')).toBeVisible();
  });

  test('selecting another layout edits that layout', async ({ page }) => {
    await openApp(page);
    const ui = editorUi(page);
    await ui.layoutItem('Priority').click();
    await expect(ui.canvas).toHaveAccessibleName('Layout Priority on Display 1 (primary)');
    await expect(ui.zones).toHaveCount(3);
    await expect(ui.layoutItem('Priority')).toHaveAttribute('aria-current', 'true');
    await expect(ui.layoutItem('Halves')).not.toHaveAttribute('aria-current', /.+/);
    await expect(ui.setActive).toBeEnabled();
    // Selecting is not activating.
    await expect(ui.layoutItem('Halves')).toContainText('Active');
  });

  test('deleting a layout takes two presses', async ({ page }) => {
    await openApp(page);
    const ui = editorUi(page);

    await ui.newLayout.click();
    await expect(ui.layoutItems).toHaveCount(5);
    const name = await ui.layoutName.inputValue();

    // First press only arms the button.
    await ui.deleteLayout.click();
    await expect(ui.deleteLayout).toContainText('Confirm delete');
    await expect(ui.status).toHaveAttribute('data-kind', 'info');
    await expect(ui.status).toHaveText(
      `Press "Delete layout" again within 5 seconds to delete "${name}".`,
    );
    await expect(ui.layoutItems).toHaveCount(5);

    // Second press deletes.
    await ui.deleteLayout.click();
    await expect(ui.status).toHaveText(`Deleted layout "${name}".`);
    await expect(ui.layoutItems).toHaveCount(4);
    await expect(ui.deleteLayout).toContainText('Delete layout');
    await expect(ui.layoutItem(name)).toHaveCount(0);
  });

  test('picking another layout disarms the delete confirmation', async ({ page }) => {
    await openApp(page);
    const ui = editorUi(page);

    await ui.layoutItem('Thirds').click();
    await ui.deleteLayout.click();
    await expect(ui.deleteLayout).toContainText('Confirm delete');

    await ui.layoutItem('Priority').click();
    await expect(ui.deleteLayout).toContainText('Delete layout');
    await expect(ui.deleteLayout).not.toContainText('Confirm');

    // So one more press only arms again; nothing was deleted.
    await ui.deleteLayout.click();
    await expect(ui.deleteLayout).toContainText('Confirm delete');
    await expect(ui.layoutItems).toHaveCount(4);
  });

  test('the delete confirmation expires after 5 seconds', async ({ page }) => {
    await page.clock.install();
    await openApp(page);
    const ui = editorUi(page);

    await ui.deleteLayout.click();
    await expect(ui.deleteLayout).toContainText('Confirm delete');

    await page.clock.fastForward(5_500);
    await expect(ui.deleteLayout).toContainText('Delete layout');
    await expect(ui.deleteLayout).not.toContainText('Confirm');
    await expect(ui.layoutItems).toHaveCount(4);
  });

  test('deleting the active layout hands activation to the first remaining one', async ({
    page,
  }) => {
    await openApp(page);
    const ui = editorUi(page);

    await expect(ui.layoutItem('Halves')).toContainText('Active');
    await ui.deleteLayout.click();
    await ui.deleteLayout.click();

    await expect(ui.status).toHaveText('Deleted layout "Halves".');
    await expect(ui.layoutItems).toHaveCount(3);
    await expect(ui.layoutItem('Halves')).toHaveCount(0);
    await expect(ui.layoutItems.first()).toContainText('Thirds');
    await expect(ui.layoutItems.first()).toContainText('Active');
    await expect(ui.canvas).toHaveAccessibleName('Layout Thirds on Display 1 (primary)');
  });

  test('the last remaining layout of a monitor cannot be deleted', async ({ page }) => {
    await openApp(page);
    const ui = editorUi(page);

    for (let left = 3; left >= 1; left--) {
      await ui.deleteLayout.click();
      await ui.deleteLayout.click();
      await expect(ui.layoutItems).toHaveCount(left);
    }
    await expect(ui.deleteLayout).toBeDisabled();
    await expect(ui.deleteLayout).toHaveAttribute('title', 'A monitor needs at least one layout');
  });

  test('layouts of one monitor are independent of the other', async ({ page }) => {
    await openApp(page);
    const ui = editorUi(page);
    await ui.newLayout.click();
    await expect(ui.layoutItems).toHaveCount(5);

    await ui.tabs.nth(1).click();
    await expect(ui.layoutItems).toHaveCount(4);
  });
});

test.describe('keyboard operation of the editor', () => {
  test('V, H and Escape switch tools', async ({ page }) => {
    await openApp(page);
    const ui = editorUi(page);

    await page.keyboard.press('v');
    await expect(ui.toolSplitVertical).toHaveAttribute('aria-pressed', 'true');
    await page.keyboard.press('h');
    await expect(ui.toolSplitHorizontal).toHaveAttribute('aria-pressed', 'true');
    await expect(ui.toolSplitVertical).toHaveAttribute('aria-pressed', 'false');
    await page.keyboard.press('Escape');
    await expect(ui.toolSelect).toHaveAttribute('aria-pressed', 'true');
  });

  test('typing in the layout name field does not trigger shortcuts', async ({ page }) => {
    await openApp(page);
    const ui = editorUi(page);
    await ui.layoutName.click();
    await ui.layoutName.pressSequentially('vh');
    await expect(ui.layoutName).toHaveValue('Halvesvh');
    await expect(ui.toolSelect).toHaveAttribute('aria-pressed', 'true');
    await expect(ui.toolSplitVertical).toHaveAttribute('aria-pressed', 'false');
  });

  test('Ctrl+S saves the layout', async ({ page }) => {
    await openApp(page);
    const ui = editorUi(page);
    await page.getByTestId('template-grid').click();
    await expect(ui.unsavedNote).toBeVisible();
    await page.keyboard.press('ControlOrMeta+s');
    await expect(ui.status).toHaveText('Saved "Halves".');
    await expect(ui.unsavedNote).toHaveCount(0);
  });

  test('a zone can be split with the keyboard alone', async ({ page }) => {
    await openApp(page);
    const ui = editorUi(page);

    await page.keyboard.press('v');
    await expect(ui.toolSplitVertical).toHaveAttribute('aria-pressed', 'true');

    // Reach zone 1 with Tab, as a keyboard user would.
    const zone1 = ui.zone(1);
    // The split help is the zone's accessible description (its name stays the visible text).
    await expect(zone1).toHaveAccessibleDescription(/Press Enter to split with a vertical divider/);
    await tabTo(page, zone1);
    await expect(zone1).toBeFocused();

    // The guide starts at the zone centre (25 %); arrows move it 1 %, Shift 5 %.
    await expect(ui.preview).toContainText('25%');
    await page.keyboard.press('ArrowRight');
    await expect(ui.preview).toContainText('26%');
    await page.keyboard.press('Shift+ArrowRight');
    await expect(ui.preview).toContainText('31%');
    await page.keyboard.press('ArrowLeft');
    await expect(ui.preview).toContainText('30%');

    await page.keyboard.press('Enter');
    await expect(ui.status).toHaveText('Split zone 1 vertically at 30%; new zone 3.');
    await expect(ui.zones).toHaveCount(3);
    await expect(ui.zone(1)).toHaveAccessibleName(/^Zone 1, \d+ × \d+, 30% × /);
  });

  test('a keyboard split too close to the edge reports the backend error', async ({ page }) => {
    await openApp(page);
    const ui = editorUi(page);
    await page.keyboard.press('v');
    await tabTo(page, ui.zone(1));

    // 25 % -> 0 % in 5 % steps: a guide on the zone start leaves a zero-width part.
    for (let i = 0; i < 5; i++) await page.keyboard.press('Shift+ArrowLeft');
    await expect(ui.preview).toContainText('too close to the edge');
    await page.keyboard.press('Enter');
    await expect(ui.status).toHaveText(TOO_SMALL);
    await expect(ui.status).toHaveAttribute('data-kind', 'error');
    await expect(ui.zones).toHaveCount(2);
  });
});
