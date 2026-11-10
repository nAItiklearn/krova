<script lang="ts">
  import { onDestroy } from 'svelte';
  import { app } from '../../lib/app.svelte';
  import { api, errorMessage } from '../../lib/backend';
  import type { RestoreReport, Workspace } from '../../lib/bindings';
  import { ext } from '../../lib/ext.svelte';
  import { rectStyle } from '../../lib/geometry';
  import {
    PRIVACY_NOTE,
    SHOW_PATH,
    appCount,
    entryPath,
    launchPaths,
    miniMonitors,
    nextWorkspaceName,
    outcomeLabel,
    workspaceShortcutKey,
  } from '../../lib/workspaces';
  import Icon from '../Icon.svelte';
  import Page from '../ui/Page.svelte';

  const ACK_KEY = 'krova.workspaces.privacy-ack';
  const UNDO_MS = 6000;
  const REVEAL_MS = 140;

  const all = $derived<Workspace[]>(app.state?.config.workspaces ?? []);
  const supported = $derived(app.state?.wave3?.capabilities.workspaces ?? true);
  const shareSafe = $derived(app.state?.config.settings.share_safe ?? false);
  const shortcuts = $derived(ext.get('shortcuts'));

  // ---- delete with undo: the card hides at once, the command runs when the toast ends.
  let pending = $state<{ id: string; name: string } | null>(null);
  let undoTimer: ReturnType<typeof setTimeout> | undefined;
  const list = $derived(all.filter((w) => w.id !== pending?.id));

  async function commitDelete(): Promise<void> {
    clearTimeout(undoTimer);
    const p = pending;
    pending = null;
    if (!p) return;
    try {
      app.setState(await api.deleteWorkspace(p.id));
    } catch (e) {
      error = `Could not delete “${p.name}”: ${errorMessage(e)}`;
    }
  }
  function askDelete(w: Workspace): void {
    if (pending) void commitDelete();
    pending = { id: w.id, name: w.name };
    undoTimer = setTimeout(() => void commitDelete(), UNDO_MS);
  }
  function undo(): void {
    clearTimeout(undoTimer);
    const id = pending?.id;
    pending = null;
    requestAnimationFrame(() => document.getElementById(`ws-restore-${id}`)?.focus());
  }
  onDestroy(() => {
    if (pending) void commitDelete();
  });

  // ---- capture, with the privacy notice before the very first save.
  let error = $state('');
  let capturing = $state(false);
  let asking = $state(false);
  let fresh = $state<string | null>(null);

  function acknowledged(): boolean {
    try {
      return localStorage.getItem(ACK_KEY) === '1';
    } catch {
      return false;
    }
  }
  function capture(): void {
    if (!acknowledged()) {
      asking = true;
      requestAnimationFrame(() => document.getElementById('ws-privacy-ok')?.focus());
      return;
    }
    void doCapture();
  }
  async function doCapture(): Promise<void> {
    if (asking) {
      asking = false;
      try {
        localStorage.setItem(ACK_KEY, '1');
      } catch {
        // fine: the notice shows again next time
      }
    }
    capturing = true;
    error = '';
    const before = new Set(all.map((w) => w.id));
    try {
      const s = await api.captureWorkspace(nextWorkspaceName(all), null);
      app.setState(s);
      fresh = s.config.workspaces.find((w) => !before.has(w.id))?.id ?? null;
    } catch (e) {
      error = errorMessage(e);
    } finally {
      capturing = false;
    }
  }

  // ---- inline rename
  let editing = $state<string | null>(null);
  let draftName = $state('');
  let renameError = $state('');
  function startRename(w: Workspace): void {
    editing = w.id;
    draftName = w.name;
    renameError = '';
    requestAnimationFrame(() => {
      const el = document.getElementById(`ws-name-${w.id}`) as HTMLInputElement | null;
      el?.focus();
      el?.select();
    });
  }
  async function saveRename(w: Workspace): Promise<void> {
    if (editing !== w.id) return;
    if (draftName.trim() === w.name) {
      editing = null;
      return;
    }
    try {
      app.setState(await api.renameWorkspace(w.id, draftName));
      editing = null;
      requestAnimationFrame(() => document.getElementById(`ws-rename-${w.id}`)?.focus());
    } catch (e) {
      renameError = errorMessage(e);
    }
  }
  function cancelRename(id: string): void {
    editing = null;
    requestAnimationFrame(() => document.getElementById(`ws-rename-${id}`)?.focus());
  }

  // ---- restore, with a confirm step listing what "Open missing apps" may start.
  let launch = $state<Record<string, boolean>>({});
  let confirming = $state<string | null>(null);
  let restoring = $state<string | null>(null);
  let reports = $state<Record<string, { report: RestoreReport; shown: number; run: number }>>({});
  let runs = 0;
  const timers: ReturnType<typeof setInterval>[] = [];
  onDestroy(() => timers.forEach(clearInterval));

  function restore(w: Workspace): void {
    if (launch[w.id]) {
      confirming = w.id;
      requestAnimationFrame(() => document.getElementById(`ws-confirm-${w.id}`)?.focus());
      return;
    }
    void doRestore(w, false);
  }
  async function doRestore(w: Workspace, withLaunch: boolean): Promise<void> {
    confirming = null;
    restoring = w.id;
    error = '';
    try {
      const report = await api.restoreWorkspace(w.id, withLaunch);
      const run = ++runs;
      reports = { ...reports, [w.id]: { report, shown: 0, run } };
      const t = setInterval(() => {
        const r = reports[w.id];
        if (!r || r.run !== run || r.shown >= report.entries.length) {
          clearInterval(t);
          return;
        }
        reports = { ...reports, [w.id]: { ...r, shown: r.shown + 1 } };
      }, REVEAL_MS);
      timers.push(t);
      void app.refresh();
    } catch (e) {
      error = `Could not restore “${w.name}”: ${errorMessage(e)}`;
    } finally {
      restoring = null;
    }
  }

  function summary(r: RestoreReport): string {
    const counts = new Map<string, number>();
    for (const e of r.entries) {
      const l = outcomeLabel(e.outcome).label.toLowerCase();
      counts.set(l, (counts.get(l) ?? 0) + 1);
    }
    const parts = [...counts].map(([l, n]) => `${n} ${l}`);
    if (r.minimized > 0) parts.push(`${r.minimized} other ${r.minimized === 1 ? 'window' : 'windows'} minimised`);
    return parts.length ? parts.join(' · ') : 'Nothing to place';
  }
</script>

<Page title="Workspaces" sub="Save the windows on screen and bring them back in one click." testid="view-workspaces">
  {#snippet actions()}
    {#if list.length > 0 && supported}
      <button type="button" class="btn primary" data-testid="ws-capture" disabled={capturing} onclick={capture}>
        <Icon name="plus" />{capturing ? 'Saving…' : 'Save what’s on screen'}
      </button>
    {/if}
  {/snippet}

  {#if !supported}
    <p class="notice" data-testid="ws-unsupported"><Icon name="info" />Workspaces are not available on this system yet.</p>
  {:else}
    {#if asking}
      <div class="ask" role="group" aria-labelledby="ws-privacy-h" aria-describedby="ws-privacy-t" data-testid="ws-privacy-ask">
        <h3 id="ws-privacy-h">Before the first save</h3>
        <p id="ws-privacy-t">{PRIVACY_NOTE}</p>
        <div class="ask-actions">
          <button type="button" class="btn" onclick={() => (asking = false)}>Cancel</button>
          <button type="button" class="btn primary" id="ws-privacy-ok" data-testid="ws-privacy-ok" onclick={() => void doCapture()}>Save workspace</button>
        </div>
      </div>
    {/if}

    {#if error}<p class="notice warn" role="alert" data-testid="ws-error"><Icon name="warning" />{error}</p>{/if}

    {#if list.length === 0}
      <div class="empty" data-testid="ws-empty">
        <svg class="art" viewBox="0 0 120 70" aria-hidden="true">
          <rect x="4" y="4" width="112" height="62" rx="8" class="a-mon" />
          <rect x="11" y="11" width="47" height="48" rx="4" class="a-z on" />
          <rect x="62" y="11" width="47" height="22" rx="4" class="a-z" />
          <rect x="62" y="37" width="47" height="22" rx="4" class="a-z on2" />
        </svg>
        <h3>No workspaces yet</h3>
        <p>Arrange your apps into zones, then save them here to bring the whole setup back later.</p>
        <button type="button" class="btn primary" data-testid="ws-capture" disabled={capturing} onclick={capture}>
          <Icon name="plus" />{capturing ? 'Saving…' : 'Save what’s on screen'}
        </button>
      </div>
    {:else}
      <ul class="cards" aria-label="Saved workspaces">
        {#each list as w (w.id)}
          {@const mons = app.state ? miniMonitors(w, app.state) : []}
          {@const key = shortcuts[workspaceShortcutKey(w.id)]}
          {@const rep = reports[w.id]}
          <li class="card" class:fresh={fresh === w.id} data-testid="ws-card" aria-labelledby={editing === w.id ? undefined : `ws-title-${w.id}`} aria-label={editing === w.id ? w.name : undefined}>
            <div class="draws" aria-hidden="true">
              {#each mons as m (m.monitorId)}
                <div class="draw" style:aspect-ratio={m.aspect} title={m.label}>
                  {#each m.zones as z (z.id)}
                    <span class="z" class:on={z.apps.length > 0} style={rectStyle(z.rect)}>
                      {#if z.apps.length && !shareSafe}
                        <span class="app" data-testid="ws-zone-app">{z.apps[0]}{z.apps.length > 1 ? ` +${z.apps.length - 1}` : ''}</span>
                      {/if}
                    </span>
                  {/each}
                </div>
              {/each}
            </div>

            <div class="body">
              <div class="head">
                {#if editing === w.id}
                  <div class="rename">
                    <input
                      type="text"
                      id="ws-name-{w.id}"
                      aria-label="Workspace name"
                      maxlength="64"
                      bind:value={draftName}
                      aria-invalid={renameError ? 'true' : undefined}
                      aria-describedby={renameError ? `ws-rename-err-${w.id}` : undefined}
                      data-testid="ws-name-input"
                      onkeydown={(e) => {
                        if (e.key === 'Enter') void saveRename(w);
                        else if (e.key === 'Escape') {
                          e.preventDefault();
                          cancelRename(w.id);
                        }
                      }}
                      onblur={() => void saveRename(w)}
                    />
                    {#if renameError}<p class="err" id="ws-rename-err-{w.id}" data-testid="ws-rename-error">{renameError}</p>{/if}
                  </div>
                {:else}
                  <h3 id="ws-title-{w.id}" data-testid="ws-name">{w.name}</h3>
                  <button
                    type="button"
                    class="icon-btn"
                    id="ws-rename-{w.id}"
                    aria-label="Rename {w.name}"
                    data-testid="ws-rename"
                    onclick={() => startRename(w)}
                  >
                    <svg viewBox="0 0 20 20" aria-hidden="true"><path d="M12.5 4.5l3 3L8 15H5v-3z" /><path d="M11 6l3 3" /></svg>
                  </button>
                {/if}
                <span class="meta" data-testid="ws-count">{appCount(w.entries.length)}</span>
                {#if key}<kbd class="chip" data-testid="ws-shortcut" title="Shortcut">{key}</kbd>{/if}
              </div>

              {#if mons.some((m) => m.stray.length)}
                <p class="hint">Some zones were removed since this was saved; those apps are skipped.</p>
              {/if}

              <div class="actions">
                <button
                  type="button"
                  class="btn"
                  id="ws-restore-{w.id}"
                  data-testid="ws-restore"
                  disabled={restoring !== null || w.entries.length === 0}
                  onclick={() => restore(w)}
                >
                  {restoring === w.id ? 'Restoring…' : 'Restore'}
                </button>
                <label class="check">
                  <input
                    type="checkbox"
                    checked={!!launch[w.id]}
                    data-testid="ws-launch"
                    onchange={(e) => (launch = { ...launch, [w.id]: e.currentTarget.checked })}
                  />
                  <span>Open missing apps</span>
                </label>
                <button type="button" class="btn small danger del" aria-label="Delete {w.name}" data-testid="ws-delete" onclick={() => askDelete(w)}>
                  <Icon name="trash" />
                </button>
              </div>

              {#if confirming === w.id}
                <div class="confirm" role="group" aria-labelledby="ws-confirm-h-{w.id}" data-testid="ws-confirm">
                  <p id="ws-confirm-h-{w.id}">Krova may start these programs if they are not open:</p>
                  <ul class="paths">
                    {#each launchPaths(w) as p (p)}<li data-testid="ws-confirm-path">{p}</li>{/each}
                  </ul>
                  <div class="ask-actions">
                    <button type="button" class="btn small" onclick={() => (confirming = null)}>Cancel</button>
                    <button type="button" class="btn small primary" id="ws-confirm-{w.id}" data-testid="ws-confirm-ok" onclick={() => void doRestore(w, true)}>
                      Open and restore
                    </button>
                  </div>
                </div>
              {/if}

              {#if rep}
                <div class="report" data-testid="ws-report">
                  <ol>
                    {#each rep.report.entries.slice(0, rep.shown) as e, i (i)}
                      {@const o = outcomeLabel(e.outcome)}
                      <li data-testid="ws-report-entry" data-outcome={e.outcome}>
                        <span class="prog">
                          <span class="pname">{e.program}</span>
                          {#if SHOW_PATH.has(e.outcome)}<span class="path" data-testid="ws-report-path">{entryPath(w, e)}</span>{/if}
                        </span>
                        <span class="badge {o.tone}">{o.label}</span>
                      </li>
                    {/each}
                  </ol>
                  <p class="sum" role="status" data-testid="ws-report-summary">
                    {rep.shown >= rep.report.entries.length ? summary(rep.report) : ''}
                  </p>
                </div>
              {/if}
            </div>
          </li>
        {/each}
      </ul>
    {/if}

    <p class="privacy" data-testid="ws-privacy">{PRIVACY_NOTE}</p>
  {/if}

  {#if pending}
    <div class="toast" role="status" data-testid="ws-toast">
      <span>Deleted “{pending.name}”</span>
      <button type="button" class="btn small" data-testid="ws-undo" onclick={undo}>Undo</button>
    </div>
  {/if}
</Page>

<style>
  .empty {
    display: grid;
    justify-items: center;
    gap: 10px;
    padding: 40px 24px 36px;
    border-radius: var(--radius-xl);
    background: var(--surface);
    box-shadow:
      0 0 0 1px var(--border),
      var(--shadow);
    text-align: center;
  }
  .empty p {
    max-width: 380px;
    color: var(--text-muted);
    margin-bottom: 6px;
  }
  .art {
    width: 150px;
    margin-bottom: 6px;
  }
  .a-mon {
    fill: var(--surface-3);
  }
  .a-z {
    fill: var(--surface);
    animation: k-float 3.2s var(--ease-out) infinite alternate;
  }
  .a-z.on {
    fill: var(--accent-soft);
    stroke: var(--accent);
    stroke-width: 1.5;
  }
  .a-z.on2 {
    fill: var(--accent-softer);
    animation-delay: 0.4s;
  }
  @keyframes k-float {
    to {
      opacity: 0.7;
    }
  }

  .cards {
    list-style: none;
    margin: 0;
    padding: 0;
    display: grid;
    gap: 14px;
  }
  .card {
    display: flex;
    gap: 18px;
    padding: 16px;
    border-radius: var(--radius-lg);
    background: var(--surface);
    box-shadow:
      0 0 0 1px var(--border),
      var(--shadow);
    animation: k-rise 0.45s var(--spring) both;
  }
  .card.fresh {
    animation: ws-pop 0.6s var(--pop) both;
  }
  @keyframes ws-pop {
    from {
      opacity: 0;
      transform: scale(0.96) translateY(6px);
    }
  }
  .draws {
    display: flex;
    gap: 8px;
    flex: none;
    align-items: flex-start;
  }
  .draw {
    position: relative;
    width: 150px;
    border-radius: 7px;
    background: var(--surface-3);
    box-shadow: inset 0 0 0 1.5px var(--border-strong);
    overflow: hidden;
  }
  .draws .draw:nth-child(n + 2) {
    width: 110px;
  }
  .z {
    position: absolute;
    box-sizing: border-box;
    border: 2px solid var(--surface-3);
    border-radius: 5px;
    background: var(--surface);
    display: grid;
    place-items: center;
    overflow: hidden;
  }
  .z.on {
    background: var(--zone-bg-selected);
    box-shadow: inset 0 0 0 1.5px var(--accent);
  }
  .app {
    max-width: 100%;
    padding: 0 3px;
    font-size: 0.66rem;
    font-weight: 600;
    color: var(--text);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .body {
    flex: 1;
    min-width: 0;
    display: flex;
    flex-direction: column;
    gap: 10px;
  }
  .head {
    display: flex;
    align-items: center;
    gap: 8px;
    min-height: 34px;
    flex-wrap: wrap;
  }
  .head h3 {
    font-size: 1.02rem;
    overflow-wrap: anywhere;
  }
  .meta {
    color: var(--text-muted);
    font-size: 0.85rem;
  }
  .chip {
    margin-left: auto;
    width: auto;
  }
  .rename {
    flex: 1;
    min-width: 180px;
  }
  .rename input {
    width: 100%;
  }
  .icon-btn {
    display: grid;
    place-items: center;
    width: 28px;
    height: 28px;
    border: 0;
    border-radius: 8px;
    background: transparent;
    color: var(--text-muted);
    cursor: pointer;
    transition:
      background 0.15s ease,
      color 0.15s ease;
  }
  .icon-btn:hover {
    background: var(--surface-2);
    color: var(--text);
  }
  .icon-btn svg {
    width: 16px;
    height: 16px;
    fill: none;
    stroke: currentColor;
    stroke-width: 1.6;
    stroke-linecap: round;
    stroke-linejoin: round;
  }
  .actions {
    display: flex;
    align-items: center;
    gap: 12px;
    flex-wrap: wrap;
  }
  .check {
    display: inline-flex;
    align-items: center;
    gap: 7px;
    color: var(--text-muted);
    cursor: pointer;
  }
  .check input {
    width: 16px;
    height: 16px;
    margin: 0;
    accent-color: var(--accent);
  }
  .del {
    margin-left: auto;
  }
  .hint {
    font-size: 0.85rem;
    color: var(--text-muted);
  }
  .err {
    margin-top: 4px;
    color: var(--danger);
    font-size: 0.85rem;
  }

  .ask,
  .confirm {
    padding: 14px 16px;
    border-radius: var(--radius);
    background: var(--surface-2);
    box-shadow: inset 0 0 0 1px var(--border);
    animation: k-rise 0.35s var(--spring) both;
  }
  .ask {
    margin-bottom: 16px;
    background: var(--surface);
    box-shadow:
      0 0 0 1px var(--border),
      var(--shadow-lg);
  }
  .ask h3 {
    margin-bottom: 4px;
  }
  .ask p,
  .confirm p {
    color: var(--text-muted);
  }
  .ask-actions {
    display: flex;
    justify-content: flex-end;
    gap: 8px;
    margin-top: 12px;
  }
  .paths {
    margin: 8px 0 0;
    padding-left: 18px;
    font: 0.82rem var(--mono);
    overflow-wrap: anywhere;
  }

  .report ol {
    list-style: none;
    margin: 0;
    padding: 0;
    border-radius: var(--radius-sm);
    box-shadow: inset 0 0 0 1px var(--border);
    overflow: hidden;
  }
  .report li {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 12px;
    padding: 7px 12px;
    animation: ws-in 0.42s var(--spring) both;
  }
  .report li + li {
    border-top: 1px solid var(--border);
  }
  @keyframes ws-in {
    from {
      opacity: 0;
      transform: translateX(-10px);
    }
  }
  .prog {
    display: flex;
    flex-direction: column;
    min-width: 0;
  }
  .pname {
    font-weight: 560;
  }
  .path {
    font: 0.76rem var(--mono);
    color: var(--text-muted);
    overflow-wrap: anywhere;
  }
  .badge {
    flex: none;
    padding: 2px 9px;
    border-radius: 999px;
    font-size: 0.76rem;
    font-weight: 620;
  }
  .badge.ok {
    background: color-mix(in srgb, var(--success) 12%, transparent);
    color: var(--success);
  }
  .badge.info {
    background: var(--surface-3);
    color: var(--text-muted);
  }
  .badge.warn {
    background: var(--warn-bg);
    color: var(--warn-text);
  }
  .sum {
    margin-top: 7px;
    min-height: 1.2em;
    font-size: 0.85rem;
    color: var(--text-muted);
  }
  .privacy {
    margin: 22px 4px 0;
    font-size: 0.82rem;
    color: var(--text-soft);
  }
  .toast {
    position: fixed;
    left: calc(50% + 118px);
    bottom: 22px;
    transform: translateX(-50%);
    display: flex;
    align-items: center;
    gap: 14px;
    padding: 8px 8px 8px 16px;
    border-radius: 12px;
    background: var(--ink);
    color: var(--on-ink);
    box-shadow: var(--shadow-lg);
    animation: ws-toast 0.45s var(--pop) both;
    z-index: 20;
  }
  .toast .btn {
    background: transparent;
    color: var(--on-ink);
    box-shadow: inset 0 0 0 1px color-mix(in srgb, var(--on-ink) 35%, transparent);
  }
  .toast .btn:hover:not(:disabled) {
    background: color-mix(in srgb, var(--on-ink) 12%, transparent);
  }
  @keyframes ws-toast {
    from {
      opacity: 0;
      transform: translate(-50%, 14px) scale(0.96);
    }
  }
  @media (max-width: 760px) {
    .toast {
      left: calc(50% + 32px);
    }
  }
  @media (max-width: 640px) {
    .card {
      flex-direction: column;
    }
  }
</style>
