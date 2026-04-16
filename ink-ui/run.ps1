$ErrorActionPreference = "Stop"
$ScriptDir = Split-Path -Parent $MyInvocation.MyCommand.Path

if (-not $env:CC_RUST_BINARY) {
    $candidates = @(
        "$ScriptDir\..\target\release\claude-code-rs.exe",
        "$ScriptDir\..\target\debug\claude-code-rs.exe"
    )
    foreach ($c in $candidates) {
        if (Test-Path $c) {
            $env:CC_RUST_BINARY = (Resolve-Path $c).Path
            break
        }
    }
}

if (-not $env:CC_RUST_BINARY) {
    Write-Error "Could not find Rust binary. Build with: cargo build --release"
    exit 1
}

& bun run "$ScriptDir\src\main.tsx" @args
