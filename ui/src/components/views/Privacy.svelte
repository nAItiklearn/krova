<script lang="ts">
  import { app } from '../../lib/app.svelte';
  import { settings } from '../../lib/settings.svelte';
  import Icon from '../Icon.svelte';
  import SaveIndicator from '../SaveIndicator.svelte';
  import Group from '../ui/Group.svelte';
  import Page from '../ui/Page.svelte';
  import Row from '../ui/Row.svelte';
  import Toggle from '../ui/Toggle.svelte';

  const s = $derived(settings.draft);

  const PROMISE = [
    'No account. Krova never asks who you are.',
    'No telemetry, no crash reports, no analytics.',
    'No network calls: nothing leaves your computer.',
    'Window titles are never logged or stored.',
    'Your settings and layouts live in one local file.',
  ];
</script>

<Page title="Privacy & about" sub="Krova works entirely on this computer." testid="view-privacy">
  {#snippet actions()}<SaveIndicator />{/snippet}

  {#if s}
    <Group title="Startup" id="startup">
      <Row
        label="Launch at login"
        desc="Start Krova in the tray or menu bar when you sign in."
        for="set-login"
        id="set-login-l"
      >
        <Toggle
          id="set-login"
          checked={s.launch_at_login}
          describedby="set-login-l-d"
          testid="setting-launch_at_login"
          onchange={(v) => settings.update({ launch_at_login: v })}
        />
      </Row>
    </Group>
  {/if}

  <Group title="The privacy promise" id="promise">
    <ul class="promise" data-testid="privacy-promise">
      {#each PROMISE as line (line)}
        <li><span class="tick" aria-hidden="true"><Icon name="check" /></span>{line}</li>
      {/each}
    </ul>
  </Group>

  <Group title="Layout files" id="layout-files">
    <Row
      label="Import and export layouts"
      desc="Copy a layout as text to share or back it up, or paste one in. Open a layout to find Export and Import."
    >
      <button type="button" class="btn small" data-testid="privacy-open-layouts" onclick={() => app.navigate('layouts')}>
        Open Layouts
      </button>
    </Row>
  </Group>

  <Group title="About" id="about" testid="about">
    <Row label="Version">
      <span class="val" data-testid="about-version">{app.state ? `Krova v${app.state.version}` : 'Krova'}</span>
    </Row>
    <Row label="Licence">
      <span class="val" data-testid="about-licence">MIT, free and open source</span>
    </Row>
    <Row label="Source code">
      <span class="val mono">github.com/nAItiklearn/krova</span>
    </Row>
    <Row label="Website">
      <span class="val mono">naitiklearn.github.io/krova</span>
    </Row>
  </Group>
</Page>

<style>
  .promise {
    list-style: none;
    margin: 0;
    padding: 6px 0;
  }
  .promise li {
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 8px 16px;
  }
  .tick {
    display: grid;
    place-items: center;
    width: 22px;
    height: 22px;
    flex: none;
    border-radius: 999px;
    background: var(--accent-soft);
    color: var(--accent-ink);
  }
  .tick :global(svg) {
    width: 14px;
    height: 14px;
  }
  .val {
    color: var(--text-muted);
    user-select: text;
  }
  .mono {
    font: 0.86rem var(--mono);
  }
</style>
