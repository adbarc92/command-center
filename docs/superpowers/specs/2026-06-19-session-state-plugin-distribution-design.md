# Session-State Plugin Distribution — Design Spec

> Status: design in adversarial-critique (Rounds 1–2 applied), pre-implementation. **Design only.**
> Date: 2026-06-19. Lane: `workflow` + `product`.
> The Python `tools/session-state` + `install.ps1` shipped to `main` via **PR #29 (MERGED 2026-06-19,
> merge `bad4016`)**. This spec **removes that merged code** and replaces it with a Node Claude Code
> plugin (`§6`). Builds on the runtime design
> (`docs/superpowers/specs/2026-06-19-session-state-resume-design.md`): behavior preserved, storage
> parse-compatible, runtime re-implemented in Node, delivery becomes a plugin so users get the hooks
> at the harness layer. Serves **context hygiene** + **ship fast**.

## 1. Goal & scope

Distribute session-state as a **Claude Code plugin** (the harness-native hook mechanism) in **Node**
(guaranteed wherever Claude Code runs), so Command Center users get the hooks with no manual setup.

**Round-2/3 restructure — ship Phase 1 first; gate the rest. Phase 1 itself has two quick spikes
(`§2` Spike 0a/0b) because its hook-invocation shape on Windows and self-marketplace install are not
yet verified.** Phases 2–3 sit on further unverified CLI/runtime behaviors.

**Honest framing:** this is a spike-heavy port. The Python tool already works and is merged; the value
of the Node plugin is harness-layer distribution, which depends on several Claude-Code-plugin behaviors
this spec gates behind spikes rather than assuming.

**In scope NOW (this spec, design only — first PR, after Spikes 0a/0b):**
- **Phase 1 — Node plugin artifact** (`§2`, `§3`): re-implement the runtime in dependency-free Node
  ESM (`node:` builtins only, no build), packaged as a plugin with a self-marketplace in this repo,
  hooks invoked via the **shape chosen by Spike 0a**. Behavior preserved; storage **parse-compatible**.
- **Phase 4 — Migration & retirement** (`§6`): remove the merged Python tool/skill/installer (atomic
  skill swap), after de-conflicting the live machine; the Node plugin replaces them.

**In scope as GATED follow-ons (designed only to gate-depth here; full design after their spikes):**
- **Phase 2 — App ensure** (`§4`): gated on a **CLI-idempotency spike** (`§4` Spike B). Sketched, not
  fully specced.
- **Phase 3 — Fleet agent container** (`§5`): gated on a **hook-firing + reachability spike**
  (`§5` Spike A). Sketched, not fully specced.

**Out of scope:**
- **Changing capture/resume semantics.** Runtime port only; the **one** runtime change is the lock
  (`§3`), forced by the absence of a Node equivalent to the Python OS-handle lock.
- **A public marketplace listing; bundling a runtime; TypeScript / a build step.** Plain `.mjs`.

## 2. Plugin artifact & repo layout (Phase 1)

Self-marketplace in this repo (verified pattern: the official `claude-plugins-official` marketplace
uses `"source":"./plugins/<name>"`; `context-curator` is a `"directory"`-source whole-repo entry).

```
command-center/
  .claude-plugin/marketplace.json     # repo = self-marketplace; source "./plugins/session-state"
  plugins/session-state/
    .claude-plugin/plugin.json        # name, version, description, author
    hooks/
      hooks.json                      # 3 entries, each invoking the wrapper (below)
      run-hook.cmd                    # polyglot cmd/sh wrapper: locates node, runs src/<entry>.mjs
    skills/save-state/SKILL.md        # plugin SKILL (skills, not commands — see §3)
    src/                              # *.mjs — node: builtins only, no deps, no build
      keying.mjs gitfacts.mjs lock.mjs store.mjs merge.mjs resolve.mjs
      capture_scratch.mjs capture_end.mjs capture_rich.mjs resume.mjs cli.mjs
    test/*.test.mjs                   # node:test
    README.md
```

**The hook-invocation shape is decided by Spike 0a — do not assume one.** Important correction
(round 3, verified): the official superpowers `run-hook.cmd` resolves **bash** and runs **bash entry
scripts** (`exec bash "$dir/<entry>"`), it does **not** resolve node or run `.mjs`. So there is *no*
proven node-resolving wrapper to copy, and the bare `{"command":"node","args":[…]}` exec form is also
unproven on Windows. The three candidate shapes are:
- (i) bare `{"command":"node","args":["${CLAUDE_PLUGIN_ROOT}/src/<entry>.mjs"]}` exec form;
- (ii) a node-resolving `run-hook.cmd` cmd/sh polyglot (hand-written; quotes every path expansion;
  resolves `node` via PATH then known locations) invoked as a single command string;
- (iii) the superpowers pattern verbatim — extensionless **bash** entry scripts that each
  `exec node "$ROOT/src/<entry>.mjs"`, which on Windows **depends on Git-Bash** (no-op when bash is
  absent, as superpowers does).

**Spike 0a** (`spikes/SPIKE-RESULTS-session-state-plugin.md`): on a real Windows box, determine which
of (i)/(ii)/(iii) actually fires a plugin hook, forwards the hook **stdin JSON**, and returns exit 0 —
**without assuming Git-Bash**. Phase-1 hooks.json uses the winner. Tentative `hooks.json` (matchers
included so SessionStart fires only on `startup|resume`, matching the runtime source-gate at the
manifest, not only in-script):
```json
{ "hooks": {
  "SessionStart": [{ "matcher":"startup|resume", "hooks": [{ "type":"command","command":"<shape from Spike 0a → resume>" }] }],
  "Stop":         [{ "hooks": [{ "type":"command","command":"<shape from Spike 0a → capture_scratch>","timeout":5 }] }],
  "SessionEnd":   [{ "hooks": [{ "type":"command","command":"<shape from Spike 0a → capture_end>" }] }]
}}
```
Whatever shape wins, **stdin forwarding is a tested invariant** (`§7`) — `resume`'s source-gate and
`capture_end`'s reason-gate read the hook stdin JSON, so a wrapper that drops stdin silently breaks
gating.

**Spike 0b** (same file): `claude plugin marketplace add <command-center repo>` + `claude plugin
install session-state@<marketplace>` — confirm a *large app repo* can serve as a self-marketplace,
observe the install dir, and confirm the **skill + hooks register and a hook fires**. (Plugin-skill
auto-discovery and the `source:"./plugins/<name>"` shape are verified; *this repo as a marketplace* is
not.)

- `${CLAUDE_PLUGIN_ROOT}` changes on update; state lives under `~/.claude/state/sessions/`, so history
  survives updates. Plugin hooks **merge** with settings.json hooks (coexist with `recall.py` /
  cache-timer / budget).
- **Version contract (anti-drift):** the **app-bundled copy is authoritative** for product users; the
  git self-marketplace is for standalone host users and they are told to **track a release tag, not the
  branch**; a test asserts `plugin.json.version` matches across the bundled copy and the marketplace
  entry. (Drift across the three install paths was a named risk.)
- `marketplace.json` matches the verified official shape: `$schema`, `name`, `owner{name,email}`,
  `plugins:[{name, source:"./plugins/session-state", description, version}]`.

## 3. Node runtime port

Each Python module → a `.mjs` peer, same responsibility/behavior. **`node:` builtins only**; no deps;
no compile. A test asserts every import is `node:`-prefixed or relative (`§7`).

- **keying.mjs** — `claudeHome()` (honors `CLAUDE_CONFIG_DIR`), `canonicalProjectRoot`, `pathToSlug`
  (`\`,`/`,`:` → `-`), `repoRoot`, `repoKey`, `stateDir(cwd,{create=true})`, `checkMeta`. Parity target
  = Claude Code's `~/.claude/projects/<slug>` encoding (test against a real on-disk dir).
- **gitfacts.mjs** — `execFileSync('git',…,{timeout:5000,encoding:'utf8'})`; `parsePorcelainV2`
  (`1`/`2`/`u`/`?`; `(detached)`; `ab` unused); `inProgress`; `collectGitFacts` → object / `null`
  non-git / `gitUnavailable:true` on `git` `ENOENT`.
- **lock.mjs — O_EXCL lockfile with a liveness + age backstop + ownership-checked release.**
  `fs.openSync(lockfile,'wx')` (atomic create-exclusive; no `rmdir`-nonempty hazard). The lockfile
  **content is a token = `{pid, startTimeMs, rand}`**. `fileLock(lockfile,{tries,backoff})`:
  - acquire: create with `wx`; on `EEXIST`, decide whether to **steal** using this truth table, then
    retry; bounded → `LockTimeout`.
  - **steal decision (combines liveness AND age, fixing both PID-reuse deadlock and the time-race):**
    read the token; `dead = process.kill(pid,0)` throws `ESRCH`. Steal iff **`dead` OR the lockfile
    mtime is older than `MAX_AGE` (60s — far above the 5s hook timeout, so a live holder is never
    stolen, but a PID-reused zombie still clears)**. `EPERM` from `kill` ⇒ treat as **alive** (don't
    steal on liveness) but the age backstop still applies. Steal is atomic: `unlink` then re-create
    with `wx`; if re-create loses to a racer, back off and retry (single-winner).
  - **release is ownership-checked:** re-read the lockfile; unlink **only if the token still matches
    ours**. A holder that was stolen sees a mismatch and does **not** unlink (prevents deleting a
    stealer's lock → the A/B handoff race).
  - On `LockTimeout`: auto = skip; rich = preserve temp + print retry (Python contract).
- **store.mjs** — `nowIso()` (`…Z`), `makeRecord`, scratch I/O, `readTimeline({tail})` (skip corrupt),
  `appendRecord` (lock → append `JSON.stringify(rec)+"\n"` → render `latest.md`, same hold; `false` on
  `LockTimeout`), `prune` (**locked** truncate + orphan-scratch cleanup outside lock).
- **merge.mjs** — `resolveState` / `renderResumeBlock` / `renderLatestMd` (freshest-by-ts git, newest
  rich, branch banner suppressed when detached/in-progress).
- **resolve.mjs** — `pluginRoot()` reads **`~/.claude/plugins/installed_plugins.json`** (Claude Code's
  own stable registry) to find this plugin's install dir, **validating the path exists** and falling
  back to scanning the plugins cache. This is how the `save-state` skill locates `capture_rich.mjs`
  without any hook-written breadcrumb.
- **Entries** — same contracts as Python: kill-switch (`CC_SESSION_STATE_DISABLE`) first; read stdin
  JSON; hooks always `process.exit(0)` (UTF-8 native); `capture_scratch` throttles (30s unless git
  changed) + overwrites own scratch; `capture_end` skips `clear`/`resume`, appends `auto`, deletes only
  its own scratch, prunes; `capture_rich --input <file>` preserves temp + prints retry on failure;
  `resume` source-gated, emits the envelope, **writes no files** (`create:false`). **No entry writes an
  env/breadcrumb file** (the prior `session-state-env.json` mechanism is removed).

**`save-state` is a plugin SKILL, invoked by the model via its `description` trigger** (e.g. the user
says "save state"/"checkpoint", or the model decides at a boundary) — exactly like any other skill.
Correction (round 3, verified): the earlier "`end-session` drives `save-state`" rationale was **false**
— `end-session`'s SKILL.md is self-contained and never invokes `save-state`. So the producer guarantee
is **model-invocation via the skill's description**, not an end-session call. (If we later want
end-session to compose with save-state, that is a *separate* edit to end-session's body — explicitly
out of this spec's scope.)

The skill body: compose `{did,next,open_threads}` → write a temp JSON → **resolve the script by
reading `~/.claude/plugins/installed_plugins.json`** at key `"session-state@<marketplace-name>"` →
`installPath` (validate the path exists; fall back to scanning the plugins cache if missing/stale) →
run `node <installPath>/src/capture_rich.mjs --input <temp>` → handle success / preserve-on-failure.
The registry read in the body is **unavoidable** (a skill body gets no `${CLAUDE_PLUGIN_ROOT}`); the
`<marketplace-name>` is therefore a **hardcoded coupling** in the skill body, noted as such. `cli.mjs`
exposes the same resolution for terminal use, but the skill's first hop must read the registry itself.

**Storage is parse-compatible, not byte-identical** (`JSON.stringify` vs `json.dumps` differ in
whitespace/ASCII-escaping; irrelevant — readers `JSON.parse`). What MUST match: on-disk paths/slug,
record key names, and `ts` format. A test asserts a Node-written record re-parses to the runtime
spec's fields.

## 4. Phase 2 — App ensure (GATED on Spike B; sketched only)

Goal: on Command Center launch, idempotently install/enable the plugin so "use the product = hooks
running." **Gated** because two behaviors are unverified:

- **Spike B (record to `spikes/SPIKE-RESULTS-session-state-app.md`):** is `claude plugin marketplace
  add <localdir>` + `claude plugin install` **idempotent on re-run** (no-op vs error when already
  present)? What exit codes / state does it produce?

Sketched design (full spec after Spike B):
- **Probe by reading `~/.claude/plugins/installed_plugins.json`** (stable schema) for
  `session-state@<marketplace>` — **not** by scraping `claude plugin list` stdout.
- **Resolve `claude` robustly** — a GUI `.app` from Finder has a minimal `launchd` PATH; resolve via
  known install locations, `where`/`which`, and macOS login-shell (`$SHELL -lc 'command -v claude'`).
- Install the **app-bundled** copy as a local-path marketplace (offline).
- **Visible degraded indicator** in the cockpit when resolve/install fails (never silent; startup not
  blocked).
- **Concurrency is cross-PROCESS, not cross-window** (Tauri is single-process; `.setup` runs once per
  process). Mitigate with a cross-process lockfile under `~/.claude` and/or Tauri's `single-instance`
  plugin — guarding against two app instances or a hand-run `claude` racing the install.
- Component: cockpit `src-tauri` setup (what the user launches), not fleetd.

## 5. Phase 3 — Fleet agent container (GATED on Spike A; sketched only)

Agents run **one-shot `claude -p … --dangerously-skip-permissions`** as `USER node`, ephemeral volume
at `/work`, **no `~/.claude` mount** (verified `steps.rs` / `local_docker.rs`). Three unverified
assumptions (hooks fire in `-p`? captured state reachable? build-time `claude plugin install` works
with no key/network?) make naïve "bake the plugin in" likely worthless.

- **Spike A (record to `spikes/SPIKE-RESULTS-session-state-container.md`):** in a throwaway
  `node:22-slim`+claude-code container, install via local path with **no key, no network**; run a
  `claude -p` that triggers a tool use; observe whether each hook fires and whether a record is
  written.
- **If hooks fire:** the only value-bearing form is fleetd **bind-mounting host `~/.claude/state/
  sessions` into the container** — promoted from optional to the actual deliverable (no mount = no
  value).
- **If hooks do NOT fire (likely for `Stop`/`SessionEnd` in `-p`):** drop the in-container plugin;
  **fleetd writes an `auto` record around each `claude` exec** (it already knows the unit's repo/
  branch/lifecycle) — host-side, no container-hook dependency.
- **If inconclusive/low-value:** Phase 3 ships as a documented no-op; Phases 1–2 stand alone.

No Phase-3 code before Spike A is recorded.

## 6. Phase 4 — Migration & retirement (the Python tool is MERGED on main)

PR #29 is **merged**; `tools/session-state/**`, `.claude/skills/save-state/SKILL.md`, and `install.ps1`
are on `main`. Migration is a **new PR off main that removes them** — not "close a PR." Order:

1. **De-conflict the live machine FIRST:** run `tools/session-state/install.ps1 -Uninstall` (removes
   the three manual hook entries by basename marker). **Hard precondition / abort gate:** if
   `-Uninstall` reports 0 removed yet `~/.claude/settings.json` still references the Python scripts (it
   points at the installed `~/.claude/tools/session-state/` copy, not the repo), **abort migration** —
   do not install the plugin, or capture/resume will double-fire. Only proceed once settings.json is
   confirmed clean.
2. **Atomic skill swap (no name-collision window):** both the merged repo skill
   `.claude/skills/save-state/` and the new plugin skill are named `save-state`; while both are
   discoverable, precedence is undefined and the project-local one (still pointing at Python) could
   win. So **remove `.claude/skills/save-state/` in the SAME commit that the plugin (with its skill) is
   installed/active** — never "add new, remove old later." Acceptance: after install, **exactly one
   `save-state` resolves and it runs `capture_rich.mjs`** (not the Python script).
3. **Remove the rest of the merged Python code** (`tools/session-state/` + `install.ps1`) in the same
   PR.
4. **Install the Node plugin for this machine** (marketplace add + install, per Spike 0b) so the user
   runs the port; optionally delete the now-unreferenced installed `~/.claude/tools/session-state/`
   copy (cosmetic).

Parse-compatible format ⇒ state the Python version already wrote is read by the plugin.

## 7. Testing

- **`node:test`** in `plugins/session-state/test/`:
  - **import guard:** every `src/*.mjs` import is `node:`-prefixed or relative.
  - **slug parity** vs Claude Code `~/.claude/projects` encoding (Windows drive; worktree path).
  - **porcelain-v2 parse:** clean/dirty(`1`,`2`,`?`)/detached/conflict(`u`).
  - **lock:** N concurrent appenders (child procs) → N valid lines; **dead-PID steal** recovers;
    **live holder never stolen**; **age-backstop** steals a PID-reused zombie; **`EPERM`→treated
    alive**; **ownership-checked release** does not unlink a stealer's lock (A/B handoff); `LockTimeout`
    clean.
  - **store/merge** as the runtime spec (locked append+render one hold; locked prune; corrupt skip;
    freshest/banner/detached-suppression).
  - **format parse-compat:** Node-written record re-parses to the exact fields/`ts` shape.
  - **entries (real invocation form):** spawn the wrapper / `node src/<entry>.mjs` with stdin in a temp
    git repo (isolated `CLAUDE_CONFIG_DIR`); assert all entry behaviors + kill-switch no-ops.
  - **resolve.mjs:** finds the plugin via a fixture `installed_plugins.json`; validates path exists;
    falls back when the recorded path is missing (post-update).
- **Manifest validation:** `plugin.json` / `marketplace.json` / `hooks.json` valid; referenced paths
  exist; `marketplace.json` matches the official schema; **version-contract** assertion.
  - the **entries test must invoke through the chosen Spike-0a shape** (the wrapper/command form), not
    only `node src/<entry>.mjs`, so the **stdin round-trip** (source-gate, reason-gate) is exercised
    end-to-end; a fixture path **containing a space** is included.
- **Spikes are the gates for the unverified mechanisms** (recorded to spike-files, not unit tests):
  **0a** hook-invocation shape on Windows (`§2`), **0b** self-marketplace install of this repo (`§2`),
  **B** app-ensure CLI idempotency (`§4`), **A** container hook firing (`§5`).
- **Windows hook rewriting can only be confirmed by Spike 0a on a real Windows box** — CI can spawn the
  script with fixture stdin (tests the *script*) but cannot exercise Claude Code's command rewriting of
  a real plugin hook on hosted runners (same gap the CI header documents).
- **Migration acceptance (manual):** `-Uninstall` removes the manual hooks (verified vs live
  settings.json, with the abort gate `§6.1`); after install **exactly one `save-state` resolves and
  runs `capture_rich.mjs`**; invoking `save-state` writes a rich record; a new session resumes via the
  plugin. (No "end-session drives it" test — that contract does not exist.)

## 8. Edge cases & risks

- **Windows hook command rewriting + hook-invocation shape:** the shape is **not assumed** — Spike 0a
  picks among bare-`node`-exec / hand-written node-cmd-polyglot / superpowers-style-bash (the last
  depends on Git-Bash). Superpowers' `run-hook.cmd` runs *bash*, not node, so there is no node wrapper
  to copy; whichever shape wins must forward stdin and exit 0.
- **`node`/`bash` availability:** depends on the Spike-0a winner — the cmd-polyglot resolves `node`
  (PATH then known locations); the bash variant needs Git-Bash on Windows. If the resolver fails the
  hook no-ops (exit 0) — "no state captured," not a crash.
- **Migration name collision:** two `save-state` skills (merged repo + plugin) are briefly both
  discoverable; resolved by the **atomic swap** (`§6.2`) + an acceptance check that exactly one
  resolves and runs `capture_rich.mjs`.
- **Lock — PID reuse / EPERM / A-B handoff:** addressed by liveness **plus** 60s age-backstop, the
  `EPERM`→alive rule, and ownership-checked release (`§3`); all covered by tests (`§7`).
- **`save-state` script resolution across updates:** via `installed_plugins.json` (registry), validated
  + fallback — no hook-written breadcrumb, no `${CLAUDE_PLUGIN_ROOT}`-in-skill-body assumption.
- **Migration double-fire:** manual hooks uninstalled + verified gone before the plugin is active;
  producer (`save-state`) replaced before the old skill is removed.
- **Version drift across install paths:** bundled copy authoritative; git users track a tag; CI asserts
  version parity.
- **App-ensure silent failure:** prevented by robust `claude` resolution + a visible degraded indicator
  (`§4`).
- **Format:** parse-compatible (not byte-identical).
- **Plugin hooks can't be selectively disabled** (whole-plugin only); scripts honor
  `CC_SESSION_STATE_DISABLE`.

## 9. Relationship to existing work

- **Removes** the merged Python `tools/session-state` + `install.ps1` + `.claude/skills/save-state`
  (PR #29, on `main`) and replaces them with the Node plugin (same behavior; parse-compatible format).
- **Coexists** with the user's `recall.py` SessionStart hook and cache-timer/budget Stop hooks.
- **Touches** the repo root (self-marketplace) + `plugins/session-state` now; the cockpit `src-tauri`
  (Phase 2, after Spike B) and the agent image (Phase 3, after Spike A) later.
- **Feeds, later:** the dashboard (#4) and a future SQLite upgrade consume the same JSONL.

## Design Critique Log

Three independent adversarial rounds (fresh subagent each, each seeing the prior revision), verified
against the live machine (installed plugins, `installed_plugins.json`, the official marketplace, the
agent Dockerfile, fleetd, the cockpit `src-tauri`, `end-session`).

### Critique Round 1
**Findings:** Phase 3 (container) likely delivers nothing — agents run one-shot `claude -p
--dangerously-skip-permissions` with a fresh ephemeral volume and no `~/.claude` mount, so hook-firing
is unverified, there's no prior state to resume, captures vanish, and build-time `claude plugin
install` (auth/network) is unproven. The mkdir-lock's 30s time-based stale-break had a double-steal
race. The Tauri silent-ensure fails silently on macOS GUI PATH (`.app` from Finder has minimal PATH).
"Byte-identical" storage was an overclaim. Migration step order was wrong. The moved `save-state`
couldn't locate its script after a plugin update.
**Resolved:** Phase 3 made **spike-gated** (Spike A) with the host bind-mount promoted to the actual
deliverable and a daemon-side fallback; lock moved to O_EXCL + PID; app-ensure given robust `claude`
resolution + a **visible degraded indicator**; "byte-identical" → "parse-compatible"; migration
reordered (uninstall before delete); script resolution via a (then) hook-written env file.

### Critique Round 2
**Findings (two fatal):** (1) plugin **commands are model-facing markdown prompts, not executables** —
the command + env-file resolution mechanism was a category error, and skills can't drive slash
commands. (2) **PR #29 is MERGED**, not open — "close PR #29" is impossible; the Python tool is on
`main`. Also: the `args`-array hook form is **unproven on Windows** (official plugins use a wrapper);
the liveness lock traded the time-race for a **PID-reuse deadlock** and an **ownership-release race**;
`claude plugin list` stdout-scraping is fragile; the "single-flight across windows" guarded a non-risk
(Tauri is single-process); the "Windows CI gate" was **vapor** (no such job); marketplace **version
drift** across three install paths; and the spec was **three specs in one**.
**Resolved:** `save-state` → a **plugin skill** resolving via `installed_plugins.json` (env-file
dropped); migration rewritten to **remove merged code** in a new PR; hooks via a **polyglot wrapper**;
lock given a **liveness + 60s age-backstop + EPERM rule + ownership-checked release**; probe via
`installed_plugins.json`; **version contract** added; Windows downgraded to a manual gate; scope
**narrowed to Phase 1 + migration**, with Phases 2/3 spike-gated and only sketched.

### Critique Round 3
**Findings (two fatal-as-specified, verified):** (1) **`end-session` does not drive `save-state`** —
its SKILL.md is self-contained; the skill-vs-command justification and the migration acceptance test
described a non-existent contract. (2) **The "proven node-resolving wrapper" does not exist** —
superpowers' `run-hook.cmd` resolves **bash** and runs **bash scripts**, never node/`.mjs`; the real
Windows path depends on Git-Bash, and a node wrapper is unverified. Also: the SessionStart **matcher
was missing**; the migration **skill-name collision** (`save-state` in both repo `.claude/skills` and
the plugin) needed an **atomic swap**; even Phase 1 had **unverified install assumptions** (this big
repo as a marketplace) deserving a spike. Lock logic confirmed **correct** (all interleavings traced),
registry schema + plugin-skill auto-discovery confirmed real.
**Resolved:** `save-state` re-justified as **model-invoked via its `description`** (end-session-drives-it
claim and its test removed; composing them noted as out-of-scope); all "superpowers resolves node"
statements corrected; hook-invocation shape made **Spike 0a** (choose among bare-node-exec /
node-cmd-polyglot / bash-pattern, stdin-forwarding tested); **Spike 0b** added for the self-marketplace
install; **SessionStart matcher** added; migration made an **atomic skill swap** with an
abort-on-marker-mismatch precondition and an "exactly one save-state resolves" acceptance check; lock
edges (age resets on steal, Windows EPERM test) documented.
