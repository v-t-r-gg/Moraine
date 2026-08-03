# Stage a coherent Windows suite for W2-E acceptance (not a supported installer).
[CmdletBinding()]
param(
    [Parameter(Mandatory = $true)]
    [string]$SourceDir,

    [string]$Destination = (Join-Path $env:LOCALAPPDATA "Programs\Moraine"),

    [string]$GitCommit = "",

    [string]$Version = "0.1.0",

    [ValidateSet("release", "debug")]
    [string]$Profile = "release",

    [string]$EvidenceDir = ""
)

$ErrorActionPreference = "Stop"
. (Join-Path $PSScriptRoot "_common.ps1")

if (Test-W2EIsAdministrator) {
    throw "Refuse to stage under an elevated token. Use a standard-user shell."
}

$SourceDir = (Resolve-Path $SourceDir).Path
$required = @("moraine.exe", "moraine-service.exe", "moraine-app.exe")
foreach ($name in $required) {
    $p = Join-Path $SourceDir $name
    if (-not (Test-Path $p)) {
        throw "Missing required suite binary: $p"
    }
}

New-Item -ItemType Directory -Force -Path $Destination | Out-Null
foreach ($name in $required) {
    Copy-Item -Force (Join-Path $SourceDir $name) (Join-Path $Destination $name)
}

# Optional assets if the developer produced a richer suite tree.
foreach ($rel in @(
        "share\moraine",
        "WebView2",
        "resources"
    )) {
    $src = Join-Path $SourceDir $rel
    if (Test-Path $src) {
        $dst = Join-Path $Destination $rel
        New-Item -ItemType Directory -Force -Path (Split-Path $dst -Parent) | Out-Null
        Copy-Item -Recurse -Force $src $dst
    }
}

$share = Join-Path $Destination "share\moraine"
New-Item -ItemType Directory -Force -Path $share | Out-Null

if (-not $GitCommit) {
    try {
        $GitCommit = (git -C (Get-W2ERepoRoot) rev-parse HEAD).Trim()
    } catch {
        $GitCommit = "unknown"
    }
}

$manifestPath = Join-Path $share "manifest.json"
$manifest = @{
    product                 = "Moraine"
    version                 = $Version
    gitCommit               = $GitCommit
    buildTimestamp          = [DateTimeOffset]::UtcNow.ToString("o")
    target                  = "x86_64-pc-windows-msvc"
    profile                 = $Profile
    schema                  = @{
        minimumReadable = 3
        maximumReadable = 6
        currentWritable = 6
    }
    serviceProtocolVersion  = 1
    mcpImplementationVersion = 1
    components              = @{
        cli     = $Version
        service = $Version
        desktop = $Version
    }
}
$manifest | ConvertTo-Json -Depth 6 | Set-Content -Encoding utf8 $manifestPath

$hashes = foreach ($name in $required) {
    $path = Join-Path $Destination $name
    $hash = (Get-FileHash -Algorithm SHA256 -Path $path).Hash.ToLowerInvariant()
    "{0}  {1}" -f $hash, $name
}
$hashText = ($hashes -join "`n") + "`n"
$hashPath = Join-Path $Destination "binary-hashes.txt"
Set-Content -Encoding utf8 -Path $hashPath -Value $hashText

$evidence = Get-W2EEvidenceRoot -Path $EvidenceDir
Write-W2EText -Path (Join-Path $evidence "binary-hashes.txt") -Text $hashText
Write-W2EJson -Path (Join-Path $evidence "staging.json") -Object ([pscustomobject]@{
        stagingCategory = "user-local Programs\Moraine (manual stage; not installer)"
        destinationCategory = "%LOCALAPPDATA%\Programs\Moraine"
        gitCommit       = $GitCommit
        version         = $Version
        profile         = $Profile
        target          = "x86_64-pc-windows-msvc"
        files           = $required
        manifestPathCategory = "share\moraine\manifest.json under staged prefix"
        notes           = @(
            "No global registry installation.",
            "No machine-wide PATH mutation.",
            "Session uses MORAINE_PREFIX via session-env.ps1."
        )
    })

Write-Host "Staged suite at: $Destination"
Write-Host "Hashes written to evidence: $evidence\binary-hashes.txt"
Write-Host "Dot-source session-env.ps1 in this shell (or a new one) before CLI use."
