import type { Modifier, ShortcutModifier } from './bindings';

/** macOS (and iOS-family) webviews; falls back to the user agent. */
export function isMac(): boolean {
  if (typeof navigator === 'undefined') return false;
  // navigator.platform is deprecated but still the most reliable signal in WebView2/WKWebView.
  const hint = navigator.platform || navigator.userAgent || '';
  return /mac|iphone|ipad/i.test(hint);
}

/** Human label for the activation modifier on this OS. */
export function modifierLabel(m: Modifier, mac: boolean = isMac()): string {
  switch (m) {
    case 'shift':
      return 'Shift';
    case 'ctrl':
      return mac ? 'Control' : 'Ctrl';
    case 'alt':
      return mac ? 'Option' : 'Alt';
    case 'super':
      return mac ? 'Command' : 'Windows key';
  }
}

export const MODIFIERS: Modifier[] = ['shift', 'ctrl', 'alt', 'super'];

/** Choices for the keyboard-shortcut modifier, default first. */
export const SHORTCUT_MODIFIERS: ShortcutModifier[] = [
  'win_alt',
  'ctrl_alt',
  'ctrl_alt_shift',
  'win_ctrl',
];

/**
 * The keys of a shortcut modifier, in the words of the platform: "Win" (or "Cmd"),
 * "Ctrl" (or "Control"), "Alt" (or "Option"), "Shift". The macOS words are only used
 * when `platformWords` says shortcuts really work there (capabilities.shortcuts); until
 * then the Windows words are shown, whatever the OS.
 */
export function shortcutKeys(m: ShortcutModifier, platformWords: boolean = false): string[] {
  const mac = platformWords && isMac();
  const win = mac ? 'Cmd' : 'Win';
  const ctrl = mac ? 'Control' : 'Ctrl';
  const alt = mac ? 'Option' : 'Alt';
  switch (m) {
    case 'ctrl_alt':
      return [ctrl, alt];
    case 'win_alt':
      return [win, alt];
    case 'ctrl_alt_shift':
      return [ctrl, alt, 'Shift'];
    case 'win_ctrl':
      return [win, ctrl];
  }
}

/** "Win + Alt", "Ctrl + Alt + Shift", ... */
export function shortcutLabel(m: ShortcutModifier, platformWords: boolean = false): string {
  return shortcutKeys(m, platformWords).join(' + ');
}
