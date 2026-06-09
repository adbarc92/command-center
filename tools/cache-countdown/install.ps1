# install.ps1 — installer for the cache-aware approval timer (Windows / PowerShell 7)
#
# Roadmap item 1 (Command Center). Copies the hook scripts + UV/Python ticker
# into ~/.claude/tools/cache-countdown/, prepares the UV environment, and PRINTS
# the two settings.json hook entries for the orchestrator (Lane Z) to paste.
#
# This installer deliberately does NOT modify ~/.claude/settings.json — that
# file is owned by Lane Z. Run with -PrintHooksOnly to emit just the hook JSON.
#
#   pwsh -NoProfile -File install.ps1                 # install + print hooks
#   pwsh -NoProfile -File install.ps1 -PrintHooksOnly # just print the hook JSON

param(
    [switch]$PrintHooksOnly,
    [string]$InstallDir = (Join-Path $env:USERPROFILE ".claude\tools\cache-countdown")
)

$ErrorActionPreference = "Stop"
[Console]::OutputEncoding = [System.Text.Encoding]::UTF8

$srcRoot = $PSScriptRoot
$writeScript  = Join-Path $InstallDir "hooks\cache-timer-write.ps1"
$resumeScript = Join-Path $InstallDir "hooks\cache-timer-resume.ps1"

function Get-HookEntries {
    param([string]$WritePath, [string]$ResumePath)
    # Matches the live ~/.claude/settings.json schema: hooks.<Event> is an array
    # of { matcher, hooks: [ { type, command, timeout } ] }. Empty matcher = all.
    $stop = [ordered]@{
        matcher = ""
        hooks   = @(
            [ordered]@{
                type    = "command"
                command = "pwsh.exe -NoProfile -File `"$WritePath`""
                timeout = 5
            }
        )
    }
    $submit = [ordered]@{
        matcher = ""
        hooks   = @(
            [ordered]@{
                type    = "command"
                command = "pwsh.exe -NoProfile -File `"$ResumePath`""
                timeout = 5
            }
        )
    }
    return [ordered]@{
        Stop             = @($stop)
        UserPromptSubmit = @($submit)
    }
}

if (-not $PrintHooksOnly) {
    Write-Host "Installing cache-countdown to $InstallDir ..."
    New-Item -ItemType Directory -Path (Join-Path $InstallDir "hooks") -Force | Out-Null

    Copy-Item -Path (Join-Path $srcRoot "hooks\*.ps1") -Destination (Join-Path $InstallDir "hooks") -Force
    Copy-Item -Path (Join-Path $srcRoot "src") -Destination $InstallDir -Recurse -Force
    Copy-Item -Path (Join-Path $srcRoot "pyproject.toml") -Destination $InstallDir -Force
    if (Test-Path (Join-Path $srcRoot "tests")) {
        Copy-Item -Path (Join-Path $srcRoot "tests") -Destination $InstallDir -Recurse -Force
    }

    $state = Join-Path $env:USERPROFILE ".claude\state"
    New-Item -ItemType Directory -Path $state -Force | Out-Null

    # Prepare the UV environment so `uv run cache-countdown` works.
    $uv = Get-Command uv -ErrorAction SilentlyContinue
    if ($uv) {
        Write-Host "Syncing UV environment ..."
        Push-Location $InstallDir
        try { & uv sync } catch { Write-Warning "uv sync failed: $_" }
        Pop-Location
    } else {
        Write-Warning "uv not found on PATH. Install UV, then run 'uv sync' in $InstallDir."
    }

    Write-Host "Installed. Run the ticker with:"
    Write-Host "  uv run --project `"$InstallDir`" cache-countdown --session <session-id>"
    Write-Host ""
}

$entries = Get-HookEntries -WritePath $writeScript -ResumePath $resumeScript

Write-Host "=== Add these two entries to ~/.claude/settings.json under `"hooks`" (Lane Z applies) ==="
$entries | ConvertTo-Json -Depth 6
