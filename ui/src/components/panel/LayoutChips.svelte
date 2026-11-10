<script lang="ts">
  // The layouts of the monitor under the notch, as chips with a live zone thumbnail.
  // A click makes the layout active (the active one wears the accent ring).
  import { app } from '../../lib/app.svelte';
  import { api, errorMessage } from '../../lib/backend';
  import type { MonitorProfile } from '../../lib/bindings';

  let { profile }: { profile: MonitorProfile } = $props();

  let popped = $state<string | null>(null);
  let busy = $state(false);
  let error = $state('');
  let popTimer: ReturnType<typeof setTimeout> | undefined;

  async function apply(id: string): Promise<void> {
    if (busy) return;
    popped = null;
    // Restart the pop animation even on a second click of the same chip.
    requestAnimationFrame(() => (popped = id));
    clearTimeout(popTimer);
    popTimer = setTimeout(() => (popped = null), 600);
    if (id === profile.active_layout) return;
    busy = true;
    error = '';
    try {
      app.setState(await api.setActiveLayout(profile.id, id));
    } catch (e) {
      error = errorMessage(e);
    } finally {
      busy = false;
    }
  }
</script>

<section class="layouts" aria-labelledby="pl-title">
  <h2 id="pl-title" class="cap">Layouts</h2>
  <div class="grid" data-testid="panel-layouts">
    {#each profile.layouts as l (l.id)}
      {@const on = l.id === profile.active_layout}
      <button
        type="button"
        class="chip"
        class:on
        class:pop={popped === l.id}
        aria-pressed={on}
        title={l.name}
        data-testid="panel-layout-{l.id}"
        onclick={() => apply(l.id)}
      >
        <svg viewBox="0 0 32 20" aria-hidden="true">
          {#each l.zones as z (z.id)}
            <rect
              x={z.rect.x * 32 + 0.8}
              y={z.rect.y * 20 + 0.8}
              width={Math.max(z.rect.w * 32 - 1.6, 0.5)}
              height={Math.max(z.rect.h * 20 - 1.6, 0.5)}
              rx="1.6"
            />
          {/each}
        </svg>
        <span class="name">{l.name}</span>
      </button>
    {/each}
  </div>
  {#if error}<p class="err" role="alert">{error}</p>{/if}
</section>

<style>
  .layouts {
    margin-top: 14px;
  }
  .cap {
    margin: 0 2px 7px;
    color: var(--text-muted);
    font-size: 0.72rem;
    font-weight: 600;
    letter-spacing: 0.02em;
  }
  .grid {
    display: grid;
    grid-template-columns: 1fr 1fr;
    gap: 6px;
  }
  .chip {
    display: flex;
    align-items: center;
    gap: 8px;
    min-width: 0;
    min-height: 38px;
    padding: 7px 9px;
    border: 0;
    border-radius: 11px;
    background: var(--surface-2);
    color: var(--text-muted);
    font: inherit;
    font-size: 0.8rem;
    font-weight: 550;
    text-align: left;
    cursor: pointer;
    transition:
      background-color 0.15s ease,
      box-shadow 0.2s ease,
      transform 0.25s var(--spring);
  }
  .chip:hover {
    background: var(--surface-3);
    color: var(--text);
  }
  .chip:active {
    transform: scale(0.96);
  }
  .chip svg {
    width: 30px;
    height: 19px;
    flex: none;
  }
  .chip rect {
    fill: var(--surface);
    stroke: var(--border-strong);
    stroke-width: 0.8;
    transition: stroke 0.2s ease;
  }
  .name {
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .chip.on {
    background: var(--accent-soft);
    color: var(--text);
    box-shadow: inset 0 0 0 1.5px var(--accent);
  }
  .chip.on rect {
    stroke: var(--accent);
  }
  .chip.pop {
    animation: pop 0.5s var(--pop);
  }
  @keyframes pop {
    0% {
      transform: scale(0.94);
    }
    55% {
      transform: scale(1.04);
    }
    100% {
      transform: scale(1);
    }
  }
  .err {
    margin-top: 6px;
    color: var(--danger);
    font-size: 0.75rem;
  }
  @media (prefers-reduced-motion: reduce) {
    .chip.pop {
      animation: none;
    }
  }
</style>
