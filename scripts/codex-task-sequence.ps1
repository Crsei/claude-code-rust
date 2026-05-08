param(
    [string[]]$Tasks = @(),
    [string]$TasksFile = "codex-tasks.txt",
    [string]$Codex = "codex",
    [string]$WorkDir = (Get-Location).Path,
    [string]$Model = "",
    [ValidateSet("minimal", "low", "medium", "high", "xhigh")]
    [string]$ReasoningEffort = "",
    [string]$Profile = "",
    [string]$Sandbox = "danger-full-access",
    [string]$OutputDir = "target/codex-runs",
    [switch]$ContinueOnError
)

Set-StrictMode -Version Latest
$ErrorActionPreference = "Stop"

function Get-TaskList {
    param(
        [string[]]$InlineTasks,
        [string]$FilePath
    )

    if ($InlineTasks.Count -gt 0) {
        return $InlineTasks
    }

    if (Test-Path $FilePath) {
        return @(
            Get-Content $FilePath | ForEach-Object { $_.Trim() } | Where-Object {
            $_ -and -not $_.StartsWith("#")
            }
        )
    }

    throw "No tasks provided. Pass -Tasks or create $FilePath."
}

$repoRoot = (Resolve-Path (Join-Path $PSScriptRoot "..")).Path
$resolvedWorkDir = if ([System.IO.Path]::IsPathRooted($WorkDir)) {
    $WorkDir
} else {
    Join-Path $repoRoot $WorkDir
}
$resolvedTasksFile = if ([System.IO.Path]::IsPathRooted($TasksFile)) {
    $TasksFile
} else {
    Join-Path $repoRoot $TasksFile
}
$resolvedOutputDir = if ([System.IO.Path]::IsPathRooted($OutputDir)) {
    $OutputDir
} else {
    Join-Path $repoRoot $OutputDir
}

$taskList = @(Get-TaskList -InlineTasks $Tasks -FilePath $resolvedTasksFile)
if ($taskList.Count -eq 0) {
    throw "Task list is empty."
}

New-Item -ItemType Directory -Force -Path $resolvedOutputDir | Out-Null

Push-Location $repoRoot
try {
    for ($i = 0; $i -lt $taskList.Count; $i++) {
        $taskNumber = $i + 1
        $taskLabel = '{0:00}' -f $taskNumber
        $lastMessageFile = Join-Path $resolvedOutputDir "task-$taskLabel.last-message.txt"
        $prompt = @"
You are executing task $taskNumber of $($taskList.Count).
Work only on this task. Do not continue to later tasks.

Task:
$($taskList[$i])

Return a concise completion note with changed files, verification run, and any remaining risk.
"@

        Write-Host ""
        Write-Host "=== Task $taskNumber / $($taskList.Count) ===" -ForegroundColor Cyan
        Write-Host $taskList[$i]
        Write-Host ""

        $args = @(
            "exec",
            "--cd", $resolvedWorkDir,
            "--sandbox", $Sandbox,
            "--output-last-message", $lastMessageFile
        )
        if ($Model) { $args += @("--model", $Model) }
        if ($ReasoningEffort) {
            $args += @("--config", "model_reasoning_effort=""$ReasoningEffort""")
        }
        if ($Profile) { $args += @("--profile", $Profile) }
        $args += "-"

        $prompt | & $Codex @args
        if ($LASTEXITCODE -ne 0) {
            Write-Error "Task $taskNumber failed with exit code $LASTEXITCODE"
            if (-not $ContinueOnError) {
                exit $LASTEXITCODE
            }
        }
    }
} finally {
    Pop-Location
}

Write-Host ""
Write-Host "All tasks finished." -ForegroundColor Green
Write-Host "Last-message files: $resolvedOutputDir"
