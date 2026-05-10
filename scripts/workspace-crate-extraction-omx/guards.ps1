Set-StrictMode -Version Latest

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

function Get-AgentReportedFindings {
    param([string]$BatchOutputDir)

    $findings = New-Object System.Collections.Generic.List[string]
    if (-not (Test-Path -LiteralPath $BatchOutputDir)) {
        return @()
    }

    $files = @(Get-ChildItem -LiteralPath $BatchOutputDir -Filter "task-*.last-message.txt" -File)
    foreach ($file in $files) {
        $lineNumber = 0
        foreach ($line in @(Get-Content -LiteralPath $file.FullName -Encoding UTF8)) {
            $lineNumber++
            if ($line -match '^\s*(ERROR|BLOCKER)\b\s*[:：-]?\s*(.*)$') {
                $findings.Add("$($Matches[1]): agent reported $($Matches[1]) in $($file.Name):$lineNumber - $($Matches[2])")
            } elseif ($line -match '^\s*Status\s*[:：-]\s*(ERROR|BLOCKER)\b\s*(.*)$') {
                $findings.Add("$($Matches[1]): agent status $($Matches[1]) in $($file.Name):$lineNumber - $($Matches[2])")
            } elseif ($line -match '^\s*WARNING\b\s*[:：-]?\s*(.*)$') {
                $findings.Add("WARNING: agent reported WARNING in $($file.Name):$lineNumber - $($Matches[1])")
            }
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
