// Regenerates every raster icon from the SVGs in this folder. Run from the repo root:
//
//   node assets/icon/build.mjs
//
// Needs Node 18+ and network access for `npx @tauri-apps/cli` (it rasterises the SVGs with resvg).
//
//   src-tauri/icons/*              what tauri.conf.json bundles and what the tray reuses
//   website/public/apple-touch-icon.png   180 x 180, full-bleed ink square
//
// icon.ico is assembled here instead of by `tauri icon` so the small entries come from the
// pixel-snapped cuts in small/ (16, 20, 24 px, and 2x of each for 32, 40, 48 px) instead of
// shrinking the 512 px master, which turns the 1 px gaps grey. Entries: 16 20 24 32 40 48 64 256.
import { execSync } from 'node:child_process';
import { copyFileSync, mkdirSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { dirname, join, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';

const TAURI_CLI = '@tauri-apps/cli@2.12.3';
const here = dirname(fileURLToPath(import.meta.url));
const root = resolve(here, '../..');
const icons = join(root, 'src-tauri', 'icons');
const tmp = mkdtempSync(join(tmpdir(), 'krova-icons-'));

function tauriIcon(svg, outDir, extra = '') {
  mkdirSync(outDir, { recursive: true });
  execSync(`npx -y ${TAURI_CLI} icon "${svg}" -o "${outDir}" ${extra}`, { stdio: 'inherit', cwd: root });
}

try {
  // 1. Master -> the default set (icns, 64/128/256/512 px PNGs).
  const full = join(tmp, 'full');
  tauriIcon(join(here, 'krova.svg'), full);

  // 2. Pixel-snapped cuts, each at 1x and 2x.
  const cuts = {};
  for (const n of [16, 20, 24]) {
    const dir = join(tmp, `cut${n}`);
    tauriIcon(join(here, 'small', `krova-${n}.svg`), dir, `--png ${n},${n * 2}`);
    cuts[n] = dir;
  }

  // 3. src-tauri/icons: same file names tauri.conf.json lists.
  copyFileSync(join(full, 'icon.icns'), join(icons, 'icon.icns'));
  copyFileSync(join(full, 'icon.png'), join(icons, 'icon.png'));
  copyFileSync(join(full, '64x64.png'), join(icons, '64x64.png'));
  copyFileSync(join(full, '128x128.png'), join(icons, '128x128.png'));
  copyFileSync(join(full, '128x128@2x.png'), join(icons, '128x128@2x.png'));
  copyFileSync(join(cuts[16], '32x32.png'), join(icons, '32x32.png'));

  const entries = [
    [16, join(cuts[16], '16x16.png')],
    [20, join(cuts[20], '20x20.png')],
    [24, join(cuts[24], '24x24.png')],
    [32, join(cuts[16], '32x32.png')],
    [40, join(cuts[20], '40x40.png')],
    [48, join(cuts[24], '48x48.png')],
    [64, join(full, '64x64.png')],
    [256, join(full, '128x128@2x.png')],
  ].map(([size, file]) => ({ size, data: readFileSync(file) }));
  writeFileSync(join(icons, 'icon.ico'), buildIco(entries));

  // 4. Website touch icon: master without the rounded corners (iOS rounds them itself).
  const touchSvg = join(tmp, 'touch.svg');
  writeFileSync(touchSvg, readFileSync(join(here, 'krova.svg'), 'utf8').replace('rx="112"', 'rx="0"'));
  const touch = join(tmp, 'touch');
  tauriIcon(touchSvg, touch, '--png 180');
  copyFileSync(join(touch, '180x180.png'), join(root, 'website', 'public', 'apple-touch-icon.png'));

  console.log('icons regenerated');
} finally {
  rmSync(tmp, { recursive: true, force: true });
}

// ICO container with PNG-compressed entries (the same thing `tauri icon` writes for rounded icons).
function buildIco(images) {
  const header = Buffer.alloc(6);
  header.writeUInt16LE(1, 2); // type: icon
  header.writeUInt16LE(images.length, 4);
  const dir = Buffer.alloc(16 * images.length);
  let offset = header.length + dir.length;
  images.forEach(({ size, data }, i) => {
    const e = i * 16;
    dir.writeUInt8(size >= 256 ? 0 : size, e); // width
    dir.writeUInt8(size >= 256 ? 0 : size, e + 1); // height
    dir.writeUInt16LE(1, e + 4); // colour planes
    dir.writeUInt16LE(32, e + 6); // bits per pixel
    dir.writeUInt32LE(data.length, e + 8);
    dir.writeUInt32LE(offset, e + 12);
    offset += data.length;
  });
  return Buffer.concat([header, dir, ...images.map((i) => i.data)]);
}
