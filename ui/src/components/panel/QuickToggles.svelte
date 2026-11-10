<script lang="ts">
  // Focus, Calm desk, Share-safe and Show zones. The first three drive the wave3
  // backend: focus dim and calm desk through their toggle commands, share-safe
  // through the settings store (auto-saved like the Focus view). Show zones has no
  // command yet, so it keeps local state and says "Coming soon".
  import { app } from '../../lib/app.svelte';
  import { api, errorMessage } from '../../lib/backend';
  import type { AppState } from '../../lib/bindings';
  import { settings } from '../../lib/settings.svelte';
  import PanelIcon, { type PanelIconName } from './PanelIcon.svelte';

  type Id = 'focus' | 'calm' | 'share' | 'zones';
  const ITEMS: { id: Id; label: string; icon: PanelIconName; hint: string }[] = [
    { id: 'focus', label: 'Focus', icon: 'focus', hint: 'Dim everything but the active window' },
    { id: 'calm', label: 'Calm desk', icon: 'calm', hint: 'Minimise the windows outside your zones' },
    { id: 'share', label: 'Share-safe', icon: 'share', hint: 'Overlays show zone shapes only, never app names' },
    { id: 'zones', label: 'Show zones', icon: 'zones', hint: 'Show the zones of every screen' },
  ];

  let zones = $state(false);
  let busy = $state<Id | null>(null);
  let error = $state('');

  const w3 = $derived(app.state?.wave3);
  const saved = $derived(app.state?.config.settings);
  const on = $derived<Record<Id, boolean>>({
    focus: settings.draft?.focus.enabled ?? saved?.focus.enabled ?? false,
    calm: w3?.calm_desk.active ?? false,
    share: settings.draft?.share_safe ?? saved?.share_safe ?? false,
    zones,
  });
  const available = $derived<Record<Id, boolean>>({
    focus: w3?.capabilities.focus_dim ?? true,
    calm: w3?.capabilities.calm_desk ?? true,
    share: true,
    zones: true,
  });

  function tip(id: Id, hint: string): string {
    if (id === 'zones') return `${hint}. Coming soon.`;
    if (!available[id]) return `${hint}. Not available on this computer.`;
    if (id === 'calm' && on.calm && w3) {
      const n = w3.calm_desk.count;
      return `${hint}. ${n} window${n === 1 ? '' : 's'} minimised; press again to bring them back.`;
    }
    return `${hint}.`;
  }

  /** Run a toggle command; unsaved settings go first so the save cannot undo it. */
  async function command(id: Id, call: () => Promise<AppState>): Promise<void> {
    busy = id;
    error = '';
    try {
      if (settings.pending) await settings.flush();
      const state = await call();
      app.setState(state);
      settings.sync(state);
    } catch (e) {
      error = errorMessage(e);
    } finally {
      busy = null;
    }
  }

  function press(id: Id): void {
    if (busy || !available[id]) return;
    if (id === 'focus') void command(id, () => api.toggleFocusDim());
    else if (id === 'calm') void command(id, () => api.toggleCalmDesk());
    else if (id === 'share') {
      error = '';
      settings.update({ share_safe: !on.share });
    } else zones = !zones;
  }
</script>

<section class="toggles" aria-labelledby="pt-title">
  <h2 id="pt-title" class="sr-only">Quick toggles</h2>
  {#each ITEMS as t (t.id)}
    <span class="wrap">
      <button
        type="button"
        class="tog"
        aria-pressed={on[t.id]}
        aria-disabled={!available[t.id] || undefined}
        aria-busy={busy === t.id || undefined}
        aria-describedby="pt-tip-{t.id}"
        data-testid="panel-toggle-{t.id}"
        onclick={() => press(t.id)}
      >
        <span class="ic"><PanelIcon name={t.icon} /></span>
        <span class="lbl">{t.label}</span>
      </button>
      <span class="tip" role="tooltip" id="pt-tip-{t.id}">{tip(t.id, t.hint)}</span>
    </span>
  {/each}
  {#if error || (settings.status === 'error' && settings.error)}
    <p class="err" role="alert" data-testid="panel-toggle-error">{error || settings.error}</p>
  {/if}
</section>

<style>
  .toggles {
    display: grid;
    grid-template-columns: repeat(4, 1fr);
    gap: 6px;
    margin-top: 12px;
  }
  .wrap {
    position: relative;
    display: flex;
  }
  .tog {
    display: flex;
    flex: 1;
    flex-direction: column;
    align-items: center;
    gap: 5px;
    padding: 9px 2px 8px;
    border: 0;
    border-radius: 12px;
    background: var(--surface-2);
    color: var(--text-muted);
    font: inherit;
    font-size: 0.7rem;
    font-weight: 600;
    cursor: pointer;
    transition:
      background-color 0.15s ease,
      box-shadow 0.2s ease,
      transform 0.25s var(--spring);
  }
  .tog:hover {
    background: var(--surface-3);
    color: var(--text);
  }
  .tog[aria-disabled='true'] {
    opacity: 0.55;
    cursor: not-allowed;
  }
  .tog:active {
    transform: scale(0.94);
  }
  .ic {
    display: grid;
    place-items: center;
    width: 30px;
    height: 30px;
    border-radius: 999px;
    background: var(--surface);
    box-shadow: inset 0 0 0 1px var(--border);
    transition:
      background-color 0.2s ease,
      color 0.2s ease;
  }
  .tog[aria-pressed='true'] {
    background: var(--accent-soft);
    color: var(--text);
    box-shadow: inset 0 0 0 1.5px var(--accent);
  }
  .tog[aria-pressed='true'] .ic {
    background: var(--ink);
    color: var(--on-ink);
    box-shadow: none;
  }
  .lbl {
    white-space: nowrap;
  }
  .err {
    grid-column: 1 / -1;
    margin: 2px 0 0;
    color: var(--danger);
    font-size: 0.72rem;
    line-height: 1.35;
  }
  .tip {
    position: absolute;
    bottom: calc(100% + 6px);
    left: 50%;
    z-index: 5;
    width: max-content;
    max-width: 170px;
    padding: 6px 9px;
    border-radius: 8px;
    background: var(--ink);
    color: var(--on-ink);
    font-size: 0.7rem;
    font-weight: 500;
    line-height: 1.35;
    text-align: center;
    pointer-events: none;
    opacity: 0;
    transform: translate(-50%, 3px);
    transition:
      opacity 0.15s ease,
      transform 0.2s var(--spring);
  }
  .wrap:first-child .tip {
    left: 0;
    transform: translate(0, 3px);
  }
  .wrap:last-child .tip {
    left: auto;
    right: 0;
    transform: translate(0, 3px);
  }
  .wrap:hover .tip,
  .tog:focus-visible + .tip {
    opacity: 1;
    transform: translate(-50%, 0);
    transition-delay: 0.35s;
  }
  .wrap:first-child:hover .tip,
  .wrap:first-child .tog:focus-visible + .tip,
  .wrap:last-child:hover .tip,
  .wrap:last-child .tog:focus-visible + .tip {
    transform: translate(0, 0);
  }
  .tog:focus-visible + .tip {
    transition-delay: 0s;
  }
</style>
