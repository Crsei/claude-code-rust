param(
    [string]$TasksFile = "docs/scripts/ratatui-ui-parity-omx-tasks.txt",
    [string]$Runner = "scripts/codex-task-sequence.ps1",
    [string]$Omx = "omx",
    [string]$WorkDir = ".",
    [string]$OutputRoot = "target/codex-runs/ratatui-ui-parity-omx",
    [ValidateSet("gpt-5.5")]
    [string]$Model = "gpt-5.5",
    [ValidateSet("medium")]
    [string]$ReasoningEffort = "medium",
    [string]$Sandbox = "danger-full-access",
    [int]$InitialBatchSize = 2,
    [int]$MinBatchSize = 1,
    [int]$MaxBatchSize = 4,
    [int]$MaxChangedFiles = 8,
    [int]$MaxFileDeltaLines = 300,
    [double]$MaxFileDeltaRatio = 0.25,
    [int]$MaxCodeFileLines = 900,
    [switch]$ContinueOnError,
    [switch]$SkipCommit,
    [switch]$DryRun
)

Set-StrictMode -Version Latest
$ErrorActionPreference = "Stop"

function Resolve-RepoPath {
    param(
        [string]$RepoRoot,
        [string]$Path
    )

    if ([System.IO.Path]::IsPathRooted($Path)) {
        return $Path
    }
    return (Join-Path $RepoRoot $Path)
}

function Get-TaskList {
    param([string]$Path)

    if (-not (Test-Path -LiteralPath $Path)) {
        throw "Task file not found: $Path"
    }

    return @(
        Get-Content -LiteralPath $Path -Encoding UTF8 |
            ForEach-Object { $_.Trim() } |
            Where-Object { $_ -and -not $_.StartsWith("#") }
    )
}

function Test-CheckpointTask {
    param([string]$Task)
    return $Task -match '^\[(checkpoint|review|final)\]'
}

function Get-GitChangedPaths {
    $tracked = @(& git diff --name-only HEAD -- | Where-Object { $_ })
    $untracked = @(& git ls-files --others --exclude-standard | Where-Object { $_ })
    return @($tracked + $untracked | Sort-Object -Unique)
}

function Get-PathSet {
    param([string[]]$Paths)

    $set = [System.Collections.Generic.HashSet[string]]::new([System.StringComparer]::OrdinalIgnoreCase)
    foreach ($path in $Paths) {
        [void]$set.Add($path)
    }
    return $set
}

function Test-TrackedPath {
    param([string]$Path)

    & git ls-files --error-unmatch -- $Path *> $null
    return $LASTEXITCODE -eq 0
}

function Get-FileLineCount {
    param([string]$Path)

    if (-not (Test-Path -LiteralPath $Path)) {
        return 0
    }
    return (Get-Content -LiteralPath $Path -ErrorAction SilentlyContinue | Measure-Object -Line).Lines
}

function Invoke-DiffGuards {
    param(
        [string[]]$ChangedPaths,
        [string]$RepoRoot,
        [int]$MaxChangedFiles,
        [int]$MaxFileDeltaLines,
        [double]$MaxFileDeltaRatio,
        [int]$MaxCodeFileLines
    )

    $violations = New-Object System.Collections.Generic.List[string]

    if ($ChangedPaths.Count -eq 0) {
        return @()
    }

    if ($ChangedPaths.Count -gt $MaxChangedFiles) {
        $violations.Add("BLOCKER: changed file count $($ChangedPaths.Count) exceeds limit $MaxChangedFiles")
    }

    & git diff --check -- @ChangedPaths
    if ($LASTEXITCODE -ne 0) {
        $violations.Add("BLOCKER: git diff --check failed")
    }

    $statusLines = @(& git diff --name-status HEAD -- @ChangedPaths)
    foreach ($line in $statusLines) {
        if ($line -match '^[RC]') {
            $violations.Add("BLOCKER: rename/copy style change detected: $line")
        }
    }

    $deltaByPath = @{}
    $numstatLines = @(& git diff --numstat HEAD -- @ChangedPaths)
    foreach ($line in $numstatLines) {
        $parts = $line -split "`t"
        if ($parts.Count -lt 3) {
            continue
        }
        if ($parts[0] -eq "-" -or $parts[1] -eq "-") {
            continue
        }
        $deltaByPath[$parts[2]] = ([int]$parts[0] + [int]$parts[1])
    }

    foreach ($path in $ChangedPaths) {
        $absolute = Join-Path $RepoRoot $path
        $lineCount = Get-FileLineCount -Path $absolute
        $delta = if ($deltaByPath.ContainsKey($path)) { [int]$deltaByPath[$path] } else { 0 }
        $tracked = Test-TrackedPath -Path $path

        if (-not $tracked -and (Test-Path -LiteralPath $absolute)) {
            $delta = $lineCount
        }

        if ($delta -gt $MaxFileDeltaLines) {
            $violations.Add("BLOCKER: $path changed $delta lines, above $MaxFileDeltaLines")
        }

        if ($tracked -and $lineCount -gt 0) {
            $ratio = $delta / [double]$lineCount
            if ($ratio -gt $MaxFileDeltaRatio) {
                $violations.Add(("BLOCKER: {0} changed {1:P1} of current file size" -f $path, $ratio))
            }
        }

        if ([System.IO.Path]::GetExtension($path) -eq ".rs" -and $lineCount -gt $MaxCodeFileLines) {
            $violations.Add("BLOCKER: Rust file $path has $lineCount lines, above $MaxCodeFileLines; split or document a refactor decision")
        }
    }

    return @($violations)
}

function Write-BatchArtifacts {
    param(
        [string]$OutputRoot,
        [int]$BatchNumber,
        [string[]]$Tasks,
        [int]$ExitCode,
        [string[]]$ChangedPaths,
        [string[]]$Violations,
        [string]$BatchOutputDir
    )

    $batchLabel = "{0:00}" -f $BatchNumber
    $lastMessages = @()
    if (Test-Path -LiteralPath $BatchOutputDir) {
        $lastMessages = @(Get-ChildItem -LiteralPath $BatchOutputDir -Filter "task-*.last-message.txt" -File | ForEach-Object { $_.FullName })
    }

    $status = if ($ExitCode -ne 0) {
        "ERROR"
    } elseif ($Violations.Count -gt 0) {
        "BLOCKER"
    } else {
        "PASS"
    }

    $jsonPath = Join-Path $OutputRoot "batch-$batchLabel.summary.json"
    $mdPath = Join-Path $OutputRoot "batch-$batchLabel.summary.md"

    $record = [ordered]@{
        batch = $BatchNumber
        status = $status
        exit_code = $ExitCode
        model = "gpt-5.5"
        reasoning_effort = "medium"
        tasks = $Tasks
        changed_files = $ChangedPaths
        guard_violations = $Violations
        last_message_files = $lastMessages
        written_at = (Get-Date).ToString("o")
    }

    $record | ConvertTo-Json -Depth 6 | Set-Content -LiteralPath $jsonPath -Encoding UTF8

    $lines = New-Object System.Collections.Generic.List[string]
    $lines.Add("# Ratatui UI parity OMX batch $batchLabel")
    $lines.Add("")
    $lines.Add("Status: $status")
    $lines.Add("Exit code: $ExitCode")
    $lines.Add("Model: gpt-5.5")
    $lines.Add("Reasoning effort: medium")
    $lines.Add("")
    $lines.Add("## Tasks")
    foreach ($task in $Tasks) {
        $lines.Add("- $task")
    }
    $lines.Add("")
    $lines.Add("## Changed files")
    if ($ChangedPaths.Count -eq 0) {
        $lines.Add("- (none)")
    } else {
        foreach ($path in $ChangedPaths) {
            $lines.Add("- $path")
        }
    }
    $lines.Add("")
    $lines.Add("## Guard findings")
    if ($Violations.Count -eq 0) {
        $lines.Add("- PASS")
    } else {
        foreach ($violation in $Violations) {
            $lines.Add("- $violation")
        }
    }
    $lines.Add("")
    $lines.Add("## Last-message files")
    if ($lastMessages.Count -eq 0) {
        $lines.Add("- (none)")
    } else {
        foreach ($path in $lastMessages) {
            $lines.Add("- $path")
        }
    }
    $lines | Set-Content -LiteralPath $mdPath -Encoding UTF8
}

function Commit-Batch {
    param(
        [string[]]$ChangedPaths,
        [int]$BatchNumber,
        [string]$OutputRoot
    )

    if ($ChangedPaths.Count -eq 0) {
        Write-Host "No new changes to commit for this batch." -ForegroundColor Yellow
        return
    }

    & git add -- @ChangedPaths
    if ($LASTEXITCODE -ne 0) {
        throw "git add failed for batch $BatchNumber"
    }

    $batchLabel = "{0:00}" -f $BatchNumber
    $messagePath = Join-Path $OutputRoot "batch-$batchLabel.commit-message.txt"
    $message = @"
Advance ratatui UI parity batch $batchLabel

This checkpoint keeps the OMX execution lane reviewable by committing only
paths that changed after the runner started. Run artifacts are written under
$OutputRoot for local inspection.

Constraint: Model fixed to gpt-5.5 with medium reasoning
Constraint: Commit path set excludes baseline dirty worktree paths
Rejected: One large final commit | review and rollback would be harder
Confidence: medium
Scope-risk: moderate
Directive: Do not continue from a failed batch without reading the batch summary
Tested: See $OutputRoot/batch-$batchLabel.summary.md
Not-tested: Full workspace test unless the final verification task reports it
"@
    $message | Set-Content -LiteralPath $messagePath -Encoding UTF8

    & git commit -F $messagePath
    if ($LASTEXITCODE -ne 0) {
        throw "git commit failed for batch $BatchNumber"
    }
}

$repoRoot = (Resolve-Path (Join-Path $PSScriptRoot "..")).Path
$resolvedTasksFile = Resolve-RepoPath -RepoRoot $repoRoot -Path $TasksFile
$resolvedRunner = Resolve-RepoPath -RepoRoot $repoRoot -Path $Runner
$resolvedWorkDir = Resolve-RepoPath -RepoRoot $repoRoot -Path $WorkDir
$resolvedOutputRoot = Resolve-RepoPath -RepoRoot $repoRoot -Path $OutputRoot

if (-not (Test-Path -LiteralPath $resolvedRunner)) {
    throw "Runner not found: $resolvedRunner"
}

$tasks = @(Get-TaskList -Path $resolvedTasksFile)
if ($tasks.Count -eq 0) {
    throw "Task file is empty: $resolvedTasksFile"
}

$batchSize = [Math]::Max($MinBatchSize, [Math]::Min($InitialBatchSize, $MaxBatchSize))
$cleanBatchCount = 0
$batchNumber = 0
$index = 0
$baselineDirty = Get-PathSet -Paths @(Get-GitChangedPaths)

New-Item -ItemType Directory -Force -Path $resolvedOutputRoot | Out-Null

$globalContract = "Global contract: use omx/codex with concise output, no long reasoning transcript; model and reasoning are fixed by the runner; use at most 2 native subagents only for independent bounded exploration or review; stay inside the task ownership scope; do not git commit because the runner owns commits; make errors explicit with ERROR/BLOCKER/WARNING/PASS; avoid redundant safety layers and prefer clear fail-fast diagnostics."

Push-Location $repoRoot
try {
    while ($index -lt $tasks.Count) {
        $batchNumber++
        $batchLabel = "{0:00}" -f $batchNumber
        $remaining = $tasks.Count - $index

        if (Test-CheckpointTask -Task $tasks[$index]) {
            $take = 1
        } else {
            $take = [Math]::Min($batchSize, $remaining)
            for ($offset = 1; $offset -lt $take; $offset++) {
                if (Test-CheckpointTask -Task $tasks[$index + $offset]) {
                    $take = $offset
                    break
                }
            }
        }

        $batchTasks = @($tasks[$index..($index + $take - 1)])
        $batchTasksFile = Join-Path $resolvedOutputRoot "batch-$batchLabel.tasks.txt"
        $batchOutputDir = Join-Path $resolvedOutputRoot "batch-$batchLabel"
        New-Item -ItemType Directory -Force -Path $batchOutputDir | Out-Null

        $augmentedTasks = @($batchTasks | ForEach-Object { "$globalContract Task: $_" })
        $augmentedTasks | Set-Content -LiteralPath $batchTasksFile -Encoding UTF8

        Write-Host ""
        Write-Host "=== Ratatui UI parity batch $batchLabel ($take task(s), batch size $batchSize) ===" -ForegroundColor Cyan
        foreach ($task in $batchTasks) {
            Write-Host "- $task"
        }

        if ($DryRun) {
            Write-Host "DryRun: would invoke $Omx through $resolvedRunner" -ForegroundColor Yellow
            $index += $take
            continue
        }

        $runnerArgs = @(
            "-NoProfile",
            "-ExecutionPolicy", "Bypass",
            "-File", $resolvedRunner,
            "-TasksFile", $batchTasksFile,
            "-Codex", $Omx,
            "-WorkDir", $resolvedWorkDir,
            "-OutputDir", $batchOutputDir,
            "-Model", $Model,
            "-ReasoningEffort", $ReasoningEffort,
            "-Sandbox", $Sandbox
        )
        if ($ContinueOnError) {
            $runnerArgs += "-ContinueOnError"
        }

        & powershell @runnerArgs
        $exitCode = $LASTEXITCODE

        $currentChanged = @(Get-GitChangedPaths)
        $newChanged = @($currentChanged | Where-Object { -not $baselineDirty.Contains($_) } | Sort-Object -Unique)
        $violations = @(Invoke-DiffGuards `
            -ChangedPaths $newChanged `
            -RepoRoot $repoRoot `
            -MaxChangedFiles $MaxChangedFiles `
            -MaxFileDeltaLines $MaxFileDeltaLines `
            -MaxFileDeltaRatio $MaxFileDeltaRatio `
            -MaxCodeFileLines $MaxCodeFileLines)

        Write-BatchArtifacts `
            -OutputRoot $resolvedOutputRoot `
            -BatchNumber $batchNumber `
            -Tasks $batchTasks `
            -ExitCode $exitCode `
            -ChangedPaths $newChanged `
            -Violations $violations `
            -BatchOutputDir $batchOutputDir

        if ($exitCode -eq 0 -and $violations.Count -eq 0) {
            if (-not $SkipCommit) {
                Commit-Batch -ChangedPaths $newChanged -BatchNumber $batchNumber -OutputRoot $resolvedOutputRoot
            } else {
                Write-Host "SkipCommit enabled; not committing batch $batchLabel." -ForegroundColor Yellow
            }
            $cleanBatchCount++
            if ($cleanBatchCount -ge 2 -and $batchSize -lt $MaxBatchSize) {
                $batchSize++
                $cleanBatchCount = 0
            }
        } else {
            $cleanBatchCount = 0
            $batchSize = [Math]::Max($MinBatchSize, $batchSize - 1)
            Write-Host "Batch $batchLabel did not pass. See batch summary in $resolvedOutputRoot." -ForegroundColor Red
            if (-not $ContinueOnError) {
                exit $(if ($exitCode -ne 0) { $exitCode } else { 1 })
            }
        }

        $index += $take
    }
} finally {
    Pop-Location
}

Write-Host ""
Write-Host "Ratatui UI parity OMX run finished. Artifacts: $resolvedOutputRoot" -ForegroundColor Green
