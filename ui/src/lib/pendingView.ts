// The real app destroys its window on close and creates it again on demand. A tray
// click that asks for a view ("editor" / "settings") while the window is still being
// created is parked by the backend and pushed as a `navigate` event about 250 ms after
// the page loaded. `take_pending_view` is the exact pull path: call it once, after the
// `navigate` listener is registered, and open the view it returns (the later push is a
// no-op because the view was taken). See docs/features-contract.md, "Window lifecycle".

import type { Backend } from './backend';
import type { View } from './bindings';

/** `value` if it is a view the app knows, else null (the backend is not trusted). */
export function asView(value: unknown): View | null {
  return value === 'editor' || value === 'settings' ? value : null;
}

/**
 * Take the parked view and open it. Only the real backend has a window lifecycle, so
 * the browser demo (mock) never asks. Any failure, for example a build that does not
 * have the command yet, leaves the app on its current view.
 *
 * @returns the view that was opened, or null
 */
export async function applyPendingView(
  backend: Pick<Backend, 'kind' | 'takePendingView'>,
  navigate: (view: View) => void,
): Promise<View | null> {
  if (backend.kind !== 'tauri') return null;
  try {
    const view = asView(await backend.takePendingView());
    if (view) navigate(view);
    return view;
  } catch {
    return null;
  }
}
