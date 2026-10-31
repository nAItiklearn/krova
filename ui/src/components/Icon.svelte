<script lang="ts">
  type IconName =
    | 'select'
    | 'split-vertical'
    | 'split-horizontal'
    | 'trash'
    | 'undo'
    | 'revert'
    | 'save'
    | 'plus'
    | 'check'
    | 'warning'
    | 'info'
    | 'copy'
    | 'export'
    | 'import'
    | 'monitor';

  let { name }: { name: IconName } = $props();
</script>

<svg
  viewBox="0 0 24 24"
  fill="none"
  stroke="currentColor"
  stroke-width="1.8"
  stroke-linecap="round"
  stroke-linejoin="round"
  aria-hidden="true"
  focusable="false"
>
  {#if name === 'select'}
    <path d="M5 3l14 7-6 2.2L10.8 19z" />
  {:else if name === 'split-vertical'}
    <rect x="3" y="4" width="18" height="16" rx="2" />
    <path d="M12 4v16" />
  {:else if name === 'split-horizontal'}
    <rect x="3" y="4" width="18" height="16" rx="2" />
    <path d="M3 12h18" />
  {:else if name === 'trash'}
    <path d="M4 7h16" />
    <path d="M10 11v6M14 11v6" />
    <path d="M6 7l1 13h10l1-13" />
    <path d="M9 7V4h6v3" />
  {:else if name === 'undo'}
    <path d="M9 14L4 9l5-5" />
    <path d="M4 9h10a6 6 0 010 12h-3" />
  {:else if name === 'revert'}
    <path d="M3 12a9 9 0 109-9 9 9 0 00-6.4 2.6L3 8" />
    <path d="M3 3v5h5" />
  {:else if name === 'save'}
    <path d="M5 3h12l4 4v14H5z" />
    <path d="M8 3v6h8V3" />
    <path d="M8 21v-7h8v7" />
  {:else if name === 'plus'}
    <path d="M12 5v14M5 12h14" />
  {:else if name === 'check'}
    <path d="M5 12.5l4.5 4.5L19 7.5" />
  {:else if name === 'warning'}
    <path d="M12 3l10 18H2z" />
    <path d="M12 10v5M12 18.2v.1" />
  {:else if name === 'info'}
    <circle cx="12" cy="12" r="9" />
    <path d="M12 11v6M12 7.6v.1" />
  {:else if name === 'copy'}
    <rect x="8" y="8" width="12" height="12" rx="2" />
    <path d="M16 8V6a2 2 0 00-2-2H6a2 2 0 00-2 2v8a2 2 0 002 2h2" />
  {:else if name === 'export'}
    <path d="M12 15V4M8 8l4-4 4 4" />
    <path d="M5 13v6h14v-6" />
  {:else if name === 'import'}
    <path d="M12 4v11M8 11l4 4 4-4" />
    <path d="M5 13v6h14v-6" />
  {:else if name === 'monitor'}
    <rect x="3" y="4" width="18" height="12" rx="2" />
    <path d="M8 20h8M12 16v4" />
  {/if}
</svg>
