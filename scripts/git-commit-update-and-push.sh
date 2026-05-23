#!/usr/bin/env bash
set -euo pipefail

usage() {
  cat <<'USAGE'
Usage:
  scripts/git-commit-update-and-push.sh -m "<commit message>" [options] [--] <files...>

Options:
  -m, --message <text>   Commit message. Required unless COMMIT_MESSAGE is set.
  -b, --branch <name>    Branch to push. Defaults to the current branch.
  -A, --all               Stage all changes in the current workspace (including new/removed files).
      --no-build         Skip cargo build --workspace --release.
      --skip-push        Create the commit but do not push.
  -h, --help             Show this help.

By default, the script stages only the file paths passed on the command line.
Use --all (-A) to stage all workspace changes at once. This avoids
accidentally committing unrelated local changes in a shared worktree unless
explicitly requested.
USAGE
}

commit_message="${COMMIT_MESSAGE:-}"
branch=""
run_build=1
push_after_commit=1
stage_all=0
paths=()

while [[ $# -gt 0 ]]; do
  case "$1" in
    -m|--message)
      if [[ $# -lt 2 ]]; then
        echo "error: $1 requires a value" >&2
        exit 2
      fi
      commit_message="$2"
      shift 2
      ;;
    -b|--branch)
      if [[ $# -lt 2 ]]; then
        echo "error: $1 requires a value" >&2
        exit 2
      fi
      branch="$2"
      shift 2
      ;;
    -A|--all)
      stage_all=1
      shift
      ;;
    --no-build)
      run_build=0
      shift
      ;;
    --skip-push)
      push_after_commit=0
      shift
      ;;
    -h|--help)
      usage
      exit 0
      ;;
    --)
      shift
      paths+=("$@")
      break
      ;;
    -*)
      echo "error: unknown option: $1" >&2
      usage >&2
      exit 2
      ;;
    *)
      paths+=("$1")
      shift
      ;;
  esac
done

if [[ -z "${commit_message// }" ]]; then
  echo "error: commit message is required" >&2
  usage >&2
  exit 2
fi

repo_root="$(git rev-parse --show-toplevel)"
cd "$repo_root"

if [[ -z "$branch" ]]; then
  branch="$(git branch --show-current)"
fi
if [[ -z "$branch" ]]; then
  echo "error: unable to determine branch; pass --branch <name>" >&2
  exit 2
fi

git config user.name "Crsei"
git config user.email "Crsei@protonmail.com"

export CARGO_HOME=/data2-HDD-SATA-20T/Digital_avatar/haoweiyao/.rust/cargo
export RUSTUP_HOME=/data2-HDD-SATA-20T/Digital_avatar/haoweiyao/.rust/rustup
export PATH="$CARGO_HOME/bin:$PATH"

git status --short

if [[ "$run_build" -eq 1 ]]; then
  cargo build --workspace --release
fi

if [[ "$stage_all" -eq 0 && "${#paths[@]}" -eq 0 ]]; then
  echo "error: no files provided to stage" >&2
  echo "Pass the exact files to commit, or stage files manually and use git directly." >&2
  exit 2
fi

if [[ "$stage_all" -eq 1 ]]; then
  git add -A
else
  git add -A -- "${paths[@]}"
fi

if git diff --cached --quiet; then
  echo "error: no staged changes to commit" >&2
  exit 1
fi

git commit -m "$commit_message"

if [[ "$push_after_commit" -eq 0 ]]; then
  exit 0
fi

set +x
askpass_script="$(mktemp)"
cleanup() {
  rm -f "$askpass_script"
}
trap cleanup EXIT

cat > "$askpass_script" <<'EOF'
#!/bin/sh
case "$1" in
  *Username*) printf '%s\n' 'Crsei' ;;
  *Password*) cat /data2-HDD-SATA-20T/Digital_avatar/haoweiyao/github_token.txt ;;
  *) printf '%s\n' 'Crsei' ;;
esac
EOF
chmod 700 "$askpass_script"

GIT_CONFIG_GLOBAL=/dev/null \
GIT_ASKPASS="$askpass_script" \
GIT_TERMINAL_PROMPT=0 \
git -c credential.helper= push https://github.com/Crsei/claude-code-rust.git "$branch"
