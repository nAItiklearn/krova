// @ts-check
import { defineConfig } from 'astro/config';
import {
  BASE_PATH,
  DOWNLOADS_ENABLED,
  REPO_CONFIGURED,
  REPO_LINKS_ENABLED,
  SITE_URL,
} from './src/site.ts';

if (!REPO_CONFIGURED) {
  console.warn(
    '\n[krova-website] REPO_URL in src/site.ts is still the TODO placeholder.\n' +
      '  Every repository, source, releases and issues link is hidden. Set REPO_URL first.\n',
  );
}
console.log(
  `[krova-website] repo links: ${REPO_LINKS_ENABLED ? 'shown' : 'hidden (REPO_PUBLIC is false)'}; ` +
    `download buttons: ${DOWNLOADS_ENABLED ? 'shown' : 'hidden (no published release)'}`,
);

// https://astro.build/config
export default defineConfig({
  // Static HTML only. No adapter, no SSR, no UI framework integrations.
  // The only client-side JavaScript is public/theme.js (light/dark choice).
  output: 'static',
  // Left unset until src/site.ts provides a real origin.
  site: SITE_URL || undefined,
  base: BASE_PATH,
  trailingSlash: 'always',
  build: {
    format: 'directory',
    // Inline the stylesheet so there is no render-blocking request.
    inlineStylesheets: 'always',
  },
  compressHTML: true,
  devToolbar: { enabled: false },
});
