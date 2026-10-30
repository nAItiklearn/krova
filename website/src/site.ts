/**
 * Single source of truth for every repository, release and deployment URL on
 * the Krova website. Nothing else in the codebase hard-codes these.
 *
 * Keep this file free of `import.meta` and framework imports: it is also
 * loaded by astro.config.mjs.
 */

export const SITE_NAME = 'Krova';
export const VERSION_LABEL = 'v0.1';
export const COPYRIGHT_HOLDER = 'Krova contributors';

/* ------------------------------------------------------------------ *
 * Repository
 * ------------------------------------------------------------------ */

/**
 * TODO(owner): confirm this once the repository exists. The owner is taken
 * from the Windy repo's remote (nAItiklearn); the repository name "krova" is
 * an assumption. No trailing slash.
 *
 * While the value contains "TODO", the site treats the repository as not
 * configured: the download section shows "Releases will appear here", and
 * links to the source code are hidden.
 */
export const REPO_URL = 'https://github.com/nAItiklearn/krova';

/** True once REPO_URL no longer contains the TODO placeholder. */
export const REPO_CONFIGURED = !/todo/i.test(REPO_URL);

const repoBase = REPO_URL.replace(/\/+$/, '');

/** Derived from REPO_URL. Do not edit. */
export const RELEASES_URL = `${repoBase}/releases`;
export const ISSUES_URL = `${repoBase}/issues`;

/* ------------------------------------------------------------------ *
 * Downloads
 * ------------------------------------------------------------------ */

/**
 * TODO(owner): flip to true only after the first release exists and its
 * assets (listed in DOWNLOADS) are attached. Until then, even with REPO_URL
 * set, the site will not render direct download buttons, so no link can point
 * at a release that does not exist yet.
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
    requirement: 'Windows 10 or 11',
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

/** Direct links are only rendered when this is true. */
export const DOWNLOADS_ENABLED = REPO_CONFIGURED && RELEASE_PUBLISHED;

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
export const SITE_URL = '';

/**
 * Sub-path the site is served from. Use '/' for a domain root, or for
 * example '/krova' when hosting on GitHub project pages.
 */
export const BASE_PATH = '/';
