Set-StrictMode -Version Latest

function Get-GitChangedPaths {
    $tracked = @(& git diff --name-only HEAD -- | Where-Object { $_ })
    $untracked = @(& git ls-files --others --exclude-standard | Where-Object { $_ })
    return @($tracked + $untracked | Sort-Object -Unique)
}

function Get-PathSet {
    param([string[]]$Paths)

    $set = [System.Collections.Generic.HashSet[string]]::new([System.StringComparer]::OrdinalIgnoreCase)
    foreach ($path in $Paths) {
        [void]$set.Add($path)
    }
    return ,$set
}

function Get-RustLineCount {
    param([string]$Path)

    if (-not (Test-Path -LiteralPath $Path)) {
        return 0
    }
    return @((Get-Content -LiteralPath $Path -Encoding UTF8)).Count
}

function Get-ChangedLineCount {
    param([string]$Path)

    $numstat = @(& git diff --numstat HEAD -- $Path | Where-Object { $_ })
    if ($numstat.Count -gt 0) {
        $parts = $numstat[0] -split "`t"
        if ($parts.Count -ge 3) {
            $added = 0
            $deleted = 0
            if (
                [int]::TryParse($parts[0], [ref]$added) -and
                [int]::TryParse($parts[1], [ref]$deleted)
            ) {
                return $added + $deleted
            }
        }
    }

    if (Test-Path -LiteralPath $Path) {
        return @((Get-Content -LiteralPath $Path -Encoding UTF8)).Count
    }
    return 0
}

function Invoke-DiffGuards {
    param(
        [string[]]$ChangedPaths,
        [int]$MaxFiles = 8,
        [int]$MaxChangedLines = 300,
        [double]$MaxChangeRatio = 0.25,
        [int]$MaxRustFileLines = 900
    )

    $findings = New-Object System.Collections.Generic.List[string]
    $paths = @($ChangedPaths | Where-Object { $_ } | Sort-Object -Unique)
    if ($paths.Count -eq 0) {
        return @()
    }

    if ($paths.Count -gt $MaxFiles) {
        $findings.Add("BLOCKER: batch changed $($paths.Count) files, above max $MaxFiles; split unrelated surface work")
    }

    $nameStatus = @(& git diff --name-status HEAD -- @paths | Where-Object { $_ })
    $renameCount = @($nameStatus | Where-Object { $_ -match '^[RC]' }).Count
    if ($renameCount -gt 0) {
        $findings.Add("BLOCKER: batch has $renameCount rename/copy entries; split mechanical moves from UI surface work")
    }

    foreach ($path in $paths) {
        $changedLines = Get-ChangedLineCount -Path $path
        if ($changedLines -gt $MaxChangedLines) {
            $findings.Add("BLOCKER: $path has $changedLines changed lines, above max $MaxChangedLines")
        }

        if (-not (Test-Path -LiteralPath $path)) {
            continue
        }

        $lineCount = @((Get-Content -LiteralPath $path -Encoding UTF8)).Count
        if ($lineCount -gt 0) {
            $ratio = $changedLines / [double]$lineCount
            if ($ratio -gt $MaxChangeRatio) {
                $percent = [Math]::Round($ratio * 100, 1)
                $limitPercent = [Math]::Round($MaxChangeRatio * 100, 1)
                $findings.Add("BLOCKER: $path changed $percent% of current file size, above max $limitPercent%")
            }
        }

        if ($path.EndsWith(".rs", [System.StringComparison]::OrdinalIgnoreCase)) {
            $rustLines = Get-RustLineCount -Path $path
            if ($rustLines -gt $MaxRustFileLines) {
                $findings.Add("BLOCKER: $path has $rustLines lines, above Rust max $MaxRustFileLines; split the touched area or record an explicit narrow-scope rationale")
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
