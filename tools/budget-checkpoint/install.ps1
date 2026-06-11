# install.ps1 — installer for the budget-checkpoint Stop hook (Windows / PowerShell 7)
#
# Roadmap item 6D (Command Center). Copies the hook shim + UV/Python package into
# ~/.claude/tools/budget-checkpoint/, prepares the UV environment, and PRINTS the
# settings.json Stop-hook entry for the orchestrator (Lane C) to paste.
#
# This installer deliberately does NOT modify ~/.claude/settings.json — that file
# is owned by Lane C. Run with -PrintHooksOnly to emit just the hook JSON.
#
#   pwsh -NoProfile -File install.ps1                 # install + print hook
#   pwsh -NoProfile -File install.ps1 -PrintHooksOnly # just print the hook JSON

param(
    [switch]$PrintHooksOnly,
    [string]$InstallDir = (Join-Path $env:USERPROFILE ".claude\tools\budget-checkpoint")
)

$ErrorActionPreference = "Stop"
[Console]::OutputEncoding = [System.Text.Encoding]::UTF8

$srcRoot = $PSScriptRoot
$hookScript = Join-Path $InstallDir "hooks\budget-checkpoint.ps1"

function Get-HookEntries {
    param([string]$HookPath)
    # Matches the live ~/.claude/settings.json schema: hooks.<Event> is an array
    # of { matcher, hooks: [ { type, command, timeout } ] }. Empty matcher = all
    # Stop events; the hook itself decides whether a boundary warrants a nudge.
    $stop = [ordered]@{
        matcher = ""
        hooks   = @(
            [ordered]@{
                type    = "command"
                command = "pwsh.exe -NoProfile -File `"$HookPath`""
                timeout = 10
            }
        )
    }
    return [ordered]@{
        Stop = @($stop)
    }
}

if (-not $PrintHooksOnly) {
    Write-Host "Installing budget-checkpoint to $InstallDir ..."
    New-Item -ItemType Directory -Path (Join-Path $InstallDir "hooks") -Force | Out-Null

    Copy-Item -Path (Join-Path $srcRoot "hooks\*.ps1") -Destination (Join-Path $InstallDir "hooks") -Force
    Copy-Item -Path (Join-Path $srcRoot "src") -Destination $InstallDir -Recurse -Force
    Copy-Item -Path (Join-Path $srcRoot "pyproject.toml") -Destination $InstallDir -Force
    if (Test-Path (Join-Path $srcRoot "tests")) {
        Copy-Item -Path (Join-Path $srcRoot "tests") -Destination $InstallDir -Recurse -Force
    }

    # Prepare the UV environment so `uv run budget-checkpoint` works.
    $uv = Get-Command uv -ErrorAction SilentlyContinue
    if ($uv) {
        Write-Host "Syncing UV environment ..."
        Push-Location $InstallDir
        try { & uv sync } catch { Write-Warning "uv sync failed: $_" }
        Pop-Location
    } else {
        Write-Warning "uv not found on PATH. Install UV, then run 'uv sync' in $InstallDir."
    }

    Write-Host "Installed. Test the hook with:"
    Write-Host "  '{`"session_id`":`"x`",`"transcript_path`":`"<path>`",`"stop_hook_active`":false}' | uv run --project `"$InstallDir`" budget-checkpoint"
    Write-Host ""
}

$entries = Get-HookEntries -HookPath $hookScript

Write-Host "=== Add this entry to ~/.claude/settings.json under `"hooks`" (Lane C applies) ==="
$entries | ConvertTo-Json -Depth 6
