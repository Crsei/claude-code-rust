param(
    [string]$TasksFile = "docs/scripts/workspace-crate-extraction-omx-tasks-2026-05-10.txt",
    [string]$Runner = "scripts/codex-task-sequence.ps1",
    [string]$Codex = "codex",
    [string]$WorkDir = ".",
    [string]$OutputRoot = "target/codex-runs/workspace-crate-extraction-omx",
    [ValidateSet("gpt-5.5")]
    [string]$Model = "gpt-5.5",
    [ValidateSet("medium")]
    [string]$ReasoningEffort = "medium",
    [string]$Sandbox = "danger-full-access",
    [int]$InitialBatchSize = 1,
    [int]$MinBatchSize = 1,
    [int]$MaxBatchSize = 3,
    [int]$WarnRustFileLines = 500,
    [int]$MaxRustFileLines = 800,
    [int]$MaxFilesPerBatch = 12,
    [int]$MaxPerFileDiffLines = 600,
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
    return ,$set
}

function Convert-ToExitCode {
    param([AllowNull()][object]$Value)

    if ($null -eq $Value) {
        return 0
    }

    $items = @($Value)
    for ($i = $items.Count - 1; $i -ge 0; $i--) {
        $candidate = $items[$i]
        if ($null -eq $candidate) {
            continue
        }
        if ($candidate -is [int]) {
            return [int]$candidate
        }

        $parsed = 0
        if ([int]::TryParse(([string]$candidate).Trim(), [ref]$parsed)) {
            return $parsed
        }
    }

    $sample = ($items | Select-Object -First 3 | ForEach-Object { [string]$_ }) -join " | "
    throw "Runner exit code did not contain an integer. Received $($items.Count) object(s): $sample"
}

function Invoke-RunnerProcess {
    param([string[]]$ArgumentList)

    & powershell @ArgumentList | Out-Host
    return (Convert-ToExitCode -Value $LASTEXITCODE)
}

function Get-RustLineCount {
    param([string]$Path)

    if (-not (Test-Path -LiteralPath $Path)) {
        return 0
    }
    return @((Get-Content -LiteralPath $Path -Encoding UTF8)).Count
}

function Invoke-DiffGuards {
    param(
        [string[]]$ChangedPaths,
        [int]$WarnLines,
        [int]$MaxLines,
        [int]$MaxFiles,
        [int]$MaxDiffLines
    )

    $findings = New-Object System.Collections.Generic.List[string]
    if ($ChangedPaths.Count -eq 0) {
        return @()
    }

    if ($ChangedPaths.Count -gt $MaxFiles) {
        $findings.Add("WARNING: batch changed $($ChangedPaths.Count) files; consider reducing batch size for reviewability")
    }

    $nameStatus = @(& git diff --name-status HEAD -- @ChangedPaths | Where-Object { $_ })
    $renameCount = @($nameStatus | Where-Object { $_ -match '^[RC]' }).Count
    if ($renameCount -gt 5) {
        $findings.Add("BLOCKER: batch has $renameCount rename/copy entries; split mechanical moves before committing")
    }

    foreach ($path in $ChangedPaths) {
        if (-not $path.EndsWith(".rs", [System.StringComparison]::OrdinalIgnoreCase)) {
            continue
        }
        if (-not (Test-Path -LiteralPath $path)) {
            continue
        }

        $lineCount = Get-RustLineCount -Path $path
        if ($lineCount -gt $MaxLines) {
            $findings.Add("BLOCKER: $path has $lineCount lines, above max $MaxLines; split before commit")
        } elseif ($lineCount -gt $WarnLines) {
            $findings.Add("WARNING: $path has $lineCount lines, above warning threshold $WarnLines")
        }
    }

    $numstat = @(& git diff --numstat HEAD -- @ChangedPaths | Where-Object { $_ })
    foreach ($line in $numstat) {
        $parts = $line -split "`t"
        if ($parts.Count -lt 3) {
            continue
        }
        $added = 0
        $deleted = 0
        if (-not [int]::TryParse($parts[0], [ref]$added)) {
            continue
        }
        if (-not [int]::TryParse($parts[1], [ref]$deleted)) {
            continue
        }
        $total = $added + $deleted
        if ($total -gt $MaxDiffLines) {
            $findings.Add("WARNING: $($parts[2]) has $total changed lines; review whether the task should be split")
        }
    }

    return @($findings)
}

function Test-BlockingGuardFinding {
    param([string[]]$Findings)
    return [bool]@($Findings | Where-Object { $_ -match '^(BLOCKER|ERROR):' }).Count
}

function Test-WarningGuardFinding {
    param([string[]]$Findings)
    return [bool]@($Findings | Where-Object { $_ -match '^WARNING:' }).Count
}

function Get-BatchStatus {
    param(
        [int]$ExitCode,
        [string[]]$Violations
    )

    if ($ExitCode -ne 0) {
        return "ERROR"
    }
    if (Test-BlockingGuardFinding -Findings $Violations) {
        return "BLOCKER"
    }
    if (Test-WarningGuardFinding -Findings $Violations) {
        return "WARNING"
    }
    return "PASS"
}

function Get-BatchDiagnosticLines {
    param(
        [int]$ExitCode,
        [string[]]$Violations
    )

    $diagnostics = New-Object System.Collections.Generic.List[string]
    if ($ExitCode -ne 0) {
        $diagnostics.Add("ERROR: task runner exited with code $ExitCode; inspect task last-message files and runner logs")
    }
    foreach ($violation in $Violations) {
        $diagnostics.Add($violation)
    }
    if ($diagnostics.Count -eq 0) {
        $diagnostics.Add("PASS: runner exit code and diff guards passed")
    }
    return @($diagnostics)
}

function Write-BatchArtifacts {
    param(
        [string]$OutputRoot,
        [int]$BatchNumber,
        [string[]]$Tasks,
        [object]$ExitCode,
        [string[]]$ChangedPaths,
        [string[]]$Violations,
        [string]$BatchOutputDir
    )

    $exitCodeValue = Convert-ToExitCode -Value $ExitCode
    $batchLabel = "{0:00}" -f $BatchNumber
    $lastMessages = @()
    if (Test-Path -LiteralPath $BatchOutputDir) {
        $lastMessages = @(Get-ChildItem -LiteralPath $BatchOutputDir -Filter "task-*.last-message.txt" -File | ForEach-Object { $_.FullName })
    }

    $status = Get-BatchStatus -ExitCode $exitCodeValue -Violations $Violations
    $diagnostics = @(Get-BatchDiagnosticLines -ExitCode $exitCodeValue -Violations $Violations)

    $jsonPath = Join-Path $OutputRoot "batch-$batchLabel.summary.json"
    $mdPath = Join-Path $OutputRoot "batch-$batchLabel.summary.md"

    $record = [ordered]@{
        batch = $BatchNumber
        status = $status
        exit_code = $exitCodeValue
        model = "gpt-5.5"
        reasoning_effort = "medium"
        tasks = $Tasks
        changed_files = $ChangedPaths
        guard_violations = $Violations
        diagnostics = $diagnostics
        last_message_files = $lastMessages
        written_at = (Get-Date).ToString("o")
    }

    $record | ConvertTo-Json -Depth 6 | Set-Content -LiteralPath $jsonPath -Encoding UTF8

    $lines = New-Object System.Collections.Generic.List[string]
    $lines.Add("# Workspace crate extraction batch $batchLabel")
    $lines.Add("")
    $lines.Add("Status: $status")
    $lines.Add("Exit code: $exitCodeValue")
    $lines.Add("Model: gpt-5.5")
    $lines.Add("Reasoning effort: medium")
    $lines.Add("Summary JSON: $jsonPath")
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
    $lines.Add("## Diagnostic summary")
    foreach ($diagnostic in $diagnostics) {
        $lines.Add("- $diagnostic")
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

    & git diff --cached --quiet --
    if ($LASTEXITCODE -eq 0) {
        Write-Host "No staged changes to commit for this batch." -ForegroundColor Yellow
        return
    }

    $batchLabel = "{0:00}" -f $BatchNumber
    $messagePath = Join-Path $OutputRoot "batch-$batchLabel.commit-message.txt"
    $message = @"
Advance workspace crate extraction batch $batchLabel

This checkpoint keeps the extraction lane reviewable by committing only paths
that changed after the runner started. Batch artifacts are written under
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
$globalContract = "Global contract: use concise output with no long reasoning transcript; model and reasoning are fixed by the runner to gpt-5.5 medium; use native subagents only for independent bounded work; stay inside the task ownership scope; do not git commit because the runner owns commits; report PASS/WARNING/BLOCKER/ERROR; avoid redundant safety layers; make errors explicit and agent-debuggable; record effects/defects/follow-ups when the task changes code or docs; split touched Rust files over guard thresholds instead of growing large files."

New-Item -ItemType Directory -Force -Path $resolvedOutputRoot | Out-Null

Push-Location $repoRoot
try {
    $baselineDirty = Get-PathSet -Paths @(Get-GitChangedPaths)

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
        Write-Host "=== Workspace crate extraction batch $batchLabel ($take task(s), batch size $batchSize) ===" -ForegroundColor Cyan
        foreach ($task in $batchTasks) {
            Write-Host "- $task"
        }

        if ($DryRun) {
            Write-Host "DryRun: would invoke $Codex through $resolvedRunner" -ForegroundColor Yellow
            $index += $take
            continue
        }

        $runnerArgs = @(
            "-NoProfile",
            "-ExecutionPolicy", "Bypass",
            "-File", $resolvedRunner,
            "-TasksFile", $batchTasksFile,
            "-Codex", $Codex,
            "-WorkDir", $resolvedWorkDir,
            "-OutputDir", $batchOutputDir,
            "-Model", $Model,
            "-ReasoningEffort", $ReasoningEffort,
            "-Sandbox", $Sandbox
        )
        if ($ContinueOnError) {
            $runnerArgs += "-ContinueOnError"
        }

        $exitCode = Invoke-RunnerProcess -ArgumentList $runnerArgs
        $currentChanged = @(Get-GitChangedPaths)
        $newChanged = @($currentChanged | Where-Object { -not $baselineDirty.Contains($_) } | Sort-Object -Unique)
        $violations = @(Invoke-DiffGuards `
            -ChangedPaths $newChanged `
            -WarnLines $WarnRustFileLines `
            -MaxLines $MaxRustFileLines `
            -MaxFiles $MaxFilesPerBatch `
            -MaxDiffLines $MaxPerFileDiffLines)

        Write-BatchArtifacts `
            -OutputRoot $resolvedOutputRoot `
            -BatchNumber $batchNumber `
            -Tasks $batchTasks `
            -ExitCode $exitCode `
            -ChangedPaths $newChanged `
            -Violations $violations `
            -BatchOutputDir $batchOutputDir

        $hasBlockingFinding = Test-BlockingGuardFinding -Findings $violations
        if ($exitCode -eq 0 -and -not $hasBlockingFinding) {
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
            $summaryPath = Join-Path $resolvedOutputRoot "batch-$batchLabel.summary.md"
            Write-Host "Batch $batchLabel did not pass. Summary: $summaryPath" -ForegroundColor Red
            foreach ($diagnostic in @(Get-BatchDiagnosticLines -ExitCode $exitCode -Violations $violations)) {
                Write-Host "- $diagnostic" -ForegroundColor Red
            }
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
Write-Host "Workspace crate extraction run finished. Artifacts: $resolvedOutputRoot" -ForegroundColor Green
