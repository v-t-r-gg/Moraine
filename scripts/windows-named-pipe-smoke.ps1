$ErrorActionPreference = "Stop"

$repo = Split-Path -Parent $PSScriptRoot
$cli = Join-Path $repo "target\debug\moraine.exe"
$service = Join-Path $repo "target\debug\moraine-service.exe"
if (!(Test-Path $cli) -or !(Test-Path $service)) {
    throw "Windows smoke requires target\debug\moraine.exe & moraine-service.exe"
}

$token = [Guid]::NewGuid().ToString("N")
$pipe = "\\.\pipe\moraine.capture.smoke.$token"
$temp = Join-Path ([System.IO.Path]::GetTempPath()) "moraine-capture-smoke-$token"
$spool = Join-Path $temp "spool"
$stdout = Join-Path $temp "service.stdout.log"
$stderr = Join-Path $temp "service.stderr.log"
New-Item -ItemType Directory -Force -Path $spool | Out-Null

$portProbe = [System.Net.Sockets.TcpListener]::new(
    [System.Net.IPAddress]::Loopback,
    0
)
$portProbe.Start()
$port = ([System.Net.IPEndPoint]$portProbe.LocalEndpoint).Port
$portProbe.Stop()

$process = $null
try {
    $process = Start-Process `
        -FilePath $service `
        -ArgumentList @(
            "--named-pipe", $pipe,
            "--spool-dir", $spool,
            "--http", "127.0.0.1:$port"
        ) `
        -RedirectStandardOutput $stdout `
        -RedirectStandardError $stderr `
        -PassThru

    $ready = $false
    for ($attempt = 0; $attempt -lt 100; $attempt++) {
        if ($process.HasExited) {
            throw "moraine-service exited before capture became ready"
        }
        try {
            $status = Invoke-RestMethod `
                -Uri "http://127.0.0.1:$port/status" `
                -TimeoutSec 1
            if ($status.captureReady -eq $true -and
                $status.captureEndpoint.kind -eq "windows_named_pipe" -and
                $status.captureEndpoint.value -eq $pipe) {
                $ready = $true
                break
            }
        } catch {
            Start-Sleep -Milliseconds 50
        }
    }
    if (!$ready) {
        throw "moraine-service did not report the explicit named pipe ready"
    }

    $session = "windows-smoke-$token"
    $payload = @{
        hook_event_name = "SessionStart"
        session_id = $session
        cwd = $repo
        model = "w2-smoke"
    } | ConvertTo-Json -Compress
    $payload | & $cli hook-codex `
        --named-pipe $pipe `
        --spool-dir $spool
    if ($LASTEXITCODE -ne 0) {
        throw "hook-codex returned $LASTEXITCODE"
    }

    $captured = $false
    for ($attempt = 0; $attempt -lt 100; $attempt++) {
        $captured = Get-ChildItem -Path $spool -Recurse -Filter "*.json" |
            Where-Object {
                $_.Name -ne "index.json" -and
                (Get-Content -Raw $_.FullName) -match [regex]::Escape($session)
            } |
            Select-Object -First 1
        if ($captured) {
            break
        }
        Start-Sleep -Milliseconds 50
    }
    if (!$captured) {
        throw "hook event did not reach the durable spool"
    }
} finally {
    if ($process -and !$process.HasExited) {
        Stop-Process -Id $process.Id -Force
        $process.WaitForExit()
    }
    if (Test-Path $temp) {
        Remove-Item -Recurse -Force $temp
    }
}

Write-Host "Windows named-pipe hook-to-spool smoke passed"
