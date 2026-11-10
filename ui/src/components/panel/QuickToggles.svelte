<script lang="ts">
  // Focus, Calm desk, Share-safe and Show zones. The backend commands for these are
  // not in this build yet, so the toggles keep local state and say "Coming soon".
  import PanelIcon, { type PanelIconName } from './PanelIcon.svelte';

  type Id = 'focus' | 'calm' | 'share' | 'zones';
  const ITEMS: { id: Id; label: string; icon: PanelIconName; hint: string }[] = [
    { id: 'focus', label: 'Focus', icon: 'focus', hint: 'Dim everything but the active window' },
    { id: 'calm', label: 'Calm desk', icon: 'calm', hint: 'Hide the clutter around your work' },
    { id: 'share', label: 'Share-safe', icon: 'share', hint: 'Hide private windows while sharing' },
    { id: 'zones', label: 'Show zones', icon: 'zones', hint: 'Show the zones of every screen' },
  ];
  let on = $state<Record<Id, boolean>>({ focus: false, calm: false, share: false, zones: false });
</script>

<section class="toggles" aria-labelledby="pt-title">
  <h2 id="pt-title" class="sr-only">Quick toggles</h2>
  {#each ITEMS as t (t.id)}
    <span class="wrap">
      <button
        type="button"
        class="tog"
        aria-pressed={on[t.id]}
        aria-describedby="pt-tip-{t.id}"
        data-testid="panel-toggle-{t.id}"
        onclick={() => (on[t.id] = !on[t.id])}
      >
        <span class="ic"><PanelIcon name={t.icon} /></span>
        <span class="lbl">{t.label}</span>
      </button>
      <span class="tip" role="tooltip" id="pt-tip-{t.id}">{t.hint}. Coming soon.</span>
    </span>
  {/each}
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
