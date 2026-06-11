# budget-checkpoint.ps1 — Stop hook shim for Claude Code (Windows / PowerShell 7)
#
# Roadmap item 6D — nudges the agent to /handoff or /end-session at work
# boundaries so the next session starts compact. This thin shim just pipes the
# Stop event (JSON on stdin) into the UV/Python hook and passes its stdout
# straight through to Claude Code. All the logic lives in Python
# (src/budget_checkpoint/), keeping the hook headless, fast, and stdlib-only.
#
# Install: wire into ~/.claude/settings.json under hooks.Stop (Lane C owns
# settings.json — see the contract request in the lane report). This script does
# NOT touch settings.json itself.

param(
    [string]$ProjectDir = (Join-Path $env:USERPROFILE ".claude\tools\budget-checkpoint")
)

$ErrorActionPreference = "Continue"
[Console]::OutputEncoding = [System.Text.Encoding]::UTF8

# Read the Stop event payload from stdin and forward it to the UV-run hook.
$hookInput = [Console]::In.ReadToEnd()

$uv = Get-Command uv -ErrorAction SilentlyContinue
if (-not $uv) { exit 0 }   # no UV -> degrade to no-op, never fail the turn

try {
    $hookInput | & uv run --project $ProjectDir budget-checkpoint
} catch {
    # A Stop hook must never crash the turn.
}
exit 0
