# Handoff — Cockpit design-language overhaul (brainstorming)

_2026-07-07 · branch `docs/local-project-tracker-spec` · session paused mid-brainstorm,
awaiting the user's execution-flavor pick._

## Resume in one line

Re-enter **superpowers:brainstorming**, restart the visual companion (see server section),
and ask the user to pick a flavor (A/B/C) from `flavors.html` — then continue to design
sections → spec → writing-plans.

## What we're doing

Overhauling the visual design language of the **core cockpit UI** (`cockpit/ui/`, the
Tauri + Svelte 5 "FLEET COMMAND" HUD). Started via `/design-sync` but that skill was the
wrong fit (it imports a React/Storybook design system to claude.ai/design; this repo is a
Svelte app). User pivoted to: summarize current design → create a new design language.

Baseline audit written & saved: **`docs/design/current-state.md`** (committed? no — untracked).
Currently inside the **superpowers:brainstorming** skill flow.

## Decisions locked (via clarifying questions)

1. **Direction:** "Refined command center" — keep operator soul (density, semantic tone
   model, 5-step elevation ramp) and **strip the costume** (scanlines, heavy cyan glow,
   ASCII glyphs, Oxanium font).
2. **Palette:** neutral desaturated base + evolved *calmer* cyan accent. **Architect
   layered tokens (primitives → semantic roles → theme overlay) so the theme/accent is a
   single-file swap.** Ship the default theme + ~1 alternate to prove the seam. **No
   in-app theme switcher** (YAGNI).
3. **Scope:** **visual language only** — new tokens/type/color/spacing/chrome + shared
   components + a real icon set, applied to the *existing* screens (topbar, 3-col Fleet,
   Project board, modal). No IA/layout redesign.
4. **Typography:** grotesk UI font (Inter/Geist/Söhne-like) + monospace for data/numbers/
   IDs/logs. **Bundle fonts locally** (current app CDN-loads them — a Tauri offline gap).

## Current step (blocking on user)

Chose the **execution flavor** via the brainstorming **visual companion** (browser mockups).
Pushed one screen `flavors.html` showing the Fleet view in 3 flavors:
- **A · Quiet Instrument** (recommended) — flat, hairline borders, 6px radius, no glow.
- **B · Precision Console** — sharp 2px, faint hairline grid, mono-forward labels; most distinctive.
- **C · Soft Depth** — 10px rounded, soft-shadow elevation, airier; most approachable.

Awaiting the user's pick (or blend, e.g. A's calm + B's grid). Then: propose/confirm full
design in sections → write spec to `docs/superpowers/specs/2026-07-07-cockpit-design-language-design.md`
→ spec self-review → user review → **writing-plans** skill (terminal state of brainstorming).

## Visual companion server (flaky in this VS Code env)

- Start script: `C:\Users\barclay\.claude\plugins\cache\claude-plugins-official\superpowers\6.1.1\skills\brainstorming\scripts\start-server.sh`
- Restart on same port/key with: `bash <script> --project-dir "/d/MajorProjects/CURRENT/command-center"`
  (reuses port **60390**, key `8877fe95…b720686`; the user's open tab reconnects itself).
- It has died once between turns (bg task exit 127). On restart it makes a NEW session dir under
  `.superpowers/brainstorm/<pid>-<ts>/` — **re-copy `flavors.html` into the new `content/` dir**
  or the reconnected tab shows nothing.
- `.superpowers/` should be added to `.gitignore` (not yet done).

## Suggested skills (for the successor)

- **superpowers:brainstorming** — resume here; we are mid-flow at the "present design
  sections → get approval" stage (flavor pick is the last open input). Its terminal state
  is invoking writing-plans; do NOT jump to any implementation skill before design approval.
- **superpowers:writing-plans** — after the design is approved and the spec is written
  (`docs/superpowers/specs/2026-07-07-cockpit-design-language-design.md`).
- **frontend-design:frontend-design** — later, at *implementation* time only (the actual
  re-skin: tokens, type, components). Not before the plan exists.

## Files

- `docs/design/current-state.md` — baseline audit (the full current-design breakdown)
- `cockpit/ui/src/app.css` — tokens + global atmosphere (the thing being replaced)
- `cockpit/ui/src/{App.svelte, views/Dashboard.svelte, lib/Switcher.svelte, lib/ApprovalOverlay.svelte}` — the screens to re-skin
