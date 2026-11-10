<script lang="ts">
  import type { Snippet } from 'svelte';
  // A section page: large title, one-line subtitle, optional actions, content.
  interface Props {
    title: string;
    sub?: string;
    testid?: string;
    actions?: Snippet;
    children: Snippet;
    wide?: boolean;
  }
  let { title, sub, testid, actions, children, wide = false }: Props = $props();
</script>

<section class="page" class:wide aria-labelledby="page-title" data-testid={testid}>
  <header>
    <div>
      <h2 id="page-title">{title}</h2>
      {#if sub}<p>{sub}</p>{/if}
    </div>
    {#if actions}<div class="actions">{@render actions()}</div>{/if}
  </header>
  {@render children()}
</section>

<style>
  .page {
    max-width: 760px;
    margin: 0 auto;
    padding: 34px 32px 48px;
    animation: k-rise 0.5s var(--ease-out) both;
  }
  .page.wide {
    max-width: 1180px;
  }
  header {
    display: flex;
    align-items: flex-end;
    justify-content: space-between;
    gap: 16px;
    margin-bottom: 26px;
  }
  h2 {
    font-size: 1.75rem;
    font-weight: 720;
    letter-spacing: -0.035em;
  }
  header p {
    margin-top: 4px;
    color: var(--text-muted);
    font-size: 0.95rem;
  }
  .actions {
    display: flex;
    gap: 8px;
  }
</style>
