// Top-level app state: the last AppState from the backend, the active view and
// the initial load. Everything that talks to Rust goes through `api`.

import { api, errorMessage } from './backend';
import type { AppState, NavigateTarget } from './bindings';

/** The sections of the main window (the sidebar). */
export type Section =
  | 'overview'
  | 'layouts'
  | 'arrange'
  | 'shortcuts'
  | 'appearance'
  | 'corners'
  | 'displays'
  | 'privacy';

const SECTIONS: Section[] = ['overview', 'layouts', 'arrange', 'shortcuts', 'appearance', 'corners', 'displays', 'privacy'];

class AppStore {
  /** Replaced wholesale on every backend response, never mutated in place. */
  state = $state.raw<AppState | null>(null);
  loadError = $state<string | null>(null);
  loading = $state(true);
  view = $state<Section>('overview');

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

  /** Backend targets map onto sections ("editor" opens Layouts, "settings" the Overview);
   *  a section name is accepted as is. Other targets (switcher, panel) are ignored here. */
  navigate(view: NavigateTarget | Section): void {
    if (view === 'editor') this.view = 'layouts';
    else if (view === 'settings') this.view = 'overview';
    else if (SECTIONS.includes(view as Section)) this.view = view as Section;
  }
}

export const app = new AppStore();
