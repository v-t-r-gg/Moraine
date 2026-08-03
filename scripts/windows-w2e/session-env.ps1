# Dot-source in a non-elevated Account A PowerShell session.
# Sets MORAINE_PREFIX + PATH for the acceptance staging directory.
$ErrorActionPreference = "Stop"

$prefix = Join-Path $env:LOCALAPPDATA "Programs\Moraine"
if (-not (Test-Path (Join-Path $prefix "moraine.exe"))) {
    Write-Warning "Staged CLI not found at $prefix\moraine.exe — run stage-suite.ps1 first."
}

$env:MORAINE_PREFIX = $prefix
$env:PATH = "$prefix;$env:PATH"

# Avoid accidental cargo-dev shadowing during acceptance.
$env:MORAINE_W2E_SESSION = "1"

Write-Host "MORAINE_PREFIX=$env:MORAINE_PREFIX"
Write-Host "moraine.exe => $((Get-Command moraine.exe -ErrorAction SilentlyContinue).Source)"
Write-Host "Elevation check: $([Security.Principal.WindowsPrincipal]::new([Security.Principal.WindowsIdentity]::GetCurrent()).IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator))"
