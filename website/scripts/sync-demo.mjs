// Builds the real zone-editor UI (../ui) and copies its static output into
// website/public/demo/, so the site serves it at /demo/. The UI falls back to
// its in-memory mock backend when there is no Tauri runtime, so the demo runs
// in any browser and never touches real windows.
//
// public/demo/ is generated and git-ignored (see website/.gitignore).
//
// Usage:
//   node scripts/sync-demo.mjs            build only if the copy is missing or older than the UI sources
//   node scripts/sync-demo.mjs --force    always rebuild (used by `npm run build`)
//
// Fails loudly, with the reason, if the UI cannot be built: a site without its
// live demo would silently break the "Try the editor" section.
import { spawnSync } from 'node:child_process';
import { cpSync, existsSync, mkdirSync, readdirSync, rmSync, statSync } from 'node:fs';
import { dirname, join, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';

const here = dirname(fileURLToPath(import.meta.url));
const site = resolve(here, '..');
const ui = resolve(site, '../ui');
const uiDist = join(ui, 'dist');
const target = join(site, 'public', 'demo');
const force = process.argv.includes('--force');
const tag = '[sync-demo]';

function fail(message) {
  console.error(`\n${tag} ERROR: ${message}\n`);
  console.error(`${tag} The site needs the live demo. Fix the problem above, or build the UI by hand:`);
  console.error(`${tag}   npm ci --prefix ../ui && npm run build --prefix ../ui\n`);
  process.exit(1);
}

/** Newest modification time of any file under `dir`, ignoring node_modules, dist and dot-folders. */
function newest(dir) {
  let t = 0;
  for (const entry of readdirSync(dir, { withFileTypes: true })) {
    if (entry.name === 'node_modules' || entry.name === 'dist' || entry.name.startsWith('.')) continue;
    const p = join(dir, entry.name);
    t = Math.max(t, entry.isDirectory() ? newest(p) : statSync(p).mtimeMs);
  }
  return t;
}

function run(args, label) {
  console.log(`${tag} ${label}: npm ${args.join(' ')}`);
  // npm is a .cmd file on Windows, which current Node refuses to spawn without a shell. The arguments are
  // fixed strings from this file, so one command line is safe (and avoids Node's args-with-shell warning).
  const res =
    process.platform === 'win32'
      ? spawnSync(`npm ${args.join(' ')}`, { cwd: site, stdio: 'inherit', shell: true })
      : spawnSync('npm', args, { cwd: site, stdio: 'inherit' });
  if (res.error) fail(`could not start npm (${res.error.message}). Is Node.js installed and on PATH?`);
  if (res.status !== 0) fail(`"npm ${args.join(' ')}" exited with code ${res.status}.`);
}

if (!existsSync(join(ui, 'package.json'))) {
  fail(
    `${ui} does not contain a package.json. The website builds the live demo from the repository's ui/ folder, ` +
      'so website/ must stay inside the Krova repository (not copied out on its own).',
  );
}

const copy = join(target, 'index.html');
const fresh =
  existsSync(copy) && existsSync(uiDist) && statSync(copy).mtimeMs >= newest(ui) && statSync(copy).mtimeMs > 0;

if (!force && fresh) {
  console.log(`${tag} public/demo/ is up to date (use --force to rebuild)`);
  process.exit(0);
}

if (!existsSync(join(ui, 'node_modules'))) {
  run(['ci', '--prefix', '../ui'], 'UI dependencies are missing, installing');
}
run(['run', 'build', '--prefix', '../ui'], 'building the UI');

if (!existsSync(join(uiDist, 'index.html'))) {
  fail(`the UI build finished but ${join(uiDist, 'index.html')} does not exist.`);
}

rmSync(target, { recursive: true, force: true });
mkdirSync(target, { recursive: true });
cpSync(uiDist, target, { recursive: true });
console.log(`${tag} copied ui/dist -> public/demo/ (served at /demo/)`);
