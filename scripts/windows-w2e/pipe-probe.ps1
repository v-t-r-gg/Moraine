# Account B: independent write-only named-pipe probe against Account A's pipe.
# Expect access denied while the pipe exists — not "pipe not found".
[CmdletBinding()]
param(
    [Parameter(Mandatory = $true)]
    [string]$PipePath,

    [int]$TimeoutMs = 3000,

    [string]$EvidenceDir = "",

    [string]$Payload = '{"hook_event_name":"SessionStart","session_id":"w2e-cross-account-probe","cwd":"C:\\","model":"probe"}'
)

$ErrorActionPreference = "Stop"
. (Join-Path $PSScriptRoot "_common.ps1")

if (Test-W2EIsAdministrator) {
    throw "Pipe probe must run as a non-elevated standard user (Account B)."
}

# Normalize to pipe name for .NET client (strip \\.\pipe\ prefix if present).
$pipeName = $PipePath
if ($pipeName -match '^[\\/]{2}\.[\\/]pipe[\\/](.+)$') {
    $pipeName = $Matches[1]
}

$evidence = Get-W2EEvidenceRoot -Path $EvidenceDir
$token = Get-W2ETokenSummary

# Confirm the pipe object exists from this session's view before connecting.
$pipeExists = $false
try {
    $pipeExists = Test-Path "\\.\pipe\$pipeName"
} catch {
    $pipeExists = $false
}

$result = [ordered]@{
    probeAccountClass = $token.accountClass
    elevated          = $token.elevated
    pipePathRedacted  = (Protect-W2EText $PipePath)
    pipeNameRedacted  = (Protect-W2EText $pipeName)
    pipeExistsProbe   = $pipeExists
    connected         = $false
    bytesWritten      = 0
    outcome           = "unknown"
    exceptionType     = $null
    exceptionMessage  = $null
    notes             = @()
}

$client = $null
try {
    $client = [System.IO.Pipes.NamedPipeClientStream]::new(
        ".",
        $pipeName,
        [System.IO.Pipes.PipeDirection]::Out,
        [System.IO.Pipes.PipeOptions]::None,
        [System.Security.Principal.TokenImpersonationLevel]::Anonymous
    )
    $client.Connect($TimeoutMs)
    $result.connected = $true
    $bytes = [System.Text.Encoding]::UTF8.GetBytes($Payload)
    $client.Write($bytes, 0, $bytes.Length)
    $client.Flush()
    $result.bytesWritten = $bytes.Length
    $result.outcome = "UNEXPECTED_WRITE_SUCCESS"
    $result.notes += "Cross-account write succeeded; W2-E gate fails."
} catch {
    $result.exceptionType = $_.Exception.GetType().FullName
    $result.exceptionMessage = Protect-W2EText ($_.Exception.Message)
    $msg = $_.Exception.Message
    if ($msg -match 'access|denied|unauthorized|forbidden' -or
        $_.Exception -is [System.UnauthorizedAccessException]) {
        $result.outcome = "ACCESS_DENIED"
        $result.notes += "Connection denied or access denied before payload delivery (expected)."
    } elseif ($msg -match 'not found|cannot find|timed out|timeout|No process') {
        if ($pipeExists) {
            $result.outcome = "TIMEOUT_OR_CONNECT_FAIL_WITH_PIPE_PRESENT"
            $result.notes += "Pipe appeared present but connect failed; inspect whether this is ACL denial vs other error."
        } else {
            $result.outcome = "PIPE_NOT_FOUND"
            $result.notes += "Result is only 'pipe not found' — insufficient for W2-E. Ensure Account A runtime is healthy."
        }
    } else {
        $result.outcome = "CONNECT_FAILED"
        $result.notes += "Classify manually against access-denied vs not-found requirements."
    }
} finally {
    if ($client) { $client.Dispose() }
}

Write-W2EJson -Path (Join-Path $evidence "cross-account-pipe-probe.json") -Object ([pscustomobject]$result)
Write-Host ("Pipe probe outcome: {0}" -f $result.outcome)
if ($result.outcome -ne "ACCESS_DENIED") {
    Write-Warning "Expected ACCESS_DENIED for a live Account A pipe from Account B."
}
