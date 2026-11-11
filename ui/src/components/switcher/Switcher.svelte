<script lang="ts">
  // Root of the `switcher` window (switcher.html): loads the monitor the
  // switcher was opened for, themes the page like the main app, runs the
  // deck's actions against the backend (one at a time, in order) and hides
  // the window when it closes. The window is frameless and transparent; the
  // deck's scrim is the only background (see README "Layout switcher window").
  import { onMount, tick } from 'svelte';
  import { app } from '../../lib/app.svelte';
  import { api, errorMessage, insideTauri } from '../../lib/backend';
  import type { AppState } from '../../lib/bindings';
  import { ext } from '../../lib/ext.svelte';
  import CheatSheet from './CheatSheet.svelte';
  import Deck, { type DeckActions } from './Deck.svelte';

  const demo = api.kind === 'mock';
  const tauri = !demo && insideTauri();

  let monitorId = $state<string | null>(null);
  let open = $state(false);
  /** The window shows the layout deck, or the cheat sheet (`navigate: "cheat_sheet"`). */
  let mode = $state<'deck' | 'sheet'>('deck');
  let loadError = $state<string | null>(null);
  let reopenBtn = $state<HTMLButtonElement>();

  const st = $derived(app.state);
  const profile = $derived(st?.config.monitors.find((m) => m.id === monitorId));
  const snap = $derived(st?.monitors.find((m) => m.profile_id === monitorId));
  const monitorLabel = $derived(snap?.label ?? profile?.label ?? 'this display');
  const subtitle = $derived(
    loadError ?? (snap ? `${snap.label} · ${snap.rect.w} × ${snap.rect.h}` : (profile?.label ?? 'No display connected')),
  );

  // Theme and accent: same rules as the main window (App.svelte).
  const theme = $derived(st?.config.settings.theme ?? 'system');
  $effect(() => {
    const root = document.documentElement;
    if (theme === 'light' || theme === 'dark') root.dataset.theme = theme;
    else delete root.dataset.theme;
  });
  const accent = $derived(ext.get('appearance').accent);
  $effect(() => {
    const s = document.documentElement.style;
    s.setProperty('--accent', accent);
    s.setProperty('--accent-soft', `color-mix(in srgb, ${accent} 12%, transparent)`);
    s.setProperty('--accent-softer', `color-mix(in srgb, ${accent} 7%, transparent)`);
  });

  async function load(): Promise<void> {
    try {
      const [state, mid] = await Promise.all([api.getState(), api.switcherMonitor()]);
      app.setState(state);
      monitorId =
        mid ?? state.monitors.find((m) => m.primary)?.profile_id ?? state.config.monitors[0]?.id ?? null;
      loadError = null;
    } catch (e) {
      loadError = errorMessage(e);
    }
  }

  // Backend calls run one after another. The deck already shows the result
  // optimistically, so a response is only adopted once nothing else is queued
  // (an older answer must not undo a newer change on screen).
  let chain: Promise<unknown> = Promise.resolve();
  let queued = 0;
  let latest: AppState | null = null;
  let failedSince = false;
  function run(op: () => Promise<AppState>): Promise<void> {
    queued++;
    const p = chain
      .then(op)
      .then(
        (s) => {
          latest = s;
        },
        (e: unknown) => {
          failedSince = true;
          throw errorMessage(e);
        },
      )
      .finally(() => {
        if (--queued > 0) return;
        if (failedSince) {
          failedSince = false;
          latest = null;
          void api.getState().then((s) => app.setState(s), () => {});
        } else if (latest) {
          app.setState(latest);
          latest = null;
        }
      });
    chain = p.catch(() => {});
    return p;
  }

  async function hideWindow(): Promise<void> {
    if (!tauri) return;
    try {
      const { getCurrentWindow } = await import('@tauri-apps/api/window');
      await getCurrentWindow().hide();
    } catch {
      // The backend may already have hidden or destroyed it.
    }
  }

  function close(): void {
    if (!open) return;
    open = false;
    if (tauri) {
      const reduced = matchMedia('(prefers-reduced-motion: reduce)').matches;
      setTimeout(() => void hideWindow(), reduced ? 0 : 220);
    } else {
      void tick().then(() => reopenBtn?.focus());
    }
  }

  async function reopen(view: 'deck' | 'sheet' = 'deck'): Promise<void> {
    await load();
    mode = view;
    open = true;
  }

  const mid = (): string => {
    if (!monitorId) throw 'No display is connected.';
    return monitorId;
  };

  const actions: DeckActions = {
    async apply(id) {
      await run(() => api.setActiveLayout(mid(), id));
      setTimeout(close, 150);
    },
    remove: (id) => run(() => api.deleteLayout(mid(), id)),
    restore: (layout, index, activate) => run(() => api.insertLayout(mid(), layout, index, activate)),
    move: (id, to) => run(() => api.moveLayout(mid(), id, to)),
    close,
  };

  onMount(() => {
    const offs: (() => void)[] = [];
    let alive = true;
    const keep = (p: Promise<() => void>): void => {
      void p.then(
        (off) => (alive ? offs.push(off) : off()),
        () => {},
      );
    };
    keep(
      api.onStateChanged((s) => {
        if (queued === 0) app.setState(s);
      }),
    );
    // The backend shows the window and sends `navigate: "switcher"` every time
    // the shortcut is pressed; re-read which monitor it is for.
    let asked = false;
    keep(
      api.onNavigate((v) => {
        if (v !== 'switcher' && v !== 'cheat_sheet') return;
        asked = true;
        void reopen(v === 'cheat_sheet' ? 'sheet' : 'deck');
      }),
    );
    if (tauri) {
      keep(
        import('@tauri-apps/api/window').then(({ getCurrentWindow }) =>
          getCurrentWindow().onFocusChanged(({ payload: focused }) => {
            if (!focused) close();
          }),
        ),
      );
    }
    // The real app sends the view about 250 ms after the page has loaded; open the
    // deck only if none arrives, so the cheat sheet does not flash the deck first.
    // The demo page takes `?view=cheat_sheet`.
    const first = new URLSearchParams(location.search).get('view') === 'cheat_sheet' ? 'sheet' : 'deck';
    if (tauri) {
      void load();
      const t = setTimeout(() => !asked && void reopen(), 700);
      offs.push(() => clearTimeout(t));
    } else void reopen(first);
    return () => {
      alive = false;
      for (const off of offs) off();
    };
  });
</script>

{#if demo}
  <!-- Demo only: something to blur behind the scrim (the real window is transparent). -->
  <div class="desk" aria-hidden="true">
    <div></div>
    <div></div>
    <div></div>
  </div>
  <button
    bind:this={reopenBtn}
    type="button"
    class="btn reopen"
    data-testid="switcher-reopen"
    hidden={open}
    onclick={() => void reopen()}
  >Reopen layout switcher</button>
{/if}

<Deck
  layouts={profile?.layouts ?? []}
  activeId={profile?.active_layout ?? null}
  open={open && mode === 'deck'}
  title="Layouts"
  {subtitle}
  {monitorLabel}
  {actions}
/>

<CheatSheet actions={st?.shortcut_actions ?? []} open={open && mode === 'sheet'} onclose={close} />

<style>
  /* The window is transparent: only the deck's scrim paints. */
  :global(html.switcher),
  :global(html.switcher body) {
    overflow: hidden;
    background: transparent;
  }
  :global(html.switcher #app) {
    min-height: 0;
  }

  .desk {
    position: fixed;
    inset: 0;
    display: grid;
    grid-template-columns: 1.4fr 1fr;
    gap: 12px;
    padding: 12px 12px 64px;
    background: var(--wall);
  }
  .desk div {
    border-radius: 12px;
    background: var(--surface);
    box-shadow:
      0 0 0 1px var(--border),
      var(--shadow-lg);
  }
  .desk div:first-child {
    grid-row: span 2;
  }
  .reopen {
    position: fixed;
    bottom: 16px;
    left: 50%;
    transform: translateX(-50%);
  }
  .reopen:active:not(:disabled) {
    transform: translateX(-50%) scale(0.97);
  }
</style>
