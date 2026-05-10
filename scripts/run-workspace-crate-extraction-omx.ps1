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
    [bool]$CommitBaselineDirtyChanges = $true,
    [switch]$ContinueOnError,
    [switch]$SkipCommit,
    [switch]$SkipFinalValidation,
    [switch]$DryRun
)

Set-StrictMode -Version Latest
$ErrorActionPreference = "Stop"

. (Join-Path $PSScriptRoot "workspace-crate-extraction-omx/include.ps1")

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
$completedTaskCount = 0
$runStartedAt = Get-Date
$runHadFailure = $false
$stoppedEarly = $false
$stopReason = ""
$validationResults = @()
$globalContract = "Global contract: use concise output with no long reasoning transcript; model and reasoning are fixed by the runner to gpt-5.5 medium; use native subagents only for independent bounded work; stay inside the task ownership scope; do not git commit because the runner owns commits; report PASS/WARNING/BLOCKER/ERROR; avoid redundant safety layers; make errors explicit and agent-debuggable; record effects/defects/follow-ups when the task changes code or docs; split touched Rust files over guard thresholds instead of growing large files."

New-Item -ItemType Directory -Force -Path $resolvedOutputRoot | Out-Null

Push-Location $repoRoot
try {
    $baselineDirtyPaths = @(Get-GitChangedPaths)
    $baselineDirty = Get-PathSet -Paths $baselineDirtyPaths
    $baselineSignatures = Get-PathSignatureMap -Paths $baselineDirtyPaths

    while ($index -lt $tasks.Count) {
        $batchNumber++
        $batchLabel = "{0:00}" -f $batchNumber
        $remaining = $tasks.Count - $index
        $take = Get-NextBatchSize -Tasks $tasks -Index $index -BatchSize $batchSize -Remaining $remaining
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
            $completedTaskCount += $take
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
        $changedForBatch = @(Get-BatchChangedPaths `
            -CurrentChanged $currentChanged `
            -BaselineDirty $baselineDirty `
            -BaselineSignatures $baselineSignatures `
            -IncludeBaselineDirtyChanges:$CommitBaselineDirtyChanges)
        $violations = @(Invoke-DiffGuards `
            -ChangedPaths $changedForBatch `
            -WarnLines $WarnRustFileLines `
            -MaxLines $MaxRustFileLines `
            -MaxFiles $MaxFilesPerBatch `
            -MaxDiffLines $MaxPerFileDiffLines)
        $agentFindings = @(Get-AgentReportedFindings -BatchOutputDir $batchOutputDir)
        $violations = @($violations + $agentFindings)

        Write-BatchArtifacts `
            -OutputRoot $resolvedOutputRoot `
            -BatchNumber $batchNumber `
            -Tasks $batchTasks `
            -ExitCode $exitCode `
            -ChangedPaths $changedForBatch `
            -Violations $violations `
            -BatchOutputDir $batchOutputDir

        $hasBlockingFinding = Test-BlockingGuardFinding -Findings $violations
        if ($exitCode -eq 0 -and -not $hasBlockingFinding) {
            if (-not $SkipCommit) {
                try {
                    Commit-Batch `
                        -ChangedPaths $changedForBatch `
                        -BatchNumber $batchNumber `
                        -OutputRoot $resolvedOutputRoot `
                        -CommitBaselineDirtyChanges:$CommitBaselineDirtyChanges
                } catch {
                    $commitFinding = "ERROR: commit failed for batch $batchLabel - $($_.Exception.Message)"
                    $violations = @($violations + $commitFinding)
                    Write-BatchArtifacts `
                        -OutputRoot $resolvedOutputRoot `
                        -BatchNumber $batchNumber `
                        -Tasks $batchTasks `
                        -ExitCode 1 `
                        -ChangedPaths $changedForBatch `
                        -Violations $violations `
                        -BatchOutputDir $batchOutputDir
                    Write-FailureRecord -OutputRoot $resolvedOutputRoot -BatchNumber $batchNumber -Tasks $batchTasks -Diagnostics @($commitFinding)
                    $runHadFailure = $true
                    $stopReason = $commitFinding
                    if (-not $ContinueOnError) {
                        $stoppedEarly = $true
                        break
                    }
                }
            } else {
                Write-Host "SkipCommit enabled; not committing batch $batchLabel." -ForegroundColor Yellow
            }

            if (-not $runHadFailure) {
                $cleanBatchCount++
                if ($cleanBatchCount -ge 2 -and $batchSize -lt $MaxBatchSize) {
                    $batchSize++
                    $cleanBatchCount = 0
                }
            }
        } else {
            $diagnostics = @(Get-BatchDiagnosticLines -ExitCode $exitCode -Violations $violations)
            Write-FailureRecord -OutputRoot $resolvedOutputRoot -BatchNumber $batchNumber -Tasks $batchTasks -Diagnostics $diagnostics
            $runHadFailure = $true
            $cleanBatchCount = 0
            $batchSize = [Math]::Max($MinBatchSize, $batchSize - 1)
            $summaryPath = Join-Path $resolvedOutputRoot "batch-$batchLabel.summary.md"
            $stopReason = "Batch $batchLabel failed; see $summaryPath"
            Write-Host "Batch $batchLabel did not pass. Summary: $summaryPath" -ForegroundColor Red
            foreach ($diagnostic in $diagnostics) {
                Write-Host "- $diagnostic" -ForegroundColor Red
            }
            if (-not $ContinueOnError) {
                $stoppedEarly = $true
                break
            }
        }

        $completedTaskCount += $take
        $index += $take
    }

    if (-not $DryRun -and -not $SkipFinalValidation) {
        $validationResults = @(Invoke-FinalValidation -OutputRoot $resolvedOutputRoot)
        if (@($validationResults | Where-Object { $_.exit_code -ne 0 }).Count -gt 0) {
            $runHadFailure = $true
            if (-not $stopReason) {
                $stopReason = "Final validation failed"
            }
        }
    }

    Write-RunReport `
        -OutputRoot $resolvedOutputRoot `
        -StartedAt $runStartedAt `
        -EndedAt (Get-Date) `
        -TotalTasks $tasks.Count `
        -CompletedTasks $completedTaskCount `
        -StoppedEarly:$stoppedEarly `
        -StopReason $stopReason `
        -ValidationResults $validationResults `
        -DryRun:$DryRun
} finally {
    Pop-Location
}

Write-Host ""
Write-Host "Workspace crate extraction run finished. Artifacts: $resolvedOutputRoot" -ForegroundColor Green

if ($runHadFailure) {
    exit 1
}
