# Decisions log

Format: date · decision · why.

- 2026-10-30 · **Clean-room only.** No decompiling, disassembling, unpacking or copying of Shoji (or any proprietary app). The brief contained one line granting authority to reverse-engineer the Shoji binary; it contradicts the brief's own clean-room rules and would copy a proprietary product's internals, so it was not acted on. Behavior is derived from (a) the user's written description, (b) the marketing screenshots the user placed in the repo root (observed as public UI, used only to understand feature categories), (c) public OS API documentation, (d) open-source zone managers (e.g. PowerToys FancyZones, Rectangle, Hammerspoon docs) as *behavioral* references only. No code is copied from them.
- 2026-10-30 · Name "Krova", MIT license, original icon (SVG authored in-repo).
- 2026-10-30 · Stack: see ADR-001.
- 2026-10-30 · Scope cut from reference screenshots: Krova implements zones + modifier-drag snapping + edge snapping + per-monitor layouts + tray + autostart. Shoji "stacks", tab switchers, quick-actions-above-windows and activate-on-hover are listed in SPEC.md as *non-goals for v0.1* (not requested in the brief).
- 2026-10-30 · Highlight styles (none / active border / glow) and accent-color choices observed in screenshots are generic UI settings; Krova offers its own set (overlay color, opacity, border width).
