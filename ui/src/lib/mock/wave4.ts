// Demo-mode mirrors of the wave 4 rules in Rust: reserved edges
// (krova-core edges.rs), the focus delay (ffm.rs) and the language list (i18n.rs).

import type { Language, LanguageOption, PxRect, ReservedEdges } from '../bindings';

export const MAX_RESERVED_PX = 400;
export const MIN_USABLE_PX = 200;
export const MIN_FOCUS_DELAY_MS = 150;
export const MAX_FOCUS_DELAY_MS = 800;
export const DEFAULT_FOCUS_DELAY_MS = 350;

export const LANGUAGES: LanguageOption[] = [
  { id: 'system', name: 'System' },
  { id: 'en', name: 'English' },
  { id: 'es', name: 'Español' },
  { id: 'de', name: 'Deutsch' },
  { id: 'ja', name: '日本語' },
  { id: 'zh-Hans', name: '简体中文' },
];

export const NO_RESERVE: ReservedEdges = { top: 0, right: 0, bottom: 0, left: 0 };

const side = (v: unknown): number =>
  typeof v === 'number' && Number.isFinite(v)
    ? Math.min(MAX_RESERVED_PX, Math.max(0, Math.round(v)))
    : 0;

/** Every side rounded and clamped to 0..400, like the Rust loader. */
export function sanitizeReserved(r: Partial<ReservedEdges> | null | undefined): ReservedEdges {
  return { top: side(r?.top), right: side(r?.right), bottom: side(r?.bottom), left: side(r?.left) };
}

/** The work area left once `r` (logical px) is reserved on a monitor at `scale`. */
export function applyReserved(work: PxRect, scale: number, r: ReservedEdges | undefined): PxRect {
  const e = sanitizeReserved(r);
  if (work.w <= 0 || work.h <= 0) return { ...work };
  const s = Number.isFinite(scale) && scale > 0 ? Math.min(8, Math.max(0.5, scale)) : 1;
  const px = (v: number) => Math.round(v * s);
  const out = { ...work };
  const [left, right] = [px(e.left), px(e.right)];
  if (work.w - left - right >= MIN_USABLE_PX) {
    out.x = work.x + left;
    out.w = work.w - left - right;
  }
  const [top, bottom] = [px(e.top), px(e.bottom)];
  if (work.h - top - bottom >= MIN_USABLE_PX) {
    out.y = work.y + top;
    out.h = work.h - top - bottom;
  }
  return out;
}

export function clampFocusDelay(ms: unknown): number {
  const v = typeof ms === 'number' && Number.isFinite(ms) ? Math.round(ms) : DEFAULT_FOCUS_DELAY_MS;
  return Math.min(MAX_FOCUS_DELAY_MS, Math.max(MIN_FOCUS_DELAY_MS, v));
}

export function sanitizeLanguage(v: unknown): Language {
  return LANGUAGES.some((l) => l.id === v) ? (v as Language) : 'system';
}

/** `system` resolves from the browser's language in the demo (Windows' in the app). */
export function resolveLanguage(
  setting: Language,
  browser: string | undefined,
): Exclude<Language, 'system'> {
  if (setting !== 'system') return setting;
  const tag = (browser ?? '').toLowerCase();
  if (tag.startsWith('es')) return 'es';
  if (tag.startsWith('de')) return 'de';
  if (tag.startsWith('ja')) return 'ja';
  if (tag === 'zh-cn' || tag === 'zh-sg' || tag.startsWith('zh-hans')) return 'zh-Hans';
  return 'en';
}

/** The releases page "Check for updates" opens (the same address as in Rust). */
export const RELEASES_URL = 'https://github.com/nAItiklearn/krova/releases';
