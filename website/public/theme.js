/*
  Krova site script. The only JavaScript on the site. Same-origin, no network,
  no cookies.

  1. Light / dark choice. Runs in <head>, before first paint, so there is no
     flash. The choice ("light" or "dark") is kept in localStorage under
     "krova-theme"; no stored value means "follow the system".
  2. Wires the theme buttons (header toggle, footer System / Light / Dark).
  3. Closes the Guides dropdown and the mobile menu on Escape, on a click
     outside them, and after a link inside them is used.
  4. Passes an explicit theme on to the same-origin live-demo frame.

  Without JavaScript the site still works and follows the system theme; the
  theme buttons stay hidden because they would do nothing.
*/
(function () {
  'use strict';
  var KEY = 'krova-theme';
  var root = document.documentElement;
  var media = window.matchMedia ? window.matchMedia('(prefers-color-scheme: dark)') : null;

  function stored() {
    try {
      var v = localStorage.getItem(KEY);
      return v === 'light' || v === 'dark' ? v : 'system';
    } catch (e) {
      return 'system';
    }
  }

  function setAttr(mode) {
    if (mode === 'light' || mode === 'dark') root.setAttribute('data-theme', mode);
    else root.removeAttribute('data-theme');
  }

  function effective() {
    var a = root.getAttribute('data-theme');
    if (a === 'light' || a === 'dark') return a;
    return media && media.matches ? 'dark' : 'light';
  }

  // Apply the saved choice immediately (this file is loaded synchronously in <head>).
  setAttr(stored());

  function save(mode) {
    try {
      if (mode === 'system') localStorage.removeItem(KEY);
      else localStorage.setItem(KEY, mode);
    } catch (e) {
      /* storage blocked: the choice lasts for this page only */
    }
  }

  function syncFrame() {
    var frames = document.querySelectorAll('iframe[data-demo]');
    for (var i = 0; i < frames.length; i++) {
      try {
        var el = frames[i].contentDocument && frames[i].contentDocument.documentElement;
        if (!el) continue;
        var mode = stored();
        if (mode === 'light' || mode === 'dark') {
          el.setAttribute('data-theme', mode);
          el.setAttribute('data-site-theme', '1');
        } else if (el.getAttribute('data-site-theme')) {
          el.removeAttribute('data-theme');
          el.removeAttribute('data-site-theme');
        }
      } catch (e) {
        /* frame not ready or not same-origin: leave it alone */
      }
    }
  }

  function refresh() {
    var mode = stored();
    var eff = effective();
    var toggles = document.querySelectorAll('[data-theme-toggle]');
    for (var i = 0; i < toggles.length; i++) {
      toggles[i].setAttribute('aria-label', eff === 'dark' ? 'Switch to light theme' : 'Switch to dark theme');
      toggles[i].hidden = false;
    }
    var groups = document.querySelectorAll('[data-theme-set-group]');
    for (var g = 0; g < groups.length; g++) groups[g].hidden = false;
    var sets = document.querySelectorAll('[data-theme-set]');
    for (var j = 0; j < sets.length; j++) {
      sets[j].setAttribute('aria-pressed', sets[j].getAttribute('data-theme-set') === mode ? 'true' : 'false');
    }
    syncFrame();
  }

  function choose(mode) {
    save(mode);
    setAttr(mode);
    refresh();
  }

  function closeAll(except) {
    var open = document.querySelectorAll('details[data-menu][open]');
    for (var i = 0; i < open.length; i++) if (open[i] !== except) open[i].removeAttribute('open');
  }

  function init() {
    refresh();

    document.addEventListener('click', function (e) {
      var t = e.target;
      if (!(t instanceof Element)) return;

      var toggle = t.closest('[data-theme-toggle]');
      if (toggle) {
        choose(effective() === 'dark' ? 'light' : 'dark');
        return;
      }
      var set = t.closest('[data-theme-set]');
      if (set) {
        choose(set.getAttribute('data-theme-set'));
        return;
      }

      var menu = t.closest('details[data-menu]');
      closeAll(menu);
      // A link used inside a menu closes it (in-page links do not reload the page).
      if (menu && t.closest('a')) menu.removeAttribute('open');
    });

    document.addEventListener('keydown', function (e) {
      if (e.key !== 'Escape') return;
      var open = document.querySelector('details[data-menu][open]');
      if (!open) return;
      open.removeAttribute('open');
      var summary = open.querySelector('summary');
      if (summary) summary.focus();
    });

    // Close a dropdown when keyboard focus leaves it.
    document.addEventListener('focusin', function (e) {
      var t = e.target;
      if (!(t instanceof Element)) return;
      closeAll(t.closest('details[data-menu]'));
    });

    var frames = document.querySelectorAll('iframe[data-demo]');
    for (var i = 0; i < frames.length; i++) frames[i].addEventListener('load', syncFrame);

    if (media && media.addEventListener) media.addEventListener('change', refresh);
    window.addEventListener('storage', function (e) {
      if (e.key === KEY || e.key === null) {
        setAttr(stored());
        refresh();
      }
    });
  }

  if (document.readyState === 'loading') document.addEventListener('DOMContentLoaded', init);
  else init();
})();
