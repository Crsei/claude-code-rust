#!/bin/bash
# Launch cc-rust with ink-terminal UI
#
# Usage:
#   ./run.sh                  # use default binary path
#   CC_RUST_BINARY=./my-bin ./run.sh  # custom binary
#
# Prerequisites:
#   - cargo build --release (in parent directory)
#   - bun install (in this directory)

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
cd "$SCRIPT_DIR"

# Default binary path
if [ -z "$CC_RUST_BINARY" ]; then
  if [ -f "../target/release/claude-code-rs.exe" ]; then
    export CC_RUST_BINARY="../target/release/claude-code-rs.exe"
  elif [ -f "../target/release/claude-code-rs" ]; then
    export CC_RUST_BINARY="../target/release/claude-code-rs"
  elif [ -f "../target/debug/claude-code-rs.exe" ]; then
    export CC_RUST_BINARY="../target/debug/claude-code-rs.exe"
  elif [ -f "../target/debug/claude-code-rs" ]; then
    export CC_RUST_BINARY="../target/debug/claude-code-rs"
  else
    echo "Error: Rust binary not found. Run 'cargo build' first."
    exit 1
  fi
fi

echo "Using binary: $CC_RUST_BINARY"
exec bun run src/main.tsx "$@"
