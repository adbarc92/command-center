# cache-timer-write.ps1 — Stop hook for Claude Code (Windows / PowerShell 7)
#
# Marks the cache timer state file as "stopped" so the ticker knows the cache
# is now genuinely draining and should show the awaiting-approval countdown.
# "Task awaiting user approval" == the Stop hook fires (roadmap item 1).
#
# Reads the hook payload (JSON on stdin) for session_id + cwd; writes
# ~/.claude/state/cache-timer-<session_id>.json with timestamp = NOW.
#
# Install: wire into ~/.claude/settings.json under hooks.Stop (Lane Z owns
# settings.json — see the contract request in the lane report). This script
# does NOT touch settings.json itself.

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

# Preserve any existing fields (e.g. cached_tokens, project) across the write.
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

# The cache starts draining NOW — timestamp the stop moment.
$timer["session_id"] = $sid
$timer["stopped"] = $true
$timer["timestamp"] = (Get-Date -Format "o")

$timer | ConvertTo-Json -Compress | Set-Content $timerPath -Force
exit 0
