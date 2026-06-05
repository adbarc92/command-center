# Command Center — Vision & Decomposition

> Status: **living doc**. Captures project-level decisions that span all sub-projects.
> Last updated: 2026-06-05

## What we're building

A combined **command center** for creating and managing software projects: a place to
run and supervise Claude agents, track project progress and issues, facilitate
deployment, and host other UIs (e.g. a job-seeker app, a content-generation app) as
plugins. Delivered as a **lightweight desktop app**.

## The framing trap (named up front)

The original ask is five genuinely independent subsystems wearing one coat:

1. **Project scaffolding** — spinning up new projects
2. **Claude agent management** — running/monitoring agents
3. **Tracking** — progress + issues
4. **Deployment** — shipping
5. **Plugin host** — embedding the job-seeker and content-gen apps

…plus one cross-cutting constraint: **VERY lightweight desktop app.**

Two tensions drove the early decisions:

- **"Very lightweight" vs. "host arbitrary other UIs."** Electron embeds full web apps
  easily but is heavy (~150MB+, a Chromium per window). Tauri is light (~10MB, native
  webview) but makes hosting arbitrary third-party UIs harder. You can't max both knobs.
- **4 of the 5 subsystems already exist** as tools the user uses (Claude Code, GitHub,
  Vercel/Railway dashboards). So the real question isn't "can we build all this" — it's
  **"what's the spine that makes this worth building instead of alt-tabbing between five
  tools you already have?"**

## North star

**Useful autonomy** — autonomy that produces a useful, mergeable result, not autonomy
for its own sake. This resolves the autonomy ladder cleanly: the top tier stays
human-gated not to slow things down but because shipping junk autonomously isn't useful.
This is the thing the system optimizes for.

## Hard constraints

- **Cross-platform desktop:** must run on **Windows, macOS, and Linux** (the user has all
  three). This is why Docker is the chosen agent-isolation primitive — it's the only one
  that runs on all three (Firecracker/gVisor are Linux-only) and, as a bonus, runs inside
  a VM on Win/Mac (free VM-grade host isolation). See the SP1 spec's isolation section.
- **Lightweight:** the cockpit is Tauri (native webview), not Electron.

## Key strategic decisions

### Spine = the agent fleet engine

The heart of the product is **safe autonomous Claude-Code agents that produce mergeable
code**, not the dashboard. The dashboard is the easy part.

### "Own the engine, design for the skin"

We build the daemon + cockpit ourselves so the differentiated pipeline
(containerized `--dangerously-skip-permissions` → git-native isolated-clone → ≥3-round
review gate → PR) is exactly as specced. We do **not** fork
[AgentCraft](https://www.getagentcraft.com/), but we stay **contract-compatible**: the
daemon exposes one observable **fleet-state stream + command API**, and every UI is just
a renderer over that stream. An AgentCraft-style game view can plug in later as another
renderer — and via an adapter we could even mount AgentCraft itself.

#### Why not just use AgentCraft?

AgentCraft (getagentcraft.com, `@idosal/agentcraft` by Ido Salomon) is a Warcraft-3-themed
RTS where agents are heroes on a map. It already orchestrates Claude Code / OpenCode /
Cursor, runs many agents in parallel, and ships remote tunnels + a mobile PWA + push
notifications + Telegram/Discord quick-reply — roughly **60% of the spine**. But its
engine almost certainly does **not** do the user's hard differentiators: containerized
YOLO with isolated-clone→PR, the ≥3-round review gate, or the PRD/test "objectivity
oracle." Those are the safety story and the reason to build. Hence: own the engine.

### View layer is pluggable over a shared fleet-state model (first-class constraint)

The default view is **B · Ops dashboard (fleet grid)** — live tiles with status /
progress / $ / token burn + a detail rail. RTS-*inspired*, dense, scales. The literal
RTS game view ("AgentCraft / StarCraft / Factorio" skin) is an **aspirational alternate
skin**, implemented later as a *view plugin*, not a rewrite. See
[command-surface-mockup.html](./assets/command-surface-mockup.html) for the three
directions considered (A literal RTS map, B ops grid, C pipeline lanes).

## Decomposition — each is its own spec → plan → build cycle

| ID  | Sub-project | Scope | Status |
|-----|-------------|-------|--------|
| **SP1** | **Fleet engine + cockpit + core oracle (the spine)** | Daemon orchestrates a containerized Claude Code agent through dispatch → **oracle phase (generate + freeze tests, tier-gated human approval)** → build/check/review loop → verified-mergeable PR; bare cockpit renders it. Now includes the *core* objectivity oracle. | **designing now** |
| SP2 | Rich onboarding UX | Interactive PRD / grill-me, test-quality scoring on top of SP1's core oracle. (Core oracle moved into SP1.) | later |
| SP3 | Tracking pillar | Progress + issues; the pipeline-lanes (Kanban) view. | later |
| SP4 | Deployment pillar | Shipping projects live. | later |
| SP5 | Plugin system | View plugins (the game skin); app plugins (job-seeker, content-gen); mobile/remote control à la AgentCraft. | later |

## SP1 build order — walking skeleton first

Chosen slice: **walking skeleton** — one unit, end-to-end. Dispatch a task → **oracle phase
generates + freezes a test set (tier-gated human approval)** → one container → isolated
clone → build/check/review loop (reviewer = repo's `code-review` skill) with hard caps →
open a verified-mergeable PR → see it in a bare cockpit. No fleet, no pretty view.

**Scope note (2026-06-05):** the user chose to pull the **objectivity oracle forward into
SP1** — tests are *generated*, not hand-written, kept objective via separation-of-powers
(oracle agent ≠ builder ≠ reviewer), a frozen/builder-immutable test set, and a tier-gated
human approval. SP1 thus subsumes the *core* of the old SP2; SP2 is now just the **rich
onboarding UX** (interactive PRD/grill-me, test-quality scoring). This made the skeleton
bigger but bought the actual soul of "useful autonomy" instead of hollow plumbing.

Rejected alternatives: *fleet-first* (risks polishing a cockpit over an engine that
can't yet safely ship code) and *onboarding-first* (delays the core risk).

The detailed SP1 design lives in
[specs/2026-06-05-command-center-sp1-design.md](./superpowers/specs/2026-06-05-command-center-sp1-design.md).
