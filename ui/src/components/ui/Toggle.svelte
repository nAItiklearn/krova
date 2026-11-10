<script lang="ts">
  // A switch drawn over a native checkbox (role="switch"), so forms, tests and
  // screen readers keep working. The knob stretches while pressed, then springs.
  interface Props {
    id: string;
    checked: boolean;
    onchange: (value: boolean) => void;
    label?: string;
    labelledby?: string;
    describedby?: string;
    disabled?: boolean;
    testid?: string;
  }
  let { id, checked, onchange, label, labelledby, describedby, disabled = false, testid }: Props = $props();
</script>

<span class="tg" class:on={checked} class:disabled>
  <input
    {id}
    type="checkbox"
    role="switch"
    {checked}
    {disabled}
    aria-label={label}
    aria-labelledby={labelledby}
    aria-describedby={describedby}
    aria-checked={checked}
    data-testid={testid}
    onchange={(e) => onchange(e.currentTarget.checked)}
  />
  <span class="track" aria-hidden="true"><span class="knob"></span></span>
</span>

<style>
  .tg {
    position: relative;
    display: inline-flex;
    flex: none;
    width: 42px;
    height: 25px;
  }
  input {
    position: absolute;
    inset: 0;
    margin: 0;
    opacity: 0;
    cursor: pointer;
    z-index: 1;
  }
  .disabled input {
    cursor: not-allowed;
  }
  .track {
    position: absolute;
    inset: 0;
    border-radius: 999px;
    background: var(--surface-3);
    box-shadow: inset 0 0 0 1px var(--border);
    transition: background-color 0.25s ease;
  }
  .on .track {
    background: var(--accent);
    box-shadow: none;
  }
  .knob {
    position: absolute;
    top: 2px;
    left: 2px;
    width: 21px;
    height: 21px;
    border-radius: 999px;
    background: #fff;
    box-shadow:
      0 1px 2px rgb(0 0 0 / 0.18),
      0 3px 8px -2px rgb(0 0 0 / 0.2);
    transition:
      transform 0.38s var(--pop),
      width 0.22s var(--spring);
  }
  .on .knob {
    transform: translateX(17px);
  }
  .tg:active:not(.disabled) .knob {
    width: 25px;
  }
  .on.tg:active:not(.disabled) .knob {
    transform: translateX(13px);
  }
  .disabled {
    opacity: 0.45;
  }
  input:focus-visible + .track {
    outline: 2px solid var(--focus);
    outline-offset: 2px;
  }
</style>
