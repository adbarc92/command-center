# What I need from you — human-gated spikes & tasks (2026-06-24)

> ⚠️ **SUPERSEDED (2026-06-25)** — status here is stale (it predates the committed P3 fix and the
> committed P4 harness). Use **[`2026-06-25-spikes-handoff.md`](2026-06-25-spikes-handoff.md)**, the
> current human-gated master doc (P3/P4 + S3 + Certs). Kept for history.

> **Why this doc exists.** The Command Center is ~85% to practical daily use. The
> *engineering* backlog is cleared — what remains can only be done by **you**, because each
> needs your eyes on a live GUI, a real credential + token spend, or out-of-repo
> procurement. This is the single checklist of those tasks, with copy-paste runbooks and
> exact pass criteria, so each one is a short watched session and not a re-derivation.
>
> Source of truth for status: [`docs/ROADMAP.md` → "⚠️ Requires your attention"](../ROADMAP.md).
> Readiness context: [`2026-06-23-practical-usage-readiness.md`](2026-06-23-practical-usage-readiness.md).

## The four tasks at a glance

| # | Task | Kind | Your effort | Blocks | Unblocks |
|---|---|---|---|---|---|
| **P3** | App-plugin **webview spike** (gates 2–5) | 🔴 watched GUI | ~30–45 min | — (fix applied, needs your eyes) | App-plugin **embedding** swarm (~2 days) |
| **P4** | View-plugin **handshake spike** (100 reloads) | 🟠 watched run (dev + packaged) | ~1–2 hr (incl. scaffold) | not yet scaffolded | View-plugin **runtime** swarm (~3 days) |
| **S3** | One **live paid T1 mission** | 🟠 credential + spend | ~1 watched hour + a few $ | needs `ANTHROPIC_API_KEY` | Confidence in the autonomous spine |
| **Certs** | **Code-signing certs** | 🟣 procurement | ~1 wk lead, then ~30 min wiring | money + identity verification | The signed cross-platform release |

**Recommended order:** P3 first (fix is in, largest downstream unlock) → P4 → then S3 and
Certs in parallel whenever (Certs has a ~1 week procurement lead, so kick it off early if
you want the signed release sooner).

---

## P3 — App-plugin webview spike  🔴 (fix applied; needs your visual go/no-go)

**What it proves:** that a Rust-positioned **child webview** can render a whole web app
(Audience) under a Svelte-owned screen rect inside the cockpit window — the riskiest
dependency of the app-plugin "overlay B" embedding approach.

**Status:** I fixed the `spike_show` **hang this session** — the three spike commands were
synchronous Tauri commands deadlocking the main thread on webview creation; they're now
`async fn` (verified `cargo check` = clean). **What's left is the part only you can do:**
look at the window and judge whether it renders/resizes/hides correctly.

**Where it lives:** the spike is a **throwaway worktree**, not `main`:
`.claude/worktrees/agent-a709aaf1bcad07d41` on branch `spike/app-plugins-webview-v2`.

### Runbook

```bash
# 1) Audience web on :3000 (dev posture, deterministic fake providers)
#    If docker infra is down first: cd /d/MajorProjects/CURRENT/audience && docker compose up -d
cd /d/MajorProjects/CURRENT/audience
AI_PROVIDER=fake MEDIA_PROVIDER=fake pnpm --filter @audience/web dev     # http://localhost:3000
#    /compose renders 200 (the spike points here). /dashboard, /queue, /calendar need the api service.

# 2) Cockpit harness — the WORKTREE, not the main checkout
cd "/d/MajorProjects/CURRENT/command-center/.claude/worktrees/agent-a709aaf1bcad07d41/cockpit/ui"
npm run sidecar          # builds the fleetd-serve sidecar (required before tauri)
npm run tauri dev        # ~1.5 min cached; opens the GUI; fleetd on :8787
```

In the GUI, click **"SPIKE: show webview"** and watch the `tauri dev` **stderr**.

### Pass criteria (record each)

| Gate | What to look for |
|---|---|
| **Hang gone** | stderr now prints `[SPIKE] add_child RETURNED ok` (it never did before the fix). |
| **Gate 2 — renders** | Audience `/compose` paints over the purple placeholder rect. |
| **Gate 3 — real origin** | It's the real `localhost:3000` page (forms/scroll interactive), not a stub. |
| **Gate 4 — resize ≤150ms** | Drag/resize the host window; the child rect tracks the Svelte `<div>` within ~150ms (the `ResizeObserver` streams `spike_set_rect`). |
| **Hide-on-overlay** | "SPIKE: hide" hides it cleanly, no flash/orphan paint. |
| **Gate 5 — no orphans** | Quit the app, then `docker ps -a`. Ignore the baseline `audience-*` **Exited** containers (pre-existing from a prior full-stack run) — you're checking the spike left nothing new behind. |

**If `add_child RETURNED ok` prints but nothing renders:** that's the secondary **z-order**
hypothesis (child webview behind the opaque main webview). Ping me — I'll take that branch
(`set_focus`/raise, or a transparent slot region).

**Record the decision** (go/no-go + the exact webview API used) to
`spikes/SPIKE-RESULTS-app-plugins.md`. A **go** unblocks the app-plugin embedding swarm
(`app-plugins-design.md` §6, ~2 days, pre-carved).

> The fix is currently **uncommitted** in the worktree. Tell me if you want it committed to
> `spike/app-plugins-webview-v2` before you run.

---

## P4 — View-plugin handshake spike  🟠 (harness not yet scaffolded)

**What it proves:** the make-or-break unknown for the **sandboxed view-plugin runtime** —
that an untrusted `<iframe sandbox="allow-scripts">` (opaque `"null"` origin) can complete
the **`plugin-hello → init` MessagePort handshake** reliably, load its own code under a
strict CSP, and do so in **both** dev and packaged builds. Full design (read before running):
[`docs/superpowers/specs/2026-06-07-view-plugins-design.md`](../superpowers/specs/2026-06-07-view-plugins-design.md)
§"Sandbox, CSP, loader & the Tauri reality".

**Why only you:** it needs a **watched run across dev and packaged** — the handshake timing
race (gate b) only shows up live, and "packaged" means a real `tauri build` artifact.

**Status:** ✅ **harness scaffolded** (branch `spike/view-plugins-handshake`) — `cargo check`
clean, `npm run check` 0/0, `npm test` 72/72. The `ccplugin://` scheme handler + embedded
plugin + 100-reload host driver are wired; the ops grid is untouched (a `⌬ VP SPIKE` button
bottom-right opens the harness). All that's left is **your watched run**.

### Runbook

```bash
cd cockpit/ui
# DEV — click "⌬ VP SPIKE" (bottom-right) → "▶ RUN 100-RELOAD HANDSHAKE", watch the scoreboard
npm run desktop
# PACKAGED — same buttons, but in an installed bundle (must also pass)
npm run bundle      # then install the MSI/NSIS from src-tauri/target/release/bundle/
```

The harness shows a live gate scoreboard (a/b/c-CORS/c-CSP) and a log. Gate (d) — the
host-app CSP / HMR check — is a documented manual swap; both the gate criteria and the
fill-in results table now live in [`spikes/SPIKE-RESULTS.md`](../../spikes/SPIKE-RESULTS.md)
(the "P4 — View-plugin handshake spike" section).

### The separated go/no-go gates (record each, dev AND packaged)

| Gate | Pass condition |
|---|---|
| **(a) renders** | The sandboxed iframe served from the `ccplugin://` scheme renders. |
| **(b) handshake** | `plugin-hello → init` MessagePort round-trip succeeds across **100 reloads, zero dropped handshakes** (this is the headline number — it catches the timing race). |
| **(c-CORS)** | A `<script type=module>` imports a 2nd file from the same scheme **OR** the single inlined bundle (fallback #1) runs. |
| **(c-CSP)** | The plugin-doc CSP (`default-src 'none'; script-src 'self'; connect-src 'none'; …`) permits self-scripts and **blocks network**. |
| **(d) host CSP** | The host app CSP is authored without breaking Vite HMR. |

**Pre-committed fallbacks** (use if a gate fails, don't redesign): **#1** single inlined
bundle if cross-origin module load fails; **#2** loopback static server on a random port
serving only plugin assets (iframe still sandboxed) — ⚠️ that loopback **must emit the same
plugin-doc CSP header** (`connect-src 'none'`), or the "no network" guarantee dissolves.

**Record** to `spikes/SPIKE-RESULTS.md` (same file as the SP1 Phase-0 results). A **go**
unblocks the view-plugin runtime swarm (+ the `feat/view-plugins` de-stale pre-step; that
branch already exists).

---

## S3 — One live paid T1 mission  🟠 (real credential + real spend)

**What it proves:** the end-to-end autonomous spine on **real tokens** — oracle → build →
review → PR — on a throwaway repo. It's the last unproven slice of SP1; everything else runs
green on synthetic fixtures. **This is a validation run, not a build gate.**

**Why only you:** needs a real `ANTHROPIC_API_KEY`, real token spend (a few dollars), and a
live human watching the run.

### Runbook (sketch — confirm exact dispatch flags against `fleetd --help`)

```bash
# 1) Provide the key (PowerShell):  $env:ANTHROPIC_API_KEY = "sk-ant-…"
# 2) Point at a THROWAWAY repo (so a real PR is harmless).
# 3) Dispatch a T1 mission with the dollar ceiling on (the cost cap is enforceable —
#    --max-budget-usd is a hard ceiling that holds even if fleetd dies; see SPIKE-RESULTS.md §2).
#    Watch: oracle test-set → build loop → review gate → PR creation, with the live $ counter.
```

**Watch for:** the `total_cost_usd` accounting matches the cap; the review gate actually
gates; a real PR lands on the throwaway repo. **Record** the run + final cost. No file to
update is mandated — it's confidence, not a gate — but note the outcome in the next handoff.

> Practical note: `--max-turns` is **not** a flag in this Claude Code version — the budget
> control is `--max-budget-usd` + a wall-clock `timeout` watchdog.

---

## Certs — Code-signing certificates  🟣 (procurement, out of repo)

**What it unblocks:** the **signed cross-platform release run**. CI wiring + the exact secret
names are **already done** ([`release.yml`] consumes them by name); nothing in the repo
blocks. Full reference: [`docs/release/signing-and-updates.md`](../release/signing-and-updates.md) §4.

**Why only you:** buying certs requires money + identity verification (Apple Developer
Program, a CA) — ~1 week procurement lead. Wiring them after is ~30 min.

### Shopping list

| Platform | Obtain | Cost / lead |
|---|---|---|
| **macOS** | Apple Developer Program membership → **Developer ID Application** cert (export as password-protected `.p12`) + Team ID + an app-specific password for notarization. | $99/yr; instant–days |
| **Windows** | An Authenticode **code-signing cert** from a CA (DigiCert/Sectigo/…) as a password-protected `.pfx`. EV gives best SmartScreen reputation; OV also works. | ~$varies; **days–1 wk** (identity vetting) |
| **Updater keypair** | Generate locally — no purchase: `cd cockpit/ui && npm run tauri signer generate -- -w ~/.tauri/cc-updater.key`. Put the printed **public** key in `tauri.conf.json → plugins.updater.pubkey`; keep the private half secret. | free; minutes |

### Then set these CI secrets (canonical names — `${{ secrets.<NAME> }}`)

- **macOS:** `APPLE_CERTIFICATE` (base64 `.p12`), `APPLE_CERTIFICATE_PASSWORD`, `APPLE_ID`,
  `APPLE_PASSWORD` (app-specific), `APPLE_TEAM_ID`.
- **Windows:** `WINDOWS_CERTIFICATE` (base64 `.pfx`), `WINDOWS_CERTIFICATE_PASSWORD`.
- **Updater (all platforms):** `TAURI_SIGNING_PRIVATE_KEY`, `TAURI_SIGNING_PRIVATE_KEY_PASSWORD`.

`base64 -i cert.p12` / `base64 -i cert.pfx` to produce the base64 values. The matching
**public** updater key is not a secret — it lives in `tauri.conf.json`. Local unsigned
`npm run tauri build` keeps working with no certs present (all identities are `null`
placeholders), so none of this blocks development.

---

## One more, not a spike — the CI billing unblock (5 min, highest leverage)

GitHub Actions is **red on every branch** due to a **billing failure** (no runner is
allocated — jobs die in ~3s, `runner=0`, `steps=0`). This is **confirmed unrelated to code**
(`cargo test --workspace` and the Tauri build both pass locally). Until it's fixed, every
PR's checks stay red and merges need `--admin`.

**Fix:** GitHub → **Settings → Billing & plans** (clear the failed payment / raise the
spending limit), then `gh run rerun <id>`. This is the cheapest restore of honest CI signal
for everything above.

---

## Quick reference — verify commands

- **P3 worktree compiles:** `cargo check` in
  `.claude/worktrees/agent-a709aaf1bcad07d41/cockpit/ui/src-tauri` (already green this session).
- **Rust gate (main):** `cargo test --workspace` (104 green / 3 ignored Docker ITs).
- **Cockpit build:** `cd cockpit/ui && npm ci && npm run sidecar && npm run tauri build`.
- **Cockpit dev:** `cd cockpit/ui && npm run desktop` (sidecar + `tauri dev`, fleetd on :8787).
