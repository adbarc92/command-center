# SP1 Phase 0 — Validation Spike Results

> Run 2026-06-05 on the target machine (Windows 11, Docker 28.3.3 / Linux-WSL2, git 2.45,
> Claude Code 2.1.163, Rust 1.93). Feeds the Phase 1 plan.
> Plan: [docs/superpowers/plans/2026-06-05-sp1-phase0-validation-spikes.md](../docs/superpowers/plans/2026-06-05-sp1-phase0-validation-spikes.md)

## Spike 1 — cross-platform git escape  → **PASS** ✅

**Claim proven:** an agent committing inside a Linux container, with the clone in a **named
Docker volume** (no NTFS bind-mount of `.git`), can have its branch exported as a complete
self-contained bundle, pulled to the Windows host, and reconstructed byte-identically.

**Evidence:**
- Repo init + 2 commits inside the volume (`alpine/git`, `--entrypoint sh`) with
  `core.autocrlf=false`, `core.fileMode=false`, `core.symlinks=false` — **no `index.lock`,
  fileMode, or autocrlf errors.**
- `git bundle create out.bundle agent/spike` → `git bundle verify`: *"The bundle records a
  complete history."* — **no prerequisites** (not incremental, as required).
- Export via temp container: `docker create … -v cc_spike_vol:/work` → `docker cp
  tmp:/work/out.bundle <host>` → `docker rm`. Bundle landed on host (551 B).
- Host (NTFS) `git clone out.bundle host_clone` → `git fsck --full`: **clean** (only a
  benign "unborn branch main" notice because the bundle carried just `agent/spike`; in
  production the host clone already holds the base and we `fetch` the branch into it).
- **SHA match:** container `81877030b00adfb9e536c08af57498ede8f72478` ==
  host `81877030b00adfb9e536c08af57498ede8f72478`.

**Reusable command sequence → becomes `Runner::export_bundle` (Phase 2):**
1. in-container: `git bundle create /work/out.bundle <branch>` (+ `git bundle verify`)
2. host: `docker create --name <tmp> -v <vol>:/work <img>`; `docker cp <tmp>:/work/out.bundle <host>`; `docker rm <tmp>`
3. host clone: `git fetch <bundle> <branch>` (non-bare clone; avoid `--bare` due to host
   `safe.bareRepository=explicit`).

**Notes for Phase 2:** set the three `core.*` configs at clone time; the daemon's host clone
is **non-bare**; provision the in-volume clone from the base (network is open, so the
container can clone the origin URL directly, or the daemon seeds the volume).

## Spike 2 — Claude Code cost/token metering  → **PASS (best case)** ✅

**Claim proven:** `claude --print --output-format stream-json --verbose` emits a terminal
`result` record with parseable per-invocation **cost AND tokens**; summable across calls.

**Evidence — fields in the `type:"result"` record:**
- `total_cost_usd`: `0.20697875` (top-level, real dollars — the ~$0.20 is Opus-1M
  cache-creation on a trivial prompt; field accuracy is the point)
- `usage.input_tokens` `9457`, `usage.output_tokens` `4`,
  `usage.cache_creation_input_tokens` `25535`, `usage.cache_read_input_tokens` `0`
- `modelUsage["claude-opus-4-8[1m]"].costUSD` `0.20697875` (per-model breakdown)
- `num_turns` `1`, `duration_ms` `6734`

**Cap decision (supersedes the spec's hedge):** USD cost is **directly enforceable**:
- **Daemon-side:** parse `total_cost_usd` from each `exec`'s `result` record; sum across
  the unit's runs → enforce the USD cap. No price table needed (cost is reported).
- **Daemon-independent backstop:** pass **`--max-budget-usd <remaining_cap>`** into the
  in-container agent invocation. This is a built-in CLI dollar ceiling that holds **even if
  `fleetd` dies** — strictly better than the `--max-turns`/token proxy the spec assumed.
- Relevant flags confirmed present: `--print`, `--output-format stream-json`, `--verbose`,
  `--max-budget-usd`, `--dangerously-skip-permissions` (and `--allow-dangerously-skip-permissions`),
  `--model`, `--fallback-model`, `--agents`, `--json-schema`, `--permission-mode`.
- `--max-turns` is **not** a flag in 2.1.163; use `--max-budget-usd` + wall-clock
  (`timeout`) for the watchdog instead.

## Net effect on the design

Both load-bearing assumptions hold. One improvement to fold into the spec: the
daemon-independent cost cap is **`--max-budget-usd`** (a real dollar ceiling), not a token
proxy — so the "honest cost caveat" in Section 4 is largely resolved.
