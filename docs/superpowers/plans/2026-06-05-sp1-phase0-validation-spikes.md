# SP1 Phase 0 — Validation Spikes Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Empirically prove (or disprove) the two load-bearing assumptions the SP1 spec depends on, before any production code is written: (1) a git branch can escape a Linux container to a byte-valid host repo cross-platform, and (2) Claude Code emits parseable per-invocation token/cost usage.

**Architecture:** Two self-contained, throwaway spikes driven by the `docker` and `claude` CLIs (no Rust yet — if the mechanism works via CLI, the `bollard`-based daemon can reproduce it). Each spike ends with a recorded PASS/FAIL verdict written into a results doc that feeds the Phase 1 plan.

**Tech Stack:** Docker CLI (Linux containers, WSL2 backend), git 2.45, Claude Code CLI 2.1.x, PowerShell 7 (the user's shell). Commands are given for PowerShell; bash equivalents noted where they differ.

**Why spikes, not TDD:** these are experiments to discover unknowns, not features. Each step is "run command → observe expected → record verdict." The reusable output is the proven command sequence (which becomes `export_bundle` and the metering parser in Phase 1/2) plus a decision record.

---

## Parent spec

[../specs/2026-06-05-command-center-sp1-design.md](../specs/2026-06-05-command-center-sp1-design.md) — see "Validation spikes required BEFORE building SP1".

## Prerequisites (verified 2026-06-05 on the target machine)

- git 2.45.2.windows.1 ✓
- Docker 28.3.3, Linux containers (WSL2) ✓
- Rust 1.93.1 ✓ (not used in Phase 0, used Phase 1+)
- Claude Code 2.1.163 ✓
- A scratch directory for spike artifacts: `d:\MajorProjects\CURRENT\command-center\spikes\` (git-ignored).

- [ ] **Step 0: Create the git-ignored scratch dir**

Run (PowerShell):
```powershell
New-Item -ItemType Directory -Force d:\MajorProjects\CURRENT\command-center\spikes\artifacts | Out-Null
Add-Content -Path d:\MajorProjects\CURRENT\command-center\.gitignore -Value "`n/spikes/artifacts/`n/target/`n"
```
Expected: `spikes\artifacts\` exists; `.gitignore` contains `/spikes/artifacts/` and `/target/`.

---

## Spike 1 — Cross-platform git escape round-trip

**Hypothesis:** An agent committing inside a Linux container, with the clone in a **named Docker volume** (never an NTFS bind-mount of `.git`), can have its branch exported as a **complete, self-contained bundle** (`git bundle ... --branches` / explicit ref; `git bundle verify` reports no prerequisites), pulled to the Windows host via a temp-container `docker cp`, fetched into a host clone, and `git fsck`'d clean.

**Pinned image:** use `alpine/git:latest` for the spike (git preinstalled, tiny). The production pinned-by-digest image is a Phase 2 concern.

- [ ] **Step 1: Create the named volume**

Run:
```powershell
docker volume create cc_spike_vol
```
Expected: prints `cc_spike_vol`.

- [ ] **Step 2: Initialize a repo with a base + feature branch INSIDE the volume**

This simulates: daemon-provisioned clone, then the agent makes a feature commit on `agent/<id>`. We set the cross-platform-safety git config the production code will set (`core.autocrlf=false`, `core.fileMode=false`, `core.symlinks=false`).

Run (single container, shell script via `sh -c`):
```powershell
docker run --rm -v cc_spike_vol:/work alpine/git sh -c "
  set -e
  cd /work
  git init -q repo
  cd repo
  git config core.autocrlf false
  git config core.fileMode false
  git config core.symlinks false
  git config user.email spike@local
  git config user.name Spike
  echo 'base' > README.md
  git add README.md
  git commit -q -m 'base commit'
  git checkout -q -b agent/spike
  printf 'feature line\nsecond line\n' > feature.txt
  git add feature.txt
  git commit -q -m 'feat: agent work'
  git log --oneline
"
```
Expected: two log lines (`feat: agent work`, `base commit`), no errors. If git throws `index.lock` / fileMode / autocrlf errors here, that is a **FAIL signal** (record it).

- [ ] **Step 3: Create a complete, self-contained bundle inside the volume and verify it has no prerequisites**

Run:
```powershell
docker run --rm -v cc_spike_vol:/work alpine/git sh -c "
  set -e
  cd /work/repo
  git bundle create /work/out.bundle agent/spike
  echo '--- verify ---'
  git bundle verify /work/out.bundle
"
```
Expected: `git bundle verify` prints `The bundle records a complete history.` and lists `agent/spike` as a ref it provides, with **no "requires these commits" / prerequisite lines**. Prerequisite lines = **FAIL** (the bundle is incremental; the spec forbids that).

- [ ] **Step 4: Export the bundle to the Windows host via a temp container `docker cp` (this is the future `export_bundle`)**

Run:
```powershell
docker create --name cc_spike_tmp -v cc_spike_vol:/work alpine/git
docker cp cc_spike_tmp:/work/out.bundle d:\MajorProjects\CURRENT\command-center\spikes\artifacts\out.bundle
docker rm cc_spike_tmp | Out-Null
Get-Item d:\MajorProjects\CURRENT\command-center\spikes\artifacts\out.bundle | Select-Object Length
```
Expected: `out.bundle` exists on the host with non-zero `Length`.

- [ ] **Step 5: Fetch the bundle into a fresh HOST clone and fsck it (this is the daemon's host clone)**

Run (host git on NTFS — normal git, the operation the daemon does):
```powershell
cd d:\MajorProjects\CURRENT\command-center\spikes\artifacts
git clone --bare out.bundle host_clone.git
cd host_clone.git
git fsck --full
git log --oneline agent/spike
git rev-parse agent/spike
```
Expected: `git fsck --full` reports **no errors/dangling-corruption** (dangling commits from a bare clone are fine; corruption is not); `git log` shows both commits; `git rev-parse` prints a SHA.

- [ ] **Step 6: Confirm the host SHA equals the in-container SHA (byte-identical history)**

Run:
```powershell
docker run --rm -v cc_spike_vol:/work alpine/git sh -c "cd /work/repo && git rev-parse agent/spike"
# compare with the host value from Step 5
git -C d:\MajorProjects\CURRENT\command-center\spikes\artifacts\host_clone.git rev-parse agent/spike
```
Expected: the two SHAs are **identical**. Mismatch = FAIL (history altered in transit).

- [ ] **Step 7: Teardown spike 1 resources**

Run:
```powershell
docker volume rm cc_spike_vol | Out-Null
Remove-Item -Recurse -Force d:\MajorProjects\CURRENT\command-center\spikes\artifacts\host_clone.git, d:\MajorProjects\CURRENT\command-center\spikes\artifacts\out.bundle -ErrorAction SilentlyContinue
```
Expected: no errors.

- [ ] **Step 8: Record the Spike 1 verdict**

Write the result (PASS/FAIL + any error output + the SHAs) into `spikes/SPIKE-RESULTS.md` under a `## Spike 1 — git escape` heading. If FAIL, capture the exact error and stop — Phase 1/2 PR mechanics depend on this; we'd need to redesign the escape (fallback options: `git daemon` over `host.docker.internal`, or a host-side bind of only the bundle output dir).

---

## Spike 2 — Cost / token metering from Claude Code

**Hypothesis:** `claude` in non-interactive mode emits machine-parseable per-invocation token usage (and ideally a cost figure) that the daemon can sum across many `exec` calls, so the USD/token caps are enforceable.

**Note:** exact flags/output schema may have changed; discovering them IS the spike. If a flag below errors, run `claude --help` and adapt, recording what actually worked.

- [ ] **Step 1: Capture a single non-interactive run as stream-json**

Run (PowerShell; `2>&1` keeps stderr with stdout for inspection):
```powershell
claude -p "Reply with exactly the word: OK" --output-format stream-json --max-turns 1 *>&1 |
  Tee-Object d:\MajorProjects\CURRENT\command-center\spikes\artifacts\meter1.jsonl
```
Expected: a stream of JSON objects (one per line). If `--output-format stream-json` is rejected, try `--output-format json`; if `-p` is rejected, try `--print`. Record the flag set that works.

- [ ] **Step 2: Identify the usage/cost fields in the output**

Run:
```powershell
Get-Content d:\MajorProjects\CURRENT\command-center\spikes\artifacts\meter1.jsonl |
  Select-String -Pattern 'usage|tokens|cost|input_tokens|output_tokens|total_cost' 
```
Expected: at least one line containing token counts (look for keys like `input_tokens`, `output_tokens`, `cache_*`, and possibly `total_cost_usd`). **Record the exact JSON path** to: input tokens, output tokens, and cost-if-present. This path becomes the Phase 2 metering parser.

- [ ] **Step 3: Confirm a terminal `result` record carries cumulative usage**

Run:
```powershell
Get-Content d:\MajorProjects\CURRENT\command-center\spikes\artifacts\meter1.jsonl |
  Select-Object -Last 1 |
  ForEach-Object { $_ | ConvertFrom-Json | ConvertTo-Json -Depth 8 }
```
Expected: the final object is a `type: "result"` (or similar) record containing total `usage` and possibly `total_cost_usd` / `duration_ms`. Record its shape.

- [ ] **Step 4: Prove summation across two invocations**

Run a second invocation and confirm two independent usage records can be summed (the daemon issues many `exec` calls per unit):
```powershell
claude -p "Reply with exactly the word: DONE" --output-format stream-json --max-turns 1 *>&1 |
  Tee-Object d:\MajorProjects\CURRENT\command-center\spikes\artifacts\meter2.jsonl
# eyeball both result records' token totals; they should be independent positive integers
```
Expected: `meter2.jsonl` has its own `result` usage with positive token counts, summable with `meter1`.

- [ ] **Step 5: Decide the cap primitive and record the verdict**

Decision rule (write into `SPIKE-RESULTS.md` under `## Spike 2 — metering`):
- If a reliable cost figure (`total_cost_usd`) IS present per run → **USD cap is enforceable** (sum across runs); token cap is the daemon-independent backstop.
- If only token counts are present → **token cap becomes the primary cap**; USD is computed best-effort from a price table (record which models appear and that a price table is needed), and `--max-turns` is the in-container watchdog backstop.
- If neither is parseable → **FAIL**; the daemon must fall back to `--max-turns` + wall-clock only as hard caps, and the spec's cost section must be downgraded. Record this.

- [ ] **Step 6: Teardown spike 2 artifacts**

Run:
```powershell
Remove-Item d:\MajorProjects\CURRENT\command-center\spikes\artifacts\meter1.jsonl, d:\MajorProjects\CURRENT\command-center\spikes\artifacts\meter2.jsonl -ErrorAction SilentlyContinue
```

---

## Finalize Phase 0

- [ ] **Step 1: Ensure `spikes/SPIKE-RESULTS.md` records both verdicts**

The file must contain: `## Spike 1 — git escape` (PASS/FAIL, both SHAs, the working `git bundle`/`docker cp` command sequence) and `## Spike 2 — metering` (PASS/FAIL, the exact JSON path to tokens/cost, the chosen cap primitive). This file is the input to the Phase 1 plan.

- [ ] **Step 2: Reconcile the spec with reality**

If either spike's result differs from the spec's assumption, update
[../specs/2026-06-05-command-center-sp1-design.md](../specs/2026-06-05-command-center-sp1-design.md):
- Spike 1 FAIL → revise Section 5 (PR mechanics) with the chosen fallback escape.
- Spike 2 not-USD → revise Section 4 caps table to make token/`--max-turns` the primary cap.

- [ ] **Step 3: Commit Phase 0 (only when the user approves a commit)**

Per the user's git rules: branch first, never commit to main without asking. When approved:
```powershell
git checkout -b feat/sp1-phase0-spikes
git add docs/ spikes/SPIKE-RESULTS.md .gitignore
git commit -m "docs(sp1): record validation spike results and reconcile spec"
```
(`spikes/artifacts/` stays ignored.)

---

## Roadmap for Phases 1–3 (detailed AFTER Phase 0 results land)

These are deliberately not yet bite-sized — the spike outcomes change their details (cap primitive, escape mechanism). Each phase produces working, testable software.

- **Phase 1 — `fleetd` core (no Docker).** Cargo workspace; `Runner` trait
  (`provision`/`exec`(streaming,cancellable)/`health`/`export_bundle`/`teardown`);
  `FakeRunner`; the full state machine (incl. `SPEC`, `AWAITING_ORACLE_APPROVAL`,
  `MERGE_CHECK`, `NO_CHANGE`, re-entry edges); the evidence-based gate + oracle-tampering
  guard; caps (using the Spike-2-chosen primitive); the event ring-buffer + WS `/stream`;
  REST control + oracle approval endpoints. **Fully TDD against `FakeRunner` — no Docker,
  no Claude.** Deliverable: a daemon whose entire decision logic is unit-tested.
- **Phase 2 — `LocalDockerRunner` + real pipeline.** Implement the trait with `bollard`
  using the Spike-1 command sequence; pinned image by digest; label-based reconciliation +
  in-container watchdog; the oracle agent / builder / reviewer (`code-review` skill) `exec`
  wiring; PR push + async `mergeable` polling. Deliverable: one real unit dispatch → PR,
  end-to-end smoke test.
- **Phase 3 — Cockpit (Tauri + Svelte).** Sidecar launch + health-check of `fleetd`;
  new-mission form (tier selector); one unit card (phase, log, meters, findings);
  oracle-approval panel (T2/T3); halt/resume/abandon; PR link. Deliverable: the walking
  skeleton, visible.
