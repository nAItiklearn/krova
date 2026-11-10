<script lang="ts">
  import type { Snippet } from 'svelte';
  // A grouped list: an optional heading above a rounded card whose rows are
  // separated by hairlines.
  interface Props {
    title?: string;
    desc?: string;
    id?: string;
    testid?: string;
    children: Snippet;
    aside?: Snippet;
    flush?: boolean;
  }
  let { title, desc, id, testid, children, aside, flush = false }: Props = $props();
</script>

<section class="group" aria-labelledby={title && id ? `${id}-h` : undefined} data-testid={testid}>
  {#if title}
    <header>
      <div>
        <h3 id={id ? `${id}-h` : undefined}>{title}</h3>
        {#if desc}<p>{desc}</p>{/if}
      </div>
      {#if aside}<div class="aside">{@render aside()}</div>{/if}
    </header>
  {/if}
  <div class="card" class:flush>{@render children()}</div>
</section>

<style>
  .group + :global(.group) {
    margin-top: 26px;
  }
  header {
    display: flex;
    align-items: flex-end;
    justify-content: space-between;
    gap: 12px;
    margin: 0 4px 9px;
  }
  h3 {
    font-size: 0.82rem;
    font-weight: 620;
    color: var(--text-muted);
    letter-spacing: 0;
  }
  header p {
    margin-top: 2px;
    font-size: 0.82rem;
    color: var(--text-soft);
  }
  .card {
    border-radius: var(--radius-lg);
    background: var(--surface);
    box-shadow:
      0 0 0 1px var(--border),
      var(--shadow);
    overflow: hidden;
  }
  .card:not(.flush) > :global(* + *) {
    border-top: 1px solid var(--border);
  }
</style>
