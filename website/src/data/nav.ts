/**
 * Navigation and footer link data. Paths start with "/" and are passed through
 * url() (src/lib/url.ts) by the components, so they respect BASE_PATH.
 */

export interface NavLink {
  label: string;
  href: string;
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
    summary: 'Shift-drag a window into a zone, and how to cancel.',
  },
  {
    slug: 'layout-editor',
    title: 'The layout editor',
    short: 'Layout editor',
    summary: 'Templates, splitting, dividers, deleting zones, saving and switching layouts, and the limits.',
  },
  {
    slug: 'multiple-displays',
    title: 'Multiple displays',
    short: 'Multiple displays',
    summary: 'One layout set per monitor, how Krova recognises a monitor, and what happens on reconnect.',
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
    summary: 'The editor shortcuts that exist, and what does not exist yet.',
  },
];

export const guidePath = (slug: string): string => `/guides/${slug}/`;

/** Header links before the Guides dropdown. */
export const navBeforeGuides: readonly NavLink[] = [
  { label: 'How it works', href: '/#how' },
  { label: 'Features', href: '/#features' },
  { label: 'Compare', href: '/#compare' },
  { label: 'Performance', href: '/performance/' },
];

/** Header links after the Guides dropdown. */
export const navAfterGuides: readonly NavLink[] = [
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
      { label: 'How it works', href: '/#how' },
      { label: 'Features', href: '/#features' },
      { label: 'Compare', href: '/#compare' },
      { label: 'Performance', href: '/performance/' },
      { label: 'Changelog', href: '/changelog/' },
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
    title: 'Support',
    links: [
      { label: 'Troubleshooting', href: '/support/' },
      { label: 'Report a problem', href: '/support/#report' },
    ],
  },
  {
    title: 'Legal',
    links: [
      { label: 'Privacy', href: '/privacy/' },
      { label: 'License and terms', href: '/license/' },
    ],
  },
];
