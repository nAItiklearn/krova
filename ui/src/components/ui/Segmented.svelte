<script lang="ts" generics="T extends string">
  // A segmented control over native radio inputs, with a pill that slides to the
  // chosen option on a spring.
  interface Option {
    value: T;
    label: string;
    hint?: string;
    testid?: string;
  }
  interface Props {
    name: string;
    options: Option[];
    value: T;
    onchange: (value: T) => void;
    label: string;
    disabled?: boolean;
    full?: boolean;
  }
  let { name, options, value, onchange, label, disabled = false, full = false }: Props = $props();

  let root: HTMLElement | undefined = $state();
  let pill = $state({ x: 0, w: 0, ready: false });

  function measure(): void {
    if (!root) return;
    const on = root.querySelector<HTMLElement>('label.on');
    if (!on) return;
    pill = { x: on.offsetLeft, w: on.offsetWidth, ready: true };
  }
  $effect(() => {
    void value;
    void options.length;
    requestAnimationFrame(measure);
  });
  $effect(() => {
    if (!root) return;
    const ro = new ResizeObserver(measure);
    ro.observe(root);
    void document.fonts?.ready.then(measure);
    return () => ro.disconnect();
  });
</script>

<div class="seg" class:full class:disabled role="radiogroup" aria-label={label} bind:this={root}>
  <span
    class="pill"
    class:ready={pill.ready}
    style="transform: translateX({pill.x - 3}px); width: {pill.w}px"
    aria-hidden="true"
  ></span>
  {#each options as o (o.value)}
    <label class:on={o.value === value} title={o.hint}>
      <input
        type="radio"
        {name}
        value={o.value}
        checked={o.value === value}
        {disabled}
        data-testid={o.testid}
        onchange={() => onchange(o.value)}
      />
      <span>{o.label}</span>
    </label>
  {/each}
</div>

<style>
  .seg {
    position: relative;
    display: inline-flex;
    padding: 3px;
    border-radius: 11px;
    background: var(--surface-3);
    isolation: isolate;
  }
  .seg.full {
    display: flex;
    width: 100%;
  }
  .full label {
    flex: 1;
  }
  .pill {
    position: absolute;
    top: 3px;
    bottom: 3px;
    left: 3px;
    border-radius: 8px;
    background: var(--surface);
    box-shadow:
      0 1px 2px rgb(0 0 0 / 0.08),
      0 0 0 0.5px var(--border-strong);
    opacity: 0;
    z-index: -1;
  }
  .pill.ready {
    opacity: 1;
    transition:
      transform 0.42s var(--spring),
      width 0.42s var(--spring);
  }
  label {
    position: relative;
    display: inline-flex;
    align-items: center;
    justify-content: center;
    min-height: 28px;
    padding: 0 12px;
    border-radius: 8px;
    color: var(--text-muted);
    font-weight: 550;
    font-size: 0.93em;
    white-space: nowrap;
    cursor: pointer;
    transition: color 0.2s ease;
    user-select: none;
  }
  label.on,
  label:hover {
    color: var(--text);
  }
  input {
    position: absolute;
    inset: 0;
    margin: 0;
    opacity: 0;
    cursor: pointer;
  }
  label:has(input:focus-visible) {
    outline: 2px solid var(--focus);
    outline-offset: 1px;
  }
  .disabled {
    opacity: 0.5;
    pointer-events: none;
  }
</style>
