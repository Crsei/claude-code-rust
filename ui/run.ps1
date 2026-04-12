# Launch cc-rust with OpenTUI frontend
#
# Usage:
#   .\run.ps1                                    # auto-detect binary
#   $env:CC_RUST_BINARY=".\my-bin" ; .\run.ps1   # custom binary
#
# Global shortcut (run from any directory):
#   Add to PowerShell $PROFILE:
#     function cc-rust { & "F:\AIclassmanager\cc\rust\ui\run.ps1" @args }
#   Then use: cc-rust
#
# Prerequisites:
#   - cargo build --release (in parent directory)
#   - bun install (in this directory)

$ScriptDir = Split-Path -Parent $MyInvocation.MyCommand.Path

if (-not $env:CC_RUST_BINARY) {
    $candidates = @(
        (Join-Path $ScriptDir "..\target\release\claude-code-rs.exe"),
        (Join-Path $ScriptDir "..\target\debug\claude-code-rs.exe")
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
$entryPoint = Join-Path $ScriptDir "src\main.tsx"
bun run $entryPoint @args
