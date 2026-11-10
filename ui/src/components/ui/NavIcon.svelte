<script lang="ts">
  // Line icons for the sidebar sections, 20px grid, 1.6 stroke.
  export type NavIconName =
    | 'overview'
    | 'layouts'
    | 'arrange'
    | 'shortcuts'
    | 'appearance'
    | 'corners'
    | 'displays'
    | 'privacy'
    | 'bolt'
    | 'switcher'
    | 'sheet'
    | 'pause';
  let { name }: { name: NavIconName } = $props();
</script>

<svg viewBox="0 0 20 20" fill="none" stroke="currentColor" stroke-width="1.6" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true" focusable="false">
  {#if name === 'overview'}
    <rect x="3" y="3" width="6" height="6" rx="1.6" /><rect x="11" y="3" width="6" height="6" rx="1.6" /><rect x="3" y="11" width="6" height="6" rx="1.6" /><rect x="11" y="11" width="6" height="6" rx="1.6" />
  {:else if name === 'layouts'}
    <rect x="2.5" y="3.5" width="15" height="13" rx="2.2" /><path d="M9 3.5v13M9 10h8.5" />
  {:else if name === 'arrange'}
    <rect x="2.5" y="3.5" width="15" height="13" rx="2.2" /><path d="M6 8.5l2 1.5-2 1.5M10.5 12h3.5" />
  {:else if name === 'shortcuts'}
    <rect x="2" y="5" width="16" height="10" rx="2" /><path d="M5.5 8h1M9.5 8h1M13.5 8h1M6 12h8" />
  {:else if name === 'appearance'}
    <circle cx="10" cy="10" r="7" /><path d="M10 3a7 7 0 0 0 0 14z" fill="currentColor" stroke="none" />
  {:else if name === 'corners'}
    <path d="M3 7V5a2 2 0 0 1 2-2h2M13 3h2a2 2 0 0 1 2 2v2M17 13v2a2 2 0 0 1-2 2h-2M7 17H5a2 2 0 0 1-2-2v-2" /><rect x="7.5" y="2.6" width="5" height="1.8" rx="0.9" fill="currentColor" stroke="none" />
  {:else if name === 'displays'}
    <rect x="2.5" y="3.5" width="15" height="10" rx="2" /><path d="M7 17h6M10 13.5V17" />
  {:else if name === 'privacy'}
    <path d="M10 2.5l6 2.5v4.5c0 3.6-2.6 6.4-6 8-3.4-1.6-6-4.4-6-8V5z" /><path d="M7.5 10l1.8 1.8 3.4-3.6" />
  {:else if name === 'bolt'}
    <path d="M11 2.5L4.5 11H10l-1 6.5L15.5 9H10z" />
  {:else if name === 'switcher'}
    <rect x="5" y="8" width="10" height="8.5" rx="1.8" /><path d="M6.5 5.5h7M8 3h4" />
  {:else if name === 'sheet'}
    <rect x="3.5" y="2.5" width="13" height="15" rx="2" /><path d="M6.5 6.5h7M6.5 10h7M6.5 13.5h4" />
  {:else if name === 'pause'}
    <rect x="5.5" y="4" width="3" height="12" rx="1" /><rect x="11.5" y="4" width="3" height="12" rx="1" />
  {/if}
</svg>

<style>
  svg {
    width: 18px;
    height: 18px;
    flex: none;
  }
</style>
