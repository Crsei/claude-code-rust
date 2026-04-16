# Codex Backend

cc-rust can delegate model execution to the local official Codex CLI instead of
calling the native HTTP providers directly.

## Enable

Use `.cc-rust/settings.json`:

```json
{
  "backend": "codex",
  "model": "gpt-5.2-codex"
}
```

Or use environment variables:

```env
CC_BACKEND=codex
CLAUDE_BACKEND=codex
CLAUDE_MODEL=gpt-5.2-codex
```

## Requirements

- `codex` is installed locally
- `codex login status` succeeds on this machine
- the current process can access Codex's local config and auth state

## Behavior

- `backend=codex` skips direct provider HTTP calls in cc-rust
- each turn is delegated to local `codex exec --json`
- Codex command executions are mapped back into cc-rust tool-use/tool-result UI events
