// Settings form state with debounced auto-save (~300 ms after the last change).

import { app } from './app.svelte';
import { api, errorMessage } from './backend';
import type { AppState, Settings } from './bindings';

export const AUTOSAVE_DELAY_MS = 300;
const SAVED_BADGE_MS = 2500;

export type SaveStatus = 'idle' | 'saving' | 'saved' | 'error';

class SettingsStore {
  /** The values shown in the form. Replaced wholesale on every change. */
  draft = $state.raw<Settings | null>(null);
  status = $state<SaveStatus>('idle');
  error = $state('');

  private timer: ReturnType<typeof setTimeout> | undefined;
  private badgeTimer: ReturnType<typeof setTimeout> | undefined;
  private inflight = false;
  private again = false;

  /** True while a change has not reached the backend yet. */
  get pending(): boolean {
    return this.timer !== undefined || this.inflight;
  }

  /** Adopt the backend's settings unless the user has unsaved edits. */
  sync(state: AppState): void {
    if (!this.pending || this.draft === null) this.draft = state.config.settings;
  }

  update(patch: Partial<Settings>): void {
    if (this.draft === null) return;
    this.draft = { ...this.draft, ...patch };
    this.status = 'idle';
    clearTimeout(this.timer);
    this.timer = setTimeout(() => void this.flush(), AUTOSAVE_DELAY_MS);
  }

  /** Save now (used by the debounce timer and by "Retry"). */
  async flush(): Promise<void> {
    clearTimeout(this.timer);
    this.timer = undefined;
    const sent = this.draft;
    if (sent === null) return;
    if (this.inflight) {
      this.again = true;
      return;
    }
    this.inflight = true;
    this.status = 'saving';
    clearTimeout(this.badgeTimer);
    try {
      const state = await api.saveSettings(sent);
      app.setState(state);
      // Only adopt the backend's (possibly clamped) values if nothing changed meanwhile.
      if (this.draft === sent) this.draft = state.config.settings;
      this.status = 'saved';
      this.badgeTimer = setTimeout(() => {
        if (this.status === 'saved') this.status = 'idle';
      }, SAVED_BADGE_MS);
    } catch (e) {
      this.status = 'error';
      this.error = errorMessage(e);
    } finally {
      this.inflight = false;
      if (this.again) {
        this.again = false;
        void this.flush();
      }
    }
  }
}

export const settings = new SettingsStore();
