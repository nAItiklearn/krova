# Krova website

Static marketing site for Krova, the free, offline window zone manager, built with [Astro](https://astro.build) 5
(pinned to 5.18.2, the last 5.x release).
It ships plain HTML and CSS only: **no client-side JavaScript, no third-party scripts, fonts, analytics or embeds,
and no external requests of any kind.** System font stack only. This mirrors the product promise that nothing leaves
your computer.

## Run it

Requires Node `18.20.8`, `20.3+` or `22+`.

```bash
cd website
npm install
npm run dev       # http://localhost:4321
npm run build     # static output in website/dist/
npm run preview   # serve website/dist/ locally
```

Run the scripts from the `website/` folder. The `/license` page reads `../LICENSE` (the repository's license file) at
build time, so the site always shows the real text; the build fails with a clear message if it cannot find it.

Astro's own CLI sends anonymous usage telemetry by default. To turn it off for your machine:
`npx astro telemetry disable`.

## Where to set the repository URL

Everything repository- or release-related lives in one file: **`src/site.ts`**.

| Constant | What it does |
| --- | --- |
| `REPO_URL` | **Set this first.** It starts as `https://github.com/TODO-OWNER/krova`. While it contains `TODO`, source links are hidden and the Download section reads "Releases will appear here". |
| `RELEASES_URL`, `ISSUES_URL` | Derived from `REPO_URL`. Don't edit. |
| `RELEASE_PUBLISHED` | Leave `false` until the first release exists. Direct download buttons only render when `REPO_URL` is set **and** this is `true`, so a link can never point at a release that doesn't exist. |
| `DOWNLOADS` | The asset file names behind `releases/latest/download/<name>`. They must match the names the release workflow uploads, and must not contain a version number. The values here are placeholders. |
| `SITE_URL` | Optional. Public origin (for example `https://krova.example`). When set, `canonical` and `og:url` are emitted. |
| `BASE_PATH` | Sub-path the site is served from. `/` for a domain root, `/krova` for GitHub project pages. |

Download section states:

1. `REPO_URL` still a placeholder: "Releases will appear here" plus a link to `RELEASES_URL` (a placeholder until you
   set it; the build prints a warning).
2. `REPO_URL` set, `RELEASE_PUBLISHED = false`: the same message, with a working link to the repository's releases page.
3. `REPO_URL` set, `RELEASE_PUBLISHED = true`: one button per entry in `DOWNLOADS`, each pointing at
   `<REPO_URL>/releases/latest/download/<file>`.

## Layout

```
website/
  astro.config.mjs        static output, reads SITE_URL / BASE_PATH from src/site.ts
  public/
    favicon.svg           copy of ../assets/icon/krova.svg (see scripts/sync-icon.mjs)
    apple-touch-icon.png  180x180 raster of the icon for iOS home screens
    robots.txt
  scripts/sync-icon.mjs   keeps public/favicon.svg identical to assets/icon/krova.svg (runs before dev/build)
  src/
    site.ts               the one config file (repo, releases, downloads, site URL)
    lib/url.ts            base-path aware URL helper
    layouts/BaseLayout.astro
    components/           Header, Footer, Hero, HeroMonitor, HowItWorks, ZoneDiagram, Features,
                          PrivacySection, Limitations, Download, Faq, Icon
    pages/                index, privacy, license, 404
    styles/tokens.css     design tokens: type scale, spacing, color (light + dark)
    styles/base.css       reset, layout primitives, buttons, focus, reduced motion
```

## Design and accessibility notes

- Colors come from the icon gradient (`#4f46e5` to `#0ea5e9`). Light and dark follow `prefers-color-scheme`; there
  is no toggle and no script. Text colors are chosen for WCAG AA contrast (ratios are listed at the top of
  `src/styles/tokens.css`); white text is only used on the indigo end of the gradient.
- All visuals are inline SVG and CSS. There are **no screenshots**; the hero is an illustration and is labelled as one.
- The hero animation only runs when `prefers-reduced-motion` is not set to reduce, and has a CSS-only "Pause
  animation" checkbox.
- Skip link, landmarks (`header`, `nav`, `main`, `footer`), visible focus rings, native `<details>` for the FAQ.
- In production builds a `<meta http-equiv="Content-Security-Policy">` tag (`default-src 'none'`, same-origin images
  and connections, inline styles) makes the browser itself refuse any external request. It is omitted in
  `npm run dev`. If you host behind a CDN that injects scripts or beacons, this policy will block them by design.
- There is no `og:image` because social cards need an absolute URL and no origin is known yet. Add one (a local file in
  `public/` plus `SITE_URL`) when the site has a home.

## Regenerating the Apple touch icon

`public/apple-touch-icon.png` is a one-off 180x180 raster of the icon on a full-bleed square canvas (iOS applies its
own rounding, and fills transparent corners with black). It was made with `sharp` (already installed as a dependency of
Astro). Re-create it the same way, or with any SVG rasterizer, if the icon changes.
