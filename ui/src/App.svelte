<script lang="ts">
  import { onMount, untrack } from 'svelte';
  import DemoBadge from './components/DemoBadge.svelte';
  import Editor from './components/Editor.svelte';
  import PermissionBanner from './components/PermissionBanner.svelte';
  import SettingsView from './components/SettingsView.svelte';
  import { app } from './lib/app.svelte';
  import { api } from './lib/backend';
  import type { View } from './lib/bindings';
  import { editor } from './lib/editor.svelte';
  import { settings } from './lib/settings.svelte';

  const VIEWS: { id: View; label: string }[] = [
    { id: 'editor', label: 'Editor' },
    { id: 'settings', label: 'Settings' },
  ];

  // Theme: follow the OS (CSS prefers-color-scheme) unless Settings forces one.
  // The form's pending value wins so the change is visible immediately.
  const theme = $derived(settings.draft?.theme ?? app.state?.config.settings.theme ?? 'system');
  $effect(() => {
    const root = document.documentElement;
    if (theme === 'light' || theme === 'dark') root.dataset.theme = theme;
    else delete root.dataset.theme;
  });

  // Keep the settings form in step with pushes from the backend (tray, other windows).
  $effect(() => {
    const state = app.state;
    if (state) untrack(() => settings.sync(state));
  });

  onMount(() => {
    void app.load();
    void editor.loadTemplates();

    let disposed = false;
    const unlisteners: Array<() => void> = [];
    const track = (pending: Promise<() => void>): void => {
      pending
        .then((off) => (disposed ? off() : unlisteners.push(off)))
        .catch(() => {
          // No event bridge (for example in a plain browser): the UI still works.
        });
    };
    track(api.onNavigate((view) => app.navigate(view)));
    track(api.onStateChanged((state) => app.setState(state)));

    return () => {
      disposed = true;
      for (const off of unlisteners) off();
    };
  });

  function onNavKey(e: KeyboardEvent, i: number): void {
    const n = VIEWS.length;
    let next = i;
    if (e.key === 'ArrowRight') next = (i + 1) % n;
    else if (e.key === 'ArrowLeft') next = (i - 1 + n) % n;
    else return;
    e.preventDefault();
    app.navigate(VIEWS[next].id);
    document.getElementById(`nav-${VIEWS[next].id}`)?.focus();
  }
</script>

<header class="topbar">
  <div class="brand">
    <svg viewBox="0 0 24 24" width="26" height="26" aria-hidden="true" focusable="false">
      <rect class="logo-bg" x="2" y="3" width="20" height="18" rx="4" />
      <path class="logo-line" d="M12 3v18M12 12h10" />
    </svg>
    <span class="name">Krova</span>
  </div>

  <nav aria-label="Main">
    {#each VIEWS as v, i (v.id)}
      <button
        type="button"
        id="nav-{v.id}"
        class="navbtn"
        aria-current={app.view === v.id ? 'page' : undefined}
        data-testid="nav-{v.id}"
        onclick={() => app.navigate(v.id)}
        onkeydown={(e) => onNavKey(e, i)}
      >
        {v.label}
      </button>
    {/each}
  </nav>

  <div class="spacer"></div>

  {#if api.kind === 'mock'}
    <DemoBadge />
  {/if}
</header>

<PermissionBanner />

<main id="main">
  {#if app.loading}
    <p class="info" role="status">Loading...</p>
  {:else if app.loadError || !app.state}
    <div class="info" role="alert" data-testid="load-error">
      <p>Could not load Krova's state: {app.loadError ?? 'no data'}</p>
      <button type="button" class="btn" onclick={() => app.load()}>Try again</button>
    </div>
  {:else if app.view === 'editor'}
    <Editor />
  {:else}
    <SettingsView />
  {/if}
</main>

<footer>
  <span data-testid="version">Krova {app.state ? `v${app.state.version}` : ''}</span>
  <span aria-hidden="true">·</span>
  <span data-testid="privacy"
    >MIT licensed - no accounts, no telemetry, nothing leaves your computer.</span
  >
</footer>

<style>
  .topbar {
    position: sticky;
    top: 0;
    z-index: 10;
    display: flex;
    align-items: center;
    gap: 20px;
    padding: 10px 20px;
    background: var(--surface);
    border-bottom: 1px solid var(--border);
  }

  .brand {
    display: flex;
    align-items: center;
    gap: 8px;
  }

  .logo-bg {
    fill: var(--accent);
  }

  .logo-line {
    fill: none;
    stroke: var(--accent-contrast);
    stroke-width: 1.6;
  }

  .name {
    font-size: 1.15rem;
    font-weight: 700;
    letter-spacing: 0.01em;
  }

  nav {
    display: flex;
    gap: 4px;
  }

  .navbtn {
    padding: 6px 14px;
    border: 0;
    border-radius: var(--radius-sm);
    background: transparent;
    color: var(--text-muted);
    font: inherit;
    font-weight: 600;
    cursor: pointer;
  }

  .navbtn:hover {
    background: var(--surface-2);
    color: var(--text);
  }

  .navbtn[aria-current='page'] {
    background: var(--accent-soft);
    color: var(--accent);
  }

  .spacer {
    flex: 1;
  }

  main {
    flex: 1;
    padding: 16px 20px 12px;
  }

  .info {
    padding: 32px;
    text-align: center;
    color: var(--text-muted);
  }

  footer {
    display: flex;
    flex-wrap: wrap;
    justify-content: center;
    gap: 8px;
    padding: 10px 20px 14px;
    color: var(--text-muted);
    font-size: 0.85rem;
    border-top: 1px solid var(--border);
  }
</style>
