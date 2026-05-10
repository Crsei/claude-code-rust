Set-StrictMode -Version Latest

function Commit-Batch {
    param(
        [string[]]$ChangedPaths,
        [int]$BatchNumber,
        [string]$OutputRoot
    )

    if ($ChangedPaths.Count -eq 0) {
        Write-Host "No new changes to commit for this batch." -ForegroundColor Yellow
        return
    }

    & git add -- @ChangedPaths
    if ($LASTEXITCODE -ne 0) {
        throw "git add failed for batch $BatchNumber"
    }

    $batchLabel = "{0:00}" -f $BatchNumber
    $messagePath = Join-Path $OutputRoot "batch-$batchLabel.commit-message.txt"
    $message = @"
Advance ratatui UI parity batch $batchLabel

This checkpoint keeps the OMX execution lane reviewable by committing only
paths that changed after the runner started. Run artifacts are written under
$OutputRoot for local inspection.

Constraint: Model fixed to gpt-5.5 with medium reasoning
Constraint: Commit path set excludes baseline dirty worktree paths
Rejected: One large final commit | review and rollback would be harder
Confidence: medium
Scope-risk: moderate
Directive: Do not continue from a failed batch without reading the batch summary
Tested: See $OutputRoot/batch-$batchLabel.summary.md
Not-tested: Full workspace test unless the final verification task reports it
"@
    $message | Set-Content -LiteralPath $messagePath -Encoding UTF8

    & git commit -F $messagePath
    if ($LASTEXITCODE -ne 0) {
        throw "git commit failed for batch $BatchNumber"
    }
}
