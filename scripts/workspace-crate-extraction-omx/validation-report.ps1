Set-StrictMode -Version Latest

function Invoke-LoggedCargoCommand {
    param(
        [string[]]$CargoArgs,
        [string]$LogPath
    )

    $stdoutPath = "$LogPath.stdout.tmp"
    $stderrPath = "$LogPath.stderr.tmp"
    Remove-Item -LiteralPath $stdoutPath, $stderrPath -ErrorAction SilentlyContinue

    try {
        $process = Start-Process `
            -FilePath "cargo" `
            -ArgumentList $CargoArgs `
            -NoNewWindow `
            -Wait `
            -PassThru `
            -RedirectStandardOutput $stdoutPath `
            -RedirectStandardError $stderrPath
        $exitCode = $process.ExitCode
    } catch {
        "ERROR: failed to start cargo: $($_.Exception.Message)" |
            Add-Content -LiteralPath $LogPath -Encoding UTF8
        Write-Host "ERROR: failed to start cargo: $($_.Exception.Message)" -ForegroundColor Red
        return 1
    }

    foreach ($path in @($stdoutPath, $stderrPath)) {
        if (-not (Test-Path -LiteralPath $path)) {
            continue
        }
        $lines = @(Get-Content -LiteralPath $path -Encoding UTF8)
        if ($lines.Count -gt 0) {
            $lines | Add-Content -LiteralPath $LogPath -Encoding UTF8
            $lines | Out-Host
        }
        Remove-Item -LiteralPath $path -ErrorAction SilentlyContinue
    }

    return $exitCode
}

function Invoke-FinalValidation {
    param([string]$OutputRoot)

    $validationDir = Join-Path $OutputRoot "final-validation"
    New-Item -ItemType Directory -Force -Path $validationDir | Out-Null

    $commands = @(
        @{ name = "fmt"; args = @("fmt", "--all", "--check") },
        @{ name = "check"; args = @("check", "--workspace", "--all-targets") },
        @{ name = "clippy"; args = @("clippy", "--workspace", "--all-targets", "--", "-D", "warnings") },
        @{ name = "test"; args = @("test", "--workspace") }
    )
    $results = New-Object System.Collections.Generic.List[object]

    foreach ($command in $commands) {
        $name = [string]$command.name
        $args = [string[]]$command.args
        $logPath = Join-Path $validationDir "$name.log"
        $display = "cargo $($args -join ' ')"
        "Command: $display" | Set-Content -LiteralPath $logPath -Encoding UTF8
        "Started: $((Get-Date).ToString("o"))" | Add-Content -LiteralPath $logPath -Encoding UTF8
        "" | Add-Content -LiteralPath $logPath -Encoding UTF8

        Write-Host ""
        Write-Host "=== Final validation: $display ===" -ForegroundColor Cyan
        $exitCode = Invoke-LoggedCargoCommand -CargoArgs $args -LogPath $logPath
        "Finished: $((Get-Date).ToString("o"))" | Add-Content -LiteralPath $logPath -Encoding UTF8
        "Exit code: $exitCode" | Add-Content -LiteralPath $logPath -Encoding UTF8

        $results.Add([ordered]@{
            name = $name
            command = $display
            exit_code = $exitCode
            log = $logPath
        })
    }

    $jsonPath = Join-Path $validationDir "summary.json"
    $mdPath = Join-Path $validationDir "summary.md"
    @($results) | ConvertTo-Json -Depth 5 | Set-Content -LiteralPath $jsonPath -Encoding UTF8

    $lines = New-Object System.Collections.Generic.List[string]
    $lines.Add("# Final validation")
    $lines.Add("")
    foreach ($result in $results) {
        $status = if ($result.exit_code -eq 0) { "PASS" } else { "ERROR" }
        $lines.Add("- ${status}: `$($result.command)` -> $($result.log)")
    }
    $lines | Set-Content -LiteralPath $mdPath -Encoding UTF8

    return @($results)
}

function Write-RunReport {
    param(
        [string]$OutputRoot,
        [datetime]$StartedAt,
        [datetime]$EndedAt,
        [int]$TotalTasks,
        [int]$CompletedTasks,
        [switch]$StoppedEarly,
        [string]$StopReason,
        [object[]]$ValidationResults,
        [switch]$DryRun
    )

    $summaryFiles = @(Get-ChildItem -LiteralPath $OutputRoot -Filter "batch-*.summary.json" -File -ErrorAction SilentlyContinue | Sort-Object Name)
    $summaries = @($summaryFiles | ForEach-Object { Get-Content -Raw -LiteralPath $_.FullName | ConvertFrom-Json })
    $statusGroups = @($summaries | Group-Object status | Sort-Object Name)
    $reportPath = Join-Path $OutputRoot "final-report.md"
    $jsonPath = Join-Path $OutputRoot "final-report.json"
    $currentStatus = @(& git status --short | Select-Object -First 200)

    $record = [ordered]@{
        started_at = $StartedAt.ToString("o")
        ended_at = $EndedAt.ToString("o")
        dry_run = [bool]$DryRun
        total_tasks = $TotalTasks
        completed_tasks = $CompletedTasks
        stopped_early = [bool]$StoppedEarly
        stop_reason = $StopReason
        batch_count = $summaries.Count
        status_counts = @($statusGroups | ForEach-Object { [ordered]@{ status = $_.Name; count = $_.Count } })
        validation = $ValidationResults
        git_status_sample = $currentStatus
    }
    $record | ConvertTo-Json -Depth 7 | Set-Content -LiteralPath $jsonPath -Encoding UTF8

    $lines = New-Object System.Collections.Generic.List[string]
    $lines.Add("# Workspace crate extraction final report")
    $lines.Add("")
    $lines.Add("Started: $($StartedAt.ToString("o"))")
    $lines.Add("Ended: $($EndedAt.ToString("o"))")
    $lines.Add("Dry run: $([bool]$DryRun)")
    $lines.Add("Tasks completed: $CompletedTasks / $TotalTasks")
    $lines.Add("Stopped early: $([bool]$StoppedEarly)")
    if ($StopReason) { $lines.Add("Stop reason: $StopReason") }
    $lines.Add("")
    $lines.Add("## Batch status counts")
    if ($statusGroups.Count -eq 0) {
        $lines.Add("- (none)")
    } else {
        foreach ($group in $statusGroups) {
            $lines.Add("- $($group.Name): $($group.Count)")
        }
    }
    $lines.Add("")
    $lines.Add("## Failure log")
    $failurePath = Join-Path $OutputRoot "failures.md"
    if (Test-Path -LiteralPath $failurePath) {
        $lines.Add("- $failurePath")
    } else {
        $lines.Add("- No batch failures recorded.")
    }
    $lines.Add("")
    $lines.Add("## Final validation")
    if ($ValidationResults.Count -eq 0) {
        $lines.Add("- Not run.")
    } else {
        foreach ($result in $ValidationResults) {
            $status = if ($result.exit_code -eq 0) { "PASS" } else { "ERROR" }
            $lines.Add("- ${status}: `$($result.command)` -> $($result.log)")
        }
    }
    $lines.Add("")
    $lines.Add("## Current git status sample")
    if ($currentStatus.Count -eq 0) {
        $lines.Add("- clean")
    } else {
        foreach ($line in $currentStatus) {
            $lines.Add("- $line")
        }
    }
    $lines.Add("")
    $lines.Add("JSON: $jsonPath")
    $lines | Set-Content -LiteralPath $reportPath -Encoding UTF8
}
