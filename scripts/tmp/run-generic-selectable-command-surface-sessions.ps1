param(
    [string]$Codex = "codex",
    [string]$Model = "",
    [ValidateRange(0, 5)]
    [int]$StartAt = 0,
    [switch]$ContinueOnError
)

Set-StrictMode -Version Latest
$ErrorActionPreference = "Stop"

$repoRoot = (Resolve-Path (Join-Path $PSScriptRoot "..\..")).Path
$runner = Join-Path $repoRoot "scripts\codex-task-sequence.ps1"
$plan = ".omx/plans/generic-selectable-command-surface-medium-session-plan-2026-05-08.md"
$sourcePlan = "docs/plan/generic-selectable-command-surface-plan-2026-05-08.md"
$reasoningEffort = "medium"

function Invoke-PlanSession {
    param(
        [string]$Session,
        [string]$OutputDir
    )

    $task = "Execute only '$Session' from $plan. Read $sourcePlan and $plan first, obey the Medium-Reasoning Contract, keep model reasoning effort at medium, stay within that session's owned files and forbidden areas, and return a concise completion note with changed files, verification run, review findings when applicable, and remaining risk."

    $params = @{
        Tasks           = @($task)
        Codex           = $Codex
        WorkDir         = $repoRoot
        OutputDir       = $OutputDir
        Sandbox         = "danger-full-access"
        ReasoningEffort = $reasoningEffort
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
    @{ Start = 0; Name = "Session 0 - Baseline Status Gate"; OutputDir = "target/codex-runs/generic-selectable-session-00-baseline" }
    @{ Start = 1; Name = "Session 1 - SelectableListState Foundation"; OutputDir = "target/codex-runs/generic-selectable-session-01-list-state" }
    @{ Start = 2; Name = "Session 2 - Plugin Rows Adapter"; OutputDir = "target/codex-runs/generic-selectable-session-02-plugin-adapter" }
    @{ Start = 3; Name = "Session 3 - Plugin Command Surface Integration"; OutputDir = "target/codex-runs/generic-selectable-session-03-plugin-surface" }
    @{ Start = 3; Name = "Review Session A - First-Version Integration Review"; OutputDir = "target/codex-runs/generic-selectable-review-a" }
    @{ Start = 4; Name = "Session 4 - Documentation Closure"; OutputDir = "target/codex-runs/generic-selectable-session-04-docs-closure" }
    @{ Start = 5; Name = "Session 5 - Final Verification"; OutputDir = "target/codex-runs/generic-selectable-session-05-final-verification" }
)

foreach ($session in $sessions) {
    if ($session.Start -lt $StartAt) {
        continue
    }

    Invoke-PlanSession $session.Name $session.OutputDir
}
