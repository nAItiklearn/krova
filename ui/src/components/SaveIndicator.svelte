<script lang="ts">
  import { settings } from '../lib/settings.svelte';
  import Icon from './Icon.svelte';

  // Auto-save state of the settings form, for a Page's actions slot. A polite live
  // region, so "Saved" and errors are announced.
</script>

<p class="save-state" role="status" aria-live="polite" data-testid="save-indicator" data-state={settings.status}>
  {#if settings.status === 'saving'}
    Saving...
  {:else if settings.status === 'saved'}
    <Icon name="check" /> Saved
  {:else if settings.status === 'error'}
    <span class="err"><Icon name="warning" /> Could not save: {settings.error}</span>
    <button type="button" class="btn small" onclick={() => settings.flush()}>Retry</button>
  {/if}
</p>

<style>
  .save-state {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    min-height: 28px;
    color: var(--success);
    font-weight: 600;
    font-size: 0.9rem;
  }
  .save-state :global(svg) {
    width: 17px;
    height: 17px;
  }
  .err {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    color: var(--danger);
  }
</style>
