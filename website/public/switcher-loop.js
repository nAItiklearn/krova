/*
  Looping illustration of the layout switcher PROTOTYPE on the Roadmap page. Same-origin, no network, no storage.

  It is a small copy of the motion in docs/prototypes/layout-switcher/index.html: the same damped spring
  (fixed 240 Hz sub-steps), the same deck geometry (the focused card in front, the next ones stacked behind it
  with only their title strip showing, each a little smaller and darker; cards already flipped past slide down
  and fade out). The layout switcher is not in the app yet.

  - prefers-reduced-motion: nothing moves. The cards stay where the stylesheet puts them (the front card
    with the others stacked behind it).
  - It only runs while it is on screen and the tab is visible, and the "Pause animation" button stops it.
  - Without JavaScript the same stacked picture is drawn by CSS alone.
*/
(function () {
  'use strict';

  var root = document.querySelector('[data-switcher]');
  if (!root) return;

  var reduced = window.matchMedia && window.matchMedia('(prefers-reduced-motion: reduce)');
  var button = root.querySelector('[data-switcher-toggle]');
  if (reduced && reduced.matches) {
    if (button) button.hidden = true;
    return;
  }

  var stage = root.querySelector('[data-switcher-stage]');
  var cardEls = Array.prototype.slice.call(stage.querySelectorAll('[data-card]'));
  if (!cardEls.length) return;

  /* Spring: a damped harmonic oscillator, fixed 240 Hz sub-steps so the feel does not depend on frame rate. */
  function Spring(value, k, zeta) {
    this.value = value;
    this.target = value;
    this.vel = 0;
    this.k = k;
    this.c = 2 * Math.sqrt(k) * zeta;
  }
  Spring.prototype.set = function (target) {
    this.target = target;
  };
  Spring.prototype.step = function (dt) {
    var h = 1 / 240;
    for (var t = 0; t < dt; t += h) {
      var a = -this.k * (this.value - this.target) - this.c * this.vel;
      this.vel += a * h;
      this.value += this.vel * h;
    }
  };
  Spring.prototype.settled = function () {
    return Math.abs(this.value - this.target) < 1e-3 && Math.abs(this.vel) < 1e-2;
  };

  var scroll = new Spring(0, 300, 0.9); // same constants as the prototype's deck scroll
  var count = cardEls.length;

  var cardH = 0;
  var scale = 1;
  var focusY = 0;
  var STEP_BEHIND = 50; // prototype pixels at a 380 px wide card

  function measure() {
    var w = cardEls[0].offsetWidth;
    cardH = cardEls[0].offsetHeight;
    scale = w / 380;
    // The front card sits near the bottom of the stage, the stack behind it rises above.
    var behind = Math.min(count - 1, 3) * STEP_BEHIND * scale;
    focusY = Math.max(behind + 30, stage.clientHeight - cardH - 22);
  }

  // d = scroll - slot. d = 0 is the card in front; d < 0 are stacked behind it; d > 0 have been flipped past.
  function place(d) {
    var step = STEP_BEHIND * scale;
    if (d >= 0) {
      return { y: d * cardH * 0.9, s: 1 - Math.min(d, 1) * 0.04, o: Math.max(0, 1 - d * 1.1), shade: 0 };
    }
    var t = -d;
    return {
      y: -t * step,
      s: Math.max(0.82, 1 - t * 0.045),
      o: Math.max(0, 1 - Math.max(0, t - 3.4) * 0.8),
      shade: Math.min(0.5, t * 0.14),
    };
  }

  function render() {
    for (var i = 0; i < count; i++) {
      var p = place(scroll.value - i);
      var el = cardEls[i];
      el.style.transform = 'translate3d(0,' + (focusY + p.y).toFixed(2) + 'px,0) scale(' + p.s.toFixed(4) + ')';
      el.style.opacity = p.o.toFixed(3);
      el.style.zIndex = String(100 + (count - i) * 10);
      var shade = el.querySelector('[data-shade]');
      if (shade) shade.style.opacity = p.shade.toFixed(3);
    }
  }

  /* The animation frame loop runs only while the spring is moving. */
  var raf = 0;
  var last = 0;
  function frame(now) {
    var dt = Math.min(0.05, (now - last) / 1000);
    last = now;
    scroll.step(dt);
    render();
    raf = scroll.settled() ? 0 : requestAnimationFrame(frame);
  }
  function kick() {
    if (!raf) {
      last = performance.now();
      raf = requestAnimationFrame(frame);
    }
  }

  /* The loop: flip through the deck one card at a time, rest, then go back to the first card. */
  var index = 0;
  var timer = 0;
  var onScreen = false;
  var paused = false;

  function running() {
    return onScreen && !paused && !document.hidden;
  }
  function tick() {
    timer = 0;
    if (!running()) return;
    index = index + 1 >= count ? 0 : index + 1;
    scroll.set(index);
    kick();
    timer = window.setTimeout(tick, index === 0 ? 1800 : 1500);
  }
  function sync() {
    if (running()) {
      if (!timer) timer = window.setTimeout(tick, 900);
    } else if (timer) {
      window.clearTimeout(timer);
      timer = 0;
    }
  }

  measure();
  render();
  root.setAttribute('data-ready', '');

  if (window.ResizeObserver) {
    new ResizeObserver(function () {
      measure();
      render();
    }).observe(stage);
  }
  if (window.IntersectionObserver) {
    new IntersectionObserver(function (entries) {
      onScreen = entries[entries.length - 1].isIntersecting;
      sync();
    }).observe(root);
  } else {
    onScreen = true;
    sync();
  }
  document.addEventListener('visibilitychange', sync);

  if (button) {
    button.hidden = false;
    button.addEventListener('click', function () {
      paused = !paused;
      button.setAttribute('aria-pressed', paused ? 'true' : 'false');
      button.querySelector('[data-label]').textContent = paused ? 'Play animation' : 'Pause animation';
      sync();
    });
  }
  if (reduced && reduced.addEventListener) {
    reduced.addEventListener('change', function () {
      if (reduced.matches) {
        paused = true;
        scroll.value = scroll.target = 0;
        index = 0;
        render();
        if (button) button.hidden = true;
        sync();
      }
    });
  }
})();
