/**
 * Build a site-internal URL that respects the configured `base` path.
 * Pass paths that start with "/", e.g. url('/privacy/') or url('/') + '#faq'.
 */
export function url(path: string): string {
  const base = import.meta.env.BASE_URL.replace(/\/+$/, '');
  return base + (path.startsWith('/') ? path : `/${path}`);
}
