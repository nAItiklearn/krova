<script lang="ts">
  import { editor } from '../lib/editor.svelte';
  import Icon from './Icon.svelte';
  import LayoutThumb from './LayoutThumb.svelte';
  import LayoutTransfer from './LayoutTransfer.svelte';

  const ratio = $derived(editor.monitor ? editor.monitor.work.w / editor.monitor.work.h : 16 / 9);
  const onlyLayout = $derived(editor.layouts.length <= 1);

  // Deleting a layout asks for a second press (no blocking confirm() dialog).
  let armed = $state(false);
  let armTimer: ReturnType<typeof setTimeout> | undefined;

  $effect(() => {
    void editor.layoutId;
    void editor.monitor;
    armed = false;
    clearTimeout(armTimer);
  });

  function onDelete(): void {
    const id = editor.layoutId;
    if (!id) return;
    if (!armed) {
      armed = true;
      editor.say('info', `Press "Delete layout" again within 5 seconds to delete "${editor.layout?.name ?? id}".`);
      armTimer = setTimeout(() => (armed = false), 5000);
      return;
    }
    clearTimeout(armTimer);
    armed = false;
    void editor.deleteLayout(id);
  }
</script>

<aside class="panel" aria-label="Layouts">
  <section class="card" aria-labelledby="current-heading">
    <h3 id="current-heading">This layout</h3>
    <label class="field">
      <span>Name</span>
      <input
        type="text"
        maxlength="60"
        autocomplete="off"
        spellcheck="false"
        value={editor.layout?.name ?? ''}
        oninput={(e) => editor.rename(e.currentTarget.value)}
        data-testid="layout-name"
      />
    </label>
    <div class="row">
      <button
        type="button"
        class="btn"
        disabled={editor.isActive}
        title="Use this layout when dragging windows on this monitor"
        data-testid="set-active"
        onclick={() => editor.layoutId && editor.setActive(editor.layoutId)}
      >
        <Icon name="check" /> {editor.isActive ? 'Active layout' : 'Set as active'}
      </button>
      <button
        type="button"
        class="btn danger"
        class:armed
        disabled={onlyLayout}
        title={onlyLayout ? 'A monitor needs at least one layout' : 'Delete this layout'}
        data-testid="delete-layout"
        onclick={onDelete}
      >
        <Icon name="trash" /> {armed ? 'Confirm delete' : 'Delete layout'}
      </button>
    </div>
  </section>

  <section class="card" aria-labelledby="layouts-heading">
    <div class="head">
      <h3 id="layouts-heading">Layouts on this monitor</h3>
      <button
        type="button"
        class="btn small"
        data-testid="new-layout"
        onclick={() => editor.newLayout()}
      >
        <Icon name="plus" /> New layout
      </button>
    </div>
    <ul class="list" data-testid="layout-list">
      {#each editor.layouts as l (l.id)}
        {@const current = l.id === editor.layoutId}
        {@const unsaved = editor.monitor ? editor.isDirty(editor.monitor.profile_id, l.id) : false}
        <li>
          <button
            type="button"
            class="item"
            aria-current={current ? 'true' : undefined}
            data-testid="layout-item"
            data-layout-id={l.id}
            onclick={() => editor.selectLayout(l.id)}
          >
            <LayoutThumb layout={(current ? editor.layout : null) ?? l} {ratio} />
            <span class="name">{current && editor.layout ? editor.layout.name : l.name}</span>
            {#if l.id === editor.profile?.active_layout}
              <span class="badge">Active</span>
            {/if}
            {#if unsaved}
              <span class="badge warn">Unsaved</span>
            {/if}
          </button>
        </li>
      {/each}
    </ul>
  </section>

  <section class="card" aria-labelledby="templates-heading">
    <h3 id="templates-heading">Templates</h3>
    <p class="muted small">Replace the zones of the layout you are editing. You can undo this.</p>
    <ul class="templates">
      {#each editor.templates as t (t.id)}
        <li>
          <button
            type="button"
            class="tpl"
            data-testid="template-{t.id}"
            title="Apply the {t.name} template"
            onclick={() => editor.applyTemplate(t)}
          >
            <LayoutThumb layout={t} {ratio} />
            <span>{t.name}</span>
          </button>
        </li>
      {/each}
    </ul>
  </section>

  <!-- Last on purpose: it adds Tab stops after the templates, not in the middle. -->
  <LayoutTransfer />
</aside>

<style>
  .panel {
    display: flex;
    flex-direction: column;
    gap: 14px;
    min-width: 0;
  }

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

  .head {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 8px;
    flex-wrap: wrap;
  }

  .field {
    display: flex;
    flex-direction: column;
    gap: 4px;
    font-weight: 500;
  }

  .row {
    display: flex;
    gap: 8px;
    flex-wrap: wrap;
  }

  .small {
    font-size: 0.9em;
  }

  .list,
  .templates {
    list-style: none;
    margin: 0;
    padding: 0;
  }

  .list {
    display: flex;
    flex-direction: column;
    gap: 4px;
  }

  .item,
  .tpl {
    display: flex;
    align-items: center;
    gap: 10px;
    width: 100%;
    padding: 6px 8px;
    border: 1px solid transparent;
    border-radius: var(--radius-sm);
    background: transparent;
    color: var(--text);
    font: inherit;
    text-align: left;
    cursor: pointer;
  }

  .item:hover,
  .tpl:hover {
    background: var(--surface-2);
  }

  .item[aria-current='true'] {
    background: var(--accent-soft);
    border-color: var(--accent);
  }

  .name {
    flex: 1;
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .badge {
    flex: none;
    padding: 1px 8px;
    border-radius: 999px;
    border: 1px solid var(--accent);
    color: var(--accent-ink);
    font-size: 0.78rem;
    font-weight: 600;
  }

  .badge.warn {
    border-color: var(--warn-border);
    color: var(--warn-text);
    background: var(--warn-bg);
  }

  .templates {
    display: grid;
    grid-template-columns: repeat(2, minmax(0, 1fr));
    gap: 6px;
  }

  .tpl {
    flex-direction: column;
    align-items: stretch;
    gap: 6px;
    border-color: var(--border);
    text-align: center;
  }

  .tpl :global(.thumb) {
    width: 100%;
  }
</style>
