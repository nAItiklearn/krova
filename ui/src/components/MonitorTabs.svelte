<script lang="ts">
  import { tick } from 'svelte';
  import { editor } from '../lib/editor.svelte';
  import Icon from './Icon.svelte';

  let tabs: HTMLButtonElement[] = [];

  async function onKey(e: KeyboardEvent, i: number): Promise<void> {
    const n = editor.monitors.length;
    let next = i;
    if (e.key === 'ArrowRight') next = (i + 1) % n;
    else if (e.key === 'ArrowLeft') next = (i - 1 + n) % n;
    else if (e.key === 'Home') next = 0;
    else if (e.key === 'End') next = n - 1;
    else return;
    e.preventDefault();
    editor.selectMonitor(editor.monitors[next].profile_id);
    await tick();
    tabs[next]?.focus();
  }
</script>

<div class="tabs" role="tablist" aria-label="Monitors">
  {#each editor.monitors as m, i (m.profile_id)}
    {@const selected = editor.monitor?.profile_id === m.profile_id}
    {@const unsaved = editor.hasDrafts(m.profile_id)}
    <button
      type="button"
      role="tab"
      class="tab"
      id="tab-{m.profile_id}"
      aria-selected={selected}
      aria-controls="monitor-panel"
      tabindex={selected ? 0 : -1}
      data-testid="monitor-tab"
      data-monitor-id={m.profile_id}
      bind:this={tabs[i]}
      onclick={() => editor.selectMonitor(m.profile_id)}
      onkeydown={(e) => onKey(e, i)}
    >
      <Icon name="monitor" />
      <span class="label">{m.label}</span>
      <span class="res">{m.rect.w} × {m.rect.h}</span>
      {#if unsaved}
        <span class="dot" aria-hidden="true"></span>
        <span class="sr-only">(unsaved changes)</span>
      {/if}
    </button>
  {/each}
</div>

<style>
  .tabs {
    display: flex;
    gap: 4px;
    flex-wrap: wrap;
    border-bottom: 1px solid var(--border);
  }

  .tab {
    display: inline-flex;
    align-items: center;
    gap: 8px;
    padding: 8px 14px;
    margin-bottom: -1px;
    border: 1px solid transparent;
    border-bottom: 0;
    border-radius: var(--radius-sm) var(--radius-sm) 0 0;
    background: transparent;
    color: var(--text-muted);
    font: inherit;
    cursor: pointer;
  }

  .tab :global(svg) {
    width: 18px;
    height: 18px;
  }

  .tab:hover {
    background: var(--surface-2);
    color: var(--text);
  }

  .tab[aria-selected='true'] {
    background: var(--surface);
    border-color: var(--border);
    color: var(--text);
    font-weight: 600;
    box-shadow: inset 0 3px 0 var(--accent);
  }

  .res {
    font: 0.82rem/1 var(--mono);
    color: var(--text-muted);
  }

  .dot {
    width: 8px;
    height: 8px;
    border-radius: 50%;
    background: var(--accent);
  }
</style>
