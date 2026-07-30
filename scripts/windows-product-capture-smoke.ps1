$ErrorActionPreference = "Stop"

$repo = Split-Path -Parent $PSScriptRoot
$builtCli = Join-Path $repo "target\debug\moraine.exe"
$builtService = Join-Path $repo "target\debug\moraine-service.exe"
if (!(Test-Path $builtCli) -or !(Test-Path $builtService)) {
    throw "Windows ProductCapture smoke requires debug CLI & service binaries"
}

$token = [Guid]::NewGuid().ToString("N")
$temp = Join-Path ([System.IO.Path]::GetTempPath()) "moraine-product-smoke-$token"
$prefix = Join-Path $temp "Moraine Suite"
$project = Join-Path $temp "Project With Spaces"
$spool = Join-Path $temp "spool"
$registry = Join-Path $temp "projects.json"
$cli = Join-Path $prefix "moraine.exe"
$service = Join-Path $prefix "moraine-service.exe"
$codex = Join-Path $prefix "codex.exe"
$taskInstalled = $false

function Invoke-MoraineJson {
    param([Parameter(ValueFromRemainingArguments = $true)][string[]]$Arguments)
    $raw = & $script:cli @Arguments
    if ($LASTEXITCODE -ne 0) {
        throw "moraine $($Arguments -join ' ') returned $LASTEXITCODE`n$raw"
    }
    return ($raw | ConvertFrom-Json)
}

New-Item -ItemType Directory -Force -Path $prefix, $project, $spool | Out-Null
Copy-Item $builtCli $cli
Copy-Item $builtService $service
# Detection needs a real Windows executable named codex.exe. Its version output
# is advisory; ProductCapture still invokes the staged Moraine CLI.
Copy-Item $builtCli $codex

$share = Join-Path $prefix "share\moraine"
New-Item -ItemType Directory -Force -Path $share | Out-Null
$manifest = @{
    product = "Moraine"
    version = "0.1.0"
    gitCommit = "windows-product-smoke"
    buildTimestamp = [DateTimeOffset]::UtcNow.ToString("o")
    target = "x86_64-pc-windows-msvc"
    profile = "debug"
    schema = @{
        minimumReadable = 3
        maximumReadable = 6
        currentWritable = 6
    }
    serviceProtocolVersion = 1
    mcpImplementationVersion = 1
    components = @{
        cli = "0.1.0"
        service = "0.1.0"
        desktop = "missing"
    }
}
$manifest | ConvertTo-Json -Depth 5 |
    Set-Content -Encoding utf8 (Join-Path $share "manifest.json")

$priorPrefix = $env:MORAINE_PREFIX
$priorSpool = $env:MORAINE_SPOOL_DIR
$priorRegistry = $env:MORAINE_PROJECT_REGISTRY
$priorPath = $env:PATH

try {
    $env:MORAINE_PREFIX = $prefix
    $env:MORAINE_SPOOL_DIR = $spool
    $env:MORAINE_PROJECT_REGISTRY = $registry
    $env:PATH = "$prefix;$priorPath"

    $initial = Invoke-MoraineJson service status --json
    if ($initial.service.registrationPresent) {
        throw "refusing to replace a pre-existing SID-scoped Moraine task"
    }

    $version = Invoke-MoraineJson version --json
    if (!$version.ok) {
        throw "staged suite version report is incoherent"
    }

    $before = & $cli doctor --project $project --integration codex --json |
        ConvertFrom-Json
    if ($LASTEXITCODE -eq 0 -or $before.ok) {
        throw "doctor unexpectedly reported Ready before setup"
    }

    $enabled = Invoke-MoraineJson enable --project $project --json
    $taskInstalled = $true
    if ($enabled.outcome -ne "ready" -or $enabled.receipt.readiness -ne "ready") {
        throw "moraine enable did not return Ready"
    }

    $status = Invoke-MoraineJson service status --json
    if (!$status.service.registrationValid -or
        !$status.service.autostartEnabled -or
        !$status.service.diagnosticsReady -or
        !$status.service.captureReady) {
        throw "enabled runtime is not fully ready"
    }

    $verified = Invoke-MoraineJson self-test --project $project --json
    if (!$verified.ok -or $verified.readiness -ne "ready") {
        throw "moraine self-test did not return Ready"
    }

    $doctor = Invoke-MoraineJson doctor --project $project --integration codex --json
    if (!$doctor.ok) {
        throw "doctor did not report the configured staged suite healthy"
    }

    $logs = Invoke-MoraineJson service logs --json
    if (!$logs.ok -or $null -eq $logs.logs) {
        throw "service logs JSON envelope is invalid"
    }

    & $cli service stop --json | Out-Null
    if ($LASTEXITCODE -ne 0) {
        throw "moraine service stop returned $LASTEXITCODE"
    }
    $stopped = Invoke-MoraineJson service status --json
    if ($stopped.service.running) {
        throw "runtime remained running after stop"
    }

    $stoppedDoctorRaw = & $cli doctor --project $project --integration codex --json
    $stoppedDoctor = $stoppedDoctorRaw | ConvertFrom-Json
    if ($LASTEXITCODE -eq 0 -or
        -not (($stoppedDoctor.checks.remediation -join "`n") -match "service start")) {
        throw "doctor did not diagnose a stopped runtime with the start repair"
    }

    $repaired = Invoke-MoraineJson service start --json
    if (!$repaired.service.running) {
        throw "public start repair did not restart the runtime"
    }
    for ($attempt = 0; $attempt -lt 100; $attempt++) {
        $repaired = Invoke-MoraineJson service status --json
        if ($repaired.service.diagnosticsReady -and $repaired.service.captureReady) {
            break
        }
        Start-Sleep -Milliseconds 50
    }
    if (!$repaired.service.diagnosticsReady -or !$repaired.service.captureReady) {
        throw "runtime did not become capture-ready after repair"
    }

    Invoke-MoraineJson service uninstall --json | Out-Null
    $taskInstalled = $false
    $removed = Invoke-MoraineJson service status --json
    if ($removed.service.registrationPresent) {
        throw "runtime registration remains after uninstall"
    }
    if (!(Test-Path (Join-Path $project ".moraine"))) {
        throw "runtime uninstall removed project ledger records"
    }
} finally {
    if ($taskInstalled) {
        try {
            & $cli service uninstall --json | Out-Null
        } catch {
            Write-Warning "failed to remove disposable Moraine task: $_"
        }
    }
    $env:MORAINE_PREFIX = $priorPrefix
    $env:MORAINE_SPOOL_DIR = $priorSpool
    $env:MORAINE_PROJECT_REGISTRY = $priorRegistry
    $env:PATH = $priorPath
    if (Test-Path $temp) {
        Remove-Item -Recurse -Force $temp
    }
}

Write-Host "Windows CLI ProductCapture lifecycle smoke passed"
