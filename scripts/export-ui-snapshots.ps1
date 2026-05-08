param(
    [string]$OutputDir = "target/ui-snapshots",
    [switch]$CheckOnly,
    [switch]$SkipTests
)

Set-StrictMode -Version Latest
$ErrorActionPreference = "Stop"

$repoRoot = (Resolve-Path (Join-Path $PSScriptRoot "..")).Path
$resolvedOutputDir = if ([System.IO.Path]::IsPathRooted($OutputDir)) {
    $OutputDir
} else {
    Join-Path $repoRoot $OutputDir
}

Push-Location $repoRoot
try {
    if (-not $SkipTests) {
        $previousInstaUpdate = [Environment]::GetEnvironmentVariable("INSTA_UPDATE", "Process")
        if ($CheckOnly) {
            [Environment]::SetEnvironmentVariable("INSTA_UPDATE", "no", "Process")
        } else {
            [Environment]::SetEnvironmentVariable("INSTA_UPDATE", "always", "Process")
        }

        try {
            cargo test -p claude-code-rs ui::
            if ($LASTEXITCODE -ne 0) {
                exit $LASTEXITCODE
            }
        } finally {
            [Environment]::SetEnvironmentVariable("INSTA_UPDATE", $previousInstaUpdate, "Process")
        }
    }

    cargo run -p claude-code-rs -- --export-ui-snapshots $resolvedOutputDir
    if ($LASTEXITCODE -ne 0) {
        exit $LASTEXITCODE
    }

    Write-Host "Rust TUI snapshots exported to $resolvedOutputDir"
} finally {
    Pop-Location
}
