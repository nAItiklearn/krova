<script lang="ts">
  import { demoControls } from '../lib/backend';

  function reset(): void {
    demoControls?.reset();
    location.reload();
  }
</script>

<div class="badge" role="note" data-testid="demo-badge">
  <span>Demo mode - changes are not saved to a real config</span>
  <button type="button" class="reset" onclick={reset} title="Forget demo edits and start over">
    Reset
  </button>
</div>

<style>
  .badge {
    display: inline-flex;
    align-items: center;
    gap: 10px;
    padding: 3px 6px 3px 12px;
    border: 1px dashed var(--warn-border);
    border-radius: 999px;
    background: var(--warn-bg);
    color: var(--warn-text);
    font-size: 0.85rem;
    font-weight: 600;
  }

  .reset {
    padding: 2px 10px;
    border: 1px solid var(--warn-border);
    border-radius: 999px;
    background: transparent;
    color: inherit;
    font: inherit;
    font-size: 0.8rem;
    cursor: pointer;
  }

  .reset:hover {
    background: rgb(127 127 127 / 0.2);
  }
</style>
