param(
    [string]$Codex = "codex",
    [ValidateRange(0, 7)]
    [int]$StartAt = 0,
    [switch]$ContinueOnError
)

Set-StrictMode -Version Latest
$ErrorActionPreference = "Stop"

$repoRoot = (Resolve-Path (Join-Path $PSScriptRoot "..\..")).Path
$runner = Join-Path $repoRoot "scripts\codex-task-sequence.ps1"
$plan = ".omx/plans/codex-task-sequence-low-reasoning-session-plan-2026-05-08.md"
$model = "gpt-5.5"
$reasoningEffort = "medium"

function Invoke-PlanSession {
    param(
        [string]$Session,
        [string]$OutputDir
    )

    $task = "Execute only '$Session' from $plan. Read that plan file first, obey its Low-Reasoning Contract, stay within that session's owned files and forbidden areas, and return a concise completion note with changed files, verification run, review findings when applicable, and remaining risk."

    if ($ContinueOnError) {
        & $runner `
            -Tasks @($task) `
            -Codex $Codex `
            -WorkDir $repoRoot `
            -OutputDir $OutputDir `
            -Sandbox "danger-full-access" `
            -Model $model `
            -ReasoningEffort $reasoningEffort `
            -ContinueOnError
    } else {
        & $runner `
            -Tasks @($task) `
            -Codex $Codex `
            -WorkDir $repoRoot `
            -OutputDir $OutputDir `
            -Sandbox "danger-full-access" `
            -Model $model `
            -ReasoningEffort $reasoningEffort
    }
}

$sessions = @(
    @{ Start = 0; Name = "Session 0 - Status Gate"; OutputDir = "target/codex-runs/session-00-status-gate" }
    @{ Start = 1; Name = "Session 1 - MCP Startup Diagnostics Follow-up"; OutputDir = "target/codex-runs/session-01-mcp-startup-diagnostics" }
    @{ Start = 2; Name = "Session 2 - Auth Remaining Command Surfaces"; OutputDir = "target/codex-runs/session-02-auth-command-surfaces" }
    @{ Start = 3; Name = "Session 3 - Hook IO Public Diagnostics"; OutputDir = "target/codex-runs/session-03-hook-io-diagnostics" }
    @{ Start = 4; Name = "Session 4 - Plugin Diagnostics Schema Follow-up"; OutputDir = "target/codex-runs/session-04-plugin-diagnostics-schema" }
    @{ Start = 4; Name = "Review Session A - Midpoint Scope And Regression Review"; OutputDir = "target/codex-runs/review-a-midpoint" }
    @{ Start = 5; Name = "Session 5 - Team E2E Coverage"; OutputDir = "target/codex-runs/session-05-team-e2e-coverage" }
    @{ Start = 5; Name = "Review Session B - Final Integration Review"; OutputDir = "target/codex-runs/review-b-final-integration" }
    @{ Start = 6; Name = "Session 6 - Documentation Closure"; OutputDir = "target/codex-runs/session-06-documentation-closure" }
    @{ Start = 7; Name = "Session 7 - Final Verification"; OutputDir = "target/codex-runs/session-07-final-verification" }
)

foreach ($session in $sessions) {
    if ($session.Start -lt $StartAt) {
        continue
    }

    Invoke-PlanSession $session.Name $session.OutputDir
}
