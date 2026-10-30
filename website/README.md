# Krova website

Static marketing and download site for Krova, the free, offline window zone manager, built with
[Astro](https://astro.build) 5 (pinned to 5.18.2, the last 5.x release).

It ships plain HTML and CSS plus **one small same-origin script** (`public/theme.js`, the light/dark choice and the
menus). No third-party scripts, fonts, analytics or embeds, no cookies, and no request to any other origin. System font
stack only. This mirrors the product promise that nothing leaves your computer.

## Run it

Requires Node `18.20.8`, `20.3+` or `22+`. Run the scripts from the `website/` folder, inside the Krova repository: the
site reads `../LICENSE`, `../docs/measurements/` and builds `../ui` for the live demo.

```bash
cd website
npm install
npm run dev        # http://localhost:4321
npm run build      # static output in website/dist/
npm run preview    # serve website/dist/ locally
npm run check:links   # after a build: every internal link, #fragment and subresource in dist/
```

`npm run build` first runs two helper scripts:

| Script | What it does |
| --- | --- |
| `scripts/sync-icon.mjs` | keeps `public/favicon.svg` identical to `assets/icon/krova.svg` |
| `scripts/sync-demo.mjs` | builds the real editor UI (`npm run build --prefix ../ui`, after `npm ci --prefix ../ui` if its dependencies are missing) and copies `ui/dist` to `public/demo/`, which the site serves at `/demo/` |

`public/demo/` is generated and git-ignored (`website/.gitignore`). The script **fails loudly** if the UI cannot be built.
`npm run dev` only rebuilds the demo when it is missing or older than the UI sources; `npm run build` always rebuilds it.
The UI falls back to its in-memory mock backend when there is no Tauri runtime, so the demo runs in any browser against two
sample monitors and never touches real windows.

Astro's own CLI sends anonymous usage telemetry by default. To turn it off for your machine: `npx astro telemetry disable`.

## How to publish

The site is built to be safe to deploy at any stage: nothing links to a repository or a release that is not public.
Everything is controlled by two flags in **`src/site.ts`**.

1. **Make the repository public** (GitHub: Settings, Danger Zone, Change visibility).
2. In the repository, run **Actions, release (manual)** (`workflow_dispatch`). It builds the Windows installer and the macOS
   disk image as workflow *artifacts* only and publishes nothing. Download them and try them.
3. Push a tag **only when you are ready to publish**: `git tag v0.1.0 && git push origin v0.1.0`. The tag must equal
   `version` in `src-tauri/tauri.conf.json` (currently `0.1.0`); the workflow refuses a mismatch. The tag run creates the GitHub
   Release with `Krova-windows-x64-setup.exe`, `Krova-macos-universal.dmg` and `SHA256SUMS.txt`.
   Before the first tag, read the open items in `docs/PROGRESS.md` and `docs/SECURITY-REVIEW.md` (signing, pinning the release
   toolchain, the untested macOS build).
4. In `src/site.ts` set `REPO_PUBLIC = true` and, once the release exists, `RELEASE_PUBLISHED = true`.
   Also update `src/data/changelog.ts` (give 0.1.0 `status: 'released'` and its real `date`) and the "not released yet" wording
   (search the site for "not released"; the home hero, the download page and the FAQ switch on the flags by themselves).
5. Rebuild (`npm run build`) and deploy `dist/` to the host you chose. The site is static: any static host works
   (GitHub Pages, Cloudflare Pages, Netlify, an object store behind a CDN). Set `SITE_URL` (and `BASE_PATH` for a sub-path such
   as `/krova` on GitHub project pages) so canonical and `og:url` tags are emitted. This repository does not deploy anything.

### What the flags do

| Constant | What it does |
| --- | --- |
| `REPO_URL` | The repository, `https://github.com/nAItiklearn/krova`. While it contains `TODO` it counts as not configured. |
| `REPO_PUBLIC` | `false` until the repository is public. **Every** link to the repository, source, releases or issues renders only when `REPO_CONFIGURED && REPO_PUBLIC` (`REPO_LINKS_ENABLED`); otherwise the page says "Krova's repository is not public yet." |
| `RELEASE_PUBLISHED` | `false` until a release exists. Direct download buttons render only when `REPO_PUBLIC && RELEASE_PUBLISHED` (`DOWNLOADS_ENABLED`). Until then the Download page explains what to expect and shows "Not available yet" instead of a dead button. |
| `DOWNLOADS` | The asset names behind `releases/latest/download/<name>`. They must match `.github/workflows/release.yml` and must not contain a version number. |
| `SITE_URL`, `BASE_PATH` | Optional origin and sub-path. |

`npm run check:links` fails if any built page links to the repository while `REPO_PUBLIC` is false.

## Content that is data, not markup

| File | What it drives |
| --- | --- |
| `src/data/changelog.ts` | The Changelog page. **To add a release, put a new object at the top of `releases`.** Never invent a date for a release that has not happened. |
| `src/data/compare.ts` | The Compare table on the home page. Cells come from `docs/compare-research.md`; only verified `[V]` cells are claims, `[U]` cells are `unknown` (an em dash). Update `CHECKED_ISO` / `CHECKED_LABEL` when you re-verify. |
| `src/data/faq.ts` | The FAQ (answers use the tiny inline markup in `src/lib/inline.ts`). |
| `src/data/nav.ts` | Header and footer links, and the guide list. |
| `docs/measurements/idle-YYYY-MM-DD.txt` | The Performance page. The newest file is parsed at build time (`src/lib/measurements.ts`); no number on that page is typed by hand. Drop in a newer measurement file and rebuild. |

## Layout

```
website/
  astro.config.mjs        static output; reads SITE_URL / BASE_PATH from src/site.ts
  public/
    theme.js              the only script: theme choice (before first paint), menus; same-origin
    favicon.svg           copy of ../assets/icon/krova.svg (kept in sync by scripts/sync-icon.mjs)
    apple-touch-icon.png  180x180 raster of the icon
    robots.txt
    demo/                 GENERATED by scripts/sync-demo.mjs from ../ui (git-ignored)
  scripts/                sync-icon, sync-demo, check-links
  src/
    site.ts               the one config file (repo, releases, downloads, site URL)
    data/                 nav, changelog, compare, faq
    lib/                  url (base-path aware), inline markup, measurements parser, zone maths
    layouts/              BaseLayout, GuideLayout
    components/           Header, Footer, Hero, HowItWorks, FractionsDiagram, Features, Customize, TryEditor,
                          Compare, PerformanceTeaser, OpenSource, Faq, FinalCta, SnapFlow, Icon
      illustrations/      Desk, MockWin, MiniMonitor, EditorMock, Cursor (HTML + CSS mock desktops)
      charts/             MemoryChart, CpuChart (inline SVG made at build time, no library)
    pages/                index, download, performance, changelog, support, privacy, license, 404,
                          guides/{index,install,first-snap,layout-editor,multiple-displays,edge-snapping,keyboard-shortcuts}
    styles/               tokens.css (colours, type, space; light + dark), base.css, illustrations.css
```

## Design and accessibility notes

- One accent, taken from the icon (`#4f46e5`). Light is the `:root` default; dark applies under
  `prefers-color-scheme: dark` when the visitor has not chosen (`:root:not([data-theme])`) and under `:root[data-theme="dark"]`.
  `public/theme.js` sets `data-theme` from `localStorage["krova-theme"]` before first paint, so there is no flash. With
  JavaScript off the site follows the system theme and the theme buttons stay hidden. Contrast ratios are listed at the top of
  `src/styles/tokens.css` (WCAG AA for text in both themes).
- Illustrations are HTML/CSS and inline SVG, labelled "Illustration" where they could pass for screenshots. There are no stock
  images and no screenshots. Motion is limited to the hero loop and small transitions; `prefers-reduced-motion` turns all of it
  off, and the hero has a CSS-only pause button (a checkbox).
- Skip link, landmarks, visible focus rings, native `<details>` for the FAQ and the dropdowns, a stepper built from radio
  inputs (arrow keys work), tables and diagrams that scroll inside their own box on phones.
- Production pages carry `<meta http-equiv="Content-Security-Policy">`: `default-src 'none'`; `script-src 'self'`;
  `frame-src 'self'` (the live demo); `img-src 'self'`; `connect-src 'self'`; inline styles. The browser itself refuses any
  external request. It is omitted in `npm run dev`. The demo (`/demo/`) is its own document with its own CSP, set by the UI.
  If you host behind a CDN that injects scripts or beacons, this policy will block them by design.
- There is no `og:image`: social cards need an absolute URL and no origin is known yet.

## Regenerating the Apple touch icon

`public/apple-touch-icon.png` is a one-off 180x180 raster of the icon on a full-bleed square canvas (iOS applies its own
rounding, and fills transparent corners with black). It was made with `sharp` (already installed as a dependency of Astro).
Re-create it the same way, or with any SVG rasterizer, if the icon changes.
