<script lang="ts">
  // The cheat sheet shown in the switcher window for `navigate: "cheat_sheet"`
  // (Ctrl+Alt+/, the notch menu and the panel). Rows come from the backend's
  // `shortcut_actions`, so the user's own keys are shown. Any key, a click
  // outside or "Done" closes it.
  import { tick } from 'svelte';
  import type { ShortcutActionState } from '../../lib/bindings';
  import { GROUP_ORDER, sheetRows } from './sheet';

  let { actions, open, onclose }: { actions: readonly ShortcutActionState[]; open: boolean; onclose: () => void } =
    $props();

  const rows = $derived(sheetRows(actions));
  const groups = $derived(GROUP_ORDER.filter((g) => rows.some((r) => r.group === g)));

  let el = $state<HTMLElement>();
  $effect(() => {
    if (open) void tick().then(() => el?.focus());
  });

  function onkeydown(e: KeyboardEvent): void {
    // Tab moves to "Done"; any other key closes, like the hint says.
    if (e.key === 'Tab' || e.key === 'Shift') return;
    e.preventDefault();
    onclose();
  }
</script>

{#if open}
  <div class="scrim" role="presentation" onclick={onclose} data-testid="cheat-sheet-scrim">
    <div
      class="sheet"
      role="dialog"
      aria-modal="true"
      aria-labelledby="cheat-title"
      tabindex="-1"
      bind:this={el}
      data-testid="cheat-sheet"
      onclick={(e) => e.stopPropagation()}
      {onkeydown}
    >
      <header>
        <h2 id="cheat-title">Krova shortcuts</h2>
        <button type="button" class="btn small" onclick={onclose}>Done</button>
      </header>
      {#if rows.length === 0}
        <p class="empty">No shortcuts are set.</p>
      {:else}
        <div class="cols">
          {#each groups as g (g)}
            <section aria-label={g}>
              <h3>{g}</h3>
              {#each rows.filter((r) => r.group === g) as r (r.id)}
                <div class="row">
                  <span>{r.label}</span>
                  {#if r.keys.length}
                    <span class="keys">
                      {#each r.keys as k, i (i)}<kbd>{k}</kbd>{/each}
                      {#if r.tail}<span class="tail">+ {r.tail}</span>{/if}
                    </span>
                  {:else}
                    <span class="off">Off</span>
                  {/if}
                </div>
              {/each}
            </section>
          {/each}
        </div>
      {/if}
      <p class="hint">Press any key to close.</p>
    </div>
  </div>
{/if}

<style>
  .scrim {
    position: fixed;
    inset: 0;
    z-index: 100;
    display: grid;
    place-items: center;
    background: rgb(10 10 10 / 0.35);
    animation: fade 0.25s ease both;
  }
  @keyframes fade {
    from {
      opacity: 0;
    }
  }
  .sheet {
    width: min(900px, calc(100vw - 48px));
    max-height: calc(100vh - 64px);
    overflow: auto;
    padding: 22px 24px 18px;
    border-radius: 22px;
    background: var(--surface);
    color: var(--text);
    box-shadow:
      0 0 0 1px var(--border),
      0 40px 80px -24px rgb(0 0 0 / 0.45);
    animation: sheet-in 0.45s var(--spring) both;
    outline: none;
  }
  @keyframes sheet-in {
    from {
      opacity: 0;
      transform: translateY(18px) scale(0.97);
    }
  }
  @media (prefers-reduced-motion: reduce) {
    .scrim,
    .sheet {
      animation: none;
    }
  }
  header {
    display: flex;
    justify-content: space-between;
    align-items: center;
    margin-bottom: 16px;
  }
  h2 {
    margin: 0;
    font-size: 1.2rem;
    font-weight: 700;
    letter-spacing: -0.02em;
  }
  .cols {
    display: grid;
    grid-template-columns: repeat(3, 1fr);
    gap: 24px;
  }
  h3 {
    margin: 0 0 8px;
    font-size: 0.75rem;
    font-weight: 650;
    color: var(--text-soft);
  }
  .row {
    display: flex;
    justify-content: space-between;
    align-items: center;
    gap: 10px;
    padding: 6px 0;
    border-top: 1px solid var(--border);
    font-size: 0.85rem;
  }
  .keys {
    display: inline-flex;
    flex-shrink: 0;
    align-items: center;
    gap: 4px;
    white-space: nowrap;
  }
  .tail,
  .off,
  .empty,
  .hint {
    color: var(--text-muted);
    font-size: 0.82rem;
  }
  .hint {
    margin: 14px 0 0;
    text-align: center;
  }
  @media (max-width: 760px) {
    .cols {
      grid-template-columns: 1fr;
    }
  }
</style>
