/*
  GitHub star count for the "Star" link in the header. The only request this site
  makes to another server: one GET to the public GitHub REST API
  (https://api.github.com/repos/nAItiklearn/krova), no credentials, no cookies.

  - The count is cached in localStorage ("krova-stars") for 1 hour, so a visitor
    browsing several pages makes at most one request per hour. GitHub allows 60
    unauthenticated requests per hour per IP address.
  - A failed or rate-limited request is remembered for 10 minutes and the link
    simply stays "Star" without a number. Nothing is shown as an error.
  - Loaded with `defer`, so it never blocks the first paint.
*/
(function () {
  'use strict';
  var KEY = 'krova-stars';
  var API = 'https://api.github.com/repos/nAItiklearn/krova';
  var TTL_OK = 60 * 60 * 1000;
  var TTL_FAIL = 10 * 60 * 1000;

  var links = document.querySelectorAll('[data-stars]');
  if (!links.length) return;

  function read() {
    try {
      var v = JSON.parse(localStorage.getItem(KEY) || 'null');
      if (v && typeof v.t === 'number') return v;
    } catch (e) {
      /* storage blocked or corrupt: fetch again */
    }
    return null;
  }

  function write(n) {
    try {
      localStorage.setItem(KEY, JSON.stringify({ n: n, t: Date.now() }));
    } catch (e) {
      /* storage blocked: the count lasts for this page only */
    }
  }

  function short(n) {
    if (n < 1000) return String(n);
    if (n < 10000) return (Math.floor(n / 100) / 10).toString().replace(/\.0$/, '') + 'k';
    return Math.floor(n / 1000) + 'k';
  }

  function show(n) {
    if (typeof n !== 'number' || !isFinite(n) || n < 0) return;
    var label = 'Star Krova on GitHub, ' + n + (n === 1 ? ' star' : ' stars') + ' (opens in a new tab)';
    for (var i = 0; i < links.length; i++) {
      var c = links[i].querySelector('[data-stars-count]');
      if (c) {
        c.textContent = short(n);
        c.hidden = false;
      }
      links[i].setAttribute('aria-label', label);
    }
  }

  var cached = read();
  var age = cached ? Date.now() - cached.t : Infinity;
  if (cached && typeof cached.n === 'number') show(cached.n);
  var fresh = cached && (typeof cached.n === 'number' ? age < TTL_OK : age < TTL_FAIL);
  if (fresh || !window.fetch) return;

  fetch(API, { headers: { Accept: 'application/vnd.github+json' }, credentials: 'omit', referrerPolicy: 'no-referrer' })
    .then(function (r) {
      if (!r.ok) throw new Error('status ' + r.status);
      return r.json();
    })
    .then(function (data) {
      var n = data && data.stargazers_count;
      if (typeof n !== 'number') throw new Error('no count');
      write(n);
      show(n);
    })
    .catch(function () {
      // Keep a stale count if there is one; otherwise the link stays "Star".
      if (!(cached && typeof cached.n === 'number')) write(null);
    });
})();
