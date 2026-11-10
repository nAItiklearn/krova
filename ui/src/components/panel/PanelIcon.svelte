<script lang="ts">
  // Line icons for the quick panel, 20px grid, 1.6 stroke.
  export type PanelIconName = 'focus' | 'calm' | 'share' | 'zones' | 'switcher' | 'sheet' | 'open' | 'chevron';
  let { name }: { name: PanelIconName } = $props();
</script>

<svg viewBox="0 0 20 20" fill="none" stroke="currentColor" stroke-width="1.6" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true" focusable="false">
  {#if name === 'focus'}
    <circle cx="10" cy="10" r="6.5" /><circle cx="10" cy="10" r="2.4" fill="currentColor" stroke="none" />
  {:else if name === 'calm'}
    <path d="M16 12.2A6.5 6.5 0 1 1 7.8 4a5.2 5.2 0 0 0 8.2 8.2z" />
  {:else if name === 'share'}
    <rect x="2.5" y="4" width="15" height="10" rx="2" /><path d="M7 17h6" /><path d="M7.5 9.2l1.8 1.8 3.4-3.6" />
  {:else if name === 'zones'}
    <rect x="2.5" y="3.5" width="15" height="13" rx="2.2" stroke-dasharray="2.2 1.8" /><path d="M10 3.5v13" />
  {:else if name === 'switcher'}
    <rect x="5" y="8" width="10" height="8.5" rx="1.8" /><path d="M6.5 5.5h7M8 3h4" />
  {:else if name === 'sheet'}
    <rect x="2" y="5" width="16" height="10" rx="2" /><path d="M5.5 8h1M9.5 8h1M13.5 8h1M6 12h8" />
  {:else if name === 'open'}
    <rect x="2.5" y="3.5" width="15" height="13" rx="2.2" /><path d="M2.5 7h15" /><circle cx="5" cy="5.3" r="0.4" fill="currentColor" />
  {:else if name === 'chevron'}
    <path d="M8 5.5l4.5 4.5L8 14.5" />
  {/if}
</svg>

<style>
  svg {
    width: 18px;
    height: 18px;
    flex: none;
  }
</style>
