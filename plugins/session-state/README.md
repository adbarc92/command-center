# session-state (plugin)

Per-repo dev-session capture + zero-friction resume, shipped as a Claude Code plugin.
Spec: `docs/superpowers/specs/2026-06-19-session-state-plugin-distribution-design.md`.

## Install (standalone host)
    claude plugin marketplace add <command-center repo>
    claude plugin install session-state@command-center

## Disable temporarily
Set `CC_SESSION_STATE_DISABLE=1` in a shell to make all hooks no-op there.

## Inspect
    node plugins/session-state/src/cli.mjs list
    node plugins/session-state/src/cli.mjs show [<path-or-repo-key>]

## Tests
    node --test "plugins/session-state/test/*.test.mjs"
