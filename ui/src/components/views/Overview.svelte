<script lang="ts">
  // Overview: your monitors drawn to scale with their active layouts (click a
  // layout to switch), quick actions, and what Krova can do.
  import Page from '../ui/Page.svelte';
  import NavIcon from '../ui/NavIcon.svelte';
  import { app, type Section } from '../../lib/app.svelte';
  import { api, errorMessage } from '../../lib/backend';
  import { ext } from '../../lib/ext.svelte';
  import type { Layout } from '../../lib/bindings';

  const st = $derived(app.state!);
  const paused = $derived(ext.get('paused'));
  const auto = $derived(ext.get('auto_arrange'));
  const arranging = $derived(auto.enabled && !paused);

  // Monitors laid out by their real positions, scaled to fit.
  const box = $derived.by(() => {
    const ms = st.monitors;
    if (!ms.length) return { x: 0, y: 0, w: 1, h: 1 };
    const x = Math.min(...ms.map((m) => m.rect.x));
    const y = Math.min(...ms.map((m) => m.rect.y));
    const r = Math.max(...ms.map((m) => m.rect.x + m.rect.w));
    const b = Math.max(...ms.map((m) => m.rect.y + m.rect.h));
    return { x, y, w: r - x, h: b - y };
  });
  const profileOf = (id: string) => st.config.monitors.find((p) => p.id === id);
  const activeOf = (id: string): Layout | undefined => {
    const p = profileOf(id);
    return p?.layouts.find((l) => l.id === p.active_layout);
  };

  let busy = $state<string | null>(null);
  let note = $state('');
  async function activate(monitorId: string, layoutId: string): Promise<void> {
    busy = layoutId;
    try {
      app.setState(await api.setActiveLayout(monitorId, layoutId));
      note = 'Layout applied.';
    } catch (e) {
      note = errorMessage(e);
    } finally {
      busy = null;
    }
  }

  const go = (s: Section) => app.navigate(s);
  const TIPS: { title: string; text: string; to: Section; cta: string }[] = [
    { title: 'Windows arrange themselves', text: 'Open an app and it takes the next free zone. Close one and the rest fill in.', to: 'arrange', cta: 'Arrange settings' },
    { title: 'Your keys', text: 'Move, swap and switch layouts from the keyboard. Change any shortcut.', to: 'shortcuts', cta: 'Open shortcuts' },
    { title: 'The notch', text: 'A small pill at the edge of the screen. Click it to control Krova from anywhere.', to: 'corners', cta: 'Set up the notch' },
    { title: 'Make it yours', text: 'Accent colour, borders, glow and corner radius, with a live preview.', to: 'appearance', cta: 'Appearance' },
  ];
</script>

<Page title="Overview" sub={arranging ? 'Krova is keeping your windows in their zones.' : 'Auto-arrange is paused. Windows stay where they are.'} testid="view-overview" wide>
  {#snippet actions()}
    <button type="button" class="btn" onclick={() => ext.set('paused', !paused)}>
      <NavIcon name="pause" />{paused ? 'Resume' : 'Pause'}
    </button>
    <button type="button" class="btn primary" onclick={() => go('layouts')}>Edit layouts</button>
  {/snippet}

  <div class="desk">
    <div class="mons" style="aspect-ratio: {box.w} / {box.h}">
      {#each st.monitors as m (m.profile_id)}
        {@const lay = activeOf(m.profile_id)}
        <div
          class="mon"
          style="left:{((m.rect.x - box.x) / box.w) * 100}%; top:{((m.rect.y - box.y) / box.h) * 100}%; width:{(m.rect.w / box.w) * 100}%; height:{(m.rect.h / box.h) * 100}%"
        >
          <div class="glass">
            {#each lay?.zones ?? [] as z, i (z.id)}
              <span
                class="z"
                style="left:{z.rect.x * 100}%; top:{z.rect.y * 100}%; width:{z.rect.w * 100}%; height:{z.rect.h * 100}%; animation-delay:{i * 60}ms"
              ><b>{i + 1}</b></span>
            {/each}
          </div>
          <p class="mlabel">
            {m.label}{#if m.primary}<span class="pri">Main</span>{/if}
            <span class="res">{m.rect.w} × {m.rect.h}</span>
          </p>
        </div>
      {/each}
    </div>
  </div>

  <div class="mon-lists">
    {#each st.monitors as m (m.profile_id)}
      {@const p = profileOf(m.profile_id)}
      <section class="ml" aria-label={`Layouts on ${m.label}`}>
        <p class="ml-h">{m.label}</p>
        <div class="chips" role="radiogroup" aria-label={`Active layout on ${m.label}`}>
          {#each p?.layouts ?? [] as l (l.id)}
            <button
              type="button"
              role="radio"
              class="chip"
              aria-checked={l.id === p?.active_layout}
              disabled={busy !== null}
              onclick={() => activate(m.profile_id, l.id)}
            >
              <svg viewBox="0 0 32 20" aria-hidden="true">
                {#each l.zones as z (z.id)}
                  <rect x={z.rect.x * 32 + 0.8} y={z.rect.y * 20 + 0.8} width={z.rect.w * 32 - 1.6} height={z.rect.h * 20 - 1.6} rx="1.8" />
                {/each}
              </svg>
              <span>{l.name}</span>
            </button>
          {/each}
        </div>
      </section>
    {/each}
  </div>
  <p class="sr-only" role="status" aria-live="polite">{note}</p>

  <h3 class="tips-h">What Krova can do</h3>
  <div class="tips">
    {#each TIPS as t, i (t.title)}
      <article class="tip" style="animation-delay:{120 + i * 70}ms">
        <h4>{t.title}</h4>
        <p>{t.text}</p>
        <button type="button" class="link" onclick={() => go(t.to)}>{t.cta} →</button>
      </article>
    {/each}
  </div>
</Page>

<style>
  .desk {
    padding: 34px 40px 26px;
    border-radius: var(--radius-xl);
    background: var(--wall);
    box-shadow: inset 0 0 0 1px var(--border);
  }
  .mons {
    position: relative;
    max-height: 300px;
    margin: 0 auto;
  }
  .mon {
    position: absolute;
    padding: 0 6px 30px;
  }
  .glass {
    position: relative;
    height: 100%;
    border-radius: 12px;
    background: #1b1b1a;
    box-shadow:
      0 0 0 6px #1b1b1a,
      0 24px 40px -18px rgb(40 20 10 / 0.5);
    overflow: hidden;
  }
  .z {
    position: absolute;
    padding: 3px;
    animation: k-rise 0.55s var(--ease-out) both;
  }
  .z::before {
    content: '';
    position: absolute;
    inset: 3px;
    border-radius: 7px;
    background: color-mix(in srgb, #fff 92%, var(--accent));
  }
  .z:first-child::before {
    background: var(--accent);
  }
  .z b {
    position: absolute;
    left: 10px;
    top: 7px;
    font-size: 0.72rem;
    color: #5c5a55;
  }
  .z:first-child b {
    color: #fff;
  }
  .mlabel {
    position: absolute;
    left: 6px;
    right: 6px;
    bottom: 4px;
    display: flex;
    align-items: center;
    gap: 6px;
    font-weight: 600;
    font-size: 0.82rem;
    white-space: nowrap;
    overflow: hidden;
  }
  .pri {
    padding: 1px 7px;
    border-radius: 999px;
    background: var(--surface);
    box-shadow: 0 0 0 1px var(--border);
    font-size: 0.7rem;
  }
  .res {
    margin-left: auto;
    color: var(--text-muted);
    font-weight: 500;
    font-variant-numeric: tabular-nums;
  }

  .mon-lists {
    display: grid;
    grid-template-columns: repeat(auto-fit, minmax(300px, 1fr));
    gap: 16px;
    margin-top: 18px;
  }
  .ml-h {
    margin: 0 4px 8px;
    color: var(--text-muted);
    font-size: 0.82rem;
    font-weight: 620;
  }
  .chips {
    display: flex;
    flex-wrap: wrap;
    gap: 8px;
  }
  .chip {
    display: inline-flex;
    align-items: center;
    gap: 8px;
    padding: 7px 12px 7px 8px;
    border: 0;
    border-radius: 12px;
    background: var(--surface);
    box-shadow:
      0 0 0 1px var(--border),
      var(--shadow);
    color: var(--text-muted);
    font: inherit;
    font-weight: 550;
    cursor: pointer;
    transition:
      transform 0.3s var(--pop),
      box-shadow 0.2s ease,
      color 0.2s ease;
  }
  .chip:hover {
    transform: translateY(-1px);
    color: var(--text);
  }
  .chip:active {
    transform: scale(0.97);
  }
  .chip[aria-checked='true'] {
    color: var(--text);
    box-shadow:
      0 0 0 1.5px var(--accent),
      0 6px 16px -8px color-mix(in srgb, var(--accent) 60%, transparent);
  }
  .chip svg {
    width: 32px;
    height: 20px;
  }
  .chip rect {
    fill: var(--surface-2);
    stroke: var(--border-strong);
    stroke-width: 0.8;
  }
  .chip[aria-checked='true'] rect {
    fill: var(--accent-soft);
    stroke: var(--accent);
  }

  .tips-h {
    margin: 34px 4px 12px;
    color: var(--text-muted);
    font-size: 0.82rem;
  }
  .tips {
    display: grid;
    grid-template-columns: repeat(auto-fit, minmax(230px, 1fr));
    gap: 14px;
  }
  .tip {
    display: flex;
    flex-direction: column;
    gap: 6px;
    padding: 16px 16px 14px;
    border-radius: var(--radius-lg);
    background: var(--surface);
    box-shadow:
      0 0 0 1px var(--border),
      var(--shadow);
    animation: k-rise 0.55s var(--ease-out) both;
    transition: transform 0.35s var(--pop), box-shadow 0.25s ease;
  }
  .tip:hover {
    transform: translateY(-2px);
    box-shadow:
      0 0 0 1px var(--border),
      var(--shadow-lg);
  }
  .tip h4 {
    margin: 0;
    font-size: 0.95rem;
    font-weight: 650;
    letter-spacing: -0.01em;
  }
  .tip p {
    flex: 1;
    color: var(--text-muted);
    font-size: 0.86rem;
  }
  .link {
    align-self: flex-start;
    margin-top: 4px;
    padding: 0;
    border: 0;
    background: none;
    color: var(--accent-ink);
    font: inherit;
    font-weight: 600;
    cursor: pointer;
  }
  .link:hover {
    text-decoration: underline;
  }
</style>
