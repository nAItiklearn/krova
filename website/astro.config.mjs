// @ts-check
import { defineConfig } from 'astro/config';
import { BASE_PATH, REPO_CONFIGURED, SITE_URL } from './src/site.ts';

if (!REPO_CONFIGURED) {
  console.warn(
    '\n[krova-website] REPO_URL in src/site.ts is still the TODO placeholder.\n' +
      '  The download section will say "Releases will appear here" and its link\n' +
      '  points at a placeholder repository. Set REPO_URL before publishing.\n',
  );
}

// https://astro.build/config
export default defineConfig({
  // Static HTML only. No adapter, no SSR, no UI framework integrations,
  // therefore no client-side JavaScript is shipped.
  output: 'static',
  // Left unset until src/site.ts provides a real origin.
  site: SITE_URL || undefined,
  base: BASE_PATH,
  trailingSlash: 'always',
  build: {
    format: 'directory',
    // Inline the (small) stylesheet so there is no render-blocking request.
    inlineStylesheets: 'always',
  },
  compressHTML: true,
  devToolbar: { enabled: false },
});
