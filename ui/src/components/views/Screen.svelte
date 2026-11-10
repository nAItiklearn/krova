<script lang="ts">
  import { onDestroy } from 'svelte';
  import { app } from '../../lib/app.svelte';
  import { api, errorMessage } from '../../lib/backend';
  import type { Language, MonitorSnapshot, ReservedEdges } from '../../lib/bindings';
  import { SIDES, cleanSide, noneReserved, reservedBands, type Side } from '../../lib/edges';
  import { LANGUAGES, MAX_RESERVED_PX, NO_RESERVE } from '../../lib/mock/wave4';
  import { settings } from '../../lib/settings.svelte';
  import Group from '../ui/Group.svelte';
  import Page from '../ui/Page.svelte';
  import Row from '../ui/Row.svelte';

  const SAVE_DELAY_MS = 400;
  const LABEL: Record<Side, string> = { top: 'Top', right: 'Right', bottom: 'Bottom', left: 'Left' };

  const monitors = $derived(app.state?.monitors ?? []);
  const s = $derived(settings.draft);
  const languages = $derived(app.state?.languages ?? LANGUAGES);
  const inUse = $derived(languages.find((l) => l.id === app.state?.language)?.name ?? null);

  // What the user typed, per monitor, until the backend has it.
  let drafts = $state<Record<string, ReservedEdges>>({});
  let errors = $state<Record<string, string>>({});
  const timers = new Map<string, ReturnType<typeof setTimeout>>();
  onDestroy(() => {
    for (const [id, t] of timers) {
      clearTimeout(t);
      void save(id);
    }
  });

  function stored(id: string): ReservedEdges {
    return app.state?.config.monitors.find((p) => p.id === id)?.reserved ?? NO_RESERVE;
  }
  const edges = (id: string): ReservedEdges => drafts[id] ?? stored(id);

  function change(m: MonitorSnapshot, side: Side, raw: string): void {
    const next = { ...edges(m.profile_id), [side]: cleanSide(raw) };
    drafts = { ...drafts, [m.profile_id]: next };
    clearTimeout(timers.get(m.profile_id));
    timers.set(m.profile_id, setTimeout(() => void save(m.profile_id), SAVE_DELAY_MS));
  }

  async function save(id: string): Promise<void> {
    timers.delete(id);
    const sent = drafts[id];
    if (!sent) return;
    try {
      const st = await api.setReservedEdges(id, sent);
      app.setState(st);
      if (drafts[id] === sent) {
        const { [id]: _, ...rest } = drafts;
        drafts = rest;
      }
      errors = { ...errors, [id]: '' };
    } catch (e) {
      errors = { ...errors, [id]: errorMessage(e) };
    }
  }

  function clear(m: MonitorSnapshot): void {
    drafts = { ...drafts, [m.profile_id]: { ...NO_RESERVE } };
    clearTimeout(timers.get(m.profile_id));
    void save(m.profile_id);
  }

  function activeZones(id: string) {
    const p = app.state?.config.monitors.find((x) => x.id === id);
    return p?.layouts.find((l) => l.id === p.active_layout)?.zones ?? [];
  }
</script>

<Page title="Screen edges" sub="Keep strips of a screen free for a dock or a bar, and pick Krova's language." testid="view-screen">
  {#each monitors as m, i (m.profile_id)}
    {@const r = edges(m.profile_id)}
    {@const b = reservedBands(m.rect, m.scale, r)}
    <Group title={m.label} id="scr-{i}" testid="screen-monitor" desc="In pixels at 100% scale, up to {MAX_RESERVED_PX} per side.">
      {#snippet aside()}
        <button type="button" class="btn small" disabled={noneReserved(r)} data-testid="screen-clear" onclick={() => clear(m)}>Clear</button>
      {/snippet}
      <div class="mon">
        <div class="draw" style:aspect-ratio="{m.rect.w} / {m.rect.h}" aria-hidden="true" data-testid="screen-drawing">
          <span class="band top" style:height="{b.top * 100}%" data-testid="screen-band-top"></span>
          <span class="band bottom" style:height="{b.bottom * 100}%" data-testid="screen-band-bottom"></span>
          <span class="band left" style:width="{b.left * 100}%" style:top="{b.top * 100}%" style:bottom="{b.bottom * 100}%" data-testid="screen-band-left"></span>
          <span class="band right" style:width="{b.right * 100}%" style:top="{b.top * 100}%" style:bottom="{b.bottom * 100}%" data-testid="screen-band-right"></span>
          <div
            class="work"
            style:left="{b.left * 100}%"
            style:right="{b.right * 100}%"
            style:top="{b.top * 100}%"
            style:bottom="{b.bottom * 100}%"
          >
            {#each activeZones(m.profile_id) as z (z.id)}
              <span
                class="z"
                style:left="{z.rect.x * 100}%"
                style:top="{z.rect.y * 100}%"
                style:width="{z.rect.w * 100}%"
                style:height="{z.rect.h * 100}%"
              ></span>
            {/each}
          </div>
        </div>
        <fieldset class="sides">
          <legend class="sr-only">Reserved edges of {m.label}</legend>
          {#each SIDES as side (side)}
            <label class="side {side}">
              <span>{LABEL[side]}</span>
              <input
                type="number"
                inputmode="numeric"
                min="0"
                max={MAX_RESERVED_PX}
                step="1"
                value={r[side]}
                data-testid="screen-{side}"
                oninput={(e) => change(m, side, e.currentTarget.value)}
              />
            </label>
          {/each}
        </fieldset>
      </div>
      {#if b.ignoredX || b.ignoredY}
        <p class="note warn" role="status" data-testid="screen-ignored">
          Too little would be left {b.ignoredX && b.ignoredY ? 'either way' : b.ignoredX ? 'across' : 'top to bottom'}, so Krova ignores those edges.
        </p>
      {/if}
      {#if errors[m.profile_id]}<p class="note err" role="alert">{errors[m.profile_id]}</p>{/if}
    </Group>
  {/each}

  {#if s}
    <Group title="Language" id="lang" testid="screen-language">
      <Row
        label="Language"
        desc="Translations are in progress: for now the tray menu and notifications follow it."
        for="scr-lang"
        id="scr-lang-l"
      >
        <select
          id="scr-lang"
          value={s.language}
          aria-describedby="scr-lang-l-d"
          data-testid="screen-language-select"
          onchange={(e) => settings.update({ language: e.currentTarget.value as Language })}
        >
          {#each languages as l (l.id)}
            <option value={l.id} lang={l.id === 'system' ? undefined : l.id}>{l.name}</option>
          {/each}
        </select>
      </Row>
      {#if s.language === 'system' && inUse}
        <p class="note" data-testid="screen-language-in-use">Following Windows: {inUse}.</p>
      {/if}
    </Group>
  {/if}
</Page>

<style>
  .mon {
    display: flex;
    align-items: center;
    gap: 28px;
    padding: 18px 16px;
    flex-wrap: wrap;
  }
  .draw {
    position: relative;
    width: 260px;
    flex: none;
    border-radius: 9px;
    background: var(--surface-3);
    box-shadow: inset 0 0 0 1.5px var(--border-strong);
    overflow: hidden;
  }
  .band {
    position: absolute;
    background: repeating-linear-gradient(135deg, var(--accent-soft) 0 5px, color-mix(in srgb, var(--accent) 30%, transparent) 5px 7px);
    transition:
      height 0.45s var(--spring),
      width 0.45s var(--spring),
      top 0.45s var(--spring),
      bottom 0.45s var(--spring);
  }
  .band.top {
    left: 0;
    right: 0;
    top: 0;
  }
  .band.bottom {
    left: 0;
    right: 0;
    bottom: 0;
  }
  .band.left {
    left: 0;
  }
  .band.right {
    right: 0;
  }
  .work {
    position: absolute;
    transition:
      left 0.45s var(--spring),
      right 0.45s var(--spring),
      top 0.45s var(--spring),
      bottom 0.45s var(--spring);
  }
  .z {
    position: absolute;
    box-sizing: border-box;
    border: 2px solid var(--surface-3);
    border-radius: 5px;
    background: var(--surface);
  }
  .sides {
    display: grid;
    grid-template-columns: repeat(3, auto);
    grid-template-areas:
      '. top .'
      'left . right'
      '. bottom .';
    gap: 8px 10px;
    margin: 0;
    padding: 0;
    border: 0;
  }
  .side {
    display: grid;
    gap: 3px;
    justify-items: center;
    font-size: 0.8rem;
    color: var(--text-muted);
  }
  .side.top {
    grid-area: top;
  }
  .side.right {
    grid-area: right;
  }
  .side.bottom {
    grid-area: bottom;
  }
  .side.left {
    grid-area: left;
  }
  input[type='number'] {
    width: 74px;
    min-height: 32px;
    padding: 0 8px;
    border: 0;
    border-radius: 8px;
    background: var(--surface-2);
    box-shadow: inset 0 0 0 1px var(--border);
    color: var(--text);
    font: 0.9rem var(--mono);
    text-align: center;
  }
  input[type='number']:hover {
    box-shadow: inset 0 0 0 1px var(--border-strong);
  }
  input[type='number']:focus-visible {
    outline: none;
    box-shadow:
      inset 0 0 0 1.5px var(--accent),
      0 0 0 4px var(--accent-soft);
  }
  .note {
    padding: 10px 16px;
    font-size: 0.85rem;
    color: var(--text-muted);
  }
  .note.warn {
    color: var(--warn-text);
    background: var(--warn-bg);
  }
  .note.err {
    color: var(--danger);
  }
  select {
    min-width: 160px;
  }
</style>
