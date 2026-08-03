# Collect redacted CLI command results for W2-E phases.
[CmdletBinding()]
param(
    [Parameter(Mandatory = $true)]
    [ValidateSet("preflight", "runtime", "lifecycle", "autostart", "uninstall")]
    [string]$Phase,

    [string]$ProjectPath = "",

    [string]$EvidenceDir = ""
)

$ErrorActionPreference = "Stop"
. (Join-Path $PSScriptRoot "_common.ps1")

if (Test-W2EIsAdministrator) {
    throw "CLI collection must run non-elevated."
}

$evidence = Get-W2EEvidenceRoot -Path $EvidenceDir
$outDir = Join-Path $evidence "command-results\$Phase"
New-Item -ItemType Directory -Force -Path $outDir | Out-Null

function Save-Cmd {
    param([string]$Name, [string[]]$Args, [switch]$AllowNonZero)
    $path = Join-Path $outDir "$Name.txt"
    $result = Invoke-W2EMoraine -Arguments $Args -OutFile $path -AllowNonZero:$AllowNonZero
    Write-Host ("{0}: exit {1}" -f $Name, $result.ExitCode)
    return $result
}

switch ($Phase) {
    "preflight" {
        Save-Cmd version @( "version", "--json" )
        Save-Cmd doctor @( "doctor", "--json" ) -AllowNonZero
        Save-Cmd service_status @( "service", "status", "--json" )
    }
    "runtime" {
        if (-not $ProjectPath) { throw "-ProjectPath is required for runtime phase" }
        Save-Cmd service_status @( "service", "status", "--json" )
        Save-Cmd doctor_project @(
            "doctor", "--project", $ProjectPath, "--integration", "codex", "--json"
        ) -AllowNonZero
        Save-Cmd service_logs @( "service", "logs", "--json" ) -AllowNonZero
    }
    "lifecycle" {
        Save-Cmd service_stop @( "service", "stop", "--json" )
        Save-Cmd service_status_after_stop @( "service", "status", "--json" )
        Save-Cmd service_start @( "service", "start", "--json" )
        Save-Cmd service_status_after_start @( "service", "status", "--json" )
        Save-Cmd service_restart @( "service", "restart", "--json" )
        Save-Cmd service_status_after_restart @( "service", "status", "--json" )
        Save-Cmd service_logs @( "service", "logs", "--json" ) -AllowNonZero
        if ($ProjectPath) {
            Save-Cmd self_test @(
                "self-test", "--project", $ProjectPath, "--json"
            ) -AllowNonZero
        }
    }
    "autostart" {
        if (-not $ProjectPath) { throw "-ProjectPath is required for autostart phase" }
        Save-Cmd service_status @( "service", "status", "--json" )
        Save-Cmd self_test @(
            "self-test", "--project", $ProjectPath, "--json"
        ) -AllowNonZero
    }
    "uninstall" {
        Save-Cmd service_uninstall @( "service", "uninstall", "--json" )
        Save-Cmd service_status @( "service", "status", "--json" )
        if ($ProjectPath) {
            $ledger = Join-Path $ProjectPath ".moraine"
            $ledgerNote = if (Test-Path $ledger) {
                "project ledger present after uninstall"
            } else {
                "PROJECT LEDGER MISSING AFTER UNINSTALL"
            }
            Write-W2EText -Path (Join-Path $outDir "ledger-presence.txt") -Text $ledgerNote
            Write-Host $ledgerNote
        }
    }
}

Write-Host "CLI phase '$Phase' written under $outDir"
