import { url } from './url';

/**
 * Tiny inline markup for data files (FAQ, changelog), so copy can live in
 * plain strings without raw HTML:
 *
 *   [[/guides/install/|Install guide]]  ->  internal link (respects BASE_PATH)
 *   `config.json`                        ->  <code>
 *   **bold**                             ->  <strong>
 *
 * Everything else is HTML-escaped.
 */
const escapeHtml = (s: string): string =>
  s.replace(/&/g, '&amp;').replace(/</g, '&lt;').replace(/>/g, '&gt;').replace(/"/g, '&quot;');

export function inline(text: string): string {
  return escapeHtml(text)
    .replace(/\[\[([^|\]]+)\|([^\]]+)\]\]/g, (_m, path: string, label: string) => {
      return `<a href="${url(path.replace(/&amp;/g, '&'))}">${label}</a>`;
    })
    .replace(/`([^`]+)`/g, '<code>$1</code>')
    .replace(/\*\*([^*]+)\*\*/g, '<strong>$1</strong>');
}
