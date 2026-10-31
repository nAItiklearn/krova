<script lang="ts">
  import { onDestroy, tick, untrack } from 'svelte';
  import { editor } from '../lib/editor.svelte';
  import { isMac } from '../lib/platform';
  import Icon from './Icon.svelte';

  // Export and import work on plain text (no file dialogs, no plugins): the layout
  // is copied to and pasted from the clipboard. The panels open inline; results and
  // errors go to the shared status line under the canvas (a polite live region), so
  // the text stays in its box and the panel stays open when something is refused.

  type Mode = 'none' | 'export' | 'import';

  const COPIED_MS = 2500;

  let mode = $state<Mode>('none');
  let exportText = $state('');
  let importText = $state('');
  let copied = $state(false);
  let importFailed = $state(false);
  let importing = $state(false);

  let exportBox = $state<HTMLTextAreaElement>();
  let importBox = $state<HTMLTextAreaElement>();
  let exportButton = $state<HTMLButtonElement>();
  let importButton = $state<HTMLButtonElement>();
  let copiedTimer: ReturnType<typeof setTimeout> | undefined;

  onDestroy(() => clearTimeout(copiedTimer));

  const copyKeys = isMac() ? 'Cmd+C' : 'Ctrl+C';

  // The layout on screen by value: a new state object with the same layout in it
  // (every backend push makes one) must not re-export.
  const onScreen = $derived(JSON.stringify(editor.layout));

  // While the export panel is open the text follows the layout on screen: an edit,
  // another layout or another monitor re-exports. The call is untracked because it
  // updates the editor's queue counters.
  $effect(() => {
    if (mode !== 'export') return;
    void onScreen;
    void untrack(() => editor.exportLayout()).then((text) => {
      if (mode !== 'export') return;
      if (text === null) close(true); // refused: the reason is in the status line
      else exportText = text;
    });
  });

  async function openExport(): Promise<void> {
    if (mode === 'export') return close(true);
    mode = 'export';
    exportText = '';
    copied = false;
    await tick();
    exportBox?.focus();
  }

  async function openImport(): Promise<void> {
    if (mode === 'import') return close(true);
    mode = 'import';
    importFailed = false;
    await tick();
    importBox?.focus();
  }

  function close(returnFocus: boolean): void {
    const was = mode;
    mode = 'none';
    copied = false;
    clearTimeout(copiedTimer);
    if (returnFocus) {
      void tick().then(() => (was === 'import' ? importButton : exportButton)?.focus());
    }
  }

  function markCopied(): void {
    copied = true;
    clearTimeout(copiedTimer);
    copiedTimer = setTimeout(() => (copied = false), COPIED_MS);
  }

  /** The async clipboard first; then the old copy command on the selected text. */
  async function writeToClipboard(text: string): Promise<boolean> {
    try {
      if (navigator.clipboard?.writeText) {
        await navigator.clipboard.writeText(text);
        return true;
      }
    } catch {
      // Not allowed (permissions, focus): try the fallback below.
    }
    try {
      exportBox?.focus();
      exportBox?.select();
      return document.execCommand('copy');
    } catch {
      return false;
    }
  }

  async function copy(): Promise<void> {
    if (!exportText) return;
    if (await writeToClipboard(exportText)) {
      markCopied();
      editor.say('success', 'Copied the layout text to the clipboard.');
    } else {
      exportBox?.focus();
      exportBox?.select();
      editor.say(
        'info',
        `Could not copy automatically. The text is selected: press ${copyKeys} to copy it.`,
      );
    }
  }

  async function submitImport(): Promise<void> {
    if (importing) return;
    importing = true;
    importFailed = false;
    try {
      if (await editor.importLayout(importText)) {
        importText = '';
        close(true);
      } else {
        importFailed = true; // keep the text and the panel; the status line has the reason
      }
    } finally {
      importing = false;
    }
  }

  // Escape closes the panel the focus is in. preventDefault tells the editor's own
  // window-level Escape handler (leave the split tool, deselect) to stand down.
  function onPanelKeydown(e: KeyboardEvent): void {
    if (e.key !== 'Escape') return;
    e.preventDefault();
    close(true);
  }
</script>

<section class="card" aria-labelledby="transfer-heading" data-testid="layout-transfer">
  <h3 id="transfer-heading">Export and import</h3>
  <p class="muted small">
    Copy a layout as text to keep it, or paste text to add a layout. Nothing leaves your computer.
  </p>
  <div class="row">
    <button
      type="button"
      class="btn"
      bind:this={exportButton}
      aria-expanded={mode === 'export'}
      title="Show the layout you are editing as text you can copy"
      data-testid="export-layout"
      onclick={openExport}
    >
      <Icon name="export" /> Export layout
    </button>
    <button
      type="button"
      class="btn"
      bind:this={importButton}
      aria-expanded={mode === 'import'}
      title="Paste layout text to add it to this monitor"
      data-testid="import-layout"
      onclick={openImport}
    >
      <Icon name="import" /> Import layout
    </button>
  </div>

  {#if mode === 'export'}
    <!-- svelte-ignore a11y_no_static_element_interactions -->
    <div class="panel" onkeydown={onPanelKeydown} data-testid="export-panel">
      <div class="field">
        <label for="export-text">Layout text</label>
        <textarea
          id="export-text"
          readonly
          rows="10"
          spellcheck="false"
          bind:this={exportBox}
          value={exportText}
          aria-describedby="export-help"
          data-testid="export-text"
        ></textarea>
        <p class="muted small" id="export-help">
          {editor.dirty ? 'Includes your unsaved changes. ' : ''}Copy it, then choose "Import
          layout" on any monitor to add the layout there.
        </p>
      </div>
      <div class="row">
        <button
          type="button"
          class="btn primary"
          disabled={!exportText}
          data-testid="export-copy"
          data-copied={copied}
          onclick={copy}
        >
          <Icon name={copied ? 'check' : 'copy'} />
          {copied ? 'Copied' : 'Copy'}
        </button>
        <button type="button" class="btn" data-testid="export-close" onclick={() => close(true)}>
          Close
        </button>
      </div>
    </div>
  {/if}

  {#if mode === 'import'}
    <!-- svelte-ignore a11y_no_static_element_interactions -->
    <div class="panel" onkeydown={onPanelKeydown} data-testid="import-panel">
      <div class="field">
        <label for="import-text">Layout text to import</label>
        <textarea
          id="import-text"
          rows="10"
          spellcheck="false"
          autocomplete="off"
          bind:this={importBox}
          bind:value={importText}
          aria-invalid={importFailed}
          aria-describedby={importFailed ? 'import-help editor-status' : 'import-help'}
          data-testid="import-text"
          oninput={() => (importFailed = false)}
        ></textarea>
        <p class="muted small" id="import-help">
          Paste text exported from Krova. It is added to {editor.monitor?.label ?? 'this monitor'}
          as a new layout.
        </p>
      </div>
      <div class="row">
        <button
          type="button"
          class="btn primary"
          disabled={importing}
          data-testid="import-submit"
          onclick={submitImport}
        >
          <Icon name="import" /> Import
        </button>
        <button type="button" class="btn" data-testid="import-close" onclick={() => close(true)}>
          Close
        </button>
      </div>
    </div>
  {/if}
</section>

<style>
  .card {
    display: flex;
    flex-direction: column;
    gap: 10px;
    padding: 14px;
    background: var(--surface);
    border: 1px solid var(--border);
    border-radius: var(--radius);
    box-shadow: var(--shadow);
  }

  .row {
    display: flex;
    gap: 8px;
    flex-wrap: wrap;
  }

  .small {
    font-size: 0.9em;
  }

  .panel {
    display: flex;
    flex-direction: column;
    gap: 10px;
    padding-top: 10px;
    border-top: 1px solid var(--border);
  }

  .field {
    display: flex;
    flex-direction: column;
    gap: 6px;
  }

  .field label {
    font-weight: 600;
  }

  textarea {
    white-space: pre-wrap;
    overflow-wrap: anywhere;
  }
</style>
