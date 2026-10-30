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
      reset(): void;
    };
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

/** Load the demo and wait until the editor is on screen. */
export async function openApp(page: Page, query: string = NOPERSIST): Promise<void> {
  await page.goto(`/${query}`);
  await expect(page.getByTestId('demo-badge')).toBeVisible();
  await expect(page.getByTestId('canvas')).toBeVisible();
}

/** Open the Settings view through the real navigation button. */
export async function openSettings(page: Page, query: string = NOPERSIST): Promise<void> {
  await openApp(page, query);
  await page.getByTestId('nav-settings').click();
  await expect(page.getByTestId('settings')).toBeVisible();
  await expect(page.getByTestId('setting-enabled')).toBeVisible();
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
 * of a divider, the label span that follows a visually hidden radio input.
 */
export function focusRingVisible(page: Page): Promise<boolean> {
  return page.evaluate(() => {
    const drawn = (style: CSSStyleDeclaration): boolean =>
      style.outlineStyle !== 'none' &&
      parseFloat(style.outlineWidth) > 0 &&
      style.outlineColor !== 'rgba(0, 0, 0, 0)';
    const el = document.activeElement;
    if (!el || el === document.body) return false;
    const candidates: Array<CSSStyleDeclaration | undefined> = [
      getComputedStyle(el),
      getComputedStyle(el, '::before'),
      el.querySelector('.face') ? getComputedStyle(el.querySelector('.face')!) : undefined,
      el.nextElementSibling ? getComputedStyle(el.nextElementSibling) : undefined,
    ];
    return candidates.some((style) => style !== undefined && drawn(style));
  });
}
