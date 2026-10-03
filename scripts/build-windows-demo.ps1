# Build a versioned Windows x86-64 demo suite archive.
# This is not an installer. It does not register a scheduled task, write the
# user profile, or require elevation. Run it on Windows 11 x86-64.
# A Linux invocation is not a Windows demo.
$ErrorActionPreference = "Stop"

if ($env:OS -ne "Windows_NT") {
    Write-Host "build-windows-demo.ps1 must run on Windows. A Linux build is not a Windows demo."
    exit 1
}

$Root = Split-Path -Parent $PSScriptRoot
Set-Location $Root

function Read-WorkspaceVersion {
    $inPackage = $false
    foreach ($line in Get-Content -Path (Join-Path $Root "Cargo.toml")) {
        if ($line -match '^\[workspace\.package\]') {
            $inPackage = $true
            continue
        }
        if ($line -match '^\[') {
            if ($inPackage) { break }
            continue
        }
        if ($inPackage -and $line -match '^\s*version\s*=\s*"([^"]+)"') {
            return $Matches[1]
        }
    }
    throw "workspace.package version not found in Cargo.toml"
}

function Invoke-Checked {
    param([scriptblock]$Command)
    & $Command
    if ($LASTEXITCODE -ne 0) {
        throw "command failed with exit $LASTEXITCODE"
    }
}

$Version = if ($env:MORAINE_VERSION) { $env:MORAINE_VERSION } else { Read-WorkspaceVersion }
if ($env:MORAINE_GIT_COMMIT) {
    $Commit = $env:MORAINE_GIT_COMMIT
} else {
    $Commit = (& git -C $Root rev-parse HEAD 2>$null)
    if (-not $Commit) { $Commit = "unknown" }
    $Dirty = & git -C $Root status --porcelain 2>$null
    if ($Dirty) { $Commit = "$Commit-dirty" }
}
$Target = if ($env:MORAINE_TARGET_TRIPLE) { $env:MORAINE_TARGET_TRIPLE } else { "x86_64-pc-windows-msvc" }
$OutDir = if ($env:MORAINE_RELEASE_DIR) { $env:MORAINE_RELEASE_DIR } else { Join-Path $Root "dist" }
$StageName = "moraine-$Version-windows-x86_64"
$Stage = Join-Path $OutDir $StageName
$Archive = Join-Path $OutDir "$StageName.zip"

$env:MORAINE_GIT_COMMIT = $Commit
$env:MORAINE_TARGET_TRIPLE = $Target
$env:MORAINE_BUILD_PROFILE = "release"
$env:VERSION = $Version

Write-Host "Building Moraine $Version ($Commit) for $Target"
Write-Host "staged suite, installer unsupported, acceptance pending"

if (Test-Path $Stage) { Remove-Item -Recurse -Force $Stage }
New-Item -ItemType Directory -Force -Path (Join-Path $Stage "bin") | Out-Null

Write-Host "==> cargo release binaries"
Invoke-Checked { cargo build --release -p moraine-cli -p moraine-service }
$Release = Join-Path $Root "target\release"
Copy-Item (Join-Path $Release "moraine.exe") (Join-Path $Stage "bin\moraine.exe") -Force
Copy-Item (Join-Path $Release "moraine-service.exe") (Join-Path $Stage "bin\moraine-service.exe") -Force

# Same desktop release path as scripts/build-linux-release.sh.
# Do not run `tauri build` bundling: that emits an installer, and this demo
# must not ship MSIX, NSIS, or a setup executable.
Write-Host "==> desktop (npm run build; cargo build --release -p moraine-app)"
if (-not (Get-Command npm -ErrorAction SilentlyContinue)) {
    throw "npm is required to build moraine-app.exe"
}
& npm ci --ignore-scripts
if ($LASTEXITCODE -ne 0) {
    & npm install --ignore-scripts
    if ($LASTEXITCODE -ne 0) { throw "npm install failed with exit $LASTEXITCODE" }
}
Invoke-Checked { npm run build }
Invoke-Checked { cargo build --release -p moraine-app }
$App = Join-Path $Release "moraine-app.exe"
if (-not (Test-Path $App)) {
    throw "primary demo package missing bin/moraine-app.exe"
}
Copy-Item $App (Join-Path $Stage "bin\moraine-app.exe") -Force

Copy-Item (Join-Path $Root "docs\windows-demo.md") (Join-Path $Stage "DEMO.md") -Force
Copy-Item (Join-Path $Root "LICENSE") (Join-Path $Stage "LICENSE") -Force
Copy-Item (Join-Path $Root "scripts\packaging\WINDOWS_DEMO_NOTICES") (Join-Path $Stage "NOTICES") -Force
Copy-Item (Join-Path $Root "scripts\stage-windows-demo.ps1") (Join-Path $Stage "stage-windows-demo.ps1") -Force

$DemoDest = Join-Path $Stage "examples\demo-project"
Copy-Item -Recurse (Join-Path $Root "examples\demo-project") $DemoDest
Push-Location $DemoDest
try {
    Invoke-Checked { git init -b main }
    Invoke-Checked { git add -- . }
    Invoke-Checked {
        git -c user.email="moraine-demo@example.invalid" -c user.name="Moraine Demo" commit -m "Initial demo project"
    }
} finally {
    Pop-Location
}

$Python = $null
foreach ($Candidate in @("python", "python3")) {
    if (Get-Command $Candidate -ErrorAction SilentlyContinue) {
        $Python = $Candidate
        break
    }
}
if (-not $Python) { throw "python is required for scripts/packaging/write_manifest.py" }
Invoke-Checked { & $Python (Join-Path $Root "scripts\packaging\write_manifest.py") $Stage }

$Forbidden = Get-ChildItem -Recurse -File $Stage | Where-Object {
    $_.Name -match 'moraine-server' -or $_.Extension -eq ".msi"
}
if ($Forbidden) {
    throw "refusing to archive installer or relay files: $($Forbidden.Name -join ', ')"
}

New-Item -ItemType Directory -Force -Path $OutDir | Out-Null
if (Test-Path $Archive) { Remove-Item -Force $Archive }
Add-Type -AssemblyName System.IO.Compression
Add-Type -AssemblyName System.IO.Compression.FileSystem
$Zip = [System.IO.Compression.ZipFile]::Open($Archive, [System.IO.Compression.ZipArchiveMode]::Create)
try {
    $PrefixLength = $Stage.TrimEnd('\').Length + 1
    Get-ChildItem -Recurse -File -LiteralPath $Stage | ForEach-Object {
        $Relative = $_.FullName.Substring($PrefixLength) -replace '\\', '/'
        $EntryName = "$StageName/$Relative"
        [System.IO.Compression.ZipFileExtensions]::CreateEntryFromFile(
            $Zip,
            $_.FullName,
            $EntryName
        ) | Out-Null
    }
} finally {
    $Zip.Dispose()
}

$Hash = (Get-FileHash -Algorithm SHA256 -Path $Archive).Hash.ToLowerInvariant()
$Leaf = Split-Path $Archive -Leaf
"$Hash  $Leaf" | Set-Content -Encoding ascii -Path "$Archive.sha256"

Invoke-Checked { & $Python (Join-Path $Root "scripts\packaging\check_windows_demo_archive.py") $Archive }

Write-Host "Bundle: $Archive"
Write-Host "$Hash  $Leaf"
Get-Item $Archive | Select-Object FullName, Length
