param(
    [string]$TasksFile = "docs/scripts/achieve/ratatui-ui-parity-omx-tasks.txt",
    [string]$Runner = "scripts/codex-task-sequence.ps1",
    [string]$Omx = "omx",
    [string]$WorkDir = ".",
    [string]$OutputRoot = "target/codex-runs/ratatui-ui-parity-omx",
    [ValidateSet("gpt-5.5")]
    [string]$Model = "gpt-5.5",
    [ValidateSet("medium")]
    [string]$ReasoningEffort = "medium",
    [string]$Sandbox = "danger-full-access",
    [int]$InitialBatchSize = 1,
    [int]$MinBatchSize = 1,
    [int]$MaxBatchSize = 1,
    [switch]$ContinueOnError,
    [switch]$SkipCommit,
    [switch]$DryRun
)

Set-StrictMode -Version Latest
$ErrorActionPreference = "Stop"

. (Join-Path $PSScriptRoot "ratatui-ui-parity-omx/include.ps1")

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

$globalContract = "Global contract: use omx/codex with concise output, no long reasoning transcript; model and reasoning are fixed by the runner; use at most 2 native subagents only for independent bounded exploration or review; stay inside the task ownership scope; do not git commit because the runner owns commits; make errors explicit with ERROR/BLOCKER/WARNING/PASS; avoid redundant safety layers and prefer clear fail-fast diagnostics; for Rust file-size findings, split only when the touched area is in task scope, otherwise keep narrow changes and explain the rationale."

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

        $exitCode = Invoke-RunnerProcess -ArgumentList $runnerArgs

        $currentChanged = @(Get-GitChangedPaths)
        $newChanged = @($currentChanged | Where-Object { -not $baselineDirty.Contains($_) } | Sort-Object -Unique)
        $violations = @(Invoke-DiffGuards -ChangedPaths $newChanged)

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
Write-Host "Ratatui UI parity OMX run finished. Artifacts: $resolvedOutputRoot" -ForegroundColor Green
