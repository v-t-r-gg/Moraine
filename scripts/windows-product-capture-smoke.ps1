$ErrorActionPreference = "Stop"

$repo = Split-Path -Parent $PSScriptRoot
Push-Location $repo
try {
    cargo test `
        -p moraine-provision `
        --test windows_product_closure `
        manually_staged_windows_suite_reaches_product_ready `
        -- --exact --nocapture
    if ($LASTEXITCODE -ne 0) {
        throw "Windows ProductCapture closure smoke returned $LASTEXITCODE"
    }
} finally {
    Pop-Location
}

Write-Host "Windows ProductCapture closure smoke passed"
