<script lang="ts">
  import { app } from '../../lib/app.svelte';
  import type { Layout, MonitorSnapshot } from '../../lib/bindings';
  import { rectStyle } from '../../lib/geometry';
  import Group from '../ui/Group.svelte';
  import Page from '../ui/Page.svelte';
  import Row from '../ui/Row.svelte';
  import Toggle from '../ui/Toggle.svelte';

  const monitors = $derived(app.state?.monitors ?? []);

  function activeLayout(m: MonitorSnapshot): Layout | null {
    const profile = app.state?.config.monitors.find((p) => p.id === m.profile_id);
    if (!profile) return null;
    return profile.layouts.find((l) => l.id === profile.active_layout) ?? profile.layouts[0] ?? null;
  }

  const EDGES = ['top', 'bottom', 'left', 'right'] as const;
</script>

<Page title="Displays" sub="The monitors Krova sees right now, and the layout each one uses." testid="view-displays">
  {#if monitors.length === 0}
    <p class="muted" data-testid="displays-empty">No monitors are connected.</p>
  {/if}

  {#each monitors as m, i (m.profile_id)}
    {@const layout = activeLayout(m)}
    <Group title={m.label} id="mon-{i}" testid="display-card">
      <div class="mon">
        <div class="draw" style:aspect-ratio="{m.rect.w} / {m.rect.h}" aria-hidden="true">
          {#if layout}
            {#each layout.zones as z (z.id)}
              <span class="z" style={rectStyle(z.rect)}></span>
            {/each}
          {/if}
        </div>
        <dl class="facts">
          <div>
            <dt>Resolution</dt>
            <dd data-testid="display-resolution">{m.rect.w} × {m.rect.h}</dd>
          </div>
          <div>
            <dt>Scale</dt>
            <dd data-testid="display-scale">{Math.round(m.scale * 100)}%</dd>
          </div>
          <div>
            <dt>Layout</dt>
            <dd data-testid="display-layout">{layout?.name ?? 'None'}</dd>
          </div>
          {#if m.primary}
            <div>
              <dt>Role</dt>
              <dd><span class="badge" data-testid="display-primary">Primary</span></dd>
            </div>
          {/if}
        </dl>
      </div>
      <Row label="Reserved edges" desc="Keep a strip free for a dock or a widget bar. Coming soon." id="mon-{i}-reserved">
        {#snippet below()}
          <div class="edges" data-testid="display-reserved">
            {#each EDGES as edge (edge)}
              <span class="edge">
                <Toggle
                  id="mon-{i}-res-{edge}"
                  checked={false}
                  disabled
                  label="Reserve the {edge} edge (coming soon)"
                  onchange={() => {}}
                />
                <span aria-hidden="true">{edge[0].toUpperCase() + edge.slice(1)}</span>
              </span>
            {/each}
          </div>
        {/snippet}
      </Row>
    </Group>
  {/each}
</Page>

<style>
  .mon {
    display: flex;
    align-items: center;
    gap: 22px;
    padding: 16px;
    flex-wrap: wrap;
  }
  .draw {
    position: relative;
    width: 220px;
    max-height: 180px;
    flex: none;
    padding: 0;
    border-radius: 8px;
    background: var(--surface-3);
    box-shadow: inset 0 0 0 1.5px var(--border-strong);
    overflow: hidden;
  }
  .z {
    position: absolute;
    box-sizing: border-box;
    border: 2px solid var(--surface-3);
    border-radius: 5px;
    background: var(--surface);
  }
  .facts {
    display: grid;
    grid-template-columns: repeat(2, minmax(110px, auto));
    gap: 10px 26px;
    margin: 0;
  }
  dt {
    font-size: 0.78rem;
    color: var(--text-muted);
  }
  dd {
    margin: 1px 0 0;
    font-weight: 560;
  }
  .badge {
    display: inline-block;
    padding: 2px 9px;
    border-radius: 999px;
    background: var(--accent-soft);
    color: var(--accent-ink);
    font-size: 0.78rem;
    font-weight: 620;
  }
  .edges {
    display: flex;
    flex-wrap: wrap;
    gap: 16px;
  }
  .edge {
    display: inline-flex;
    align-items: center;
    gap: 8px;
    color: var(--text-muted);
    font-size: 0.9rem;
  }
</style>
