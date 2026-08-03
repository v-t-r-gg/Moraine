# Shared helpers for W2-E acceptance operators.
$ErrorActionPreference = "Stop"

function Get-W2ERepoRoot {
    if ($env:MORAINE_W2E_REPO) {
        return (Resolve-Path $env:MORAINE_W2E_REPO).Path
    }
    $here = $PSScriptRoot
    return (Resolve-Path (Join-Path $here "..\..")).Path
}

function Get-W2EStagingPrefix {
    return (Join-Path $env:LOCALAPPDATA "Programs\Moraine")
}

function Get-W2EEvidenceRoot {
    param([string]$Path)
    if ($Path) {
        New-Item -ItemType Directory -Force -Path $Path | Out-Null
        return (Resolve-Path $Path).Path
    }
    if ($env:MORAINE_W2E_EVIDENCE) {
        New-Item -ItemType Directory -Force -Path $env:MORAINE_W2E_EVIDENCE | Out-Null
        return (Resolve-Path $env:MORAINE_W2E_EVIDENCE).Path
    }
    $stamp = Get-Date -Format "yyyyMMdd-HHmmss"
    $root = Join-Path $env:LOCALAPPDATA "Moraine\w2e-evidence\$stamp"
    New-Item -ItemType Directory -Force -Path $root | Out-Null
    $env:MORAINE_W2E_EVIDENCE = $root
    return $root
}

function Get-W2EUsernamePatterns {
    $patterns = @(
        [regex]::Escape($env:USERNAME),
        [regex]::Escape($env:USERPROFILE)
    )
    if ($env:USERDOMAIN) {
        $patterns += [regex]::Escape($env:USERDOMAIN)
    }
    return $patterns | Select-Object -Unique
}

function Protect-W2EText {
    param([AllowNull()][string]$Text)
    if ($null -eq $Text -or $Text -eq "") {
        return $Text
    }
    $out = $Text
    # Full SIDs
    $out = [regex]::Replace($out, 'S-1-5-21-\d+(-\d+){2,}-\d+', 'S-1-5-21-***-***-***-<RID>')
    # User profile paths
    $out = [regex]::Replace(
        $out,
        '(?i)([A-Z]:\\Users\\)[^\\\/\s"]+',
        '${1}<account>'
    )
    foreach ($pat in Get-W2EUsernamePatterns) {
        if ($pat.Length -ge 2) {
            $out = [regex]::Replace($out, $pat, '<account>', 'IgnoreCase')
        }
    }
    return $out
}

function Write-W2EJson {
    param(
        [Parameter(Mandatory = $true)][string]$Path,
        [Parameter(Mandatory = $true)]$Object
    )
    $dir = Split-Path -Parent $Path
    if ($dir) {
        New-Item -ItemType Directory -Force -Path $dir | Out-Null
    }
    $json = $Object | ConvertTo-Json -Depth 12
    $json = Protect-W2EText $json
    Set-Content -Encoding utf8 -Path $Path -Value $json
}

function Write-W2EText {
    param(
        [Parameter(Mandatory = $true)][string]$Path,
        [Parameter(Mandatory = $true)][string]$Text
    )
    $dir = Split-Path -Parent $Path
    if ($dir) {
        New-Item -ItemType Directory -Force -Path $dir | Out-Null
    }
    Set-Content -Encoding utf8 -Path $Path -Value (Protect-W2EText $Text)
}

function Invoke-W2EMoraine {
    param(
        [Parameter(Mandatory = $true)][string[]]$Arguments,
        [string]$OutFile,
        [switch]$AllowNonZero
    )
    $cli = Get-Command moraine.exe -ErrorAction SilentlyContinue
    if (-not $cli) {
        $staged = Join-Path (Get-W2EStagingPrefix) "moraine.exe"
        if (Test-Path $staged) {
            $cliPath = $staged
        } else {
            throw "moraine.exe not on PATH and not found at staged prefix"
        }
    } else {
        $cliPath = $cli.Source
    }

    $raw = & $cliPath @Arguments 2>&1 | Out-String
    $code = $LASTEXITCODE
    if ($OutFile) {
        Write-W2EText -Path $OutFile -Text ("exit=$code`n" + $raw)
    }
    if (-not $AllowNonZero -and $code -ne 0) {
        throw "moraine $($Arguments -join ' ') exited $code`n$raw"
    }
    return [pscustomobject]@{
        ExitCode = $code
        Raw      = $raw
        Path     = $cliPath
    }
}

function Test-W2EIsAdministrator {
    $id = [Security.Principal.WindowsIdentity]::GetCurrent()
    $principal = [Security.Principal.WindowsPrincipal]::new($id)
    return $principal.IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)
}

function Get-W2ETokenSummary {
    $id = [Security.Principal.WindowsIdentity]::GetCurrent()
    $groups = @()
    foreach ($g in $id.Groups) {
        try {
            $n = $g.Translate([type]"System.Security.Principal.NTAccount").Value
        } catch {
            $n = "(untranslated)"
        }
        $groups += [pscustomobject]@{
            sidRedacted = (Protect-W2EText $g.Value)
            name        = (Protect-W2EText $n)
        }
    }
    $elevated = Test-W2EIsAdministrator
    $integrity = "unknown"
    try {
        # Medium integrity is expected for a standard interactive user.
        $integrity = (whoami /groups | Select-String "Mandatory Label").ToString().Trim()
        $integrity = Protect-W2EText $integrity
    } catch {
        $integrity = "unavailable"
    }
    return [pscustomobject]@{
        accountClass          = if ($elevated) { "administrator-or-elevated" } else { "standard-user" }
        elevated              = [bool]$elevated
        authenticationType    = $id.AuthenticationType
        isSystem              = $id.IsSystem
        isGuest               = $id.IsGuest
        isAnonymous           = $id.IsAnonymous
        impersonationLevel    = "$($id.ImpersonationLevel)"
        integrityLabel        = $integrity
        groupCount            = $groups.Count
        groupsSample          = $groups | Select-Object -First 24
        sidPresent            = [bool]$id.User
        sidRedacted           = if ($id.User) { Protect-W2EText $id.User.Value } else { $null }
        notes                 = @(
            "Full SID omitted from committed evidence.",
            "Account must not be a member of local Administrators for Account A/B gates."
        )
    }
}
