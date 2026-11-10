/**
 * Navigation and footer link data. Paths start with "/" and are passed through
 * url() (src/lib/url.ts) by the components, so they respect BASE_PATH.
 */

import { ISSUES_URL, REPO_LINKS_ENABLED, REPO_URL } from '../site';

export interface NavLink {
  label: string;
  href: string;
  /** A full URL to another site (opens in a new tab). Everything else is a path that goes through url(). */
  external?: boolean;
}

export interface Guide {
  slug: string;
  title: string;
  /** Short label for menus and the footer. */
  short: string;
  summary: string;
}

export const guides: readonly Guide[] = [
  {
    slug: 'install',
    title: 'Install Krova',
    short: 'Install',
    summary: 'Windows and macOS: the unsigned-installer warnings, permissions, the tray icon, uninstalling and removing all data.',
  },
  {
    slug: 'first-snap',
    title: 'Your first snap',
    short: 'Your first snap',
    summary: 'Shift-drag a window into a zone, how to cancel, and the keyboard alternative on Windows.',
  },
  {
    slug: 'layout-editor',
    title: 'The layout editor',
    short: 'Layout editor',
    summary: 'Templates, splitting, dividers, deleting zones, saving and switching layouts, exporting and importing them, and the limits.',
  },
  {
    slug: 'multiple-displays',
    title: 'Multiple displays',
    short: 'Multiple displays',
    summary: 'One layout set per monitor, how Krova recognises a monitor, and what comes back on reconnect: layouts, and on Windows the windows too.',
  },
  {
    slug: 'edge-snapping',
    title: 'Edge snapping',
    short: 'Edge snapping',
    summary: 'Halves, quarters and maximize from the screen edges. Off by default.',
  },
  {
    slug: 'keyboard-shortcuts',
    title: 'Keyboard shortcuts',
    short: 'Keyboard shortcuts',
    summary: 'Move the focused window between zones from the keyboard (Windows), choose the modifier keys, and the layout editor keys.',
  },
];

export const guidePath = (slug: string): string => `/guides/${slug}/`;

/** Header links before the Guides dropdown. */
export const navBeforeGuides: readonly NavLink[] = [
  { label: 'Try it live', href: '/#live' },
  { label: 'Features', href: '/#features' },
  { label: 'Compare', href: '/#compare' },
  { label: 'Performance', href: '/performance/' },
];

/** Header links after the Guides dropdown. */
export const navAfterGuides: readonly NavLink[] = [
  { label: 'Roadmap', href: '/roadmap/' },
  { label: 'Changelog', href: '/changelog/' },
  { label: 'FAQ', href: '/#faq' },
];

export interface FooterColumn {
  title: string;
  links: readonly NavLink[];
}

export const footerColumns: readonly FooterColumn[] = [
  {
    title: 'Product',
    links: [
      { label: 'Try it live', href: '/#live' },
      { label: 'Features', href: '/#features' },
      { label: 'Compare', href: '/#compare' },
      { label: 'Performance', href: '/performance/' },
      { label: 'Download', href: '/download/' },
    ],
  },
  {
    title: 'Guides',
    links: [
      ...guides.map((g) => ({ label: g.short, href: guidePath(g.slug) })),
      { label: 'All guides', href: '/guides/' },
    ],
  },
  {
    title: 'Project',
    links: [
      { label: 'Roadmap', href: '/roadmap/' },
      { label: 'Changelog', href: '/changelog/' },
      // Repository links only exist while REPO_LINKS_ENABLED (src/site.ts).
      ...(REPO_LINKS_ENABLED
        ? [
            { label: 'Source code', href: REPO_URL, external: true },
            { label: 'Issues', href: ISSUES_URL, external: true },
          ]
        : []),
    ],
  },
  {
    title: 'Support',
    links: [
      { label: 'Troubleshooting', href: '/support/' },
      { label: 'Report a problem', href: '/support/#report' },
      { label: 'Privacy', href: '/privacy/' },
      { label: 'License and terms', href: '/license/' },
    ],
  },
];
