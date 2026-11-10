// Keeps website/public/favicon.svg identical to the canonical icon at
// assets/icon/krova-favicon.svg. Runs automatically before `npm run dev` and
// `npm run build`. If the canonical file is not reachable (for example the
// website folder was copied out of the repository), the committed copy is kept.
import { copyFileSync, existsSync, readFileSync } from 'node:fs';
import { dirname, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';

const here = dirname(fileURLToPath(import.meta.url));
const source = resolve(here, '../../assets/icon/krova-favicon.svg');
const target = resolve(here, '../public/favicon.svg');

if (!existsSync(source)) {
  console.log('[sync-icon] assets/icon/krova-favicon.svg not found; keeping committed public/favicon.svg');
} else if (existsSync(target) && readFileSync(source).equals(readFileSync(target))) {
  console.log('[sync-icon] public/favicon.svg is up to date');
} else {
  copyFileSync(source, target);
  console.log('[sync-icon] copied assets/icon/krova-favicon.svg -> public/favicon.svg');
}
