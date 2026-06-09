# cache-timer-resume.ps1 — UserPromptSubmit hook for Claude Code (Windows / PowerShell 7)
#
# Clears the "stopped" state when the user sends a new prompt: the session is
# active again, the cache is being refreshed, so the ticker should show HOT
# instead of a draining countdown. Creates the state file if this is a brand
# new session (started after the ticker).
#
# Reads the hook payload (JSON on stdin) for session_id + cwd; writes
# ~/.claude/state/cache-timer-<session_id>.json with stopped = false and
# timestamp = NOW (the moment the cache is re-warmed).
#
# Install: wire into ~/.claude/settings.json under hooks.UserPromptSubmit
# (Lane Z owns settings.json — see the contract request in the lane report).
# This script does NOT touch settings.json itself.

param()

$ErrorActionPreference = "Continue"
[Console]::OutputEncoding = [System.Text.Encoding]::UTF8

$hookInput = [Console]::In.ReadToEnd()
if ([string]::IsNullOrWhiteSpace($hookInput)) { exit 0 }

try { $data = $hookInput | ConvertFrom-Json } catch { exit 0 }

$sid = $data.session_id
if (-not $sid) { exit 0 }

$stateDir = Join-Path $env:USERPROFILE ".claude\state"
if (-not (Test-Path $stateDir)) {
    New-Item -ItemType Directory -Path $stateDir -Force | Out-Null
}
$timerPath = Join-Path $stateDir "cache-timer-$sid.json"

$timer = @{}
if (Test-Path $timerPath) {
    try {
        $existing = Get-Content $timerPath -Raw | ConvertFrom-Json
        $existing.PSObject.Properties | ForEach-Object { $timer[$_.Name] = $_.Value }
    } catch { $timer = @{} }
}

if (-not $timer["project"]) {
    if ($data.cwd) {
        $timer["project"] = Split-Path -Leaf $data.cwd
    } elseif ($env:CLAUDE_PROJECT_DIR) {
        $timer["project"] = Split-Path -Leaf $env:CLAUDE_PROJECT_DIR
    } else {
        $timer["project"] = "unknown"
    }
}
if ($data.cwd) { $timer["cwd"] = $data.cwd }

# Session is active again — cache is warm, no countdown.
$timer["session_id"] = $sid
$timer["stopped"] = $false
$timer["timestamp"] = (Get-Date -Format "o")

$timer | ConvertTo-Json -Compress | Set-Content $timerPath -Force
exit 0
