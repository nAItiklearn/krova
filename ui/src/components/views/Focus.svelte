<script lang="ts">
  import { app } from '../../lib/app.svelte';
  import { api, errorMessage } from '../../lib/backend';
  import { ext } from '../../lib/ext.svelte';
  import { MAX_FOCUS_DELAY_MS, MIN_FOCUS_DELAY_MS } from '../../lib/mock/wave4';
  import { settings } from '../../lib/settings.svelte';
  import Group from '../ui/Group.svelte';
  import Page from '../ui/Page.svelte';
  import Row from '../ui/Row.svelte';
  import Segmented from '../ui/Segmented.svelte';
  import Toggle from '../ui/Toggle.svelte';

  const s = $derived(settings.draft);
  const w3 = $derived(app.state?.wave3);
  const caps3 = $derived(w3?.capabilities ?? { focus_dim: true, calm_desk: true, workspaces: true, highlight: true, dwm_borders: false });
  const caps4 = $derived(app.state?.wave4_capabilities ?? { focus_follows_mouse: true, hover_picker: true, open_releases_page: true });
  const calm = $derived(w3?.calm_desk ?? { active: false, count: 0 });
  const shortcuts = $derived(ext.get('shortcuts'));
  const chip = (id: string, def: string): string | null => (shortcuts[id] === null ? null : (shortcuts[id] ?? def));

  let calmBusy = $state(false);
  let calmError = $state('');
  async function toggleCalm(): Promise<void> {
    calmBusy = true;
    calmError = '';
    try {
      app.setState(await api.toggleCalmDesk());
    } catch (e) {
      calmError = errorMessage(e);
    } finally {
      calmBusy = false;
    }
  }

  const pct = $derived(s ? Math.round(s.focus.opacity * 100) : 45);
  const refused = $derived(app.state?.pointer?.focus_refused ?? 0);
</script>

<Page title="Focus" sub="Quiet the screen around the window you are working in." testid="view-focus">
  {#if s}
    {#if w3?.visuals_error}
      <p class="notice warn" role="alert" data-testid="focus-visuals-error">Krova could not draw its overlays: {w3.visuals_error}</p>
    {/if}

    {#if caps3.focus_dim}
      <Group title="Focus dim" id="focus-dim" testid="focus-dim">
        <Row label="Dim everything else" desc="The rest of the screen fades so the focused window stands out." for="fo-dim" id="fo-dim-l">
          {#if chip('focus', 'Ctrl+Alt+F')}<kbd class="chip">{chip('focus', 'Ctrl+Alt+F')}</kbd>{/if}
          <Toggle
            id="fo-dim"
            checked={s.focus.enabled}
            describedby="fo-dim-l-d"
            testid="focus-dim-enabled"
            onchange={(v) => settings.update({ focus: { ...s.focus, enabled: v } })}
          />
        </Row>
        <Row label="Strength" desc="How dark the rest of the screen gets." for="fo-strength" id="fo-strength-l">
          <div class="slide" class:off={!s.focus.enabled}>
            <input
              id="fo-strength"
              type="range"
              min="20"
              max="80"
              step="5"
              value={pct}
              aria-describedby="fo-strength-l-d"
              aria-valuetext="{pct} percent"
              data-testid="focus-dim-strength"
              oninput={(e) => settings.update({ focus: { ...s.focus, opacity: Number(e.currentTarget.value) / 100 } })}
            />
            <output for="fo-strength" class="value" data-testid="focus-dim-value">{pct}%</output>
          </div>
          {#snippet below()}
            <div class="desk" aria-hidden="true" class:on={s.focus.enabled}>
              <span class="win a"></span>
              <span class="win b"></span>
              <span class="win c"></span>
              <span class="dim" style:opacity={s.focus.enabled ? s.focus.opacity : 0}></span>
              <span class="win b lit" class:on={s.focus.enabled}></span>
            </div>
          {/snippet}
        </Row>
      </Group>
    {/if}

    {#if caps3.calm_desk}
      <Group title="Calm desk" id="calm" testid="focus-calm">
        <Row
          label="Tuck away stray windows"
          desc="Minimises every window that is not in a zone. Turn it off to bring them back."
          for="fo-calm"
          id="fo-calm-l"
        >
          {#if chip('calm_desk', 'Ctrl+Alt+D')}<kbd class="chip">{chip('calm_desk', 'Ctrl+Alt+D')}</kbd>{/if}
          <Toggle id="fo-calm" checked={calm.active} disabled={calmBusy} describedby="fo-calm-l-d" testid="focus-calm-toggle" onchange={() => void toggleCalm()} />
        </Row>
        <Row label="Which monitors" id="fo-calm-mon-l">
          <Segmented
            name="fo-calm-mon"
            label="Which monitors calm desk tidies"
            value={s.calm_desk.all_monitors ? 'all' : 'cursor'}
            options={[
              { value: 'all', label: 'Every monitor', testid: 'focus-calm-all' },
              { value: 'cursor', label: 'Under the cursor', testid: 'focus-calm-cursor' },
            ]}
            onchange={(v) => settings.update({ calm_desk: { all_monitors: v === 'all' } })}
          />
        </Row>
        {#if calm.active || calmError}
          <p class="status" role="status" data-testid="focus-calm-status">
            {calmError ? calmError : `${calm.count} ${calm.count === 1 ? 'window is' : 'windows are'} tucked away.`}
          </p>
        {/if}
      </Group>
    {/if}

    <Group title="Privacy" id="share" testid="focus-share">
      <Row
        label="Share-safe"
        desc="While you share your screen, Krova's overlays show only zone shapes, never app names or icons."
        for="fo-share"
        id="fo-share-l"
      >
        <Toggle id="fo-share" checked={s.share_safe} describedby="fo-share-l-d" testid="focus-share-safe" onchange={(v) => settings.update({ share_safe: v })} />
      </Row>
    </Group>

    {#if caps4.focus_follows_mouse || caps4.hover_picker}
      <Group title="Pointer" id="pointer" testid="focus-pointer">
        {#if caps4.focus_follows_mouse}
          <Row label="Focus follows mouse" desc="The window under the cursor gets the focus once the cursor rests." for="fo-ffm" id="fo-ffm-l">
            <Toggle
              id="fo-ffm"
              checked={s.focus_follows_mouse}
              describedby="fo-ffm-l-d"
              testid="focus-ffm"
              onchange={(v) => settings.update({ focus_follows_mouse: v })}
            />
          </Row>
          <Row label="Rest time" desc="How long the cursor has to rest first." for="fo-ffm-delay" id="fo-ffm-delay-l">
            <div class="slide" class:off={!s.focus_follows_mouse}>
              <input
                id="fo-ffm-delay"
                type="range"
                min={MIN_FOCUS_DELAY_MS}
                max={MAX_FOCUS_DELAY_MS}
                step="50"
                value={s.focus_follows_mouse_delay_ms}
                disabled={!s.focus_follows_mouse}
                aria-describedby="fo-ffm-delay-l-d"
                aria-valuetext="{s.focus_follows_mouse_delay_ms} milliseconds"
                data-testid="focus-ffm-delay"
                oninput={(e) => settings.update({ focus_follows_mouse_delay_ms: Math.round(Number(e.currentTarget.value)) })}
              />
              <output for="fo-ffm-delay" class="value" data-testid="focus-ffm-delay-value">{s.focus_follows_mouse_delay_ms} ms</output>
            </div>
          </Row>
          {#if refused > 0}
            <p class="status" data-testid="focus-ffm-refused">Windows blocked some focus changes.</p>
          {/if}
        {/if}
        {#if caps4.hover_picker}
          <Row
            label="Hover zone picker"
            desc="Hold the snap key over a title bar to see the zones and pick one."
            for="fo-hover"
            id="fo-hover-l"
          >
            <Toggle id="fo-hover" checked={s.hover_picker} describedby="fo-hover-l-d" testid="focus-hover-picker" onchange={(v) => settings.update({ hover_picker: v })} />
          </Row>
        {/if}
      </Group>
    {/if}
  {/if}
</Page>

<style>
  .chip {
    width: auto;
  }
  .slide {
    display: flex;
    align-items: center;
    gap: 10px;
    width: min(260px, 40vw);
  }
  .slide.off input {
    opacity: 0.55;
  }
  input[type='range'] {
    flex: 1;
    min-width: 0;
    height: 24px;
    margin: 0;
  }
  .value {
    min-width: 54px;
    text-align: right;
    font: 0.88rem var(--mono);
    color: var(--text-muted);
  }
  .status {
    padding: 10px 16px;
    font-size: 0.85rem;
    color: var(--text-muted);
  }
  .desk {
    position: relative;
    height: 120px;
    border-radius: 10px;
    background: var(--wall);
    overflow: hidden;
    box-shadow: inset 0 0 0 1px var(--border);
  }
  .win {
    position: absolute;
    border-radius: 6px;
    background: var(--surface);
    box-shadow: var(--shadow);
  }
  .win.a {
    left: 4%;
    top: 12%;
    width: 30%;
    height: 76%;
  }
  .win.b {
    left: 36%;
    top: 12%;
    width: 34%;
    height: 76%;
  }
  .win.c {
    left: 72%;
    top: 12%;
    width: 24%;
    height: 76%;
  }
  .dim {
    position: absolute;
    inset: 0;
    background: #000;
    transition: opacity 0.45s var(--spring);
  }
  .win.lit {
    opacity: 0;
    transition:
      opacity 0.3s ease,
      transform 0.45s var(--pop);
  }
  .win.lit.on {
    opacity: 1;
    transform: scale(1.02);
    box-shadow:
      0 0 0 2px var(--accent),
      var(--shadow-lg);
  }
</style>
