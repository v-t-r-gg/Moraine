# Place the unzipped Windows demo suite where SuitePaths already looks.
# Standard user only. This is not an installer.
#Requires -Version 5.1
param(
    [string]$Prefix
)

$ErrorActionPreference = "Stop"

function Test-ElevatedToken {
    $identity = [Security.Principal.WindowsIdentity]::GetCurrent()
    $principal = New-Object Security.Principal.WindowsPrincipal($identity)
    return $principal.IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)
}

if (Test-ElevatedToken) {
    Write-Host "This demo is a standard-user suite. Do not run elevated. Stage the demo suite; no installer is available."
    exit 2
}

if ($env:OS -ne "Windows_NT") {
    Write-Host "stage-windows-demo.ps1 runs on Windows after unzip. It does not install Linux."
    exit 1
}

# SuitePaths / default_prefix: %LOCALAPPDATA%\Moraine
# Executables are moraine.exe, moraine-service.exe, and moraine-app.exe in that
# directory. The manifest is share\moraine\manifest.json. Do not use suite\bin.
$Discovered = Join-Path $env:LOCALAPPDATA "Moraine"

if (-not $Prefix) {
    if ($env:MORAINE_PREFIX) {
        $existing = $env:MORAINE_PREFIX.TrimEnd('\')
        if ($existing -ine $Discovered.TrimEnd('\')) {
            Write-Host "MORAINE_PREFIX is '$existing', which is not the discovered prefix '$Discovered'. Pass -Prefix '$existing' or unset MORAINE_PREFIX. This script will not create a second install root."
            exit 3
        }
    }
    $Prefix = $Discovered
    $CustomPrefix = $false
} else {
    $CustomPrefix = $true
}

$FullPrefix = [System.IO.Path]::GetFullPath($Prefix)
$Blocked = @($env:ProgramFiles, ${env:ProgramFiles(x86)}, $env:SystemRoot)
foreach ($Root in $Blocked) {
    if (-not $Root) { continue }
    $BlockedRoot = [System.IO.Path]::GetFullPath($Root).TrimEnd('\')
    if ($FullPrefix.TrimEnd('\').StartsWith($BlockedRoot, [System.StringComparison]::OrdinalIgnoreCase)) {
        Write-Host "Refusing to write under $BlockedRoot. The demo uses a user prefix only."
        exit 4
    }
}

$Here = $PSScriptRoot
$Bin = Join-Path $Here "bin"
foreach ($Name in @("moraine.exe", "moraine-service.exe", "moraine-app.exe")) {
    $Source = Join-Path $Bin $Name
    if (-not (Test-Path -LiteralPath $Source)) {
        Write-Host "Missing $Source. Run this script from the unzipped demo archive."
        exit 5
    }
}
$Manifest = Join-Path $Here "manifest.json"
if (-not (Test-Path -LiteralPath $Manifest)) {
    Write-Host "Missing $Manifest. The archive manifest is required."
    exit 5
}

New-Item -ItemType Directory -Force -Path $FullPrefix | Out-Null
$Share = Join-Path $FullPrefix "share\moraine"
New-Item -ItemType Directory -Force -Path $Share | Out-Null
foreach ($Name in @("moraine.exe", "moraine-service.exe", "moraine-app.exe")) {
    Copy-Item -LiteralPath (Join-Path $Bin $Name) -Destination (Join-Path $FullPrefix $Name) -Force
}
Copy-Item -LiteralPath $Manifest -Destination (Join-Path $Share "manifest.json") -Force

function Add-UserPathOnce {
    param([string]$Entry)
    $Normalized = $Entry.TrimEnd('\')
    $UserPath = [Environment]::GetEnvironmentVariable("Path", "User")
    if (-not $UserPath) { $UserPath = "" }
    $Present = $false
    foreach ($Part in ($UserPath -split ';')) {
        if (-not $Part) { continue }
        if ($Part.TrimEnd('\') -ieq $Normalized) { $Present = $true; break }
    }
    if (-not $Present) {
        $Updated = if ($UserPath) { "$Entry;$UserPath" } else { $Entry }
        [Environment]::SetEnvironmentVariable("Path", $Updated, "User")
        Write-Host "Added to user PATH: $Entry"
    } else {
        Write-Host "User PATH already contains: $Entry"
    }
    $SessionPresent = $false
    foreach ($Part in ($env:Path -split ';')) {
        if (-not $Part) { continue }
        if ($Part.TrimEnd('\') -ieq $Normalized) { $SessionPresent = $true; break }
    }
    if (-not $SessionPresent) {
        $env:Path = "$Entry;$env:Path"
    }
}

Add-UserPathOnce -Entry $FullPrefix

if ($CustomPrefix) {
    [Environment]::SetEnvironmentVariable("MORAINE_PREFIX", $FullPrefix, "User")
    $env:MORAINE_PREFIX = $FullPrefix
    Write-Host "Set user MORAINE_PREFIX=$FullPrefix"
}

# Ask the shell to notice the user environment. This does not write HKLM.
Add-Type -Namespace MoraineDemo -Name EnvBroadcast -MemberDefinition @"
[System.Runtime.InteropServices.DllImport("user32.dll", SetLastError=true, CharSet=System.Runtime.InteropServices.CharSet.Auto)]
public static extern System.IntPtr SendMessageTimeout(System.IntPtr hWnd, uint Msg, System.UIntPtr wParam, string lParam, uint fuFlags, uint uTimeout, out System.UIntPtr lpdwResult);
"@
$BroadcastResult = [UIntPtr]::Zero
[MoraineDemo.EnvBroadcast]::SendMessageTimeout(
    [IntPtr]0xffff,
    0x001A,
    [UIntPtr]::Zero,
    "Environment",
    2,
    5000,
    [ref]$BroadcastResult
) | Out-Null

$Demo = Join-Path $Here "examples\demo-project"
Write-Host ""
Write-Host "Staged suite at $FullPrefix"
Write-Host "Installer unsupported. Acceptance pending. Product Ready remains No."
Write-Host "Open a new PowerShell window, then run:"
Write-Host ""
Write-Host "  moraine --version"
Write-Host "  moraine doctor"
Write-Host "  moraine project init `"$Demo`""
Write-Host ""
Write-Host "This script does not run setup."
return
