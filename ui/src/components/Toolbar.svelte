<script lang="ts">
  import { editor } from '../lib/editor.svelte';
  import Icon from './Icon.svelte';

  const selectedZone = $derived(
    editor.layout?.zones.find((z) => z.id === editor.selectedZone)?.id ?? null,
  );
</script>

<div class="toolbar" role="toolbar" aria-label="Editing tools">
  <div class="group" role="group" aria-label="Tool">
    <button
      type="button"
      class="btn"
      aria-pressed={editor.tool === 'select'}
      aria-keyshortcuts="Escape"
      title="Select zones and drag dividers (Esc)"
      data-testid="tool-select"
      onclick={() => editor.setTool('select')}
    >
      <Icon name="select" /> Select
    </button>
    <button
      type="button"
      class="btn"
      aria-pressed={editor.tool === 'split-vertical'}
      aria-keyshortcuts="V"
      title="Split a zone with a vertical divider (V)"
      data-testid="tool-split-vertical"
      onclick={() => editor.setTool('split-vertical')}
    >
      <Icon name="split-vertical" /> Split vertically
    </button>
    <button
      type="button"
      class="btn"
      aria-pressed={editor.tool === 'split-horizontal'}
      aria-keyshortcuts="H"
      title="Split a zone with a horizontal divider (H)"
      data-testid="tool-split-horizontal"
      onclick={() => editor.setTool('split-horizontal')}
    >
      <Icon name="split-horizontal" /> Split horizontally
    </button>
  </div>

  <div class="group" role="group" aria-label="Zone actions">
    <button
      type="button"
      class="btn danger"
      disabled={selectedZone === null}
      aria-keyshortcuts="Delete"
      title="Delete the selected zone; a neighbour takes its space (Delete)"
      data-testid="delete-zone"
      onclick={() => selectedZone !== null && editor.removeZone(selectedZone)}
    >
      <Icon name="trash" /> Delete zone
    </button>
    <button
      type="button"
      class="btn"
      disabled={!editor.canUndo}
      aria-keyshortcuts="Control+Z Meta+Z"
      title="Undo the last change (Ctrl+Z)"
      data-testid="undo"
      onclick={() => editor.undo()}
    >
      <Icon name="undo" /> Undo
    </button>
  </div>

  <div class="spacer"></div>

  {#if editor.dirty}
    <span class="note" data-testid="unsaved-note">Unsaved changes</span>
  {/if}
  <div class="group" role="group" aria-label="Save">
    <button
      type="button"
      class="btn"
      disabled={!editor.dirty}
      title="Discard unsaved changes to this layout"
      data-testid="revert"
      onclick={() => editor.revert()}
    >
      <Icon name="revert" /> Revert
    </button>
    <button
      type="button"
      class="btn primary"
      disabled={!editor.dirty}
      aria-keyshortcuts="Control+S Meta+S"
      title="Save this layout (Ctrl+S)"
      data-testid="save"
      onclick={() => editor.save()}
    >
      <Icon name="save" /> Save
    </button>
  </div>
</div>

<style>
  .toolbar {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 8px 12px;
  }

  .group {
    display: flex;
    flex-wrap: wrap;
    gap: 6px;
  }

  /* Slightly tighter buttons so the whole toolbar fits one row at ~850px. */
  .toolbar :global(.btn) {
    padding: 0 10px;
  }

  .spacer {
    flex: 1 1 0;
  }

  .note {
    color: var(--text-muted);
    font-size: 0.9em;
  }
</style>
