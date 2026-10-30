// Checks the built site in dist/ (run `npm run build` first):
//   - every internal link and asset reference resolves to a file in dist/
//   - every #fragment points at an id that exists in the target page
//   - no page references a subresource (script, stylesheet, image, frame, ...) on another origin
//   - no link to the repository / GitHub shows up while REPO_PUBLIC is false
//   - external hyperlinks are listed, not fetched (this script never touches the network)
//
// Usage: node scripts/check-links.mjs [dist-dir]
import { existsSync, readdirSync, readFileSync, statSync } from 'node:fs';
import { dirname, join, relative, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';

const here = dirname(fileURLToPath(import.meta.url));
const dist = resolve(process.argv[2] ?? join(here, '..', 'dist'));
if (!existsSync(dist)) {
  console.error(`dist not found at ${dist}. Run "npm run build" first.`);
  process.exit(2);
}

const pages = [];
(function walk(dir) {
  for (const e of readdirSync(dir, { withFileTypes: true })) {
    const p = join(dir, e.name);
    if (e.isDirectory()) walk(p);
    else if (e.name.endsWith('.html')) pages.push(p);
  }
})(dist);

// The live demo is a separate document with its own assets; check it for external references only.
const isDemo = (p) => relative(dist, p).replace(/\\/g, '/').startsWith('demo/');

const idsCache = new Map();
const idsOf = (file) => {
  if (!idsCache.has(file)) {
    const html = readFileSync(file, 'utf8');
    idsCache.set(file, new Set([...html.matchAll(/\sid=["']([^"']+)["']/g)].map((m) => m[1])));
  }
  return idsCache.get(file);
};

function resolveTarget(fromFile, href) {
  const [pathPart] = href.split('#');
  const rel = relative(dist, fromFile).replace(/\\/g, '/');
  let urlPath;
  if (pathPart === '') urlPath = '/' + rel.replace(/index\.html$/, '');
  else if (pathPart.startsWith('/')) urlPath = pathPart;
  else urlPath = new URL(pathPart, 'http://x/' + rel.replace(/index\.html$/, '')).pathname;
  urlPath = decodeURIComponent(urlPath);
  const base = join(dist, urlPath);
  const candidates = urlPath.endsWith('/') ? [join(base, 'index.html')] : [base, join(base, 'index.html')];
  return candidates.find((c) => existsSync(c) && statSync(c).isFile());
}

const problems = [];
const external = new Map();
let internalCount = 0;
let fragCount = 0;

for (const file of pages) {
  const html = readFileSync(file, 'utf8');
  const where = relative(dist, file).replace(/\\/g, '/');
  const demo = isDemo(file);

  // Subresources must be same-origin.
  for (const m of html.matchAll(/<(script|img|iframe|source|video|audio|embed|object)\b[^>]*?\s(?:src|data)=["']([^"']+)["']/gi)) {
    if (/^(https?:)?\/\//i.test(m[2])) problems.push(`${where}: external <${m[1]}> ${m[2]}`);
  }
  for (const m of html.matchAll(/<link\b[^>]*>/gi)) {
    const rel = /rel=["']([^"']+)["']/i.exec(m[0])?.[1] ?? '';
    const href = /href=["']([^"']+)["']/i.exec(m[0])?.[1];
    if (href && /^(https?:)?\/\//i.test(href) && /stylesheet|icon|preload|prefetch|preconnect|dns-prefetch|manifest/i.test(rel)) {
      problems.push(`${where}: external <link rel="${rel}"> ${href}`);
    }
  }
  if (/@import\s+url\(["']?https?:/i.test(html) || /url\(["']?https?:/i.test(html)) {
    problems.push(`${where}: external url() in CSS`);
  }
  if (demo) continue;

  for (const m of html.matchAll(/<(a|link|img|script|iframe)\b[^>]*?\s(href|src)=["']([^"']*)["']/gi)) {
    const [, tag, , href] = m;
    if (href === '' || /^(mailto:|tel:|javascript:|data:)/i.test(href)) continue;
    if (/^https?:\/\//i.test(href)) {
      if (tag.toLowerCase() === 'a') external.set(href, (external.get(href) ?? 0) + 1);
      continue;
    }
    internalCount++;
    const target = resolveTarget(file, href);
    if (!target) {
      problems.push(`${where}: broken link ${href}`);
      continue;
    }
    const frag = href.includes('#') ? href.split('#')[1] : '';
    if (frag) {
      fragCount++;
      if (!idsOf(target).has(decodeURIComponent(frag))) {
        problems.push(`${where}: ${href} -> no element with id="${frag}" in ${relative(dist, target).replace(/\\/g, '/')}`);
      }
    }
  }

  // The repository is private: nothing may link to it yet.
  if (/github\.com\/nAItiklearn/i.test(html)) {
    problems.push(`${where}: links to the repository (only allowed after REPO_PUBLIC is true)`);
  }
}

console.log(`checked ${pages.length} pages, ${internalCount} internal references, ${fragCount} fragments`);
if (external.size) {
  console.log(`external hyperlinks (not fetched): ${external.size}`);
  for (const [u, n] of [...external].sort()) console.log(`  ${n}x ${u}`);
}
if (problems.length) {
  console.error(`\n${problems.length} problem(s):`);
  for (const p of problems) console.error(`  - ${p}`);
  process.exit(1);
}
console.log('no broken links, no external subresources');
