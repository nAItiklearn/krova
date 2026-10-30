import type { Modifier } from './bindings';

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
