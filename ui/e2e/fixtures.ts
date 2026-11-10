// Shared test fixtures and helpers for the Krova UI end-to-end tests.
//
// Every test that imports `test` from this file also gets an automatic guard:
//   * the browser may only talk to the local dev server (privacy promise),
//   * the page must not log console errors or throw uncaught exceptions.
// The guard is asserted when the test finishes, so any flow that leaks a request
// or logs an error fails, not only the tests that are about that.

import { expect, test as base, type Locator, type Page } from '@playwright/test';

export { expect };

/** Hooks the demo build puts on `window.__krovaDemo` (src/lib/mock/mockBackend.ts). */
declare global {
  interface Window {
    __krovaDemo?: {
      navigate(view: string): void;
      setPermission(permission: string): void;
      toggleEnabled(): void;
      setShortcuts(mode: string): void;
      setCapabilities(mode: string): void;
      reset(): void;
    };
    /** Count of "Saved" reports, kept by the `saveWatch` init script (tests only). */
    __krovaSaves?: number;
  }
}

/** Default URL query: everything stays in memory, so tests never see each other. */
export const NOPERSIST = '?nopersist';

// ---------------------------------------------------------------------------
// Network / console guard
// ---------------------------------------------------------------------------

/**
 * Is this URL allowed for the page? Only the dev server's own origin (HTTP and
 * its HMR web socket on the same host and port) and URLs that never touch the
 * network (data:, blob:, about:).
 */
export function isAllowedUrl(raw: string, baseURL: string): boolean {
  let url: URL;
  let base: URL;
  try {
    url = new URL(raw);
    base = new URL(baseURL);
  } catch {
    return false;
  }
  if (['data:', 'blob:', 'about:'].includes(url.protocol)) return true;
  if (url.host !== base.host) return false;
  const socket = base.protocol === 'https:' ? 'wss:' : 'ws:';
  return url.protocol === base.protocol || url.protocol === socket;
}

export interface Guard {
  /** Every URL the browser asked for: requests of all frames and web sockets. */
  readonly urls: string[];
  readonly consoleErrors: string[];
  readonly pageErrors: string[];
  /** Requests that are not for the local dev server. */
  foreign(): string[];
}

interface Fixtures {
  /** Set `test.use({ strictGuards: false })` to opt out (negative controls). */
  strictGuards: boolean;
  guard: Guard;
}

export const test = base.extend<Fixtures>({
  strictGuards: [true, { option: true }],

  guard: [
    async ({ context, page, baseURL, strictGuards }, use) => {
      const origin = baseURL ?? 'http://localhost:1420';
      const urls: string[] = [];
      const consoleErrors: string[] = [];
      const pageErrors: string[] = [];

      context.on('request', (request) => urls.push(request.url()));
      page.on('websocket', (socket) => urls.push(socket.url()));
      page.on('console', (message) => {
        if (message.type() === 'error') {
          consoleErrors.push(`${message.text()} (${message.location().url})`);
        }
      });
      page.on('pageerror', (error) => pageErrors.push(error.message));
      await page.addInitScript(saveWatch);

      const guard: Guard = {
        urls,
        consoleErrors,
        pageErrors,
        foreign: () => urls.filter((u) => !isAllowedUrl(u, origin)),
      };
      await use(guard);

      if (strictGuards) {
        expect(guard.foreign(), 'requests to anything but the local dev server').toEqual([]);
        expect(consoleErrors, 'console errors').toEqual([]);
        expect(pageErrors, 'uncaught page errors').toEqual([]);
      }
    },
    { auto: true },
  ],
});

// ---------------------------------------------------------------------------
// Opening the app and driving the demo hooks
// ---------------------------------------------------------------------------

/** The sidebar sections (src/lib/app.svelte.ts `Section`). */
export type Section =
  | 'overview'
  | 'layouts'
  | 'arrange'
  | 'shortcuts'
  | 'appearance'
  | 'corners'
  | 'displays'
  | 'privacy';

export const SECTIONS: Section[] = [
  'overview',
  'layouts',
  'arrange',
  'shortcuts',
  'corners',
  'appearance',
  'displays',
  'privacy',
];

/** What is on screen when a section is open: the editor canvas for Layouts, the page otherwise. */
export function sectionContent(page: Page, section: Section): Locator {
  return section === 'layouts' ? page.getByTestId('canvas') : page.getByTestId(`view-${section}`);
}

/** Load the demo and wait until the first screen (the Overview) is up. */
export async function openHome(page: Page, query: string = NOPERSIST): Promise<void> {
  await page.goto(`/${query}`);
  await expect(page.getByTestId('demo-badge')).toBeVisible();
  await expect(page.getByTestId('view-overview')).toBeVisible();
}

/** Open a section through its sidebar button. */
export async function gotoSection(page: Page, section: Section): Promise<void> {
  await page.getByTestId(`nav-${section}`).click();
  await expect(page.getByTestId(`nav-${section}`)).toHaveAttribute('aria-current', 'page');
  await expect(sectionContent(page, section)).toBeVisible();
}

/** Load the demo and open a section. */
export async function openSection(page: Page, section: Section, query: string = NOPERSIST): Promise<void> {
  await openHome(page, query);
  await gotoSection(page, section);
}

/** Load the demo and wait until the editor (Layouts) is on screen. */
export async function openApp(page: Page, query: string = NOPERSIST): Promise<void> {
  await openSection(page, 'layouts', query);
}

/** Open Arrange (snapping, windows, app rules) and wait for its first setting. */
export async function openArrange(page: Page, query: string = NOPERSIST): Promise<void> {
  await openSection(page, 'arrange', query);
  await expect(page.getByTestId('setting-enabled')).toBeVisible();
}

/**
 * Wait until the settings auto-save reports "Saved" for the latest change.
 *
 * The badge only stays up for 2.5 s (src/lib/settings.svelte.ts). Polling the DOM
 * for it races against that timer on a busy machine: the save happens, but the
 * badge is gone again before the assertion looks. So the `saveWatch` init script
 * (below) records each switch of the indicator to `data-state="saved"` with the
 * text "Saved" as it happens, and forgets it on every later `input` / `change`
 * event and every new "saving": a report only counts if it came after the last
 * edit. This waits for such a report and then consumes it.
 */
export async function expectSaved(page: Page): Promise<void> {
  await expect
    .poll(() => page.evaluate(() => window.__krovaSaves ?? 0), { message: 'a "Saved" report' })
    .toBeGreaterThan(0);
  await page.evaluate(() => {
    window.__krovaSaves = 0;
  });
}

/** Init script: record "Saved" reports of the save indicator (see `expectSaved`). */
function saveWatch(): void {
  window.__krovaSaves = 0;
  const forget = (): void => {
    window.__krovaSaves = 0;
  };
  document.addEventListener('input', forget, true);
  document.addEventListener('change', forget, true);
  new MutationObserver((records) => {
    for (const r of records) {
      const el = r.target as Element;
      if (el.getAttribute('data-testid') !== 'save-indicator') continue;
      const state = el.getAttribute('data-state');
      if (state === 'saving') forget();
      else if (state === 'saved' && (el.textContent ?? '').includes('Saved')) {
        window.__krovaSaves = (window.__krovaSaves ?? 0) + 1;
      }
    }
  }).observe(document, { subtree: true, attributes: true, attributeFilter: ['data-state'] });
}

/** Call a `window.__krovaDemo` hook. */
export function demo<K extends keyof NonNullable<Window['__krovaDemo']>>(
  page: Page,
  hook: K,
  ...args: Parameters<NonNullable<Window['__krovaDemo']>[K]>
): Promise<void> {
  return page.evaluate(
    ([name, params]) => {
      const hooks = window.__krovaDemo;
      if (!hooks) throw new Error('window.__krovaDemo is missing (not a demo build?)');
      (hooks[name as keyof typeof hooks] as (...a: unknown[]) => void)(...params);
    },
    [hook, args as unknown[]] as const,
  );
}

// ---------------------------------------------------------------------------
// Editor helpers
// ---------------------------------------------------------------------------

export interface EditorUi {
  canvas: Locator;
  caption: Locator;
  status: Locator;
  zone(id: number): Locator;
  zones: Locator;
  dividers: Locator;
  verticalDividers: Locator;
  horizontalDividers: Locator;
  preview: Locator;
  tabs: Locator;
  layoutItems: Locator;
  toolSelect: Locator;
  toolSplitVertical: Locator;
  toolSplitHorizontal: Locator;
  deleteZone: Locator;
  undo: Locator;
  revert: Locator;
  save: Locator;
  unsavedNote: Locator;
  layoutName: Locator;
  setActive: Locator;
  deleteLayout: Locator;
  newLayout: Locator;
  layoutItem(name: string | RegExp): Locator;
  /** Page coordinates of a point given as a fraction of the canvas box. */
  pointAt(fx: number, fy: number): Promise<{ x: number; y: number }>;
  /** Move the pointer to a canvas fraction and click there. */
  clickAt(fx: number, fy: number): Promise<void>;
  /** width / height of the canvas as laid out. */
  ratio(): Promise<number>;
}

export function editorUi(page: Page): EditorUi {
  const id = (testId: string): Locator => page.getByTestId(testId);
  const canvas = id('canvas');

  const pointAt = async (fx: number, fy: number): Promise<{ x: number; y: number }> => {
    const box = await canvas.boundingBox();
    if (!box) throw new Error('the canvas has no bounding box');
    return { x: box.x + box.width * fx, y: box.y + box.height * fy };
  };

  return {
    canvas,
    caption: id('canvas-caption'),
    status: id('status'),
    zone: (n) => id(`zone-${n}`),
    zones: page.getByTestId(/^zone-\d+$/),
    dividers: id('divider'),
    verticalDividers: id('divider').and(page.locator('[data-axis="vertical"]')),
    horizontalDividers: id('divider').and(page.locator('[data-axis="horizontal"]')),
    preview: id('split-preview'),
    tabs: id('monitor-tab'),
    layoutItems: id('layout-item'),
    toolSelect: id('tool-select'),
    toolSplitVertical: id('tool-split-vertical'),
    toolSplitHorizontal: id('tool-split-horizontal'),
    deleteZone: id('delete-zone'),
    undo: id('undo'),
    revert: id('revert'),
    save: id('save'),
    unsavedNote: id('unsaved-note'),
    layoutName: id('layout-name'),
    setActive: id('set-active'),
    deleteLayout: id('delete-layout'),
    newLayout: id('new-layout'),
    layoutItem: (name) => id('layout-item').filter({ hasText: name }),
    pointAt,
    clickAt: async (fx, fy) => {
      const p = await pointAt(fx, fy);
      await page.mouse.move(p.x, p.y);
      await page.mouse.click(p.x, p.y);
    },
    ratio: async () => {
      const box = await canvas.boundingBox();
      if (!box) throw new Error('the canvas has no bounding box');
      return box.width / box.height;
    },
  };
}

/**
 * Press Tab (or Shift+Tab with `backwards`) until `target` has focus, at most
 * `max` presses, like a keyboard user looking for a control.
 */
export async function tabTo(
  page: Page,
  target: Locator,
  options: { max?: number; backwards?: boolean } = {},
): Promise<void> {
  const { max = 30, backwards = false } = options;
  for (let i = 0; i < max; i++) {
    const focused = await target.evaluate((el) => el === document.activeElement).catch(() => false);
    if (focused) return;
    await page.keyboard.press(backwards ? 'Shift+Tab' : 'Tab');
  }
  await expect(target).toBeFocused();
}

/**
 * Does the focused element draw a visible focus indicator? The app draws it on
 * the element itself, or on a part of it: the face of a zone, the ::before grip
 * of a divider, the track that follows a visually hidden switch; text fields and
 * selects use a box-shadow ring instead of an outline.
 */
export function focusRingVisible(page: Page): Promise<boolean> {
  return page.evaluate(() => {
    const drawn = (style: CSSStyleDeclaration): boolean =>
      style.outlineStyle !== 'none' &&
      parseFloat(style.outlineWidth) > 0 &&
      style.outlineColor !== 'rgba(0, 0, 0, 0)';
    const el = document.activeElement;
    if (!el || el === document.body) return false;
    // Text fields and selects draw their ring as a box-shadow: it must change on focus.
    if (el.matches('input[type="text"], select, textarea')) {
      // The ring fades in (a short transition): read the end state.
      const settle = (): void => el.getAnimations().forEach((a) => a.finish());
      settle();
      const focused = getComputedStyle(el).boxShadow;
      (el as HTMLElement).blur();
      settle();
      const blurred = getComputedStyle(el).boxShadow;
      (el as HTMLElement).focus();
      settle();
      if (focused !== 'none' && focused !== blurred) return true;
    }
    const candidates: Array<CSSStyleDeclaration | undefined> = [
      getComputedStyle(el),
      getComputedStyle(el, '::before'),
      el.querySelector('.face') ? getComputedStyle(el.querySelector('.face')!) : undefined,
      el.nextElementSibling ? getComputedStyle(el.nextElementSibling) : undefined,
    ];
    return candidates.some((style) => style !== undefined && drawn(style));
  });
}
