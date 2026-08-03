# Record sanitized environment evidence for W2-E.
[CmdletBinding()]
param(
    [string]$EvidenceDir = "",
    [string]$GitCommit = ""
)

$ErrorActionPreference = "Stop"
. (Join-Path $PSScriptRoot "_common.ps1")

if (Test-W2EIsAdministrator) {
    throw "Environment collection must run under a non-elevated standard-user token."
}

$evidence = Get-W2EEvidenceRoot -Path $EvidenceDir
$token = Get-W2ETokenSummary

if (-not $GitCommit) {
    try {
        $GitCommit = (git -C (Get-W2ERepoRoot) rev-parse HEAD).Trim()
    } catch {
        $GitCommit = "unknown"
    }
}

$os = Get-CimInstance Win32_OperatingSystem
$cs = Get-CimInstance Win32_ComputerSystem
$cpu = Get-CimInstance Win32_Processor | Select-Object -First 1

$webview = $null
try {
    $wvKey = "HKLM:\SOFTWARE\WOW6432Node\Microsoft\EdgeUpdate\Clients\{F3017226-FE2A-4295-8BDF-00C3A9A7E4C5}"
    if (Test-Path $wvKey) {
        $webview = (Get-ItemProperty $wvKey).pv
    }
} catch {
    $webview = $null
}

$codex = $null
$codexCmd = Get-Command codex -ErrorAction SilentlyContinue
if (-not $codexCmd) {
    $codexCmd = Get-Command codex.exe -ErrorAction SilentlyContinue
}
if ($codexCmd) {
    try {
        $codexVersion = & $codexCmd.Source --version 2>&1 | Out-String
        $codex = @{
            present = $true
            version = (Protect-W2EText $codexVersion.Trim())
            pathCategory = "on PATH (path redacted)"
        }
    } catch {
        $codex = @{ present = $true; version = "present but --version failed" }
    }
} else {
    $codex = @{ present = $false; version = $null }
}

$virtualized = $false
if ($cs.Model -match 'Virtual|VMware|VirtualBox|KVM|Hyper-V|QEMU|HVM') {
    $virtualized = $true
}
if ($cs.Manufacturer -match 'Microsoft Corporation' -and $cs.Model -match 'Virtual') {
    $virtualized = $true
}

$envReport = [pscustomobject]@{
    acceptanceDateUtc     = [DateTimeOffset]::UtcNow.ToString("o")
    moraineCommit         = $GitCommit
    windowsEdition        = $os.Caption
    windowsVersion        = $os.Version
    osBuild               = $os.BuildNumber
    architecture          = $env:PROCESSOR_ARCHITECTURE
    virtualizedOrPhysical = if ($virtualized) { "virtualized" } else { "physical-or-unclassified" }
    computerModel         = Protect-W2EText $cs.Model
    manufacturer          = Protect-W2EText $cs.Manufacturer
    cpuName               = Protect-W2EText $cpu.Name
    webView2Version       = $webview
    codex                 = $codex
    uacEnabled            = $null
    account               = $token
    stagingPrefixCategory = "%LOCALAPPDATA%\Programs\Moraine"
    defaultPrefixCategory = "%LOCALAPPDATA%\Moraine"
    morainePrefixSet      = [bool]$env:MORAINE_PREFIX
    notes                 = @(
        "Usernames and full SIDs are redacted.",
        "Account A and Account B must not be local Administrators.",
        "Hosted CI evidence is separate and does not substitute for this package."
    )
}

try {
    $uac = Get-ItemProperty "HKLM:\SOFTWARE\Microsoft\Windows\CurrentVersion\Policies\System"
    $envReport.uacEnabled = ($uac.EnableLUA -eq 1)
} catch {
    $envReport.uacEnabled = "unreadable"
}

Write-W2EJson -Path (Join-Path $evidence "environment.json") -Object $envReport
Write-Host "Wrote $(Join-Path $evidence 'environment.json')"
if ($token.elevated -or $token.accountClass -ne "standard-user") {
    Write-Warning "Token does not look like a standard non-admin user. W2-E cannot pass with this token."
}
