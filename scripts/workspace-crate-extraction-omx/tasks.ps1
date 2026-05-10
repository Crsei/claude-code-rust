Set-StrictMode -Version Latest

function Resolve-RepoPath {
    param(
        [string]$RepoRoot,
        [string]$Path
    )

    if ([System.IO.Path]::IsPathRooted($Path)) {
        return $Path
    }
    return (Join-Path $RepoRoot $Path)
}

function Get-TaskList {
    param([string]$Path)

    if (-not (Test-Path -LiteralPath $Path)) {
        throw "Task file not found: $Path"
    }

    return @(
        Get-Content -LiteralPath $Path -Encoding UTF8 |
            ForEach-Object { $_.Trim() } |
            Where-Object { $_ -and -not $_.StartsWith("#") }
    )
}

function Test-CheckpointTask {
    param([string]$Task)
    return $Task -match '^\[(checkpoint|review|final)\]'
}

function Get-NextBatchSize {
    param(
        [string[]]$Tasks,
        [int]$Index,
        [int]$BatchSize,
        [int]$Remaining
    )

    if (Test-CheckpointTask -Task $Tasks[$Index]) {
        return 1
    }

    $take = [Math]::Min($BatchSize, $Remaining)
    for ($offset = 1; $offset -lt $take; $offset++) {
        if (Test-CheckpointTask -Task $Tasks[$Index + $offset]) {
            return $offset
        }
    }
    return $take
}
