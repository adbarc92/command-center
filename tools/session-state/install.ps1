# install.ps1 — installer for the session-state hooks (Windows / PowerShell 7).
# Wires SessionStart(resume) + Stop(scratch) + SessionEnd(boundary) into ~/.claude/settings.json,
# invoking python.exe DIRECTLY (not uv run) for cold-start speed. Idempotent by basename marker.
#
#   pwsh -NoProfile -File install.ps1                  # install + wire hooks
#   pwsh -NoProfile -File install.ps1 -PrintHooksOnly  # print the hook JSON only
#   pwsh -NoProfile -File install.ps1 -Uninstall       # remove our 3 hook entries
#   pwsh -NoProfile -File install.ps1 -Uninstall -Purge# also delete ~/.claude/state/sessions
param(
    [switch]$PrintHooksOnly,
    [switch]$Uninstall,
    [switch]$Purge,
    [switch]$DryRun,
    [string]$InstallDir = (Join-Path $env:USERPROFILE ".claude\tools\session-state"),
    [string]$SettingsPath = (Join-Path $env:USERPROFILE ".claude\settings.json")
)
$ErrorActionPreference = "Stop"
[Console]::OutputEncoding = [System.Text.Encoding]::UTF8

$markers = @("session_state/resume.py", "session_state/capture_scratch.py", "session_state/capture_end.py")

function Resolve-Python {
    $py = Get-Command python.exe -ErrorAction SilentlyContinue
    if (-not $py) { throw "python.exe not found on PATH." }
    return $py.Source
}

function New-Entries {
    param([string]$Py, [string]$Dir)
    $d = $Dir -replace '\\','/'
    return @{
        SessionStart = @(@{ hooks = @(@{ type="command"; command="`"$Py`" `"$d/src/session_state/resume.py`"" }) })
        Stop         = @(@{ hooks = @(@{ type="command"; command="`"$Py`" `"$d/src/session_state/capture_scratch.py`""; timeout=5 }) })
        SessionEnd   = @(@{ hooks = @(@{ type="command"; command="`"$Py`" `"$d/src/session_state/capture_end.py`"" }) })
    }
}

function Remove-OurEntries {
    param($Hooks)
    foreach ($evt in @("SessionStart","Stop","SessionEnd")) {
        if ($Hooks.$evt) {
            $Hooks.$evt = @($Hooks.$evt | Where-Object {
                $cmd = ($_.hooks | ForEach-Object { $_.command }) -join " "
                -not ($markers | Where-Object { $cmd -like "*$_*" })
            })
        }
    }
    return $Hooks
}

if ($PrintHooksOnly) {
    (New-Entries -Py (Resolve-Python) -Dir $InstallDir) | ConvertTo-Json -Depth 8
    return
}

# 1. copy files (skip on uninstall)
if (-not $Uninstall -and -not $DryRun) {
    New-Item -ItemType Directory -Path $InstallDir -Force | Out-Null
    Copy-Item (Join-Path $PSScriptRoot "src") $InstallDir -Recurse -Force
    Copy-Item (Join-Path $PSScriptRoot "pyproject.toml") $InstallDir -Force
    if (Test-Path (Join-Path $PSScriptRoot "tests")) { Copy-Item (Join-Path $PSScriptRoot "tests") $InstallDir -Recurse -Force }
}

# 2. load settings
$settings = if (Test-Path $SettingsPath) { Get-Content $SettingsPath -Raw | ConvertFrom-Json -AsHashtable } else { @{} }
if (-not $settings.hooks) { $settings.hooks = @{} }

# 3. always strip our prior entries first (idempotent; replaces path-drifted ones)
$settings.hooks = Remove-OurEntries $settings.hooks

# 4. add fresh entries unless uninstalling
if (-not $Uninstall) {
    $entries = New-Entries -Py (Resolve-Python) -Dir $InstallDir
    foreach ($evt in $entries.Keys) {
        if (-not $settings.hooks.$evt) { $settings.hooks.$evt = @() }
        $settings.hooks.$evt = @($settings.hooks.$evt) + $entries.$evt
    }
}

$json = $settings | ConvertTo-Json -Depth 12
if ($DryRun) { Write-Host $json; return }
Set-Content -Path $SettingsPath -Value $json -Encoding UTF8
Write-Host "session-state hooks $($Uninstall ? 'removed' : 'installed')."

if ($Uninstall -and $Purge) {
    $state = Join-Path $env:USERPROFILE ".claude\state\sessions"
    if (Test-Path $state) { Remove-Item $state -Recurse -Force; Write-Host "purged $state" }
}
