Set-StrictMode -Version Latest

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
    Write-BatchMarkdown -Path $mdPath -BatchNumber $BatchNumber -Status $status -ExitCode $exitCodeValue -Tasks $Tasks -ChangedPaths $ChangedPaths -Violations $Violations -Diagnostics $diagnostics -LastMessages $lastMessages -JsonPath $jsonPath
}

function Write-BatchMarkdown {
    param(
        [string]$Path,
        [int]$BatchNumber,
        [string]$Status,
        [int]$ExitCode,
        [string[]]$Tasks,
        [string[]]$ChangedPaths,
        [string[]]$Violations,
        [string[]]$Diagnostics,
        [string[]]$LastMessages,
        [string]$JsonPath
    )

    $batchLabel = "{0:00}" -f $BatchNumber
    $lines = New-Object System.Collections.Generic.List[string]
    $lines.Add("# Workspace crate extraction batch $batchLabel")
    $lines.Add("")
    $lines.Add("Status: $Status")
    $lines.Add("Exit code: $ExitCode")
    $lines.Add("Model: gpt-5.5")
    $lines.Add("Reasoning effort: medium")
    $lines.Add("Summary JSON: $JsonPath")
    $lines.Add("")
    $lines.Add("## Tasks")
    foreach ($task in $Tasks) { $lines.Add("- $task") }
    $lines.Add("")
    $lines.Add("## Changed files")
    if ($ChangedPaths.Count -eq 0) { $lines.Add("- (none)") } else { foreach ($path in $ChangedPaths) { $lines.Add("- $path") } }
    $lines.Add("")
    $lines.Add("## Guard findings")
    if ($Violations.Count -eq 0) { $lines.Add("- PASS") } else { foreach ($violation in $Violations) { $lines.Add("- $violation") } }
    $lines.Add("")
    $lines.Add("## Diagnostic summary")
    foreach ($diagnostic in $Diagnostics) { $lines.Add("- $diagnostic") }
    $lines.Add("")
    $lines.Add("## Last-message files")
    if ($LastMessages.Count -eq 0) { $lines.Add("- (none)") } else { foreach ($path in $LastMessages) { $lines.Add("- $path") } }
    $lines | Set-Content -LiteralPath $Path -Encoding UTF8
}

function Write-FailureRecord {
    param(
        [string]$OutputRoot,
        [int]$BatchNumber,
        [string[]]$Tasks,
        [string[]]$Diagnostics
    )

    $jsonlPath = Join-Path $OutputRoot "failures.jsonl"
    $mdPath = Join-Path $OutputRoot "failures.md"
    $batchLabel = "{0:00}" -f $BatchNumber
    $record = [ordered]@{
        batch = $BatchNumber
        tasks = $Tasks
        diagnostics = $Diagnostics
        written_at = (Get-Date).ToString("o")
    }
    ($record | ConvertTo-Json -Depth 6 -Compress) | Add-Content -LiteralPath $jsonlPath -Encoding UTF8

    $lines = New-Object System.Collections.Generic.List[string]
    if (-not (Test-Path -LiteralPath $mdPath)) {
        $lines.Add("# Workspace crate extraction failures")
        $lines.Add("")
    }
    $lines.Add("## Batch $batchLabel")
    $lines.Add("")
    $lines.Add("Tasks:")
    foreach ($task in $Tasks) { $lines.Add("- $task") }
    $lines.Add("")
    $lines.Add("Diagnostics:")
    foreach ($diagnostic in $Diagnostics) { $lines.Add("- $diagnostic") }
    $lines.Add("")
    $lines | Add-Content -LiteralPath $mdPath -Encoding UTF8
}
