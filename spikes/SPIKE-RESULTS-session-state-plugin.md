# Spike Results — Session-State Plugin (Phase 1 gates)

Date: 2026-06-20. Spec: `docs/superpowers/specs/2026-06-19-session-state-plugin-distribution-design.md`.
Run on the live Windows box with an **isolated `CLAUDE_CONFIG_DIR`** (seeded with a copy of
`.credentials.json`, deleted after) so the real `~/.claude` was never modified. A minimal sentinel
plugin skeleton (`plugins/session-state/` + `.claude-plugin/marketplace.json`) was used.

## Spike 0a — Hook-invocation shape on Windows → **PASS (shape (i): bare `node` args form)**

`hooks/hooks.json` used the plain exec form:
```json
{ "type":"command", "command":"node", "args":["${CLAUDE_PLUGIN_ROOT}/src/resume.mjs"] }
```
Ran `claude -p "Reply with exactly: OK"` in a throwaway git repo with the plugin installed and a
`SPIKE_SENTINEL` env file armed. Result:
- **Both hooks fired** — `SessionStart` (resume) and `Stop` (capture_scratch) each appended to the
  sentinel.
- **stdin JSON is forwarded** to the script: resume received
  `{"session_id":…,"hook_event_name":"SessionStart","source":"startup", …}` — so the runtime
  **source-gate / reason-gate read from stdin works**. Stop received the full Stop payload
  (`session_id`, `cwd`, `last_assistant_message`, …).
- `${CLAUDE_PLUGIN_ROOT}` resolved; both scripts exited 0.

**Conclusion: no polyglot wrapper is needed.** The earlier concern (superpowers' `run-hook.cmd` runs
*bash*, args-form unproven) is resolved empirically: the bare `node` + `args` exec form works on
Windows, fires for SessionStart+Stop, and forwards stdin. Phase 1 uses shape (i) directly; the
`run-hook.cmd` wrapper and the three-candidate uncertainty are dropped.

**Nuance recorded:** for a **directory** marketplace, runtime `${CLAUDE_PLUGIN_ROOT}` pointed at the
**source** dir (`…/command-center/plugins/session-state`), while the registry `installPath` was the
**cache copy** (`…/plugins/cache/command-center/session-state/0.0.1`). Both contain the full file set
(verified). The `save-state` skill resolves via the registry `installPath` (cache), which is populated
— so skill resolution works. (Git-URL marketplaces would make both the cache clone.)

## Spike 0b — Self-marketplace install of this repo → **PASS**

- `claude plugin marketplace add <command-center repo>` (a **directory** source) → "Successfully added
  marketplace: command-center". (`--sparse` is **git/github-only**, not directory — noted for the
  git-URL path; not needed for directory.)
- `claude plugin install session-state@command-center` → installed + **enabled** (scope user).
- `installed_plugins.json` (v2) records key `session-state@command-center` → `installPath` (cache,
  **version-stamped** `…/0.0.1`), `version`, `installedAt`, `gitCommitSha`. **Confirms the registry
  resolution design** (path changes on version bump → resolve via registry, never hardcode).
- `claude plugin details` component inventory: **Skills (1) save-state**, **Hooks (2) SessionStart,
  Stop** — both **auto-discovered** from the plugin dir (no declaration in `plugin.json` needed),
  matching the official-plugin convention.
- The cache `installPath` contains the full tree (`src/*.mjs`, `hooks/`, `skills/`, `plugin.json`).

**Conclusion:** the big app repo serves as a directory self-marketplace; plugin + skill + hooks
register and the plugin enables. Phase 1 packaging is viable as specced.

## Net effect on the spec
- **Spike 0a PASSED** → lock hook shape (i) (bare `node`+`args`); **remove the wrapper** and the
  three-candidate uncertainty from §2/§3/§7/§8.
- **Spike 0b PASSED** → §2 self-marketplace + skill/hook auto-discovery confirmed; registry-based
  `save-state` resolution confirmed (cache `installPath` is populated).
- Remaining gates unchanged: **Spike B** (app-ensure CLI idempotency, §4), **Spike A** (container hook
  firing, §5). Note Git-Bash is NOT required for hooks (shape (i) uses `node` directly).
