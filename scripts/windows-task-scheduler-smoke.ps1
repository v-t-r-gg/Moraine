$ErrorActionPreference = "Stop"

$repo = Split-Path -Parent $PSScriptRoot
$cli = Join-Path $repo "target\debug\moraine.exe"
$service = Join-Path $repo "target\debug\moraine-service.exe"
if (!(Test-Path $cli) -or !(Test-Path $service)) {
    throw "Task Scheduler smoke requires target\debug\moraine.exe & moraine-service.exe"
}

Push-Location $repo
try {
    cargo test `
        -p moraine-provision `
        --test windows_task_scheduler_contract `
        scheduled_real_service_accepts_a_real_hook_and_writes_logs `
        -- --exact --nocapture
    if ($LASTEXITCODE -ne 0) {
        throw "production Task Scheduler smoke returned $LASTEXITCODE"
    }
} finally {
    Pop-Location
}

Write-Host "Windows Task Scheduler runtime smoke passed"
