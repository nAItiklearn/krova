// Top-level app state: the last AppState from the backend, the active view and
// the initial load. Everything that talks to Rust goes through `api`.

import { api, errorMessage } from './backend';
import type { AppState, NavigateTarget, View } from './bindings';

class AppStore {
  /** Replaced wholesale on every backend response, never mutated in place. */
  state = $state.raw<AppState | null>(null);
  loadError = $state<string | null>(null);
  loading = $state(true);
  view = $state<View>('editor');

  setState(s: AppState): void {
    this.state = s;
  }

  async load(): Promise<void> {
    this.loading = true;
    this.loadError = null;
    try {
      this.state = await api.getState();
    } catch (e) {
      this.loadError = errorMessage(e);
    } finally {
      this.loading = false;
    }
  }

  /** Quietly re-read the state (e.g. after the user returns from OS settings). */
  async refresh(): Promise<void> {
    try {
      this.state = await api.getState();
    } catch {
      // Keep showing the last known state.
    }
  }

  /** Only the editor window's own views are shown; `switcher` (if sent) is ignored here. */
  navigate(view: NavigateTarget): void {
    if (view === 'editor' || view === 'settings') this.view = view;
  }
}

export const app = new AppStore();
