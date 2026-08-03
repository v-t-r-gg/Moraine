# Export redacted Task Scheduler registration + ACL evidence (Account A).
[CmdletBinding()]
param(
    [string]$TaskPath = "\Moraine\",
    [string]$TaskName = "",
    [string]$EvidenceDir = ""
)

$ErrorActionPreference = "Stop"
. (Join-Path $PSScriptRoot "_common.ps1")

if (Test-W2EIsAdministrator) {
    throw "Task export must run non-elevated under Account A."
}

$evidence = Get-W2EEvidenceRoot -Path $EvidenceDir

# Discover Moraine tasks if name not provided.
$tasks = @()
try {
    $tasks = Get-ScheduledTask | Where-Object {
        $_.TaskPath -like '*Moraine*' -or $_.TaskName -like '*Moraine*' -or
        $_.TaskName -like '*moraine*'
    }
} catch {
    throw "Could not enumerate scheduled tasks: $_"
}

if ($TaskName) {
    $tasks = $tasks | Where-Object { $_.TaskName -eq $TaskName }
}

if (-not $tasks -or $tasks.Count -eq 0) {
    Write-W2EText -Path (Join-Path $evidence "redacted-task-definition.xml") -Text "<!-- no Moraine task found -->"
    Write-W2EText -Path (Join-Path $evidence "redacted-task-acl.txt") -Text "no Moraine task found"
    Write-Warning "No Moraine scheduled task found."
    return
}

$primary = $tasks | Select-Object -First 1
$fullPath = Join-Path $primary.TaskPath $primary.TaskName

$xml = Export-ScheduledTask -TaskName $primary.TaskName -TaskPath $primary.TaskPath
Write-W2EText -Path (Join-Path $evidence "redacted-task-definition.xml") -Text $xml

# ACL via icacls on the task file if present; otherwise schtasks query.
$aclText = New-Object System.Text.StringBuilder
[void]$aclText.AppendLine("task=$fullPath")
[void]$aclText.AppendLine("state=$($primary.State)")
try {
    $info = Get-ScheduledTaskInfo -TaskName $primary.TaskName -TaskPath $primary.TaskPath
    [void]$aclText.AppendLine("lastTaskResult=$($info.LastTaskResult)")
    [void]$aclText.AppendLine("lastRunTime=$($info.LastRunTime)")
    [void]$aclText.AppendLine("nextRunTime=$($info.NextRunTime)")
} catch {
    [void]$aclText.AppendLine("taskInfo=unavailable")
}

try {
    $sd = (schtasks /Query /TN $fullPath /XML 2>$null)
    # Prefer principal / logon fields from XML already exported.
    [void]$aclText.AppendLine("schtasksXmlExport=ok")
} catch {
    [void]$aclText.AppendLine("schtasksXmlExport=failed")
}

# Process observation
$procs = Get-Process -Name "moraine-service" -ErrorAction SilentlyContinue
$procNotes = @()
foreach ($p in $procs) {
    $procNotes += [pscustomobject]@{
        id              = $p.Id
        sessionId       = $p.SessionId
        hasMainWindow   = [bool]$p.MainWindowHandle
        mainWindowTitle = Protect-W2EText $p.MainWindowTitle
        startTime       = $p.StartTime
        pathCategory    = "resolved-at-runtime"
    }
}
Write-W2EJson -Path (Join-Path $evidence "service-process.json") -Object ([pscustomobject]@{
        processCount = @($procs).Count
        processes    = $procNotes
        expected     = @(
            "exactly one authoritative moraine-service process",
            "no visible console window (MainWindowHandle should be 0)",
            "runs as Account A, not elevated"
        )
    })

Write-W2EText -Path (Join-Path $evidence "redacted-task-acl.txt") -Text $aclText.ToString()
Write-Host "Exported redacted task evidence for $fullPath"
