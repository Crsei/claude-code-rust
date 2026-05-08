param(
    [string]$Codex = "codex",
    [string]$Model = "gpt-5.5",
    [ValidateRange(0, 4)]
    [int]$StartAt = 0,
    [switch]$ContinueOnError
)

Set-StrictMode -Version Latest
$ErrorActionPreference = "Stop"

$repoRoot = (Resolve-Path (Join-Path $PSScriptRoot "..\..")).Path
$runner = Join-Path $repoRoot "scripts\codex-task-sequence.ps1"
$plan = ".omx/plans/p1-review-b-residual-low-reasoning-session-plan-2026-05-08.md"

function Invoke-PlanSession {
    param(
        [string]$Session,
        [string]$OutputDir,
        [ValidateSet("minimal", "low", "medium", "high", "xhigh")]
        [string]$ReasoningEffort
    )

    $task = "Execute only '$Session' from $plan. Read that plan file first, obey its Low-Reasoning Contract, stay within that session's owned files and forbidden areas, and return a concise completion note with changed files, verification run, review findings when applicable, and remaining risk."

    $params = @{
        Tasks           = @($task)
        Codex           = $Codex
        WorkDir         = $repoRoot
        OutputDir       = $OutputDir
        Sandbox         = "danger-full-access"
        ReasoningEffort = $ReasoningEffort
    }

    if ($Model) {
        $params.Model = $Model
    }

    if ($ContinueOnError) {
        $params.ContinueOnError = $true
    }

    & $runner @params
}

$sessions = @(
    @{ Start = 0; Name = "Session 0 - Residual Status Gate"; OutputDir = "target/codex-runs/p1-review-b-residual-session-00-status-gate"; ReasoningEffort = "medium" }
    @{ Start = 1; Name = "Session 1 - Critical Post Hook Propagation"; OutputDir = "target/codex-runs/p1-review-b-residual-session-01-hook-propagation"; ReasoningEffort = "medium" }
    @{ Start = 2; Name = "Session 2 - Reload Plugin Global Diagnostics Visibility"; OutputDir = "target/codex-runs/p1-review-b-residual-session-02-plugin-reload-diagnostics"; ReasoningEffort = "medium" }
    @{ Start = 2; Name = "Review Session A - Residual Scope Review"; OutputDir = "target/codex-runs/p1-review-b-residual-review-a"; ReasoningEffort = "medium" }
    @{ Start = 3; Name = "Session 3 - Residual Documentation Closure"; OutputDir = "target/codex-runs/p1-review-b-residual-session-03-docs-closure"; ReasoningEffort = "medium" }
    @{ Start = 4; Name = "Session 4 - Final Verification"; OutputDir = "target/codex-runs/p1-review-b-residual-session-04-final-verification"; ReasoningEffort = "medium" }
)

foreach ($session in $sessions) {
    if ($session.Start -lt $StartAt) {
        continue
    }

    Invoke-PlanSession $session.Name $session.OutputDir $session.ReasoningEffort
}
