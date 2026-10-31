<script lang="ts">
  import { editor } from '../lib/editor.svelte';
  import Icon from './Icon.svelte';
</script>

<!-- Always rendered so screen readers pick up changes. Errors from the backend
     (for example "the change would leave a zone smaller than the minimum size")
     are shown verbatim and the previous layout is kept. -->
<p
  id="editor-status"
  class="status {editor.status.kind}"
  role="status"
  aria-live="polite"
  aria-atomic="true"
  data-testid="status"
  data-kind={editor.status.kind}
>
  {#key editor.status.seq}
    {#if editor.status.kind === 'error'}<Icon name="warning" />{/if}
    <span class="text">{editor.status.text}</span>
  {/key}
</p>

<style>
  .status {
    display: flex;
    align-items: center;
    gap: 8px;
    min-height: 28px;
    padding: 2px 2px;
    color: var(--text-muted);
  }

  .status.success {
    color: var(--success);
  }

  .status.error {
    color: var(--danger);
    font-weight: 600;
  }

  .status :global(svg) {
    width: 18px;
    height: 18px;
    flex: none;
  }
</style>
