<script lang="ts">
  import { onMount, untrack } from 'svelte';
  import DemoBadge from './components/DemoBadge.svelte';
  import Editor from './components/Editor.svelte';
  import PermissionBanner from './components/PermissionBanner.svelte';
  import NavIcon, { type NavIconName } from './components/ui/NavIcon.svelte';
  import Toggle from './components/ui/Toggle.svelte';
  import Overview from './components/views/Overview.svelte';
  import Arrange from './components/views/Arrange.svelte';
  import Shortcuts from './components/views/Shortcuts.svelte';
  import Appearance from './components/views/Appearance.svelte';
  import Corners from './components/views/Corners.svelte';
  import Displays from './components/views/Displays.svelte';
  import Privacy from './components/views/Privacy.svelte';
  import Workspaces from './components/views/Workspaces.svelte';
  import Focus from './components/views/Focus.svelte';
  import Screen from './components/views/Screen.svelte';
  import { app, type Section } from './lib/app.svelte';
  import { api } from './lib/backend';
  import { editor } from './lib/editor.svelte';
  import { ext } from './lib/ext.svelte';
  import { applyPendingView } from './lib/pendingView';
  import { settings } from './lib/settings.svelte';

  type NavItem = { id: Section; label: string; icon: NavIconName };
  const GROUPS: { label: string; items: NavItem[] }[] = [
    {
      label: 'Krova',
      items: [
        { id: 'overview', label: 'Overview', icon: 'overview' },
        { id: 'layouts', label: 'Layouts', icon: 'layouts' },
        { id: 'arrange', label: 'Arrange', icon: 'arrange' },
        { id: 'workspaces', label: 'Workspaces', icon: 'workspaces' },
        { id: 'focus', label: 'Focus', icon: 'focus' },
      ],
    },
    {
      label: 'Control',
      items: [
        { id: 'shortcuts', label: 'Shortcuts', icon: 'shortcuts' },
        { id: 'corners', label: 'Corners & notch', icon: 'corners' },
      ],
    },
    {
      label: 'Personal',
      items: [
        { id: 'appearance', label: 'Appearance', icon: 'appearance' },
        { id: 'displays', label: 'Displays', icon: 'displays' },
        { id: 'screen', label: 'Screen edges', icon: 'screen' },
        { id: 'privacy', label: 'Privacy & about', icon: 'privacy' },
      ],
    },
  ];
  const ORDER = GROUPS.flatMap((g) => g.items.map((i) => i.id));

  // Theme: follow the OS unless Appearance forces one; the pending form value wins.
  const theme = $derived(settings.draft?.theme ?? app.state?.config.settings.theme ?? 'system');
  $effect(() => {
    const root = document.documentElement;
    if (theme === 'light' || theme === 'dark') root.dataset.theme = theme;
    else delete root.dataset.theme;
  });
  // The chosen accent drives the whole UI.
  const accent = $derived(ext.get('appearance').accent);
  $effect(() => {
    const s = document.documentElement.style;
    s.setProperty('--accent', accent);
    s.setProperty('--divider', accent);
    s.setProperty('--preview', accent);
    s.setProperty('--accent-soft', `color-mix(in srgb, ${accent} 12%, transparent)`);
    s.setProperty('--accent-softer', `color-mix(in srgb, ${accent} 7%, transparent)`);
  });

  $effect(() => {
    const state = app.state;
    if (state) untrack(() => settings.sync(state));
  });

  onMount(() => {
    void app.load();
    void editor.loadTemplates();
    let disposed = false;
    const unlisteners: Array<() => void> = [];
    const track = (pending: Promise<() => void>): Promise<boolean> =>
      pending
        .then((off) => {
          if (disposed) off();
          else unlisteners.push(off);
          return true;
        })
        .catch(() => false);
    void track(api.onNavigate((view) => app.navigate(view))).then((registered) => {
      if (registered && !disposed) void applyPendingView(api, (view) => app.navigate(view));
    });
    void track(api.onStateChanged((state) => app.setState(state)));
    return () => {
      disposed = true;
      for (const off of unlisteners) off();
    };
  });

  // Sliding selection behind the active sidebar item.
  let navEl: HTMLElement | undefined = $state();
  let sel = $state({ y: 0, h: 0, ready: false });
  function measure(): void {
    const on = navEl?.querySelector<HTMLElement>('[aria-current="page"]');
    if (!on) return;
    sel = { y: on.offsetTop, h: on.offsetHeight, ready: true };
  }
  $effect(() => {
    void app.view;
    void app.loading;
    requestAnimationFrame(measure);
  });

  function onNavKey(e: KeyboardEvent): void {
    const i = ORDER.indexOf(app.view);
    let next = -1;
    if (e.key === 'ArrowDown') next = (i + 1) % ORDER.length;
    else if (e.key === 'ArrowUp') next = (i - 1 + ORDER.length) % ORDER.length;
    else if (e.key === 'Home') next = 0;
    else if (e.key === 'End') next = ORDER.length - 1;
    if (next < 0) return;
    e.preventDefault();
    app.navigate(ORDER[next]);
    document.getElementById(`nav-${ORDER[next]}`)?.focus();
  }

  const arranging = $derived(ext.get('auto_arrange').enabled && !ext.get('paused'));
  const enabled = $derived(settings.draft?.enabled ?? app.state?.config.settings.enabled ?? true);
</script>

<div class="shell">
  <aside class="sidebar">
    <div class="brand">
      <svg viewBox="0 0 32 32" width="26" height="26" aria-hidden="true" focusable="false">
        <rect width="32" height="32" rx="8" fill="var(--ink)" />
        <rect x="5" y="5" width="9" height="22" rx="2.4" fill="var(--on-ink)" />
        <rect x="16" y="5" width="11" height="12" rx="2.4" fill="var(--on-ink)" />
        <rect x="16" y="19" width="5" height="8" rx="2" fill="var(--on-ink)" />
        <rect x="23" y="19" width="4" height="8" rx="1.8" fill="var(--accent)" />
      </svg>
      <h1 class="name">Krova</h1>
    </div>

    <div class="status" class:on={arranging && enabled}>
      <div class="st-text">
        <span class="dot" aria-hidden="true"></span>
        <span>
          <strong id="st-label">{!enabled ? 'Krova is off' : arranging ? 'Arranging' : 'Paused'}</strong>
          <small>{!enabled ? 'Nothing is moved' : arranging ? 'Windows find a zone' : 'Windows stay put'}</small>
        </span>
      </div>
      <Toggle
        id="st-toggle"
        checked={arranging}
        labelledby="st-label"
        disabled={!enabled}
        onchange={(v) => {
          ext.patch('auto_arrange', { enabled: true });
          ext.set('paused', !v);
        }}
      />
    </div>

    <!-- svelte-ignore a11y_no_noninteractive_element_interactions -->
    <nav aria-label="Sections" bind:this={navEl} onkeydown={onNavKey}>
      <span class="sel" class:ready={sel.ready} style="transform: translateY({sel.y}px); height: {sel.h}px" aria-hidden="true"></span>
      {#each GROUPS as g (g.label)}
        <p class="glabel">{g.label}</p>
        {#each g.items as item (item.id)}
          <button
            type="button"
            id="nav-{item.id}"
            class="navbtn"
            aria-current={app.view === item.id ? 'page' : undefined}
            data-testid="nav-{item.id}"
            tabindex={app.view === item.id ? 0 : -1}
            onclick={() => app.navigate(item.id)}
          >
            <NavIcon name={item.icon} />
            <span>{item.label}</span>
          </button>
        {/each}
      {/each}
    </nav>

    {#if api.kind === 'mock'}<div class="demo"><DemoBadge /></div>{/if}
    <footer class="foot">
      <span data-testid="version">Krova {app.state ? `v${app.state.version}` : ''}</span>
      <span data-testid="privacy">Offline · no telemetry</span>
    </footer>
  </aside>

  <main id="main" class="content">
    <PermissionBanner />
    {#if app.loading}
      <p class="info" role="status">Loading…</p>
    {:else if app.loadError || !app.state}
      <div class="info" role="alert" data-testid="load-error">
        <p>Could not load Krova's state: {app.loadError ?? 'no data'}</p>
        <button type="button" class="btn" onclick={() => app.load()}>Try again</button>
      </div>
    {:else}
      {#key app.view}
        <div class="view">
          {#if app.view === 'overview'}
            <Overview />
          {:else if app.view === 'layouts'}
            <div class="editor-page"><Editor /></div>
          {:else if app.view === 'arrange'}
            <Arrange />
          {:else if app.view === 'shortcuts'}
            <Shortcuts />
          {:else if app.view === 'appearance'}
            <Appearance />
          {:else if app.view === 'corners'}
            <Corners />
          {:else if app.view === 'displays'}
            <Displays />
          {:else if app.view === 'workspaces'}
            <Workspaces />
          {:else if app.view === 'focus'}
            <Focus />
          {:else if app.view === 'screen'}
            <Screen />
          {:else}
            <Privacy />
          {/if}
        </div>
      {/key}
    {/if}
  </main>
</div>

<style>
  .shell {
    display: grid;
    grid-template-columns: 236px minmax(0, 1fr);
    height: 100vh;
  }
  .sidebar {
    display: flex;
    flex-direction: column;
    gap: 14px;
    padding: 16px 12px 12px;
    background: var(--sidebar);
    border-right: 1px solid var(--border);
    overflow-y: auto;
  }
  .brand {
    display: flex;
    align-items: center;
    gap: 9px;
    padding: 4px 8px 2px;
  }
  .name {
    font-size: 1.05rem;
    font-weight: 700;
    letter-spacing: -0.02em;
    margin-right: auto;
  }

  .status {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 10px;
    padding: 11px 12px;
    border-radius: 14px;
    background: var(--surface);
    box-shadow:
      0 0 0 1px var(--border),
      var(--shadow);
  }
  .st-text {
    display: flex;
    align-items: center;
    gap: 10px;
    min-width: 0;
  }
  .st-text strong {
    display: block;
    font-weight: 620;
    font-size: 0.9rem;
  }
  .st-text small {
    display: block;
    color: var(--text-muted);
    font-size: 0.76rem;
    white-space: nowrap;
  }
  .dot {
    width: 8px;
    height: 8px;
    border-radius: 50%;
    background: var(--text-soft);
    flex: none;
    transition: background 0.3s ease;
  }
  .status.on .dot {
    background: var(--accent);
    box-shadow: 0 0 0 4px var(--accent-soft);
    animation: breathe 2.6s ease-in-out infinite;
  }
  @keyframes breathe {
    50% {
      box-shadow: 0 0 0 6px var(--accent-softer);
    }
  }

  nav {
    position: relative;
    display: flex;
    flex-direction: column;
    gap: 1px;
  }
  .sel {
    position: absolute;
    left: 0;
    right: 0;
    top: 0;
    border-radius: 9px;
    background: var(--surface);
    box-shadow:
      0 0 0 1px var(--border),
      0 1px 2px rgb(0 0 0 / 0.05);
    opacity: 0;
  }
  .sel.ready {
    opacity: 1;
    transition:
      transform 0.45s var(--spring),
      height 0.45s var(--spring);
  }
  .glabel {
    margin: 12px 10px 4px;
    font-size: 0.72rem;
    font-weight: 600;
    color: var(--text-soft);
  }
  .glabel:first-of-type {
    margin-top: 2px;
  }
  .navbtn {
    position: relative;
    display: flex;
    align-items: center;
    gap: 10px;
    height: 34px;
    padding: 0 10px;
    border: 0;
    border-radius: 9px;
    background: transparent;
    color: var(--text-muted);
    font: inherit;
    font-weight: 550;
    text-align: left;
    cursor: pointer;
    transition: color 0.15s ease;
  }
  .navbtn:hover,
  .navbtn[aria-current='page'] {
    color: var(--text);
  }
  .navbtn[aria-current='page'] :global(svg) {
    color: var(--accent);
  }

  .demo {
    margin-top: auto;
    font-size: 0.78rem;
  }
  .demo + .foot {
    margin-top: 0;
  }
  .brand svg {
    flex: none;
  }
  .foot {
    margin-top: auto;
    display: flex;
    flex-direction: column;
    gap: 1px;
    padding: 8px 10px 2px;
    font-size: 0.75rem;
    color: var(--text-soft);
  }

  .content {
    position: relative;
    overflow-y: auto;
    background: var(--bg);
  }
  .view {
    min-height: 100%;
  }
  .editor-page {
    padding: 20px 22px 24px;
    animation: k-rise 0.5s var(--ease-out) both;
  }
  .info {
    padding: 48px;
    text-align: center;
    color: var(--text-muted);
  }

  @media (max-width: 760px) {
    .shell {
      grid-template-columns: 64px minmax(0, 1fr);
    }
    .navbtn span,
    .glabel,
    .status,
    .foot,
    .name {
      display: none;
    }
  }
</style>
