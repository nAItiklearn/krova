<script lang="ts">
  import { editor } from '../lib/editor.svelte';
  import LayoutCanvas from './LayoutCanvas.svelte';
  import LayoutPanel from './LayoutPanel.svelte';
  import MonitorTabs from './MonitorTabs.svelte';
  import StatusLine from './StatusLine.svelte';
  import Toolbar from './Toolbar.svelte';

  const hint = $derived(
    editor.tool === 'select'
      ? 'Click a zone to select it. Drag a divider to resize; with a divider focused, the arrow keys move it by 1% (Shift: 5%).'
      : 'Move over a zone and click to split it there. From the keyboard: focus a zone, use the arrow keys to move the guide, Enter to split. Esc to stop.',
  );

  function isTyping(t: EventTarget | null): boolean {
    const el = t as HTMLElement | null;
    if (!el) return false;
    return (
      el.tagName === 'INPUT' ||
      el.tagName === 'TEXTAREA' ||
      el.tagName === 'SELECT' ||
      el.isContentEditable
    );
  }

  function onKeydown(e: KeyboardEvent): void {
    if (e.defaultPrevented) return;
    const typing = isTyping(e.target);
    const mod = e.ctrlKey || e.metaKey;

    if (mod && !e.altKey && e.key.toLowerCase() === 's') {
      e.preventDefault();
      void editor.save();
      return;
    }
    if (mod && !e.altKey && !e.shiftKey && e.key.toLowerCase() === 'z' && !typing) {
      e.preventDefault();
      editor.undo();
      return;
    }
    if (typing || mod || e.altKey) return;

    switch (e.key) {
      case 'Escape':
        if (editor.tool !== 'select') editor.setTool('select');
        else editor.selectZone(null);
        break;
      case 'v':
      case 'V':
        editor.setTool('split-vertical');
        break;
      case 'h':
      case 'H':
        editor.setTool('split-horizontal');
        break;
      case 'Delete': {
        const id = editor.layout?.zones.find((z) => z.id === editor.selectedZone)?.id;
        if (id !== undefined) {
          e.preventDefault();
          void editor.removeZone(id);
        }
        break;
      }
    }
  }
</script>

<svelte:window onkeydown={onKeydown} />

{#if editor.monitor && editor.layout}
  {@const monitor = editor.monitor}
  {@const layout = editor.layout}
  <div class="editor" data-testid="editor">
    <!-- Keeps the heading outline h1 > h2 > h3 for the panels below. -->
    <h2 class="sr-only">Zone editor</h2>
    <MonitorTabs />

    <div
      class="grid"
      role="tabpanel"
      id="monitor-panel"
      aria-labelledby="tab-{monitor.profile_id}"
    >
      <section class="stage" aria-label="Zone editor for {monitor.label}">
        <Toolbar />

        <LayoutCanvas
          {layout}
          work={monitor.work}
          tool={editor.tool}
          selected={layout.zones.some((z) => z.id === editor.selectedZone) ? editor.selectedZone : null}
          label="Layout {layout.name} on {monitor.label}"
          onselect={(id) => editor.selectZone(id)}
          onsplit={(id, axis, at) => editor.splitZone(id, axis, at)}
          onmove={(axis, from, to) => editor.moveDivider(axis, from, to)}
          onremove={(id) => void editor.removeZone(id)}
        />

        <p class="caption muted" data-testid="canvas-caption">
          {monitor.label}: work area {monitor.work.w} × {monitor.work.h} px
          (screen {monitor.rect.w} × {monitor.rect.h}).
          {layout.zones.length}
          {layout.zones.length === 1 ? 'zone' : 'zones'}.
        </p>

        <StatusLine />

        <p class="hint muted">{hint}</p>
      </section>

      <LayoutPanel />
    </div>
  </div>
{:else}
  <p class="empty" role="status">No monitors were reported by the system.</p>
{/if}

<style>
  .editor {
    display: flex;
    flex-direction: column;
    gap: 14px;
  }

  .grid {
    display: grid;
    grid-template-columns: minmax(0, 1fr) 300px;
    gap: 18px;
    align-items: start;
  }

  .stage {
    display: flex;
    flex-direction: column;
    gap: 10px;
    min-width: 0;
  }

  .caption,
  .hint {
    text-align: center;
    font-size: 0.9em;
  }

  .empty {
    padding: 32px;
    text-align: center;
    color: var(--text-muted);
  }

  @media (max-width: 860px) {
    .grid {
      grid-template-columns: minmax(0, 1fr);
    }
  }
</style>
