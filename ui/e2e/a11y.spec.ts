// Automated accessibility audit with axe-core (WCAG 2.0/2.1/2.2 level A and AA rules)
// in several states of the editor and of Settings, in the light and the dark theme.
//
// The audit runs with reduced motion so colour transitions cannot be caught half way,
// which would make the contrast check non-deterministic.
//
// Real defects found by this audit are NOT hidden: the general scans switch off only the
// one rule that the defect trips (KNOWN_ISSUE_RULES) and each defect has its own test in
// "known UI issues", marked test.fail(). Those tests turn red ("expected to fail, but
// passed") as soon as the UI is fixed, which is the signal to delete the marker.

import AxeBuilder from '@axe-core/playwright';
import type { Page } from '@playwright/test';
import { editorUi, expect, openApp, openSettings, test } from './fixtures';

const WCAG_TAGS = ['wcag2a', 'wcag2aa', 'wcag21a', 'wcag21aa', 'wcag22aa'];

/** Rules with a tracked defect (see "known UI issues" below). */
const KNOWN_ISSUE_RULES = ['label-content-name-mismatch'];

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
    // The pointer is moved away: a hovered armed button is a known issue, tested below.
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
];

for (const scheme of ['light', 'dark'] as const) {
  test.describe(`axe, ${scheme} colour scheme`, () => {
    test.use({ colorScheme: scheme, reducedMotion: 'reduce' });

    for (const scenario of scenarios) {
      test(`no WCAG A/AA violations: ${scenario.name}`, async ({ page }) => {
        await openApp(page, scenario.query);
        await scenario.arrange(page);
        expect(await scan(page, { disable: KNOWN_ISSUE_RULES })).toEqual([]);
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
      expect(await scan(page, { disable: KNOWN_ISSUE_RULES })).toEqual([]);
    });
  });
}

test.describe('known UI issues (expected to fail until the UI is fixed)', () => {
  test.use({ reducedMotion: 'reduce' });

  test('zone buttons: the accessible name contains the visible text (WCAG 2.5.3)', async ({
    page,
  }) => {
    test.fail(
      true,
      'UI bug: aria-label is "Zone 1, 50% wide by 100% tall, 1280 by 1400 pixels" but the visible ' +
        'text is "1", "1280 × 1400", "50% × 100%"; axe: label-content-name-mismatch (serious)',
    );
    await openApp(page);
    expect(await scan(page, { rules: ['label-content-name-mismatch'] })).toEqual([]);
  });

  for (const scheme of ['light', 'dark'] as const) {
    test(`armed "Confirm delete" stays readable while the pointer is on it (${scheme})`, async ({
      page,
    }) => {
      test.fail(
        true,
        'UI bug: .btn.danger:hover:not(:disabled) (specificity 0,4,0) beats .btn.danger.armed ' +
          '(0,3,0), so a hovered armed button gets the pale --danger-soft background with the ' +
          'white/surface text of the armed state: contrast about 1.1:1',
      );
      await page.emulateMedia({ colorScheme: scheme });
      await openApp(page);
      const ui = editorUi(page);
      await ui.deleteLayout.click(); // the pointer stays on the button
      await expect(ui.deleteLayout).toContainText('Confirm delete');
      expect(await scan(page, { rules: ['color-contrast'] })).toEqual([]);
    });

    test(`text on a selected zone has enough contrast (${scheme})`, async ({ page }) => {
      test.fail(
        scheme === 'dark',
        'UI bug (dark theme only): --text-muted #a2abbd on --zone-bg-selected #35508f is 3.37:1, ' +
          'below 4.5:1, for the small size and percentage lines of a selected zone',
      );
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
  // the HTML report without failing the run. At the time of writing: page-has-heading-one
  // (the brand "Krova" is a span, there is no h1).
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
