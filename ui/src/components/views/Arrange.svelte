<script lang="ts">
  import { app } from '../../lib/app.svelte';
  import { exeNameError, MAX_EXE_LEN } from '../../lib/appRules';
  import type { Modifier, MoveMode } from '../../lib/bindings';
  import { ext, type AppRule, type AppRuleAction, type AutoArrange } from '../../lib/ext.svelte';
  import { MODIFIERS, modifierLabel } from '../../lib/platform';
  import { settings } from '../../lib/settings.svelte';
  import Icon from '../Icon.svelte';
  import SaveIndicator from '../SaveIndicator.svelte';
  import Group from '../ui/Group.svelte';
  import Page from '../ui/Page.svelte';
  import Row from '../ui/Row.svelte';
  import Segmented from '../ui/Segmented.svelte';
  import Toggle from '../ui/Toggle.svelte';

  const s = $derived(settings.draft);
  const caps = $derived(app.state?.capabilities ?? { shortcuts: false, window_memory: false });

  const auto = $derived(ext.get('auto_arrange'));
  const rules = $derived(ext.get('app_rules'));
  const ffm = $derived(ext.get('focus_follows_mouse'));

  const POLICIES: { value: AutoArrange['policy']; label: string; hint: string; testid: string }[] = [
    { value: 'next_free', label: 'Next free zone', hint: 'Fill zones in order', testid: 'arrange-policy-next_free' },
    { value: 'main_first', label: 'Main zone first', hint: 'Put a new window in the largest zone first', testid: 'arrange-policy-main_first' },
  ];

  const MOVE_MODES: { value: MoveMode; label: string; hint: string; testid: string }[] = [
    { value: 'animate', label: 'Animate', hint: 'Glide into the zone (about 120 ms)', testid: 'setting-move_mode-animate' },
    { value: 'instant', label: 'Instant', hint: 'Jump straight into the zone', testid: 'setting-move_mode-instant' },
  ];

  const modifierOptions = $derived(
    MODIFIERS.map((m: Modifier) => ({ value: m, label: modifierLabel(m), testid: `setting-modifier-${m}` })),
  );

  const RULE_ACTIONS: { value: AppRuleAction; label: string }[] = [
    { value: 'never_touch', label: 'Never touch' },
    { value: 'float', label: 'Float' },
    { value: 'preferred_zone', label: 'Zone' },
  ];

  function setRule(i: number, part: Partial<AppRule>): void {
    const next = rules.map((r, j) => {
      if (j !== i) return r;
      const merged: AppRule = { ...r, ...part };
      if (merged.action === 'preferred_zone') merged.zone = merged.zone ?? 1;
      else delete merged.zone;
      return merged;
    });
    ext.set('app_rules', next);
  }

  function removeRule(i: number): void {
    ext.set(
      'app_rules',
      rules.filter((_, j) => j !== i),
    );
  }

  function setZone(i: number, raw: string): void {
    const n = Math.round(Number(raw));
    if (!Number.isFinite(n)) return;
    setRule(i, { zone: Math.min(99, Math.max(1, n)) });
  }

  let newExe = $state('');
  let addError = $state<string | null>(null);

  function addRule(e: SubmitEvent): void {
    e.preventDefault();
    const err = exeNameError(
      newExe,
      rules.map((r) => r.exe),
    );
    addError = err;
    if (err) return;
    ext.set('app_rules', [...rules, { exe: newExe.trim(), action: 'float' }]);
    newExe = '';
  }
</script>

<Page title="Arrange" sub="How windows find their zones, by drag or on their own." testid="view-arrange">
  {#snippet actions()}<SaveIndicator />{/snippet}

  {#if s}
    <Group title="Auto-arrange" id="arrange-auto" testid="arrange-auto">
      <Row label="Arrange new windows" desc="A window that opens goes straight into a zone." for="arr-enabled" id="arr-enabled-l">
        <Toggle
          id="arr-enabled"
          checked={auto.enabled}
          describedby="arr-enabled-l-d"
          testid="arrange-enabled"
          onchange={(v) => ext.patch('auto_arrange', { enabled: v })}
        />
      </Row>
      <Row label="Which zone" desc="Where a new window goes first." id="arr-policy-l">
        <Segmented
          name="arrange-policy"
          label="Which zone"
          options={POLICIES}
          value={auto.policy}
          disabled={!auto.enabled}
          onchange={(v) => ext.patch('auto_arrange', { policy: v })}
        />
      </Row>
      <Row label="Fill gaps when a window closes" desc="The next window moves up into the empty zone." for="arr-gaps" id="arr-gaps-l">
        <Toggle
          id="arr-gaps"
          checked={auto.fill_gaps}
          disabled={!auto.enabled}
          describedby="arr-gaps-l-d"
          testid="arrange-fill_gaps"
          onchange={(v) => ext.patch('auto_arrange', { fill_gaps: v })}
        />
      </Row>
      <Row label="Stack when every zone is full" desc="Extra windows share a zone instead of floating." for="arr-stack" id="arr-stack-l">
        <Toggle
          id="arr-stack"
          checked={auto.stack_when_full}
          disabled={!auto.enabled}
          describedby="arr-stack-l-d"
          testid="arrange-stack_when_full"
          onchange={(v) => ext.patch('auto_arrange', { stack_when_full: v })}
        />
      </Row>
    </Group>

    <Group title="Snapping" id="arrange-snap" testid="arrange-snapping">
      <Row label="Enable Krova" desc="When off, dragging windows never shows or uses zones." for="set-enabled" id="set-enabled-l">
        <Toggle
          id="set-enabled"
          checked={s.enabled}
          describedby="set-enabled-l-d"
          testid="setting-enabled"
          onchange={(v) => settings.update({ enabled: v })}
        />
      </Row>
      <Row
        label="Activation modifier"
        desc="Hold {modifierLabel(s.modifier)} while dragging a window to show the zones. Let go of it to cancel."
        id="set-modifier-l"
      >
        {#snippet below()}
          <Segmented
            name="modifier"
            label="Activation modifier"
            options={modifierOptions}
            value={s.modifier}
            onchange={(v) => settings.update({ modifier: v })}
          />
        {/snippet}
      </Row>
      <Row label="Move behavior" desc="How a window gets into its zone." id="set-move-l">
        <Segmented
          name="move_mode"
          label="Move behavior"
          options={MOVE_MODES}
          value={s.move_mode}
          onchange={(v) => settings.update({ move_mode: v })}
        />
      </Row>
      <Row
        label="Edge snapping"
        desc="Drag a window to a screen edge or corner to snap it to a half or quarter. Never triggers on the border between two monitors."
        for="set-edge"
        id="set-edge-l"
      >
        <Toggle
          id="set-edge"
          checked={s.edge_snapping}
          describedby="set-edge-l-d"
          testid="setting-edge_snapping"
          onchange={(v) => settings.update({ edge_snapping: v })}
        />
      </Row>
      <Row label="Edge size" desc="How close to the edge the pointer has to be." for="set-edge-size" id="set-edge-size-l">
        <div class="slide" class:off={!s.edge_snapping}>
          <input
            id="set-edge-size"
            type="range"
            min="2"
            max="64"
            step="1"
            value={s.edge_size_px}
            disabled={!s.edge_snapping}
            aria-describedby="set-edge-size-l-d"
            aria-valuetext="{s.edge_size_px} pixels"
            data-testid="setting-edge_size_px"
            oninput={(e) => settings.update({ edge_size_px: Math.round(Number(e.currentTarget.value)) })}
          />
          <output for="set-edge-size" class="value">{s.edge_size_px} px</output>
        </div>
      </Row>
    </Group>

    <Group title="Windows" id="windows" testid="settings-windows">
      {#if !caps.window_memory}
        <div class="pad">
          <p class="notice" data-testid="window-memory-unavailable">
            <Icon name="info" />
            <span>Remembering snapped windows is not available on this system yet.</span>
          </p>
        </div>
      {/if}
      <Row
        label="Restore windows on reconnect"
        desc="When a monitor comes back, windows you snapped to it return to their zones. Kept in memory only."
        for="set-restore"
        id="set-restore-l"
      >
        <Toggle
          id="set-restore"
          checked={s.restore_on_reconnect}
          disabled={!caps.window_memory}
          describedby="set-restore-l-d"
          testid="setting-restore_on_reconnect"
          onchange={(v) => settings.update({ restore_on_reconnect: v })}
        />
      </Row>
      <Row
        label="Linked dividers"
        desc="Drag the edge between two snapped windows and both resize."
        for="set-linked"
        id="set-linked-l"
      >
        <Toggle
          id="set-linked"
          checked={s.linked_dividers}
          disabled={!caps.window_memory}
          describedby="set-linked-l-d"
          testid="setting-linked_dividers"
          onchange={(v) => settings.update({ linked_dividers: v })}
        />
      </Row>
      <Row
        label="Space between zones (px)"
        desc="Leaves a gap between neighbouring snapped windows, from 0 to 32 pixels."
        for="set-gap"
        id="set-gap-l"
      >
        <div class="slide">
          <input
            id="set-gap"
            type="range"
            min="0"
            max="32"
            step="1"
            value={s.zone_gap_px}
            aria-describedby="set-gap-l-d"
            aria-valuetext="{s.zone_gap_px} pixels"
            data-testid="setting-zone_gap_px"
            oninput={(e) => settings.update({ zone_gap_px: Math.round(Number(e.currentTarget.value)) })}
          />
          <output for="set-gap" class="value">{s.zone_gap_px} px</output>
        </div>
      </Row>
      <Row label="Focus follows mouse" desc="The window under the pointer gets the keyboard focus." for="set-ffm" id="set-ffm-l">
        <Toggle
          id="set-ffm"
          checked={ffm}
          describedby="set-ffm-l-d"
          testid="setting-focus_follows_mouse"
          onchange={(v) => ext.set('focus_follows_mouse', v)}
        />
      </Row>
    </Group>

    <Group
      title="App rules"
      desc="Programs Krova treats differently. Matched by file name only."
      id="app-rules"
      testid="app-rules"
    >
      {#if rules.length === 0}
        <p class="empty" data-testid="app-rules-empty">No rules yet. Every app is arranged the same way.</p>
      {:else}
        <ul class="rules" aria-label="App rules">
          {#each rules as r, i (r.exe)}
            <li class="rule" data-testid="app-rule">
              <span class="exe" data-testid="app-rule-exe">{r.exe}</span>
              <div class="rule-ctl">
                <Segmented
                  name="rule-{i}"
                  label="What Krova does with {r.exe}"
                  options={RULE_ACTIONS.map((a) => ({ ...a, testid: `app-rule-action-${a.value}` }))}
                  value={r.action}
                  onchange={(v) => setRule(i, { action: v })}
                />
                {#if r.action === 'preferred_zone'}
                  <input
                    type="number"
                    class="zone-num"
                    min="1"
                    max="99"
                    step="1"
                    value={r.zone ?? 1}
                    aria-label="Zone number for {r.exe}"
                    data-testid="app-rule-zone"
                    onchange={(e) => setZone(i, e.currentTarget.value)}
                  />
                {/if}
                <button
                  type="button"
                  class="btn small danger"
                  aria-label="Remove the rule for {r.exe}"
                  data-testid="app-rule-remove"
                  onclick={() => removeRule(i)}
                >
                  <Icon name="trash" />
                </button>
              </div>
            </li>
          {/each}
        </ul>
      {/if}
      <form class="add" onsubmit={addRule}>
        <label for="rule-exe" class="sr-only">Program file name</label>
        <input
          id="rule-exe"
          type="text"
          placeholder="Program, e.g. Notepad.exe"
          maxlength={MAX_EXE_LEN + 8}
          spellcheck="false"
          autocomplete="off"
          bind:value={newExe}
          aria-invalid={addError !== null}
          aria-describedby={addError ? 'rule-exe-err' : undefined}
          data-testid="app-rule-input"
          oninput={() => (addError = null)}
        />
        <button type="submit" class="btn small" data-testid="app-rule-add"><Icon name="plus" /> Add rule</button>
        {#if addError}
          <p id="rule-exe-err" class="err" role="alert" data-testid="app-rule-error">{addError}</p>
        {/if}
      </form>
    </Group>
  {:else}
    <p class="muted">Loading settings...</p>
  {/if}
</Page>

<style>
  .slide {
    display: flex;
    align-items: center;
    gap: 10px;
    width: min(260px, 40vw);
  }
  .slide.off {
    opacity: 0.55;
  }
  input[type='range'] {
    flex: 1;
    min-width: 0;
    height: 24px;
    margin: 0;
  }
  .value {
    min-width: 48px;
    text-align: right;
    font: 0.88rem var(--mono);
    color: var(--text-muted);
  }
  .pad {
    padding: 12px 16px;
  }
  .empty {
    padding: 14px 16px;
    color: var(--text-muted);
  }
  .rules {
    list-style: none;
    margin: 0;
    padding: 0;
  }
  .rule {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 12px;
    flex-wrap: wrap;
    padding: 10px 16px;
  }
  .rule + .rule {
    border-top: 1px solid var(--border);
  }
  .exe {
    font: 550 0.9rem var(--mono);
    overflow-wrap: anywhere;
    min-width: 0;
  }
  .rule-ctl {
    display: flex;
    align-items: center;
    gap: 8px;
  }
  .zone-num {
    width: 58px;
    min-height: 30px;
    padding: 0 8px;
    border: 0;
    border-radius: 8px;
    background: var(--surface-2);
    box-shadow: inset 0 0 0 1px var(--border);
    color: var(--text);
    font: inherit;
  }
  .add {
    display: flex;
    align-items: center;
    flex-wrap: wrap;
    gap: 8px;
    padding: 12px 16px;
  }
  .add input {
    flex: 1;
    min-width: 200px;
  }
  .err {
    flex-basis: 100%;
    color: var(--danger);
    font-size: 0.85rem;
  }
</style>
