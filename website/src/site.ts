/**
 * Single source of truth for every repository, release and deployment URL on
 * the Krova website. Nothing else in the codebase hard-codes these.
 *
 * Keep this file free of `import.meta` and framework imports: it is also
 * loaded by astro.config.mjs.
 *
 * Link rules (enforced by the components, not by convention):
 *   - A link to the repository, its source, its releases or its issues renders
 *     only when REPO_LINKS_ENABLED is true (repository configured AND public).
 *   - A direct download button renders only when DOWNLOADS_ENABLED is true
 *     (repository public AND a release published).
 *   - Otherwise the page says so in words ("The first release is not
 *     published yet", "Krova's repository is not public yet").
 */

export const SITE_NAME = 'Krova';
export const VERSION_LABEL = 'v0.1';
/** Must equal "version" in src-tauri/tauri.conf.json. */
export const APP_VERSION = '0.1.0';
export const COPYRIGHT_HOLDER = 'Krova contributors';

/* ------------------------------------------------------------------ *
 * Repository
 * ------------------------------------------------------------------ */

/** No trailing slash. The repository exists but is currently PRIVATE. */
export const REPO_URL = 'https://github.com/nAItiklearn/krova';

/**
 * Flip to true only after the repository has been made public. Until then no
 * link to github.com/nAItiklearn/krova is rendered anywhere on the site.
 */
export const REPO_PUBLIC = true;

/** True once REPO_URL no longer contains the TODO placeholder. */
export const REPO_CONFIGURED = !/todo/i.test(REPO_URL);

/** The one switch every repo / source / releases / issues link checks. */
export const REPO_LINKS_ENABLED = REPO_CONFIGURED && REPO_PUBLIC;

const repoBase = REPO_URL.replace(/\/+$/, '');

/** Derived from REPO_URL. Do not edit. */
export const RELEASES_URL = `${repoBase}/releases`;
export const ISSUES_URL = `${repoBase}/issues`;

/* ------------------------------------------------------------------ *
 * Downloads
 * ------------------------------------------------------------------ */

/**
 * Flip to true only after the first release exists and its assets (listed in
 * DOWNLOADS, plus SHA256SUMS.txt) are attached. Until then no download button
 * is rendered, so no link can point at a release that does not exist yet.
 */
export const RELEASE_PUBLISHED = false;

export interface DownloadAsset {
  id: 'windows' | 'macos';
  label: string;
  /** Short name used in button text. */
  short: string;
  requirement: string;
  /**
   * Must match, character for character, the asset name that
   * .github/workflows/release.yml uploads ("Give files version-free names").
   * It must NOT contain the version number, because
   * `releases/latest/download/<name>` needs a stable name.
   */
  file: string;
}

export const DOWNLOADS: readonly DownloadAsset[] = [
  {
    id: 'windows',
    label: 'Windows',
    short: 'Windows',
    requirement: 'Windows 10 or 11, 64-bit',
    file: 'Krova-windows-x64-setup.exe',
  },
  {
    id: 'macos',
    label: 'macOS',
    short: 'macOS',
    requirement: 'macOS 13 Ventura or later',
    file: 'Krova-macos-universal.dmg',
  },
];

/** Published by the release workflow next to the installers. */
export const CHECKSUMS_FILE = 'SHA256SUMS.txt';

/** Direct links are only rendered when this is true. */
export const DOWNLOADS_ENABLED = REPO_LINKS_ENABLED && RELEASE_PUBLISHED;

/** Stable link to an asset of the newest release. */
export function latestDownloadUrl(file: string): string {
  return `${RELEASES_URL}/latest/download/${encodeURIComponent(file)}`;
}

/* ------------------------------------------------------------------ *
 * Deployment (optional)
 * ------------------------------------------------------------------ */

/**
 * TODO(owner, optional): the public origin the site will be served from,
 * for example 'https://krova.example'. When set, canonical and og:url tags
 * are emitted. Leave empty until a real domain exists.
 */
export const SITE_URL = 'https://naitiklearn.github.io/krova';

/**
 * Sub-path the site is served from. Use '/' for a domain root, or for
 * example '/krova' when hosting on GitHub project pages.
 */
export const BASE_PATH = '/krova';
