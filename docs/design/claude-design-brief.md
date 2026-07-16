# Command Center — Cockpit Redesign Brief (for Claude Design)

> Paste this into Claude Design as the design brief, and point it at this repo during onboarding
> (files to read are listed in §8). Goal: a **full redesign** — a distinctive **design system +
> visual language** for the cockpit, not a reskin of the current one.

## 1. What this product is (the subject)

The **Command Center** is a lightweight **Tauri desktop app** for running and supervising a fleet of
**autonomous Claude-Code agents** that ship real, mergeable code — plus tracking the projects they
work on and hosting other web apps as plugins. Its north star is **useful autonomy**: autonomy that
produces a mergeable result, with humans gating only where it matters.

- **The one job of the UI:** give a single operator *instant situational awareness* of an autonomous
  agent fleet and the projects it's shipping — which missions are moving, which are **blocked on a
  human gate**, which failed — and the controls to intervene, **without alt-tabbing between five
  tools**. It is a *command surface*, not a marketing site or a CRUD dashboard.
- **Audience:** one power-user developer (solo operator) on Windows/macOS/Linux. Lives in this app
  for hours. Values density, glanceability, and speed over hand-holding.
- **Emotional register:** calm command under load. The operator should feel like they're at a
  capable console watching autonomous work happen — in control, never anxious, never toy-like.

## 2. The surfaces to design (design a *system*, exercised on these)

1. **App shell + view switcher** — top-level navigation between the fleet, the Project Board, and
   hosted plugin apps. (`App.svelte`, `Switcher.svelte`)
2. **Project Board** — the flagship view. A dense grid of **project cards**, each showing one
   canonical **stage** (`Idea → Spec → Plan → Build → Review → Ship → Live`, plus `Blocked / Failed
   / Idle`), a detail line, a health/staleness state, and a "needs you" affordance that deep-links to
   act. Cards come from multiple sources (fleet missions, releases, hosted apps, and now **local
   projects read from their own docs**). The board's single most valuable number is **how many
   projects need a human right now**. (`views/Dashboard.svelte`)
3. **Human-gate overlay** — the approval moment: the agent pauses, the operator approves/flips/
   resumes. This is the emotional peak of the product (autonomy handing control back). (`ApprovalOverlay.svelte`)
4. **Hosted plugin app frames** — the chrome around embedded web apps (sandboxed).
5. **States that matter more than the happy path:** blocked, failed, stale/greyed (source
   unreachable), and empty ("no projects / everything idle"). Design these as first-class.

## 3. Current baseline (what we're redesigning *away from* or *refining*)

Today's look is **"Fleet Command — tactical HUD"** (see `app.css`):
- Deep blue-black `#060a0f`; phosphor-cyan primary `#2fe3d6`; amber/green/red/blue/violet status.
- Type: **Oxanium** (display) + **IBM Plex Mono** (data/body).
- Atmosphere: fixed grid background, radial glows, a scanline shimmer overlay, zero border-radius,
  thin "tactical" scrollbars. Dark-only (`color-scheme: dark`).

It's coherent but leans on sci-fi HUD tropes (scanlines, glow). The redesign can **evolve this into
a more refined, intentional operational language**, or **stake out a genuinely different POV** — your
call to propose. Either way it should read as *designed*, not as a stock "hacker terminal" theme.

## 4. What we want from the redesign

A **token-based design system** we can implement directly as CSS custom properties + Svelte
components. Deliver:
- **Color:** 4–6 named core values + a **semantic status set** that is the backbone of this product
  (`active / building`, `blocked-needs-human`, `failed`, `live/healthy`, `idle`, `stale/degraded`).
  Status color is not decoration here — it *is* the information. Ensure the blocked and failed states
  are unmistakable at a glance across a dense grid.
- **Typography:** a deliberate display + data pairing with a real type scale (sizes, weights,
  widths, spacing). Numbers/metrics and short status tokens are everywhere — the data face matters as
  much as the display face.
- **Spacing / density / radii / elevation:** tuned for an *information-dense* console (many cards on
  screen) that still breathes. Define the card as the core component.
- **Motion:** restrained and meaningful — a phase advancing, a "needs you" arriving, a source going
  stale. One signature motion moment, not ambient effects everywhere.
- **A stated visual language:** one paragraph naming the POV/personality and the *signature element*
  the cockpit will be remembered by (the single memorable device, grounded in "supervising an
  autonomous fleet" — not a generic accent).
- **Light + dark** if you think it earns its keep (operator lives here; dark is the safe default, but
  propose if a light mode helps).

## 5. Anti-templated guardrails (important)

Do **not** hand us the current AI-design defaults: (a) warm cream + high-contrast serif + terracotta;
(b) near-black + one acid-green/vermilion accent; (c) broadsheet hairline-rule newspaper columns. If
a choice would look identical on an unrelated product, it's a default, not a decision. Every color
and type choice should be traceable to *this* subject: an operator supervising autonomous agents that
ship code. Take **one real, justifiable aesthetic risk**.

## 6. Hard constraints

- **Tech:** Svelte 5 + Tauri (native WebView2/WKWebView), not a heavy web stack. Implemented as CSS
  custom properties in `app.css` + component styles. No external font CDNs that break offline/packaged
  builds — name fonts we can bundle.
- **Performance:** native webview, must stay light and fast (no heavy runtime CSS-in-JS, minimal
  large images). Effects must not tank a dense grid or a low-power laptop.
- **Cross-platform:** Windows/macOS/Linux desktop.
- **Density is a feature:** many cards + a detail rail on one screen. Don't design for a spacious
  marketing hero; design for a working console.

## 7. Deliverable format that makes implementation clean

Ideally the output gives us: the named token set (with hex + role), the type scale, the component
specs (card, status badge, nav/switcher, overlay, empty/blocked/stale states), and the motion notes.
A downloadable/exportable token list or a codebase-aware component export is ideal — I (the coding
agent) will translate it into `app.css` `:root` variables and the Svelte components.

## 8. Point Claude Design at these files (read for accurate onboarding)

- `cockpit/ui/src/app.css` — the current token system / theme (the baseline to evolve).
- `cockpit/ui/src/App.svelte` — the app shell/layout.
- `cockpit/ui/src/views/Dashboard.svelte` — the Project Board (the flagship surface + card anatomy).
- `cockpit/ui/src/lib/ApprovalOverlay.svelte` — the human-gate overlay.
- `cockpit/ui/src/lib/Switcher.svelte` — the view switcher/nav.
- `docs/command-center-vision.md` — product vision, north star ("useful autonomy"), the "ops
  dashboard (fleet grid)" default view and the RTS-inspired-but-not-a-game intent.
- `docs/ROADMAP.md` — the three pillars (low cost / context hygiene / ship autonomously) and what
  the product optimizes for.

## 9. The round-trip (how we finish)

You design + iterate in Claude Design until the system and a few key screens (board, a blocked card,
the overlay) feel right. Then hand me the token set + component specs (export, screenshots, or a
codebase-aware export) and I'll implement it across the cockpit: map colors/type/space/motion into
`app.css` `:root`, restyle `Dashboard.svelte`/`App.svelte`/`ApprovalOverlay.svelte`/`Switcher.svelte`
to the new component specs, verify in a running `tauri dev`, and keep it on a dedicated
`design/cockpit-overhaul` branch → PR.
