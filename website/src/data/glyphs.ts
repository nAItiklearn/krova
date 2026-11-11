/**
 * Small flat diagrams for the Roadmap cards, drawn from Krova's own zone shapes on a 64 x 40 monitor.
 * They are not screenshots and do not claim to show a finished feature: they only say what the feature is about.
 * Classes (styled in components/Glyph.astro):
 *   fr  monitor frame        z   an empty zone (dashed)       w   a window
 *   a   lit / accent shape   dm  dimmed shape                  k   a keycap
 *   l   a thin line          t   text
 */
const frame = '<rect class="fr" x="1.5" y="1.5" width="61" height="37" rx="4"/>';

export const glyphs = {
  /* ---- Available now ---- */
  zones: `${frame}<rect class="z" x="5" y="5" width="16" height="30" rx="2.5"/><rect class="a" x="23" y="5" width="18" height="30" rx="2.5"/><rect class="z" x="43" y="5" width="16" height="30" rx="2.5"/>`,
  snap: `${frame}<rect class="z" x="5" y="5" width="25" height="30" rx="2.5"/><rect class="a" x="34" y="5" width="25" height="30" rx="2.5"/><rect class="w" x="20" y="13" width="22" height="16" rx="2.5"/>`,
  edge: `${frame}<rect class="a" x="5" y="5" width="26" height="30" rx="2.5"/><rect class="w" x="9" y="12" width="16" height="16" rx="2.5"/><path class="l" d="M45 20h-10M38 16l-4 4 4 4"/>`,
  monitors: `<rect class="fr" x="2" y="6" width="28" height="26" rx="3"/><rect class="z" x="5" y="9" width="10" height="20" rx="2"/><rect class="a" x="17" y="9" width="10" height="20" rx="2"/><rect class="fr" x="34" y="6" width="28" height="26" rx="3"/><rect class="a" x="37" y="9" width="10" height="9" rx="2"/><rect class="z" x="49" y="9" width="10" height="9" rx="2"/><rect class="z" x="37" y="21" width="10" height="8" rx="2"/><rect class="z" x="49" y="21" width="10" height="8" rx="2"/>`,
  keyboard: `<rect class="k" x="3" y="11" width="17" height="18" rx="4"/><rect class="k" x="23" y="11" width="17" height="18" rx="4"/><rect class="k a" x="43" y="11" width="17" height="18" rx="4"/><path class="l" d="M8 20h7M28 20h7M48 20h7"/>`,
  memory: `<rect class="fr" x="2" y="6" width="26" height="24" rx="3"/><rect class="w" x="5" y="9" width="9" height="18" rx="2"/><rect class="w" x="16" y="9" width="9" height="18" rx="2"/><path class="l" d="M32 18h10M38 14l4 4-4 4"/><rect class="fr" x="46" y="6" width="16" height="24" rx="3" stroke-dasharray="3 3"/>`,
  dividers: `${frame}<rect class="w" x="5" y="5" width="30" height="30" rx="2.5"/><rect class="w" x="38" y="5" width="21" height="30" rx="2.5"/><rect class="a" x="34.5" y="14" width="4" height="12" rx="2"/>`,
  gap: `${frame}<rect class="w" x="5" y="5" width="16" height="30" rx="2.5"/><rect class="w" x="24" y="5" width="16" height="30" rx="2.5"/><rect class="w" x="43" y="5" width="16" height="30" rx="2.5"/>`,
  files: `<rect class="fr" x="6" y="5" width="22" height="30" rx="3"/><path class="l" d="M11 14h12M11 19h12M11 24h8"/><path class="l a" d="M33 20h12M41 16l4 4-4 4"/><rect class="fr" x="48" y="9" width="12" height="22" rx="3"/>`,
  tray: `<rect class="fr" x="2" y="6" width="60" height="28" rx="4"/><path class="l" d="M2 27h60"/><rect class="w" x="46" y="29" width="4" height="3" rx="1"/><rect class="a" x="52" y="29" width="4" height="3" rx="1"/><rect class="w" x="8" y="11" width="22" height="12" rx="2.5"/>`,
  theme: `<rect class="fr" x="2" y="4" width="28" height="32" rx="4"/><rect class="w" x="6" y="9" width="20" height="22" rx="2.5"/><rect class="dm" x="34" y="4" width="28" height="32" rx="4"/><rect class="a" x="38" y="9" width="20" height="22" rx="2.5"/>`,

  /* ---- In progress ---- */
  look: `<rect class="fr" x="2" y="4" width="28" height="32" rx="4"/><rect class="w" x="6" y="9" width="20" height="5" rx="2"/><rect class="a" x="6" y="17" width="20" height="14" rx="2.5"/><rect class="dm" x="34" y="4" width="28" height="32" rx="4"/><rect class="w" x="38" y="9" width="20" height="5" rx="2"/><rect class="a" x="38" y="17" width="20" height="14" rx="2.5"/>`,
  switcher: `<rect class="w" x="14" y="4" width="36" height="6" rx="2.5" opacity=".45"/><rect class="w" x="11" y="9" width="42" height="8" rx="2.5" opacity=".7"/><rect class="a" x="8" y="15" width="48" height="21" rx="3"/><rect class="z" x="11" y="19" width="19" height="14" rx="2"/><rect class="z" x="33" y="19" width="20" height="6" rx="2"/><rect class="z" x="33" y="27" width="20" height="6" rx="2"/>`,
  shield: `${frame}<path class="a" d="M32 8l11 4v9c0 7-5 11-11 13-6-2-11-6-11-13v-9z"/><path class="l" d="M27 21l4 4 7-8"/>`,

  /* ---- Wave 1 ---- */
  arrange: `${frame}<rect class="w" x="5" y="5" width="16" height="30" rx="2.5"/><rect class="w" x="23" y="5" width="18" height="30" rx="2.5"/><rect class="z" x="43" y="5" width="16" height="30" rx="2.5"/><path class="l a" d="M51 17v6M48 20h6"/>`,
  presets: `<rect class="fr" x="3" y="3" width="26" height="15" rx="2.5"/><path class="l" d="M16 3v15"/><rect class="fr" x="35" y="3" width="26" height="15" rx="2.5"/><path class="l" d="M43.7 3v15M52.3 3v15"/><rect class="fr" x="3" y="22" width="26" height="15" rx="2.5"/><path class="l" d="M16 22v15M3 29.5h26"/><rect class="fr" x="35" y="22" width="26" height="15" rx="2.5"/><path class="l" d="M50 22v15"/>`,
  stacks: `${frame}<rect class="w" x="5" y="5" width="22" height="30" rx="2.5"/><rect class="z" x="31" y="5" width="28" height="30" rx="2.5"/><rect class="w" x="36" y="17" width="20" height="16" rx="2" opacity=".5"/><rect class="w" x="34.5" y="14" width="22" height="16" rx="2" opacity=".75"/><rect class="a" x="33" y="11" width="24" height="16" rx="2"/><rect class="a" x="33" y="7" width="8" height="3" rx="1"/><rect class="w" x="43" y="7" width="8" height="3" rx="1"/>`,
  swap: `${frame}<rect class="w" x="5" y="8" width="20" height="24" rx="2.5"/><rect class="a" x="39" y="8" width="20" height="24" rx="2.5"/><path class="l" d="M28 16h8M33 13l3 3-3 3M36 25h-8M31 22l-3 3 3 3"/>`,
  rules: `${frame}<rect class="w" x="5" y="5" width="26" height="30" rx="2.5"/><path class="l" d="M10 12h12M10 17h8"/><rect class="z" x="34" y="5" width="25" height="30" rx="2.5"/><rect class="a" x="38" y="15" width="17" height="13" rx="2"/>`,

  /* ---- Wave 2 ---- */
  rebind: `<rect class="k" x="6" y="9" width="22" height="22" rx="4"/><path class="l" d="M12 20h10"/><rect class="k a" x="36" y="9" width="22" height="22" rx="4"/><circle class="a-fill" cx="47" cy="20" r="3"/>`,
  layoutkeys: `<rect class="k a" x="8" y="7" width="48" height="26" rx="5"/><text class="t" x="32" y="25" text-anchor="middle">F1 &#8211; F9</text>`,
  cheat: `<rect class="fr" x="8" y="3" width="48" height="34" rx="4"/><rect class="k" x="12" y="8" width="10" height="6" rx="2"/><path class="l" d="M26 11h26"/><rect class="k" x="12" y="17" width="10" height="6" rx="2"/><path class="l" d="M26 20h20"/><rect class="k" x="12" y="26" width="10" height="6" rx="2"/><path class="l" d="M26 29h24"/>`,
  pause: `${frame}<rect class="a" x="24" y="11" width="5" height="18" rx="1.5"/><rect class="a" x="35" y="11" width="5" height="18" rx="1.5"/>`,
  panel: `${frame}<rect class="w" x="5" y="5" width="28" height="30" rx="2.5" opacity=".5"/><rect class="fr panel" x="37" y="8" width="22" height="27" rx="3"/><path class="l" d="M41 15h14M41 21h10M41 27h12"/>`,

  /* ---- Wave 3 ---- */
  workspaces: `<rect class="fr" x="2" y="7" width="24" height="26" rx="3"/><rect class="w" x="5" y="10" width="8" height="20" rx="2"/><rect class="w" x="15" y="10" width="8" height="20" rx="2"/><path class="l a" d="M29 20h6M33 17l3 3-3 3"/><rect class="fr" x="38" y="7" width="24" height="26" rx="3"/><rect class="a" x="41" y="10" width="8" height="20" rx="2"/><rect class="a" x="51" y="10" width="8" height="20" rx="2"/>`,
  focus: `${frame}<rect class="dm" x="5" y="5" width="14" height="30" rx="2.5"/><rect class="a" x="21" y="5" width="22" height="30" rx="2.5"/><rect class="dm" x="45" y="5" width="14" height="30" rx="2.5"/>`,
  calm: `${frame}<rect class="w" x="10" y="5" width="44" height="20" rx="2.5"/><rect class="dm" x="8" y="29" width="48" height="6" rx="2"/><rect class="w" x="12" y="30.5" width="8" height="3" rx="1"/><rect class="w" x="23" y="30.5" width="8" height="3" rx="1"/><rect class="w" x="34" y="30.5" width="8" height="3" rx="1"/>`,
  share: `${frame}<rect class="z" x="5" y="5" width="16" height="30" rx="2.5"/><rect class="z" x="23" y="5" width="18" height="30" rx="2.5"/><rect class="z" x="43" y="5" width="16" height="30" rx="2.5"/><path class="a" d="M32 9l8 3v7c0 5-4 8-8 10-4-2-8-5-8-10v-7z"/>`,
  highlight: `${frame}<rect class="w" x="5" y="6" width="24" height="28" rx="2.5"/><rect class="a thick" x="35" y="6" width="24" height="28" rx="2.5"/>`,

  /* ---- Wave 4 ---- */
  reserved: `${frame}<rect class="dm" x="4" y="4" width="6" height="32" rx="2"/><rect class="z" x="13" y="5" width="22" height="30" rx="2.5"/><rect class="z" x="37" y="5" width="22" height="30" rx="2.5"/>`,
  follow: `${frame}<rect class="z" x="5" y="5" width="26" height="30" rx="2.5"/><rect class="a" x="33" y="5" width="26" height="30" rx="2.5"/><path class="cursor" d="M42 16v13l3.3-3.2 2.3 5 2.2-1-2.3-5h4.6z"/>`,
  hover: `${frame}<rect class="w" x="6" y="6" width="52" height="28" rx="3"/><path class="l" d="M6 15h52"/><rect class="a" x="38" y="8.5" width="5" height="4" rx="1"/><rect class="a" x="45" y="8.5" width="5" height="4" rx="1"/><rect class="w" x="52" y="8.5" width="4" height="4" rx="1"/>`,
  languages: `<text class="t big" x="18" y="27" text-anchor="middle">A</text><text class="t big" x="46" y="27" text-anchor="middle">&#25991;</text><path class="l" d="M32 8v24"/>`,
  icons: `<rect class="a" x="3" y="10" width="17" height="20" rx="5"/><path class="l on" d="M11.5 10v20M11.5 20h8.5"/><rect class="w" x="23.500" y="10" width="17" height="20" rx="5"/><path class="l" d="M32 10v20M32 20h8.500"/><rect class="dm" x="44" y="10" width="17" height="20" rx="5"/><path class="l" d="M52.500 10v20M52.500 20h8.500"/>`,
  tour: `<path class="l" d="M10 20h44"/><circle class="a-fill" cx="10" cy="20" r="3.5"/><circle class="w" cx="21" cy="20" r="3.5"/><circle class="w" cx="32" cy="20" r="3.5"/><circle class="w" cx="43" cy="20" r="3.5"/><circle class="w" cx="54" cy="20" r="3.5"/>`,
  updates: `<rect class="fr" x="14" y="7" width="26" height="26" rx="4"/><path class="l a" d="M31 22l15-13M38 9h8v8"/>`,

  /* ---- Krova's own ideas ---- */
  desktops: `<rect class="fr" x="14" y="3" width="48" height="28" rx="4"/><rect class="fr" x="2" y="9" width="48" height="28" rx="4"/><rect class="z" x="5" y="12" width="20" height="22" rx="2.5"/><rect class="a" x="27" y="12" width="20" height="22" rx="2.5"/>`,
  maximize: `${frame}<rect class="z" x="5" y="5" width="14" height="30" rx="2.5"/><rect class="a" x="21" y="5" width="22" height="30" rx="2.5"/><rect class="z" x="45" y="5" width="14" height="30" rx="2.5"/><path class="l" d="M27 13l-3-3M24 10h4M24 10v4M37 27l3 3M40 30h-4M40 30v-4"/>`,
  cli: `${frame}<path class="l a" d="M10 14l7 6-7 6M22 28h14"/>`,

  /* ---- 0.2 beta additions ---- */
  notch: `${frame}<rect class="a" x="24" y="4.5" width="16" height="5" rx="2.5"/><rect class="z" x="5" y="13" width="26" height="22" rx="2.5"/><rect class="z" x="33" y="13" width="26" height="22" rx="2.5"/>`,
  corners: `${frame}<path class="a" d="M62.5 1.5v12a12 12 0 0 1-12-12z"/><rect class="z" x="5" y="12" width="26" height="23" rx="2.5"/><rect class="z" x="33" y="17" width="26" height="18" rx="2.5"/>`,
} as const;

export type GlyphName = keyof typeof glyphs;
