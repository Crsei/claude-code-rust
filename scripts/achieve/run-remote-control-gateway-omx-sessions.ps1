param(
    [string]$Codex = "codex",
    [string]$Model = "gpt-5.5",
    [ValidateSet("minimal", "low", "medium", "high", "xhigh")]
    [string]$ReasoningEffort = "medium",
    [ValidateRange(0, 99)]
    [int]$StartAt = 0,
    [ValidateRange(0, 99)]
    [int]$EndAt = 99,
    [string]$OnlySession = "",
    [ValidateRange(0, 99)]
    [int]$MaxSessions = 0,
    [switch]$ContinueOnError,
    [switch]$SkipOmxPreflight,
    [switch]$DryRun
)

Set-StrictMode -Version Latest
$ErrorActionPreference = "Stop"

$repoRoot = (Resolve-Path (Join-Path $PSScriptRoot "..\..")).Path
$runner = Join-Path $repoRoot "scripts\codex-task-sequence.ps1"
$plan = "docs/plan/remote-control-gateway-omx-execution-plan-2026-05-08.md"
$sourcePlan = "docs/plan/remote-control-gateway-execution-plan-2026-05-08.md"
$tasksFile = "docs/scripts/achieve/remote-control-gateway-omx-tasks-2026-05-08.txt"

if (-not (Test-Path $runner)) {
    throw "Missing runner: $runner"
}
if (-not (Test-Path (Join-Path $repoRoot $plan))) {
    throw "Missing execution plan: $plan"
}
if (-not (Test-Path (Join-Path $repoRoot $sourcePlan))) {
    throw "Missing source plan: $sourcePlan"
}

if (-not $SkipOmxPreflight) {
    $omx = Get-Command omx -ErrorAction SilentlyContinue
    if (-not $omx) {
        throw "omx is not available on PATH. Re-run with -SkipOmxPreflight only if this is intentional."
    }

    & omx --version
    if ($LASTEXITCODE -ne 0) {
        throw "omx preflight failed with exit code $LASTEXITCODE"
    }
}

$sessions = @(
    @{ Id = 0; Name = "Session 00 - Baseline And Partition"; OutputDir = "target/codex-runs/remote-control-gateway/session-00-baseline" }
    @{ Id = 1; Name = "Session 01 - Phase 0 Boundary Docs"; OutputDir = "target/codex-runs/remote-control-gateway/session-01-boundary-docs" }
    @{ Id = 2; Name = "Session 02 - Phase 1A Gateway Crate Foundation"; OutputDir = "target/codex-runs/remote-control-gateway/session-02-gateway-foundation" }
    @{ Id = 3; Name = "Session 03 - Phase 1B Run Store And Policy"; OutputDir = "target/codex-runs/remote-control-gateway/session-03-run-store-policy" }
    @{ Id = 4; Name = "Review A - Schema Store Review"; OutputDir = "target/codex-runs/remote-control-gateway/review-a-schema-store" }
    @{ Id = 5; Name = "Session 04 - Phase 2A Adapter Registry And Telegram"; OutputDir = "target/codex-runs/remote-control-gateway/session-04-adapter-telegram" }
    @{ Id = 6; Name = "Session 05 - Phase 2B Lark And Adapter Status API Model"; OutputDir = "target/codex-runs/remote-control-gateway/session-05-lark-adapter" }
    @{ Id = 7; Name = "Session 06 - Phase 3A Runner And Daemon Protocol Bridge"; OutputDir = "target/codex-runs/remote-control-gateway/session-06-runner-bridge" }
    @{ Id = 8; Name = "Session 07 - Phase 3B Worker Ownership Migration"; OutputDir = "target/codex-runs/remote-control-gateway/session-07-worker-ownership" }
    @{ Id = 9; Name = "Review B - Execution Ownership Review"; OutputDir = "target/codex-runs/remote-control-gateway/review-b-execution-ownership" }
    @{ Id = 10; Name = "Session 08 - Phase 4A HTTP API Capabilities And Runs"; OutputDir = "target/codex-runs/remote-control-gateway/session-08-api-capabilities-runs" }
    @{ Id = 11; Name = "Session 09 - Phase 4B Events Stop Approval AskUser Adapters API"; OutputDir = "target/codex-runs/remote-control-gateway/session-09-api-events-adapters" }
    @{ Id = 12; Name = "Review C - API Contract Review"; OutputDir = "target/codex-runs/remote-control-gateway/review-c-api-contract" }
    @{ Id = 13; Name = "Session 10 - Phase 4.5A Local Gateway Client And /remote"; OutputDir = "target/codex-runs/remote-control-gateway/session-10-remote-command" }
    @{ Id = 14; Name = "Session 11 - Phase 4.5B TUI Remote Surface"; OutputDir = "target/codex-runs/remote-control-gateway/session-11-tui-remote-surface" }
    @{ Id = 15; Name = "Review D - Local UX And Redaction Review"; OutputDir = "target/codex-runs/remote-control-gateway/review-d-local-ux-redaction" }
    @{ Id = 16; Name = "Session 12 - Phase 5 Security Hardening"; OutputDir = "target/codex-runs/remote-control-gateway/session-12-security-hardening" }
    @{ Id = 17; Name = "Review E - Security Gate"; OutputDir = "target/codex-runs/remote-control-gateway/review-e-security-gate" }
    @{ Id = 18; Name = "Session 13 - Phase 6 Declarative Webhooks"; OutputDir = "target/codex-runs/remote-control-gateway/session-13-webhooks" }
    @{ Id = 19; Name = "Session 14 - Phase 7 Delivery Router"; OutputDir = "target/codex-runs/remote-control-gateway/session-14-delivery-router" }
    @{ Id = 20; Name = "Session 15 - Phase 8 Recovery Queue E2E"; OutputDir = "target/codex-runs/remote-control-gateway/session-15-recovery-e2e" }
    @{ Id = 21; Name = "Review F - Recovery Gate"; OutputDir = "target/codex-runs/remote-control-gateway/review-f-recovery-gate" }
    @{ Id = 22; Name = "Session 16 - Phase 9 Docs Release Gate"; OutputDir = "target/codex-runs/remote-control-gateway/session-16-docs-release-gate" }
    @{ Id = 23; Name = "Session 17 - Final Verification"; OutputDir = "target/codex-runs/remote-control-gateway/session-17-final-verification" }
)

if ($OnlySession) {
    $selected = @(
        $sessions | Where-Object {
            "$($_.Id)" -eq $OnlySession -or $_.Name -like "*$OnlySession*"
        }
    )
} else {
    $selected = @(
        $sessions | Where-Object {
            $_.Id -ge $StartAt -and $_.Id -le $EndAt
        }
    )
}

if ($MaxSessions -gt 0) {
    $selected = @($selected | Select-Object -First $MaxSessions)
}

if ($selected.Count -eq 0) {
    throw "No sessions selected. Check -StartAt/-EndAt/-OnlySession."
}

function New-SessionPrompt {
    param(
        [hashtable]$Session
    )

    return @"
Execute only '$($Session.Name)' from $plan.
Read $plan, $sourcePlan, and $tasksFile first.

Fixed execution profile:
- model: $Model
- reasoning_effort: $ReasoningEffort
- reduce visible reasoning; report only actions, evidence, changed files, verification, commit hash, and remaining risk.

OMX/subagent rules:
- Use omx sparkshell or rg for simple read-only lookups.
- omx explore may be unavailable on Windows; if it fails, fall back to PowerShell + rg and mention that briefly.
- Use at most two default subagents only when work is independent and clearly bounded. Subagents must use gpt-5.5 / medium when configurable.

Implementation rules:
- Do not continue to later sessions.
- Stay inside this session's owned files and forbidden areas.
- Do not stage or revert unrelated dirty worktree files.
- If this session modifies files, run the required verification, run the line-count/refactor check from the plan, then git commit only this session's owned changes using the Lore Commit Protocol.
- If verification fails, fix within scope; if blocked, stop with the first actionable error and do not claim completion.
"@
}

Write-Host ""
Write-Host "Remote-control gateway session runner" -ForegroundColor Cyan
Write-Host "Plan: $plan"
Write-Host "Model: $Model"
Write-Host "Reasoning effort: $ReasoningEffort"
Write-Host "Selected sessions: $($selected.Count)"

foreach ($session in $selected) {
    Write-Host ("[{0:00}] {1}" -f $session.Id, $session.Name) -ForegroundColor Yellow
    Write-Host "     Output: $($session.OutputDir)"
}

if ($DryRun) {
    Write-Host ""
    Write-Host "Dry run complete. No Codex sessions were started." -ForegroundColor Green
    exit 0
}

foreach ($session in $selected) {
    $task = New-SessionPrompt $session

    $params = @{
        Tasks           = @($task)
        Codex           = $Codex
        WorkDir         = $repoRoot
        OutputDir       = $session.OutputDir
        Sandbox         = "danger-full-access"
        Model           = $Model
        ReasoningEffort = $ReasoningEffort
    }

    if ($ContinueOnError) {
        $params.ContinueOnError = $true
    }

    & $runner @params
    if ($LASTEXITCODE -ne 0 -and -not $ContinueOnError) {
        exit $LASTEXITCODE
    }
}
