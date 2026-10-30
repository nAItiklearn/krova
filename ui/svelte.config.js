import { vitePreprocess } from '@sveltejs/vite-plugin-svelte';

export default {
  // TypeScript (type stripping only) in <script lang="ts">.
  preprocess: vitePreprocess(),
};
