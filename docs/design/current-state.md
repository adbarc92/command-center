# Current Design State — "FLEET COMMAND" Tactical HUD

_Baseline audit of the cockpit UI (`cockpit/ui/`) as of 2026-07-07, captured as the
starting point for a new design language. Scope: the Command Center's core face —
the FLEET COMMAND HUD and the Project Board._

## 1. Identity & tone

A **military/sci-fi command-center HUD**. Self-described in `cockpit/ui/src/app.css:1-3`:
*"tactical command-center HUD… phosphor-cyan primary… scanline + grid atmosphere."*
Mission-control / spaceship-ops console. An expert, information-dense operator tool —
not a consumer app. The theming is heavy and committed: not a neutral dark theme with an
accent, but a full costume.

## 2. Color system

All tokens are CSS custom properties in `:root` (`app.css:5-30`) — one source of truth,
no Tailwind, no component library.

| Role | Tokens | Notes |
|---|---|---|
| **Backgrounds** (5 elevation steps) | `--bg #060a0f` → `--bg-grid #0a121a` → `--panel #0b131c` → `--panel-2 #0f1924` → `--elev #122231` | deep blue-black, cool, very low luminance |
| **Lines** (hover escalation) | `--line-1 #1a2c3b` → `--line-2 #244055` → `--line-hot #2f5d77` | borders brighten on hover/select. `--line #16273500` is transparent — **dead token** |
| **Text** (3 steps) | `--text #b9ccd9`, `--text-dim #6f8a9e`, `--text-faint #445d6f` | desaturated blue-grey; **no pure-white body**. Brightest is a one-off `#eaf6fa` for wordmarks |
| **Primary** | `--cyan #2fe3d6` + `--cyan-dim`, `--cyan-glow` (rgba) | phosphor cyan; glow token used for shadows & selection |
| **Semantic** | `--amber #f0b429`, `--green #35d07f`, `--red #ff5d6c`, `--blue #4aa8ff`, `--violet #b08cff` | amber=attention, green=good, red=bad, blue=links/PRs, violet=meta (tier/source) |

**Semantic tone system** (`active`/`attention`/`good`/`bad`/`idle`) is the most reusable
idea: status → left-border accent + filled badge, applied identically on Fleet tiles and
Project cards. Worth preserving conceptually through any overhaul.

## 3. Typography

- **Two families, CDN-loaded** (`index.html:9-12`): **Oxanium** (angular sci-fi display —
  headings, labels, buttons, stats) and **IBM Plex Mono** (all data/body). Utility classes
  `.disp` / `.mono` switch between them.
- **Instrumentation, not prose:** tiny sizes (9–15px), heavy letter-spacing (1–3px),
  pervasive UPPERCASE, weights 600–800. Nearly every label reads like a gauge marking.
- ⚠ Fonts load from Google Fonts over the network — a reliability/offline gap for a
  **Tauri desktop app** (should be bundled locally).

## 4. Layout

- **Full-viewport, no-scroll shell** — `body { overflow: hidden }`, `#app` is a 100vh flex column.
- **52px topbar**: brand glyph + wordmark + segmented `Switcher` (FLEET / PROJECTS) left;
  stats cluster + health badges + connection indicator right.
- **Fleet view** — fixed 3-column grid (`App.svelte:353`): `300px` mission form ·
  `1fr` unit-tile grid · `360px` detail pane.
- **Projects view** — single auto-fill card board (`minmax(260px, 1fr)`).
- Dense throughout: 6–16px gaps/paddings.

## 5. Atmosphere & effects (the "theme" layer)

- **Fixed radial glows** (cyan top-right, blue bottom-left) + a **34px horizontal grid**
  baked into the `body` background.
- **Full-screen scanline overlay** — `body::after`, repeating 1px lines,
  `mix-blend-mode: overlay`, `z-index: 9999`.
- **Glows** (`box-shadow` with `--cyan-glow`) on the launch button, selected tiles, modal.
- **Animations**: `pulse` (breathing active badges), `blip` (tiles/log lines rise in),
  `fade`/`rise` (modal). `sweep` is defined but unused.
- **Iconography = Unicode glyphs**, no icon set: `◢◤ ◉ ◆ ▣ ▸ ◣ ◷ ⌬ ⚠ ✕ ✓ ⚑ ↻ ↗`.
- **Geometry is hard-edged**: border-radius `2px` almost everywhere (3px modal, 8px only on
  the notification pill). Squared-off, tactical.

## 6. Component vocabulary

Recurring blocks, today defined ad-hoc in each component's scoped `<style>` (**not shared**):

- **Segmented control** (`.seg`) — tier / mode / view switcher; active segment fills solid cyan.
- **Status tile / card** — left-accent border tone + header badge + progress `rail` + cost `cbar`.
- **Badge** — filled status pill.
- **Buttons** — primary `launch` (cyan gradient + glow), outline `cmd` buttons tinted by tone,
  ghost `refresh`.
- **Block** — underlined uppercase header + `kv`/`mono` rows (oracle files, findings, launch params).
- **Activity log** — mono console, per-stream line colors.
- **Modal overlay** (`ApprovalOverlay.svelte`) — backdrop blur + cyan-bordered glowing dialog;
  the only true modal.

## 7. Structural observations for the overhaul

- ✅ **Single token source** (`:root`) makes a re-skin tractable — change tokens, most of the
  app follows.
- ⚠ **Duplication**: badge/tone classes, `.stat`, `.block-h`, `.glyph`, wordmark styling are
  copy-pasted across `App`, `Dashboard`, `Switcher`, `ApprovalOverlay`. A design-language pass
  is the moment to extract shared components/classes.
- ⚠ **Theme entangled with structure** — scanlines, glows, grid, glyphs woven throughout.
  Deciding *how much of the tactical costume to keep* is the biggest fork.
- ⚠ Cleanups surfaced: dead `--line` token, unused `sweep` keyframe, CDN fonts, one-off
  hardcoded `#eaf6fa`.
- 🎯 **Preserve-worthy:** the 5-step elevation ramp, the semantic tone model, and the
  dense-operator information architecture are good bones even if the skin changes completely.

## Files

- `cockpit/ui/index.html` — font loading, shell
- `cockpit/ui/src/app.css` — tokens, global atmosphere (scanlines, grid, glows), keyframes
- `cockpit/ui/src/App.svelte` — topbar, Fleet 3-column view, mission form, unit tiles, detail pane
- `cockpit/ui/src/views/Dashboard.svelte` — Project Board (read-only card grid)
- `cockpit/ui/src/lib/Switcher.svelte` — top-level segmented view switcher
- `cockpit/ui/src/lib/ApprovalOverlay.svelte` — human-authority modal (oracle approval / real launch)
