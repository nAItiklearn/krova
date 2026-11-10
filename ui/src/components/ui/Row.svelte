<script lang="ts">
  import type { Snippet } from 'svelte';
  // One settings row: a title and a short line of help on the left, the control
  // on the right. `for` links the title to the control's id.
  interface Props {
    label: string;
    desc?: string;
    for?: string;
    id?: string;
    icon?: Snippet;
    children?: Snippet;
    below?: Snippet;
  }
  let { label, desc, for: forId, id, icon, children, below }: Props = $props();
</script>

<div class="row">
  <div class="main">
    {#if icon}<span class="ico" aria-hidden="true">{@render icon()}</span>{/if}
    <div class="text">
      {#if forId}
        <label for={forId} {id}>{label}</label>
      {:else}
        <span class="label" {id}>{label}</span>
      {/if}
      {#if desc}<p id={id ? `${id}-d` : undefined}>{desc}</p>{/if}
    </div>
    {#if children}<div class="ctl">{@render children()}</div>{/if}
  </div>
  {#if below}<div class="below">{@render below()}</div>{/if}
</div>

<style>
  .row {
    padding: 12px 16px;
  }
  .main {
    display: flex;
    align-items: center;
    gap: 12px;
    min-height: 34px;
  }
  .ico {
    display: grid;
    place-items: center;
    width: 28px;
    height: 28px;
    flex: none;
    border-radius: 8px;
    background: var(--surface-2);
    color: var(--text-muted);
  }
  .ico :global(svg) {
    width: 16px;
    height: 16px;
  }
  .text {
    flex: 1;
    min-width: 0;
  }
  label,
  .label {
    display: block;
    font-weight: 550;
    color: var(--text);
  }
  p {
    margin-top: 1px;
    font-size: 0.85rem;
    color: var(--text-muted);
  }
  .ctl {
    display: flex;
    align-items: center;
    gap: 8px;
    flex: none;
  }
  .below {
    margin-top: 12px;
  }
</style>
