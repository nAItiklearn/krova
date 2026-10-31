// Export and import of layouts as text (the "Export and import" card under the layout list).
// Everything goes through the clipboard and two text areas: no file dialogs, no network.
//
// Errors of the backend are shown verbatim in the status line under the canvas
// (data-testid="status"); the pasted text and the panel stay where they are.

import type { Page } from '@playwright/test';
import { editorUi, expect, openApp, tabTo, test } from './fixtures';

const exportButton = (page: Page) => page.getByTestId('export-layout');
const importButton = (page: Page) => page.getByTestId('import-layout');
const exportBox = (page: Page) => page.getByTestId('export-text');
const importBox = (page: Page) => page.getByTestId('import-text');

interface Envelope {
  krova_layout: number;
  layout: { id: string; name: string; zones: Array<{ id: number; rect: Record<string, number> }> };
}

/** Open the export panel and wait for its text. */
async function openExport(page: Page): Promise<void> {
  await exportButton(page).click();
  await expect(exportBox(page)).toHaveValue(/"krova_layout": 1/);
}

async function exported(page: Page): Promise<Envelope> {
  return JSON.parse(await exportBox(page).inputValue()) as Envelope;
}

/** Paste `text` into the import panel (opening it if needed) and press Import. */
async function importText(page: Page, text: string): Promise<void> {
  if ((await importButton(page).getAttribute('aria-expanded')) !== 'true') {
    await importButton(page).click();
  }
  await importBox(page).fill(text);
  await page.getByTestId('import-submit').click();
}

const unit = { x: 0, y: 0, w: 1, h: 1 };
const file = (layout: unknown, version: unknown = 1): string =>
  JSON.stringify({ krova_layout: version, layout });
const goodFile = file({ id: 'pasted', name: 'Pasted layout', zones: [{ id: 1, rect: unit }] });

test.describe('the Export and import card', () => {
  test('is the last card of the layout panel and starts closed', async ({ page }) => {
    await openApp(page);
    const card = page.getByTestId('layout-transfer');
    await expect(card).toBeVisible();
    await expect(page.getByRole('heading', { name: 'Export and import', level: 3 })).toBeVisible();
    await expect(exportButton(page)).toHaveText(/Export layout/);
    await expect(importButton(page)).toHaveText(/Import layout/);
    await expect(exportButton(page)).toHaveAttribute('aria-expanded', 'false');
    await expect(importButton(page)).toHaveAttribute('aria-expanded', 'false');
    await expect(page.getByTestId('export-panel')).toHaveCount(0);
    await expect(page.getByTestId('import-panel')).toHaveCount(0);
    // After the templates in the DOM, so the existing Tab order is unchanged.
    const order = await page.evaluate(() => {
      const t = document.querySelector('[data-testid="template-priority"]');
      const x = document.querySelector('[data-testid="export-layout"]');
      return t && x ? Boolean(t.compareDocumentPosition(x) & Node.DOCUMENT_POSITION_FOLLOWING) : null;
    });
    expect(order).toBe(true);
  });

  test('only one panel is open at a time', async ({ page }) => {
    await openApp(page);
    await openExport(page);
    await importButton(page).click();
    await expect(page.getByTestId('import-panel')).toBeVisible();
    await expect(page.getByTestId('export-panel')).toHaveCount(0);
    await expect(exportButton(page)).toHaveAttribute('aria-expanded', 'false');
    await expect(importButton(page)).toHaveAttribute('aria-expanded', 'true');
  });
});

test.describe('Export', () => {
  test('opens a read-only text area with the envelope and the layout', async ({ page }) => {
    await openApp(page);
    await exportButton(page).click();

    const box = exportBox(page);
    await expect(page.getByTestId('export-panel')).toBeVisible();
    await expect(exportButton(page)).toHaveAttribute('aria-expanded', 'true');
    await expect(box).toHaveAttribute('readonly', '');
    await expect(box).toBeFocused();
    await expect(box).toHaveAccessibleName('Layout text');
    await expect(box).toHaveValue(/"krova_layout": 1/);
    await expect(box).toHaveValue(/"name": "Halves"/);
    await expect(box).toHaveValue(/"id": "halves"/);

    const env = await exported(page);
    expect(env.krova_layout).toBe(1);
    expect(env.layout.id).toBe('halves');
    expect(env.layout.name).toBe('Halves');
    expect(env.layout.zones).toEqual([
      { id: 1, rect: { x: 0, y: 0, w: 0.5, h: 1 } },
      { id: 2, rect: { x: 0.5, y: 0, w: 0.5, h: 1 } },
    ]);
    // Pretty printed.
    expect(await box.inputValue()).toMatch(/^\{\n {2}"krova_layout": 1,\n {2}"layout": \{/);
  });

  test('the text follows the layout on screen: edits, other layouts and other monitors', async ({
    page,
  }) => {
    await openApp(page);
    const ui = editorUi(page);
    await openExport(page);
    await expect(page.locator('#export-help')).not.toContainText('unsaved');

    // An unsaved edit is exported too, and the panel says so.
    await page.getByTestId('template-grid').click();
    await expect(ui.zones).toHaveCount(4);
    await expect.poll(async () => (await exported(page)).layout.zones.length).toBe(4);
    expect((await exported(page)).layout.name).toBe('Halves');
    await expect(page.locator('#export-help')).toContainText('Includes your unsaved changes.');

    // A renamed layout carries its new name.
    await ui.layoutName.fill('My grid');
    await expect.poll(async () => (await exported(page)).layout.name).toBe('My grid');

    // Another layout of the same monitor.
    await ui.layoutItem('Priority').click();
    await expect.poll(async () => (await exported(page)).layout.id).toBe('priority');
    expect((await exported(page)).layout.zones).toHaveLength(3);
    expect((await exported(page)).layout.zones[0].rect.w).toBe(0.25);

    // The other monitor.
    await ui.tabs.nth(1).click();
    await expect.poll(async () => (await exported(page)).layout.id).toBe('thirds');
    await expect(exportBox(page)).toHaveValue(/"name": "Thirds"/);
  });

  test('a blank layout name is exported as "Untitled layout", like Save does', async ({ page }) => {
    await openApp(page);
    const ui = editorUi(page);
    await ui.layoutName.fill('   ');
    await openExport(page);
    expect((await exported(page)).layout.name).toBe('Untitled layout');
  });

  test('Close hides the panel and puts the focus back on Export layout', async ({ page }) => {
    await openApp(page);
    await openExport(page);
    await page.getByTestId('export-close').click();
    await expect(page.getByTestId('export-panel')).toHaveCount(0);
    await expect(exportButton(page)).toHaveAttribute('aria-expanded', 'false');
    await expect(exportButton(page)).toBeFocused();
  });

  test('Escape closes the panel, and pressing Export layout again toggles it', async ({ page }) => {
    await openApp(page);
    await openExport(page);
    await page.keyboard.press('Escape');
    await expect(page.getByTestId('export-panel')).toHaveCount(0);
    await expect(exportButton(page)).toBeFocused();

    await exportButton(page).click();
    await expect(page.getByTestId('export-panel')).toBeVisible();
    await exportButton(page).click();
    await expect(page.getByTestId('export-panel')).toHaveCount(0);
  });

  test('Escape in the panel does not also leave the split tool', async ({ page }) => {
    await openApp(page);
    const ui = editorUi(page);
    await ui.toolSplitVertical.click();
    await openExport(page);
    await page.keyboard.press('Escape');
    await expect(page.getByTestId('export-panel')).toHaveCount(0);
    await expect(ui.toolSplitVertical).toHaveAttribute('aria-pressed', 'true');
  });

  test('keys typed in the text area do not trigger editor shortcuts', async ({ page }) => {
    await openApp(page);
    const ui = editorUi(page);
    await openExport(page);
    await exportBox(page).focus();
    await page.keyboard.press('v');
    await page.keyboard.press('h');
    await expect(ui.toolSelect).toHaveAttribute('aria-pressed', 'true');
  });
});

test.describe('Copy', () => {
  test('puts the text on the clipboard, says so, and shows "Copied" for a moment', async ({
    page,
    context,
  }) => {
    await context.grantPermissions(['clipboard-read', 'clipboard-write']);
    await page.clock.install();
    await openApp(page);
    const ui = editorUi(page);
    await openExport(page);

    const copy = page.getByTestId('export-copy');
    await expect(copy).toHaveText('Copy');
    await copy.click();

    await expect(copy).toHaveText('Copied');
    await expect(copy).toHaveAttribute('data-copied', 'true');
    await expect(ui.status).toHaveText('Copied the layout text to the clipboard.');
    await expect(ui.status).toHaveAttribute('data-kind', 'success');
    // Windows clipboards hold CRLF line ends; the text itself is the same.
    const clipboard = (await page.evaluate(() => navigator.clipboard.readText())).replace(
      /\r\n/g,
      '\n',
    );
    expect(clipboard).toBe(await exportBox(page).inputValue());
    expect((JSON.parse(clipboard) as Envelope).layout.id).toBe('halves');

    await page.clock.fastForward(3_000);
    await expect(copy).toHaveText('Copy');
    await expect(copy).toHaveAttribute('data-copied', 'false');
  });

  test('what was copied can be imported again', async ({ page, context }) => {
    await context.grantPermissions(['clipboard-read', 'clipboard-write']);
    await openApp(page);
    const ui = editorUi(page);
    await openExport(page);
    await page.getByTestId('export-copy').click();
    await expect(ui.status).toHaveText('Copied the layout text to the clipboard.');

    await importButton(page).click();
    await importBox(page).focus();
    await page.keyboard.press('ControlOrMeta+v');
    await expect(importBox(page)).toHaveValue(/"krova_layout": 1/);
    await page.getByTestId('import-submit').click();
    await expect(ui.status).toContainText('Imported "Halves"');
    await expect(ui.layoutItems).toHaveCount(5);
  });

  test.describe('when the clipboard cannot be written', () => {
    test('no clipboard API: the text is selected and the status line says to press Ctrl+C', async ({
      page,
    }) => {
      await page.addInitScript(() => {
        Object.defineProperty(navigator, 'clipboard', { value: undefined, configurable: true });
        document.execCommand = () => false;
      });
      await openApp(page);
      const ui = editorUi(page);
      await openExport(page);
      await page.getByTestId('export-copy').click();

      await expect(ui.status).toHaveText(
        /Could not copy automatically\. The text is selected: press (Ctrl|Cmd)\+C to copy it\./,
      );
      await expect(ui.status).toHaveAttribute('data-kind', 'info');
      await expect(page.getByTestId('export-copy')).toHaveText('Copy');
      await expect(exportBox(page)).toBeFocused();
      const [start, end, length] = await exportBox(page).evaluate((el: HTMLTextAreaElement) => [
        el.selectionStart,
        el.selectionEnd,
        el.value.length,
      ]);
      expect(start).toBe(0);
      expect(end).toBe(length);
      expect(length).toBeGreaterThan(50);
    });

    test('a refused write falls back the same way', async ({ page }) => {
      await page.addInitScript(() => {
        Object.defineProperty(navigator, 'clipboard', {
          value: { writeText: () => Promise.reject(new DOMException('no', 'NotAllowedError')) },
          configurable: true,
        });
        document.execCommand = () => false;
      });
      await openApp(page);
      const ui = editorUi(page);
      await openExport(page);
      await page.getByTestId('export-copy').click();
      await expect(ui.status).toContainText('Could not copy automatically');
      await expect(ui.status).toHaveAttribute('data-kind', 'info');
      await expect(exportBox(page)).toBeFocused();
    });

    test('the old copy command is tried before giving up', async ({ page }) => {
      await page.addInitScript(() => {
        Object.defineProperty(navigator, 'clipboard', { value: undefined, configurable: true });
        document.execCommand = (command: string) => command === 'copy';
      });
      await openApp(page);
      const ui = editorUi(page);
      await openExport(page);
      await page.getByTestId('export-copy').click();
      await expect(ui.status).toHaveText('Copied the layout text to the clipboard.');
      await expect(page.getByTestId('export-copy')).toHaveText('Copied');
    });
  });
});

test.describe('Import', () => {
  test('opens an editable text area and an Import button', async ({ page }) => {
    await openApp(page);
    await importButton(page).click();

    await expect(page.getByTestId('import-panel')).toBeVisible();
    await expect(importButton(page)).toHaveAttribute('aria-expanded', 'true');
    await expect(importBox(page)).toBeFocused();
    await expect(importBox(page)).toBeEditable();
    await expect(importBox(page)).toHaveAccessibleName('Layout text to import');
    await expect(importBox(page)).toHaveValue('');
    await expect(importBox(page)).toHaveAccessibleDescription(/added to Display 1 \(primary\)/);
    await expect(page.getByTestId('import-submit')).toHaveText(/Import/);
    await expect(page.getByTestId('import-close')).toHaveText('Close');
  });

  test('the exported text of Display 1 can be imported on Display 2', async ({ page }) => {
    await openApp(page);
    const ui = editorUi(page);
    await openExport(page);
    const text = await exportBox(page).inputValue();
    await page.getByTestId('export-close').click();

    await ui.tabs.nth(1).click();
    await expect(ui.canvas).toHaveAccessibleName('Layout Thirds on Display 2');
    await importButton(page).click();
    await expect(importBox(page)).toHaveAccessibleDescription(/added to Display 2/);
    await importBox(page).fill(text);
    await page.getByTestId('import-submit').click();

    // Success: said in the status line, selected, panel closed, focus back on the opener.
    await expect(ui.status).toHaveText(
      'Imported "Halves" as a new layout on Display 2. It is selected now.',
    );
    await expect(ui.status).toHaveAttribute('data-kind', 'success');
    await expect(page.getByTestId('import-panel')).toHaveCount(0);
    await expect(importButton(page)).toBeFocused();
    await expect(ui.layoutItems).toHaveCount(5);
    const imported = ui.layoutItems.nth(4);
    await expect(imported).toContainText('Halves');
    await expect(imported).toHaveAttribute('aria-current', 'true');
    // "halves" already exists on Display 2, so the id got a suffix.
    await expect(imported).toHaveAttribute('data-layout-id', 'halves-2');
    await expect(ui.canvas).toHaveAccessibleName('Layout Halves on Display 2');
    await expect(ui.zones).toHaveCount(2);
    await expect(ui.dividers).toHaveAttribute('data-pos', '0.5');

    // Imported means saved; the active layout of Display 2 did not change.
    await expect(ui.save).toBeDisabled();
    await expect(ui.layoutItem('Thirds')).toContainText('Active');
    await expect(imported).not.toContainText('Active');

    // Display 1 is untouched.
    await ui.tabs.nth(0).click();
    await expect(ui.layoutItems).toHaveCount(4);
  });

  test('a custom layout keeps its id when the monitor does not have it yet', async ({ page }) => {
    await openApp(page);
    const ui = editorUi(page);
    await importText(page, goodFile);
    await expect(ui.status).toContainText('Imported "Pasted layout"');
    await expect(ui.layoutItems).toHaveCount(5);
    await expect(ui.layoutItem('Pasted layout')).toHaveAttribute('data-layout-id', 'pasted');
    await expect(ui.canvas).toHaveAccessibleName('Layout Pasted layout on Display 1 (primary)');

    // The same file again: a second copy with a suffix, nothing is overwritten.
    await importText(page, goodFile);
    await expect(ui.layoutItems).toHaveCount(6);
    await expect(ui.layoutItems.nth(5)).toHaveAttribute('data-layout-id', 'pasted-2');
    await expect(ui.layoutItems.nth(4)).toHaveAttribute('data-layout-id', 'pasted');
    await expect(ui.layoutItems.nth(5)).toHaveAttribute('aria-current', 'true');
  });

  test('an imported layout can be edited, saved and made active like any other', async ({
    page,
  }) => {
    await openApp(page);
    const ui = editorUi(page);
    await importText(page, goodFile);
    await expect(ui.zones).toHaveCount(1);

    await ui.toolSplitVertical.click();
    await ui.clickAt(0.4, 0.5);
    await expect(ui.zones).toHaveCount(2);
    await ui.save.click();
    await expect(ui.status).toHaveText('Saved "Pasted layout".');

    await ui.setActive.click();
    await expect(ui.status).toContainText('"Pasted layout" is now the active layout');
    await expect(ui.layoutItem('Pasted layout')).toContainText('Active');
  });

  test('Close keeps the typed text for next time; Escape closes too', async ({ page }) => {
    await openApp(page);
    await importButton(page).click();
    await importBox(page).fill('half-typed');
    await page.getByTestId('import-close').click();
    await expect(page.getByTestId('import-panel')).toHaveCount(0);
    await expect(importButton(page)).toBeFocused();

    await importButton(page).click();
    await expect(importBox(page)).toHaveValue('half-typed');
    await page.keyboard.press('Escape');
    await expect(page.getByTestId('import-panel')).toHaveCount(0);
    await expect(importButton(page)).toBeFocused();
  });

  test('the whole import works from the keyboard', async ({ page }) => {
    await openApp(page);
    const ui = editorUi(page);
    await tabTo(page, importButton(page));
    await page.keyboard.press('Enter');
    await expect(importBox(page)).toBeFocused();
    await page.keyboard.insertText(goodFile);
    await page.keyboard.press('Tab');
    await expect(page.getByTestId('import-submit')).toBeFocused();
    await page.keyboard.press('Enter');
    await expect(ui.status).toContainText('Imported "Pasted layout"');
    await expect(importButton(page)).toBeFocused();
    await expect(ui.layoutItems).toHaveCount(5);
  });

  test('a successful import clears the text area for the next one', async ({ page }) => {
    await openApp(page);
    await importText(page, goodFile);
    await importButton(page).click();
    await expect(importBox(page)).toHaveValue('');
  });
});

test.describe('Import errors', () => {
  const bad: Array<[string, string, string]> = [
    ['nothing at all', '', 'that is not valid JSON'],
    ['text that is not JSON', 'hello there', 'that is not valid JSON'],
    ['JSON cut off in the middle', goodFile.slice(0, 40), 'that is not valid JSON'],
    ['JSON that is not a Krova file', '{"foo":1}', 'not a Krova layout file'],
    ['a list', '[1,2,3]', 'not a Krova layout file'],
    [
      'a newer file version',
      file({ id: 'a', name: 'n', zones: [{ id: 1, rect: unit }] }, 2),
      'unsupported layout file version 2',
    ],
    ['a layout without zones', file({ id: 'a', name: 'n', zones: [] }), 'layout has no zones'],
    [
      'a layout with 65 zones',
      file({
        id: 'a',
        name: 'n',
        zones: Array.from({ length: 65 }, (_, i) => ({ id: i + 1, rect: unit })),
      }),
      'layout has too many zones (at most 64)',
    ],
    [
      'a name that is too long',
      file({ id: 'a', name: 'n'.repeat(65), zones: [{ id: 1, rect: unit }] }),
      'layout id or name is too long (at most 64 characters)',
    ],
    [
      'a zone that sticks out of the screen',
      file({ id: 'a', name: 'n', zones: [{ id: 1, rect: { ...unit, w: 2 } }] }),
      'zone 1 has an invalid rectangle',
    ],
    [
      'a zone that is far too small',
      file({ id: 'a', name: 'n', zones: [{ id: 1, rect: { ...unit, h: 0.001 } }] }),
      'zone 1 has an invalid rectangle',
    ],
    [
      'a number too big to be a number (1e999)',
      '{"krova_layout":1,"layout":{"id":"a","name":"n","zones":[{"id":1,"rect":{"x":0,"y":0,"w":1e999,"h":1}}]}}',
      'zone 1 has an invalid rectangle',
    ],
    [
      'two zones with the same id',
      file({
        id: 'a',
        name: 'n',
        zones: [
          { id: 1, rect: { x: 0, y: 0, w: 0.5, h: 1 } },
          { id: 1, rect: { x: 0.5, y: 0, w: 0.5, h: 1 } },
        ],
      }),
      'duplicate zone id 1',
    ],
    ['a text over 64 KiB', 'x'.repeat(70_000), 'layout file is larger than 64 KiB'],
  ];

  for (const [name, text, message] of bad) {
    test(`${name}: shows "${message}", keeps the text and the panel`, async ({ page }) => {
      await openApp(page);
      const ui = editorUi(page);
      await importText(page, text);

      // Said verbatim in the live region under the canvas.
      await expect(ui.status).toHaveText(message);
      await expect(ui.status).toHaveAttribute('data-kind', 'error');
      await expect(page.getByRole('status').filter({ hasText: message })).toHaveCount(1);

      // The panel and the text stay; nothing was added.
      await expect(page.getByTestId('import-panel')).toBeVisible();
      await expect(importBox(page)).toHaveValue(text);
      await expect(importBox(page)).toHaveAttribute('aria-invalid', 'true');
      await expect(importBox(page)).toHaveAttribute('aria-describedby', 'import-help editor-status');
      await expect(ui.layoutItems).toHaveCount(4);
      await expect(ui.canvas).toHaveAccessibleName('Layout Halves on Display 1 (primary)');

      // Editing the text clears the error mark (the message stays until the next one).
      await importBox(page).press('End');
      await importBox(page).press('Space');
      await expect(importBox(page)).toHaveAttribute('aria-invalid', 'false');
    });
  }

  test('after an error the user can fix the text and import it', async ({ page }) => {
    await openApp(page);
    const ui = editorUi(page);
    await importText(page, goodFile.replace('"krova_layout":1', '"krova_layout":2'));
    await expect(ui.status).toHaveText('unsupported layout file version 2');

    await importBox(page).fill(goodFile);
    await page.getByTestId('import-submit').click();
    await expect(ui.status).toHaveAttribute('data-kind', 'success');
    await expect(page.getByTestId('import-panel')).toHaveCount(0);
    await expect(ui.layoutItems).toHaveCount(5);
  });

  test('a monitor can hold at most 64 layouts', async ({ page }) => {
    test.slow();
    await openApp(page);
    const ui = editorUi(page);

    // Display 1 starts with 4; 60 imports fill it up.
    for (let i = 0; i < 60; i++) {
      await importText(page, goodFile);
      await expect(ui.layoutItems).toHaveCount(5 + i);
    }
    await expect(ui.layoutItems).toHaveCount(64);

    await importText(page, goodFile);
    await expect(ui.status).toHaveText('a monitor can hold at most 64 layouts');
    await expect(ui.status).toHaveAttribute('data-kind', 'error');
    await expect(page.getByTestId('import-panel')).toBeVisible();
    await expect(importBox(page)).toHaveValue(goodFile);
    await expect(ui.layoutItems).toHaveCount(64);

    // Display 2 has its own room.
    await ui.tabs.nth(1).click();
    await expect(ui.layoutItems).toHaveCount(4);
    await page.getByTestId('import-submit').click();
    await expect(ui.status).toHaveAttribute('data-kind', 'success');
    await expect(ui.layoutItems).toHaveCount(5);
  });
});

test.describe('persistence', () => {
  test('an imported layout is kept like any saved edit', async ({ page }) => {
    await openApp(page, '?reset');
    const ui = editorUi(page);
    await importText(page, goodFile);
    await expect(ui.status).toContainText('Imported "Pasted layout"');

    await page.goto('/');
    await expect(ui.canvas).toBeVisible();
    await expect(ui.layoutItems).toHaveCount(5);
    await expect(ui.layoutItem('Pasted layout')).toHaveAttribute('data-layout-id', 'pasted');
  });
});
