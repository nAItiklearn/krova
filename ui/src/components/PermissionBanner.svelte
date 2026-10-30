<script lang="ts">
  import { app } from '../lib/app.svelte';
  import Icon from './Icon.svelte';

  let checking = $state(false);

  async function check(): Promise<void> {
    checking = true;
    await app.refresh();
    checking = false;
  }

  // Coming back from System Settings: re-check when the window regains focus.
  $effect(() => {
    if (app.state?.permission !== 'denied') return;
    const onFocus = (): void => void app.refresh();
    window.addEventListener('focus', onFocus);
    return () => window.removeEventListener('focus', onFocus);
  });
</script>

{#if app.state?.permission === 'denied'}
  <div class="banner" role="alert" data-testid="permission-banner">
    <Icon name="warning" />
    <div class="body">
      <p class="title">Krova needs Accessibility access to move windows.</p>
      <p>
        Open <strong>System Settings</strong>, go to <strong>Privacy &amp; Security</strong>, then
        <strong>Accessibility</strong>, and turn on <strong>Krova</strong>. If Krova is already in
        the list, switch it off and on again. This message goes away as soon as access is granted.
      </p>
    </div>
    <button type="button" class="btn" disabled={checking} onclick={check} data-testid="permission-recheck">
      Check again
    </button>
  </div>
{/if}

<style>
  .banner {
    display: flex;
    align-items: flex-start;
    gap: 12px;
    margin: 12px 20px 0;
    padding: 12px 14px;
    background: var(--warn-bg);
    border: 1px solid var(--warn-border);
    border-radius: var(--radius);
    color: var(--warn-text);
  }

  .banner :global(svg) {
    width: 22px;
    height: 22px;
    flex: none;
    margin-top: 1px;
  }

  .body {
    flex: 1;
    min-width: 0;
  }

  .title {
    font-weight: 700;
    margin-bottom: 2px;
  }
</style>
