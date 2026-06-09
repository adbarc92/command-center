# Lane D — Project-dashboard spec (design only)

> Paste this entire file as the prompt for a single agent. It is self-contained. Roadmap item: **4**
> (Central Project Manager). **Design only — no build.**

## Your worktree (set up first)

```bash
git worktree add .claude/worktrees/feat+dashboard-spec -b feat/dashboard-spec main
cd .claude/worktrees/feat+dashboard-spec
```

(If your harness creates the worktree for you, just confirm you are on `feat/dashboard-spec`, not `main`.)

## Goal

Produce an **approved design spec** for the Central Project Manager — a project-tracking dashboard
that tells, at a glance, **what stage every project is in**. This lane writes a spec; it builds no code.

## Owns (exclusive write)

- `docs/superpowers/specs/2026-06-09-project-dashboard-design.md`
  (the carve named a `2026-06-08-…` placeholder; use **today's date** in the filename.)

## Reads (no write)

- The **Halyard digest**: [`docs/digests/halyard-digest.md`](../../digests/halyard-digest.md) —
  Halyard is a git-backed JSON store over project/work state, the natural backend for "what stage is
  this project in."
- The **Audience digest**: [`docs/digests/audience-digest.md`](../../digests/audience-digest.md) —
  Audience contributes its own status.
- The **app-plugins spec**:
  [`docs/superpowers/specs/2026-06-07-app-plugins-design.md`](../../superpowers/specs/2026-06-07-app-plugins-design.md)
  — for the lifecycle-state signals (`building→…→healthy`) and the Halyard-head notes to aggregate.
- [`docs/ROADMAP.md`](../../ROADMAP.md) §4.

## Shared contract

- **None.** This is a pure design lane — it touches no code and no shared config. Cleanly independent
  of all other lanes.

## Resolve these open questions in the spec (no `TBD`s)

- **What *is* a "stage"** — a fixed pipeline (`spec→plan→build→review→ship`) or per-project?
- **Inferred or declared** — is stage derived from git/commits/CI, or explicitly set?
- **How do Halyard and Audience feed it** — and does this justify graduating **Halyard from headless**
  (giving it a "head")? Fleet mission phases + app-plugin lifecycle states are additional signals.

## Done when

- The spec resolves all three open questions, defines the data model + sources, and **passes the
  design-critique gate (3 rounds)** — the same bar the app-plugins spec met.
- No `TBD`s remain.

## Verify

- The spec file exists at the path above, contains a **Design Critique Log of three rounds**, and has
  zero `TBD`s. (`grep -ci TBD` on the file returns 0; the critique log shows 3 rounds.)

## Notes / open questions

- **Use the `brainstorming` skill** before writing — this item is explicitly "big enough to need its
  own brainstorm → spec cycle."
- Expect the **design-critique gate hook** to fire on spec creation; satisfy it (3 rounds).
- This spec **blocks the future dashboard build (4-build)** and informs the Halyard-head decision —
  flag those downstream dependencies in the spec.

---

## Rules of the Road (follow exactly)

1. **Stay in your lane.** Write only the spec file under **Owns**. Touch no code, no shared config.
2. **Worktree per lane.** Work on `feat/dashboard-spec`; never commit to `main`.
3. **Global/shared files are append-only + single-owner.** You own none — and need none.
4. **Don't widen scope.** Design only — do **not** start building the dashboard. Build ideas → record in the spec.
5. **Verify before done.** Confirm the 3-round critique log + zero `TBD`s; paste the `grep` output.
6. **Report for integration.** End with: the spec path; a one-paragraph summary of the resolved
   questions; your verify output; the downstream deps (4-build, Halyard head).
