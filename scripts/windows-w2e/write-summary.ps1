# Build acceptance-summary.json from collected local evidence (operator-assisted).
[CmdletBinding()]
param(
    [string]$EvidenceDir = "",
    [ValidateSet("not_executed", "in_progress", "passed", "failed")]
    [string]$Disposition = "in_progress",
    [string]$GitCommit = "",
    [string[]]$FailedGates = @(),
    [string[]]$PassedGates = @(),
    [string]$Notes = ""
)

$ErrorActionPreference = "Stop"
. (Join-Path $PSScriptRoot "_common.ps1")

$evidence = Get-W2EEvidenceRoot -Path $EvidenceDir
if (-not $GitCommit) {
    try {
        $GitCommit = (git -C (Get-W2ERepoRoot) rev-parse HEAD).Trim()
    } catch {
        $GitCommit = "unknown"
    }
}

$mandatoryGates = @(
    "environment_recorded",
    "account_a_standard_user",
    "suite_staged_no_elevation",
    "preflight_cli",
    "native_desktop_onboarding",
    "task_scheduler_registration",
    "login_autostart",
    "named_pipe_capture",
    "real_codex_activity",
    "capture_without_desktop",
    "demand_lifecycle",
    "no_service_console_window",
    "cross_account_pipe_denial",
    "graphical_health_repair",
    "graphical_rollback",
    "uninstall_preserves_ledgers",
    "evidence_sanitized"
)

$summary = [pscustomobject]@{
    milestone              = "W2-E"
    title                  = "Windows 11 standard-user runtime acceptance"
    testedCommit           = $GitCommit
    acceptanceDateUtc      = [DateTimeOffset]::UtcNow.ToString("o")
    disposition            = $Disposition
    evidenceKind           = @(
        "automated CI (separate; not sufficient alone)",
        "CLI evidence",
        "graphical evidence",
        "sign-in/restart evidence",
        "cross-account evidence",
        "manual observation"
    )
    mandatoryGates         = $mandatoryGates
    passedGates            = @($PassedGates)
    failedGates            = @($FailedGates)
    publicClaimsPromoted   = $false
    installationSupported  = $false
    architectureClaimed    = "Windows 11 x86-64 standard-user runtime (manually staged suite only)"
    notes                  = $Notes
    limitations            = @(
        "Does not validate installer, upgrade, signing, or WinGet.",
        "Does not claim architectures other than the tested host.",
        "Hosted CI uses an administrator account and cannot substitute for this package."
    )
}

Write-W2EJson -Path (Join-Path $evidence "acceptance-summary.json") -Object $summary
Write-Host "Wrote $(Join-Path $evidence 'acceptance-summary.json') disposition=$Disposition"
if ($Disposition -eq "passed" -and $FailedGates.Count -gt 0) {
    Write-Warning "Disposition is passed but failed gates were listed."
}
if ($Disposition -ne "passed") {
    Write-Host "Public README/ARCHITECTURE/ROADMAP promotion must wait for disposition=passed."
}
