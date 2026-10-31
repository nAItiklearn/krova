// Automated accessibility audit with axe-core (WCAG 2.0/2.1/2.2 level A and AA rules)
// in several states of the editor and of Settings, in the light and the dark theme.
//
// The audit runs with reduced motion so colour transitions cannot be caught half way,
// which would make the contrast check non-deterministic.
//
// The defects this audit found earlier (label-in-name of the zone buttons, the hovered
// armed "Confirm delete" button, small text on a selected zone in the dark theme) are
// fixed; "fixed UI issues (regression tests)" below pins each one on its own, and no axe
// rule is switched off any more.

import AxeBuilder from '@axe-core/playwright';
import type { Page } from '@playwright/test';
import { editorUi, expect, openApp, openSettings, test } from './fixtures';

const WCAG_TAGS = ['wcag2a', 'wcag2aa', 'wcag21a', 'wcag21aa', 'wcag22aa'];

interface Finding {
  id: string;
  impact: string | null | undefined;
  help: string;
  targets: string[];
}

async function scan(
  page: Page,
  options: { tags?: string[]; rules?: string[]; disable?: string[] } = {},
): Promise<Finding[]> {
  let builder = new AxeBuilder({ page });
  if (options.rules) builder = builder.withRules(options.rules);
  else builder = builder.withTags(options.tags ?? WCAG_TAGS);
  if (options.disable) builder = builder.disableRules(options.disable);
  const results = await builder.analyze();
  return results.violations.map((v) => ({
    id: v.id,
    impact: v.impact,
    help: v.help,
    targets: v.nodes.map((n) => n.target.join(' ')),
  }));
}

interface Scenario {
  name: string;
  query?: string;
  /** Bring the app into the state to audit. Starts on the editor. */
  arrange: (page: Page) => Promise<void>;
}

const scenarios: Scenario[] = [
  { name: 'editor, first load', arrange: async () => {} },
  {
    name: 'editor, second monitor',
    arrange: async (page) => {
      await editorUi(page).tabs.nth(1).click();
      await expect(editorUi(page).zones).toHaveCount(3);
    },
  },
  {
    name: 'editor, unsaved Grid layout',
    arrange: async (page) => {
      const ui = editorUi(page);
      await page.getByTestId('template-grid').click();
      await expect(ui.unsavedNote).toBeVisible();
      await expect(ui.zones).toHaveCount(4);
    },
  },
  {
    name: 'editor, split tool with the guide showing',
    arrange: async (page) => {
      const ui = editorUi(page);
      await ui.toolSplitVertical.click();
      const at = await ui.pointAt(0.3, 0.5);
      await page.mouse.move(at.x, at.y);
      await expect(ui.preview).toBeVisible();
    },
  },
  {
    name: 'editor, guide too close to the edge and an error message',
    arrange: async (page) => {
      const ui = editorUi(page);
      await ui.toolSplitVertical.click();
      await ui.clickAt(0.02, 0.5);
      await expect(ui.status).toHaveAttribute('data-kind', 'error');
      const at = await ui.pointAt(0.02, 0.5);
      await page.mouse.move(at.x, at.y);
      await expect(ui.preview).toContainText('too close');
    },
  },
  {
    // The pointer is moved away; the hovered armed button has its own test below.
    name: 'editor, delete layout armed (pointer elsewhere)',
    arrange: async (page) => {
      const ui = editorUi(page);
      await ui.deleteLayout.click();
      await expect(ui.deleteLayout).toContainText('Confirm delete');
      await page.mouse.move(1, 1);
      // Wait until the hover colour (--danger-soft, light or dark) is gone.
      await expect(ui.deleteLayout).not.toHaveCSS(
        'background-color',
        /^rgb\((253, 236, 235|58, 26, 26)\)$/,
      );
    },
  },
  {
    name: 'editor, Accessibility permission banner',
    query: '?nopersist&permission=denied',
    arrange: async (page) => {
      await expect(page.getByTestId('permission-banner')).toBeVisible();
    },
  },
  {
    name: 'settings, first load',
    arrange: async (page) => {
      await page.getByTestId('nav-settings').click();
      await expect(page.getByTestId('setting-enabled')).toBeVisible();
    },
  },
  {
    name: 'settings, edge snapping on and a Saved indicator',
    arrange: async (page) => {
      await page.getByTestId('nav-settings').click();
      await page.getByRole('switch', { name: 'Edge snapping' }).check();
      await expect(page.getByTestId('save-indicator')).toContainText('Saved');
    },
  },
  {
    name: 'settings, invalid highlight colour',
    arrange: async (page) => {
      await page.getByTestId('nav-settings').click();
      await page.getByTestId('setting-highlight_color').fill('nope');
      await expect(page.getByTestId('setting-highlight_color')).toHaveAttribute(
        'aria-invalid',
        'true',
      );
    },
  },
  {
    name: 'editor, export panel open',
    arrange: async (page) => {
      await page.getByTestId('export-layout').click();
      await expect(page.getByTestId('export-text')).toHaveValue(/"krova_layout": 1/);
    },
  },
  {
    name: 'editor, import panel open and empty',
    arrange: async (page) => {
      await page.getByTestId('import-layout').click();
      await expect(page.getByTestId('import-text')).toBeFocused();
    },
  },
  {
    name: 'editor, import panel after a refused file (error status, invalid text area)',
    arrange: async (page) => {
      await page.getByTestId('import-layout').click();
      await page.getByTestId('import-text').fill('{"foo":1}');
      await page.getByTestId('import-submit').click();
      await expect(editorUi(page).status).toHaveAttribute('data-kind', 'error');
      await expect(editorUi(page).status).toHaveText('not a Krova layout file');
      await expect(page.getByTestId('import-text')).toHaveAttribute('aria-invalid', 'true');
    },
  },
  {
    name: 'settings, keyboard shortcuts and windows groups after changing them',
    arrange: async (page) => {
      await page.getByTestId('nav-settings').click();
      // A Win-key choice also shows the "Windows keeps many of these" hint.
      await page.getByTestId('setting-shortcut_modifier').selectOption('win_alt');
      await page.getByTestId('setting-zone_gap_px').fill('12');
      await page.getByRole('switch', { name: 'Linked dividers' }).uncheck();
      await expect(page.getByTestId('save-indicator')).toContainText('Saved');
      await expect(page.getByTestId('keymap-left')).toHaveText('Win + Alt + Left');
      await expect(page.getByTestId('shortcuts-win-hint')).toBeVisible();
    },
  },
  {
    name: 'settings, shortcuts switched off (modifier disabled)',
    arrange: async (page) => {
      await page.getByTestId('nav-settings').click();
      await page.getByRole('switch', { name: 'Use keyboard shortcuts' }).uncheck();
      await expect(page.getByTestId('setting-shortcut_modifier')).toBeDisabled();
      await expect(page.getByTestId('save-indicator')).toContainText('Saved');
    },
  },
  {
    name: 'settings, shortcut conflict warning',
    query: '?nopersist&shortcuts=failed',
    arrange: async (page) => {
      await page.getByTestId('nav-settings').click();
      await expect(page.getByTestId('shortcuts-failed')).toBeVisible();
    },
  },
  {
    name: 'settings, shortcuts not active note',
    query: '?nopersist&shortcuts=off',
    arrange: async (page) => {
      await page.getByTestId('nav-settings').click();
      await expect(page.getByTestId('shortcuts-inactive')).toBeVisible();
    },
  },
  {
    name: 'settings, no platform support (disabled controls and notes)',
    query: '?nopersist&capabilities=none',
    arrange: async (page) => {
      await page.getByTestId('nav-settings').click();
      await expect(page.getByTestId('shortcuts-unavailable')).toBeVisible();
      await expect(page.getByTestId('window-memory-unavailable')).toBeVisible();
      await expect(page.getByTestId('setting-keyboard_shortcuts')).toBeDisabled();
    },
  },
];

for (const scheme of ['light', 'dark'] as const) {
  test.describe(`axe, ${scheme} colour scheme`, () => {
    test.use({ colorScheme: scheme, reducedMotion: 'reduce' });

    for (const scenario of scenarios) {
      test(`no WCAG A/AA violations: ${scenario.name}`, async ({ page }) => {
        await openApp(page, scenario.query);
        await scenario.arrange(page);
        expect(await scan(page)).toEqual([]);
      });
    }

    test('no WCAG A/AA violations: settings with the theme forced to the other scheme', async ({
      page,
    }) => {
      await openSettings(page);
      await page.getByTestId(`setting-theme-${scheme === 'light' ? 'dark' : 'light'}`).check();
      await expect(page.locator('html')).toHaveAttribute(
        'data-theme',
        scheme === 'light' ? 'dark' : 'light',
      );
      expect(await scan(page)).toEqual([]);
    });
  });
}

// Each of these was a real defect found by the audit above; kept as focused regression tests.
test.describe('fixed UI issues (regression tests)', () => {
  test.use({ reducedMotion: 'reduce' });

  test('zone buttons: the accessible name contains the visible text (WCAG 2.5.3)', async ({
    page,
  }) => {
    await openApp(page);
    expect(await scan(page, { rules: ['label-content-name-mismatch'] })).toEqual([]);
  });

  for (const scheme of ['light', 'dark'] as const) {
    test(`armed "Confirm delete" stays readable while the pointer is on it (${scheme})`, async ({
      page,
    }) => {
      await page.emulateMedia({ colorScheme: scheme });
      await openApp(page);
      const ui = editorUi(page);
      await ui.deleteLayout.click(); // the pointer stays on the button
      await expect(ui.deleteLayout).toContainText('Confirm delete');
      expect(await scan(page, { rules: ['color-contrast'] })).toEqual([]);
    });

    test(`text on a selected zone has enough contrast (${scheme})`, async ({ page }) => {
      await page.emulateMedia({ colorScheme: scheme });
      await openApp(page);
      const ui = editorUi(page);
      await ui.zone(2).click();
      await expect(ui.zone(2)).toHaveAttribute('aria-pressed', 'true');
      expect(await scan(page, { rules: ['color-contrast'] })).toEqual([]);
    });
  }
});

test.describe('axe, best-practice rules (informational)', () => {
  // Not a gate: best-practice findings are recorded as test annotations so they show up in
  // the HTML report without failing the run. page-has-heading-one used to show up here
  // (the brand was a span); the brand is now the page's h1.
  test('record best-practice findings for the editor and Settings', async ({ page }, testInfo) => {
    await openApp(page);
    const editorFindings = await scan(page, { tags: ['best-practice'] });
    await page.getByTestId('nav-settings').click();
    await expect(page.getByTestId('setting-enabled')).toBeVisible();
    const settingsFindings = await scan(page, { tags: ['best-practice'] });

    for (const [view, findings] of [
      ['editor', editorFindings],
      ['settings', settingsFindings],
    ] as const) {
      for (const f of findings) {
        testInfo.annotations.push({
          type: `axe best-practice (${view})`,
          description: `${f.id} [${f.impact}]: ${f.help} -> ${f.targets.join(' | ')}`,
        });
      }
    }
  });
});
