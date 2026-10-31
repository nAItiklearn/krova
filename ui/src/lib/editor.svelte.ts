// Editor state. Lives outside the components so unsaved edits and the selected
// monitor/layout survive switching to the Settings view and back.
//
// Editing model: the layout shown in the canvas is the saved layout from the
// backend, or an unsaved draft of it. Every structural edit (split, merge,
// divider) is a backend command that returns the new Layout or rejects with a
// string; a rejection leaves the draft untouched and is shown in the status
// line. "Save" upserts the draft with save_layout.

import { app } from './app.svelte';
import { api, errorMessage } from './backend';
import type { Axis, Layout, MonitorProfile, MonitorSnapshot } from './bindings';
import { pct } from './geometry';

export type Tool = 'select' | 'split-vertical' | 'split-horizontal';
export type StatusKind = 'info' | 'success' | 'error';

export interface Status {
  kind: StatusKind;
  text: string;
  /** Bumped on every message so repeated texts are announced again. */
  seq: number;
}

const clone = <T>(v: T): T => JSON.parse(JSON.stringify(v)) as T;
const sameLayout = (a: Layout, b: Layout): boolean => JSON.stringify(a) === JSON.stringify(b);
const keyOf = (monitorId: string, layoutId: string): string => `${monitorId}\u0001${layoutId}`;

class EditorStore {
  tool = $state<Tool>('select');
  selectedZone = $state<number | null>(null);
  status = $state<Status>({ kind: 'info', text: '', seq: 0 });
  templates = $state.raw<Layout[]>([]);
  /** Unsaved working copies, by monitor+layout key. Replaced wholesale. */
  drafts = $state.raw<Record<string, Layout>>({});
  /** Undo history per key (previous versions of the draft). Replaced wholesale. */
  undoStacks = $state.raw<Record<string, Layout[]>>({});
  /** Number of backend operations waiting or running. */
  pending = $state(0);

  private requestedMonitor = $state<string | null>(null);
  private requestedLayout = $state<string | null>(null);
  private queue: Promise<void> = Promise.resolve();

  /** Connected monitors (tabs). */
  monitors = $derived<MonitorSnapshot[]>(app.state?.monitors ?? []);

  /** Selected monitor; falls back to the first connected one. */
  monitor = $derived<MonitorSnapshot | null>(
    this.monitors.find((m) => m.profile_id === this.requestedMonitor) ?? this.monitors[0] ?? null,
  );

  profile = $derived<MonitorProfile | null>(
    app.state?.config.monitors.find((p) => p.id === this.monitor?.profile_id) ?? null,
  );

  /** Saved layouts of the selected monitor. */
  layouts = $derived<Layout[]>(this.profile?.layouts ?? []);

  /** Layout being edited; falls back to the monitor's active layout. */
  layoutId = $derived<string | null>(
    this.layouts.some((l) => l.id === this.requestedLayout)
      ? this.requestedLayout
      : (this.profile?.active_layout ?? this.layouts[0]?.id ?? null),
  );

  key = $derived<string | null>(
    this.monitor && this.layoutId ? keyOf(this.monitor.profile_id, this.layoutId) : null,
  );

  saved = $derived<Layout | null>(this.layouts.find((l) => l.id === this.layoutId) ?? null);

  /** The layout shown in the canvas (draft if there is one). */
  layout = $derived<Layout | null>((this.key ? this.drafts[this.key] : undefined) ?? this.saved);

  dirty = $derived<boolean>(this.key !== null && this.key in this.drafts);

  canUndo = $derived<boolean>(this.key !== null && (this.undoStacks[this.key]?.length ?? 0) > 0);

  isActive = $derived<boolean>(
    this.layoutId !== null && this.profile?.active_layout === this.layoutId,
  );

  /** Is there an unsaved draft for this layout of this monitor? */
  isDirty(monitorId: string, layoutId: string): boolean {
    return keyOf(monitorId, layoutId) in this.drafts;
  }

  /** Does any layout of this monitor have unsaved changes? */
  hasDrafts(monitorId: string): boolean {
    const prefix = `${monitorId}\u0001`;
    return Object.keys(this.drafts).some((k) => k.startsWith(prefix));
  }

  // ---- selection and tools -------------------------------------------------

  selectMonitor(profileId: string): void {
    this.requestedMonitor = profileId;
    this.requestedLayout = null;
    this.selectedZone = null;
  }

  selectLayout(layoutId: string): void {
    this.requestedLayout = layoutId;
    this.selectedZone = null;
  }

  selectZone(zoneId: number | null): void {
    this.selectedZone = zoneId;
  }

  setTool(tool: Tool): void {
    this.tool = tool;
  }

  say(kind: StatusKind, text: string): void {
    this.status = { kind, text, seq: this.status.seq + 1 };
  }

  async loadTemplates(): Promise<void> {
    try {
      this.templates = await api.listTemplates();
    } catch (e) {
      this.say('error', `Could not load templates: ${errorMessage(e)}`);
    }
  }

  // ---- drafts --------------------------------------------------------------

  private setDraft(key: string, saved: Layout | null, next: Layout): void {
    const drafts = { ...this.drafts };
    if (saved && sameLayout(saved, next)) delete drafts[key];
    else drafts[key] = next;
    this.drafts = drafts;
  }

  private pushUndo(key: string, before: Layout): void {
    const stack = (this.undoStacks[key] ?? []).slice(-49);
    this.undoStacks = { ...this.undoStacks, [key]: [...stack, before] };
  }

  private dropDraft(key: string, alsoHistory: boolean): void {
    const drafts = { ...this.drafts };
    delete drafts[key];
    this.drafts = drafts;
    if (alsoHistory) {
      const stacks = { ...this.undoStacks };
      delete stacks[key];
      this.undoStacks = stacks;
    }
  }

  private currentFor(key: string): Layout | null {
    const [monitorId, layoutId] = key.split('\u0001');
    const saved =
      app.state?.config.monitors.find((p) => p.id === monitorId)?.layouts.find((l) => l.id === layoutId) ??
      null;
    return this.drafts[key] ?? saved;
  }

  // ---- serialised operations ----------------------------------------------

  /** Run backend operations one at a time, in the order they were requested. */
  private enqueue(job: () => Promise<void>): Promise<void> {
    this.pending += 1;
    const run = this.queue.then(async () => {
      try {
        await job();
      } catch (e) {
        this.say('error', errorMessage(e));
      } finally {
        this.pending -= 1;
      }
    });
    this.queue = run;
    return run;
  }

  /**
   * Apply a backend edit command to the current layout. On success the result
   * becomes the draft; on failure the previous layout stays and the backend's
   * error message is shown.
   */
  private edit(
    op: (layout: Layout) => Promise<Layout>,
    done: (before: Layout, after: Layout) => string,
  ): Promise<void> {
    return this.enqueue(async () => {
      const key = this.key;
      const before = key ? this.currentFor(key) : null;
      const saved = this.saved;
      if (!key || !before) return;
      const result = await op(clone(before));
      // Renaming while the command was in flight must not be lost.
      const after: Layout = { ...result, name: this.currentFor(key)?.name ?? result.name };
      this.pushUndo(key, before);
      this.setDraft(key, saved, after);
      this.say('success', done(before, after));
    });
  }

  splitZone(zoneId: number, axis: Axis, at: number): Promise<void> {
    return this.edit(
      (l) => api.editSplitZone(l, zoneId, axis, at),
      (before, after) => {
        const known = new Set(before.zones.map((z) => z.id));
        const created = after.zones.find((z) => !known.has(z.id));
        if (created) this.selectedZone = created.id;
        const how = axis === 'vertical' ? 'vertically' : 'horizontally';
        return `Split zone ${zoneId} ${how} at ${pct(at)}${created ? `; new zone ${created.id}` : ''}.`;
      },
    );
  }

  removeZone(zoneId: number): Promise<void> {
    return this.edit(
      (l) => api.editRemoveZone(l, zoneId),
      (before, after) => {
        const grown = after.zones.find((z) => {
          const old = before.zones.find((o) => o.id === z.id);
          return old !== undefined && JSON.stringify(old.rect) !== JSON.stringify(z.rect);
        });
        this.selectedZone = grown?.id ?? null;
        return grown
          ? `Deleted zone ${zoneId}; zone ${grown.id} took its space.`
          : `Deleted zone ${zoneId}.`;
      },
    );
  }

  moveDivider(axis: Axis, from: number, to: number): Promise<void> {
    return this.edit(
      (l) => api.editMoveDivider(l, axis, from, to),
      () => `Moved the ${axis} divider from ${pct(from)} to ${pct(to)}.`,
    );
  }

  /** Replace the zones of the current layout with a template's zones. */
  applyTemplate(template: Layout): Promise<void> {
    return this.enqueue(async () => {
      const key = this.key;
      const before = key ? this.currentFor(key) : null;
      if (!key || !before) return;
      const after: Layout = { ...before, zones: clone(template.zones) };
      this.pushUndo(key, before);
      this.setDraft(key, this.saved, after);
      this.selectedZone = null;
      this.say('success', `Applied the ${template.name} template. Save to keep it.`);
    });
  }

  /** Rename the current layout (part of the draft until saved). */
  rename(name: string): void {
    const key = this.key;
    const before = key ? this.currentFor(key) : null;
    if (!key || !before || before.name === name) return;
    this.setDraft(key, this.saved, { ...before, name });
  }

  undo(): void {
    const key = this.key;
    if (!key) return;
    const stack = this.undoStacks[key] ?? [];
    const previous = stack[stack.length - 1];
    if (!previous) return;
    this.undoStacks = { ...this.undoStacks, [key]: stack.slice(0, -1) };
    this.setDraft(key, this.saved, previous);
    this.selectedZone = null;
    this.say('info', 'Undid the last change.');
  }

  revert(): void {
    const key = this.key;
    if (!key || !this.dirty) return;
    this.dropDraft(key, true);
    this.selectedZone = null;
    this.say('info', 'Discarded unsaved changes.');
  }

  // ---- persistence ---------------------------------------------------------

  save(): Promise<void> {
    return this.enqueue(async () => {
      const key = this.key;
      const monitor = this.monitor;
      const layout = key ? this.currentFor(key) : null;
      if (!key || !monitor || !layout) return;
      const name = layout.name.trim() || 'Untitled layout';
      const state = await api.saveLayout(monitor.profile_id, clone({ ...layout, name }));
      app.setState(state);
      this.dropDraft(key, false);
      this.say('success', `Saved "${name}".`);
    });
  }

  setActive(layoutId: string): Promise<void> {
    return this.enqueue(async () => {
      const monitor = this.monitor;
      if (!monitor) return;
      // Activate what the user sees: save unsaved edits of that layout first.
      const key = keyOf(monitor.profile_id, layoutId);
      const draft = this.drafts[key];
      if (draft) {
        const name = draft.name.trim() || 'Untitled layout';
        app.setState(await api.saveLayout(monitor.profile_id, clone({ ...draft, name })));
        this.dropDraft(key, false);
      }
      app.setState(await api.setActiveLayout(monitor.profile_id, layoutId));
      const label = this.layouts.find((l) => l.id === layoutId)?.name ?? layoutId;
      this.say('success', `"${label}" is now the active layout on ${monitor.label}.`);
    });
  }

  newLayout(): Promise<void> {
    return this.enqueue(async () => {
      const monitor = this.monitor;
      if (!monitor) return;
      const existing = new Set(this.layouts.map((l) => l.id));
      let id = '';
      do {
        id = `layout-${Date.now().toString(36)}${Math.floor(Math.random() * 1296).toString(36)}`;
      } while (existing.has(id));
      const names = new Set(this.layouts.map((l) => l.name));
      let n = this.layouts.length + 1;
      while (names.has(`Layout ${n}`)) n += 1;
      const name = `Layout ${n}`;
      const layout: Layout = { id, name, zones: [{ id: 1, rect: { x: 0, y: 0, w: 1, h: 1 } }] };
      app.setState(await api.saveLayout(monitor.profile_id, layout));
      this.requestedLayout = id;
      this.selectedZone = null;
      this.tool = 'select';
      this.say('success', `Created "${name}". Split it into zones or apply a template.`);
    });
  }

  // ---- export / import -----------------------------------------------------

  /**
   * Text of the layout on screen (unsaved edits included) for the clipboard, or
   * null if the backend refused it (the reason is then in the status line).
   */
  exportLayout(): Promise<string | null> {
    let text: string | null = null;
    return this.enqueue(async () => {
      const key = this.key;
      const layout = key ? this.currentFor(key) : null;
      if (!layout) return;
      const name = layout.name.trim() || 'Untitled layout';
      text = await api.exportLayout(clone({ ...layout, name }));
    }).then(() => text);
  }

  /**
   * Add the layout in `json` to the selected monitor and select it. Resolves to
   * whether it worked; on failure the backend's message is in the status line.
   */
  importLayout(json: string): Promise<boolean> {
    let done = false;
    return this.enqueue(async () => {
      const monitor = this.monitor;
      if (!monitor) return;
      const known = new Set(this.layouts.map((l) => l.id));
      const state = await api.importLayout(monitor.profile_id, json);
      app.setState(state);
      const added =
        state.config.monitors
          .find((p) => p.id === monitor.profile_id)
          ?.layouts.filter((l) => !known.has(l.id)) ?? [];
      const imported: Layout | undefined = added[added.length - 1];
      if (imported) this.requestedLayout = imported.id;
      this.selectedZone = null;
      this.tool = 'select';
      this.say(
        'success',
        imported
          ? `Imported "${imported.name}" as a new layout on ${monitor.label}. It is selected now.`
          : `Imported the layout on ${monitor.label}.`,
      );
      done = true;
    }).then(() => done);
  }

  deleteLayout(layoutId: string): Promise<void> {
    return this.enqueue(async () => {
      const monitor = this.monitor;
      if (!monitor) return;
      const name = this.layouts.find((l) => l.id === layoutId)?.name ?? layoutId;
      app.setState(await api.deleteLayout(monitor.profile_id, layoutId));
      this.dropDraft(keyOf(monitor.profile_id, layoutId), true);
      if (this.requestedLayout === layoutId) this.requestedLayout = null;
      this.selectedZone = null;
      this.say('success', `Deleted layout "${name}".`);
    });
  }
}

export const editor = new EditorStore();
