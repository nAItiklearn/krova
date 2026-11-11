/**
 * Pill tab bars (`[data-tabbar]` with role="tab" buttons and an `[data-ind]` sliding indicator).
 * Imported from the <script> of every component that has one; the module runs once, so each bar
 * gets its listeners exactly once.
 */
document.querySelectorAll<HTMLElement>('[data-tabbar]').forEach((bar) => {
  const tabs = [...bar.querySelectorAll<HTMLButtonElement>('[role="tab"]')];
  const ind = bar.querySelector<HTMLElement>('[data-ind]');
  if (!ind || tabs.length === 0) return;
  const current = () => tabs.find((t) => t.getAttribute('aria-selected') === 'true') ?? tabs[0];
  function move(t: HTMLElement) {
    const b = bar.getBoundingClientRect();
    const r = t.getBoundingClientRect();
    if (!r.width) return;
    ind!.style.width = r.width + 'px';
    ind!.style.transform = `translateX(${r.left - b.left}px)`;
  }
  function select(t: HTMLButtonElement) {
    tabs.forEach((x) => {
      const on = x === t;
      x.setAttribute('aria-selected', String(on));
      x.tabIndex = on ? 0 : -1;
      const p = document.getElementById(x.getAttribute('aria-controls')!);
      if (p) p.hidden = !on;
    });
    move(t);
    t.focus();
    t.scrollIntoView({ block: 'nearest', inline: 'nearest' });
  }
  tabs.forEach((t) => t.addEventListener('click', () => select(t)));
  bar.addEventListener('keydown', (e) => {
    const i = tabs.indexOf(document.activeElement as HTMLButtonElement);
    if (i < 0) return;
    let n = -1;
    if (e.key === 'ArrowRight') n = (i + 1) % tabs.length;
    else if (e.key === 'ArrowLeft') n = (i - 1 + tabs.length) % tabs.length;
    else if (e.key === 'Home') n = 0;
    else if (e.key === 'End') n = tabs.length - 1;
    if (n >= 0) {
      e.preventDefault();
      select(tabs[n]);
    }
  });
  requestAnimationFrame(() => move(current()));
  document.fonts?.ready.then(() => move(current()));
  addEventListener('load', () => move(current()));
  new ResizeObserver(() => move(current())).observe(bar);
});
