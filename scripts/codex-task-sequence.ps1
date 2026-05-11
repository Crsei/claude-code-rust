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
    [int]$TaskNumberOffset = 0,
    [int]$TotalTaskCount = 0,
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

function ConvertTo-PathKey {
    param([string]$Path)
    return $Path.Replace("\", "/").ToLowerInvariant()
}

function Get-GitChangedPaths {
    $oldPreference = $ErrorActionPreference
    $ErrorActionPreference = "Continue"
    try {
        $tracked = @(git diff --name-only HEAD -- 2>$null)
        $untracked = @(git ls-files --others --exclude-standard 2>$null)
    } finally {
        $ErrorActionPreference = $oldPreference
    }
    return @($tracked + $untracked | Where-Object { $_ } | Sort-Object -Unique)
}

function Get-PathSignature {
    param(
        [string]$RepoRoot,
        [string]$Path
    )

    $fullPath = Join-Path $RepoRoot $Path
    if (-not (Test-Path -LiteralPath $fullPath -PathType Leaf)) {
        return "missing"
    }

    $oldPreference = $ErrorActionPreference
    $ErrorActionPreference = "Continue"
    try {
        $hashOutput = @(git hash-object -- $Path 2>$null)
        $hashExitCode = $LASTEXITCODE
    } finally {
        $ErrorActionPreference = $oldPreference
    }
    if ($hashExitCode -eq 0 -and $hashOutput.Count -gt 0 -and $hashOutput[0]) {
        return "blob:$($hashOutput[0])"
    }

    $item = Get-Item -LiteralPath $fullPath
    return "file:$($item.Length):$($item.LastWriteTimeUtc.Ticks)"
}

function Get-PathSignatureMap {
    param(
        [string]$RepoRoot,
        [string[]]$Paths
    )

    $map = @{}
    foreach ($path in $Paths) {
        $map[(ConvertTo-PathKey $path)] = Get-PathSignature -RepoRoot $RepoRoot -Path $path
    }
    return $map
}

function Get-TaskChangedPaths {
    param(
        [string]$RepoRoot,
        [hashtable]$BeforeSignatures,
        [string[]]$BeforePaths
    )

    $afterPaths = @(Get-GitChangedPaths)
    $afterSignatures = Get-PathSignatureMap -RepoRoot $RepoRoot -Paths $afterPaths
    $candidateKeys = @{}
    foreach ($path in $BeforePaths) {
        $candidateKeys[(ConvertTo-PathKey $path)] = $path
    }
    foreach ($path in $afterPaths) {
        $candidateKeys[(ConvertTo-PathKey $path)] = $path
    }

    $changed = @()
    foreach ($key in $candidateKeys.Keys) {
        $path = $candidateKeys[$key]
        $before = if ($BeforeSignatures.ContainsKey($key)) { $BeforeSignatures[$key] } else { "missing" }
        $after = if ($afterSignatures.ContainsKey($key)) { $afterSignatures[$key] } else { Get-PathSignature -RepoRoot $RepoRoot -Path $path }
        if ($before -ne $after) {
            $changed += $path
        }
    }
    return @($changed | Sort-Object -Unique)
}

function Resolve-OptionalPath {
    param([string]$Path)

    $resolved = Resolve-Path -LiteralPath $Path -ErrorAction SilentlyContinue
    if ($resolved) {
        return $resolved.Path
    }
    return $Path
}

function Write-TaskExecutionSummary {
    param(
        [string]$OutputDir,
        [int]$TaskNumber,
        [int]$TaskCount,
        [int]$GlobalTaskNumber,
        [int]$GlobalTaskCount,
        [string]$Task,
        [int]$ExitCode,
        [string[]]$ChangedFiles,
        [string]$LastMessageFile,
        [string]$StartedAt,
        [string]$EndedAt,
        [string]$InterruptionReason
    )

    $taskLabel = '{0:00}' -f $TaskNumber
    $status = if ($ExitCode -eq 0) { "PASS" } else { "ERROR" }
    $summaryPath = Join-Path $OutputDir "task-$taskLabel.summary.json"
    $record = [ordered]@{
        task_number = $TaskNumber
        task_count = $TaskCount
        global_task_number = $GlobalTaskNumber
        global_task_count = $GlobalTaskCount
        task = $Task
        status = $status
        exit_code = $ExitCode
        changed_files = @($ChangedFiles)
        last_message_file = Resolve-OptionalPath $LastMessageFile
        output_dir = Resolve-OptionalPath $OutputDir
        started_at = $StartedAt
        ended_at = $EndedAt
        interruption_reason = $InterruptionReason
    }
    $record | ConvertTo-Json -Depth 6 | Set-Content -LiteralPath $summaryPath -Encoding utf8
    Write-TaskExecutionReport -OutputDir $OutputDir
}

function Write-TaskExecutionReport {
    param([string]$OutputDir)

    $summaryFiles = @(Get-ChildItem -LiteralPath $OutputDir -Filter "task-*.summary.json" | Sort-Object Name)
    $jsonPath = Join-Path $OutputDir "task-report.json"
    $mdPath = Join-Path $OutputDir "task-report.md"
    $records = @()
    foreach ($file in $summaryFiles) {
        $records += Get-Content -LiteralPath $file.FullName -Raw | ConvertFrom-Json
    }
    $report = [ordered]@{
        written_at = Get-Date -Format o
        tasks = [object[]]@($records)
    }
    $report | ConvertTo-Json -Depth 8 | Set-Content -LiteralPath $jsonPath -Encoding utf8

    $lines = @("# Task execution report", "")
    foreach ($record in $records) {
        $heading = "Task $($record.task_number)"
        if ($record.PSObject.Properties.Name -contains "global_task_number") {
            $heading = "Task $($record.global_task_number) / $($record.global_task_count)"
        }
        $lines += "## $heading"
        $lines += ""
        $lines += "- Status: $($record.status)"
        $lines += "- Exit code: $($record.exit_code)"
        if ($record.interruption_reason) {
            $lines += "- Interruption reason: $($record.interruption_reason)"
        }
        $lines += "- Last message: $($record.last_message_file)"
        $lines += "- Output dir: $($record.output_dir)"
        $lines += "- Task: $($record.task)"
        $lines += "- Changed files:"
        $changedFiles = @($record.changed_files)
        if ($changedFiles.Count -eq 0) {
            $lines += "  - (none)"
        } else {
            foreach ($path in $changedFiles) {
                $lines += "  - $path"
            }
        }
        $lines += ""
    }
    $lines | Set-Content -LiteralPath $mdPath -Encoding utf8
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
    $displayTaskCount = if ($TotalTaskCount -gt 0) { $TotalTaskCount } else { $taskList.Count }
    for ($i = 0; $i -lt $taskList.Count; $i++) {
        $taskNumber = $i + 1
        $displayTaskNumber = $TaskNumberOffset + $taskNumber
        $taskLabel = '{0:00}' -f $taskNumber
        $lastMessageFile = Join-Path $resolvedOutputDir "task-$taskLabel.last-message.txt"
        $taskStartedAt = Get-Date -Format o
        $beforePaths = @(Get-GitChangedPaths)
        $beforeSignatures = Get-PathSignatureMap -RepoRoot $repoRoot -Paths $beforePaths
        $prompt = @"
You are executing task $displayTaskNumber of $displayTaskCount.
Work only on this task. Do not continue to later tasks.

Task:
$($taskList[$i])

Return a concise completion note with changed files, verification run, and any remaining risk.
"@

        Write-Host ""
        Write-Host "=== Task $displayTaskNumber / $displayTaskCount ===" -ForegroundColor Cyan
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

        $taskExitCode = 0
        $interruptionReason = ""
        try {
            $prompt | & $Codex @args
            $taskExitCode = $LASTEXITCODE
        } catch {
            $taskExitCode = 1
            $interruptionReason = $_.Exception.Message
        }
        if ($taskExitCode -ne 0 -and -not $interruptionReason) {
            $interruptionReason = "Task $displayTaskNumber failed with exit code $taskExitCode"
        }
        $changedForTask = @(Get-TaskChangedPaths -RepoRoot $repoRoot -BeforeSignatures $beforeSignatures -BeforePaths $beforePaths)
        Write-TaskExecutionSummary `
            -OutputDir $resolvedOutputDir `
            -TaskNumber $taskNumber `
            -TaskCount $taskList.Count `
            -GlobalTaskNumber $displayTaskNumber `
            -GlobalTaskCount $displayTaskCount `
            -Task $taskList[$i] `
            -ExitCode $taskExitCode `
            -ChangedFiles $changedForTask `
            -LastMessageFile $lastMessageFile `
            -StartedAt $taskStartedAt `
            -EndedAt (Get-Date -Format o) `
            -InterruptionReason $interruptionReason

        if ($taskExitCode -ne 0) {
            Write-Error "Task $displayTaskNumber failed with exit code $taskExitCode" -ErrorAction Continue
            if (-not $ContinueOnError) {
                exit $taskExitCode
            }
        }
    }
} finally {
    Pop-Location
}

Write-Host ""
Write-Host "All tasks finished." -ForegroundColor Green
Write-Host "Last-message files: $resolvedOutputDir"
