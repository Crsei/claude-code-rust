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
    [int]$MaxRustFileLines = 2000,
    [int]$MaxFilesPerBatch = 12,
    [int]$MaxPerFileDiffLines = 600,
    [int]$MaxOversizedRustFilesBeforeBlocker = 3,
    [string]$BlockerReviewSandbox = "read-only",
    [int]$BlockerReviewTimeoutSeconds = 900,
    [bool]$CommitBaselineDirtyChanges = $true,
    [switch]$ContinueOnError,
    [switch]$SkipCommit,
    [switch]$SkipBlockerReview,
    [switch]$SkipFinalValidation,
    [switch]$DryRun
)

Set-StrictMode -Version Latest
$ErrorActionPreference = "Stop"

$pythonScript = Join-Path $PSScriptRoot "workspace_crate_extraction_omx.py"
if (-not (Test-Path -LiteralPath $pythonScript)) {
    throw "Python runner not found: $pythonScript"
}

$pythonCommand = Get-Command python -ErrorAction SilentlyContinue
$pythonArgs = @()
if ($pythonCommand) {
    $pythonExe = $pythonCommand.Source
} else {
    $pyCommand = Get-Command py -ErrorAction SilentlyContinue
    if (-not $pyCommand) {
        throw "Python was not found. Install Python or add python.exe/py.exe to PATH."
    }
    $pythonExe = $pyCommand.Source
    $pythonArgs += "-3"
}

$pythonArgs += @(
    $pythonScript,
    "--tasks-file", $TasksFile,
    "--runner", $Runner,
    "--codex", $Codex,
    "--work-dir", $WorkDir,
    "--output-root", $OutputRoot,
    "--model", $Model,
    "--reasoning-effort", $ReasoningEffort,
    "--sandbox", $Sandbox,
    "--initial-batch-size", [string]$InitialBatchSize,
    "--min-batch-size", [string]$MinBatchSize,
    "--max-batch-size", [string]$MaxBatchSize,
    "--warn-rust-file-lines", [string]$WarnRustFileLines,
    "--max-rust-file-lines", [string]$MaxRustFileLines,
    "--max-files-per-batch", [string]$MaxFilesPerBatch,
    "--max-per-file-diff-lines", [string]$MaxPerFileDiffLines,
    "--max-oversized-rust-files-before-blocker", [string]$MaxOversizedRustFilesBeforeBlocker,
    "--blocker-review-sandbox", $BlockerReviewSandbox,
    "--blocker-review-timeout-seconds", [string]$BlockerReviewTimeoutSeconds
)

if ($CommitBaselineDirtyChanges) {
    $pythonArgs += "--commit-baseline-dirty-changes"
} else {
    $pythonArgs += "--no-commit-baseline-dirty-changes"
}
if ($ContinueOnError) { $pythonArgs += "--continue-on-error" }
if ($SkipCommit) { $pythonArgs += "--skip-commit" }
if ($SkipBlockerReview) { $pythonArgs += "--skip-blocker-review" }
if ($SkipFinalValidation) { $pythonArgs += "--skip-final-validation" }
if ($DryRun) { $pythonArgs += "--dry-run" }

& $pythonExe @pythonArgs
exit $LASTEXITCODE
