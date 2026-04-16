#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"

if [ -z "${CC_RUST_BINARY:-}" ]; then
  for candidate in \
    "$SCRIPT_DIR/../target/release/claude-code-rs" \
    "$SCRIPT_DIR/../target/release/claude-code-rs.exe" \
    "$SCRIPT_DIR/../target/debug/claude-code-rs" \
    "$SCRIPT_DIR/../target/debug/claude-code-rs.exe"; do
    if [ -x "$candidate" ] || [ -f "$candidate" ]; then
      CC_RUST_BINARY="$candidate"
      break
    fi
  done
fi

if [ -z "${CC_RUST_BINARY:-}" ]; then
  echo "Error: Could not find Rust binary. Build with: cargo build --release" >&2
  exit 1
fi

export CC_RUST_BINARY
exec bun run "$SCRIPT_DIR/src/main.tsx" "$@"
