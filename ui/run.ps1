# Launch cc-rust with ink-terminal UI
#
# Usage:
#   .\run.ps1                                    # auto-detect binary
#   $env:CC_RUST_BINARY=".\my-bin" ; .\run.ps1   # custom binary
#
# Prerequisites:
#   - cargo build --release (in parent directory)
#   - bun install (in this directory)

$ScriptDir = Split-Path -Parent $MyInvocation.MyCommand.Path
Set-Location $ScriptDir

if (-not $env:CC_RUST_BINARY) {
    $candidates = @(
        "..\target\release\claude-code-rs.exe",
        "..\target\debug\claude-code-rs.exe"
    )
    foreach ($c in $candidates) {
        if (Test-Path $c) {
            $env:CC_RUST_BINARY = Resolve-Path $c
            break
        }
    }
    if (-not $env:CC_RUST_BINARY) {
        Write-Host "Error: Rust binary not found. Run 'cargo build' first." -ForegroundColor Red
        exit 1
    }
}

Write-Host "Using binary: $env:CC_RUST_BINARY" -ForegroundColor Cyan
bun run src/main.tsx @args
