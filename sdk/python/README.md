# claude-code-rs Python SDK

A thin Python wrapper around the `claude-code-rs` CLI binary that provides a typed, streaming interface for programmatic interaction.

## Requirements

- Python 3.10+
- `claude-code-rs` binary (on PATH, or set `CLAUDE_CODE_RS_PATH`)
- Zero runtime dependencies — only Python standard library

## Installation

```bash
pip install -e sdk/python
```

## Quick Start

```python
from claude_code_rs import ClaudeCode

client = ClaudeCode()
session = client.start_session()
turn = session.run("What files are in this directory?")
print(turn.final_response)
```

## Streaming

```python
from claude_code_rs import ClaudeCode, ItemCompletedEvent, AgentMessageItem

client = ClaudeCode()
session = client.start_session()

streamed = session.run_streamed("Explain this codebase")
for event in streamed.events:
    if isinstance(event, ItemCompletedEvent):
        if isinstance(event.item, AgentMessageItem):
            print(event.item.text, end="", flush=True)
print()
```

## Configuration

```python
from claude_code_rs import ClaudeCode, ClientOptions, SessionOptions

client = ClaudeCode(ClientOptions(
    executable_path="/path/to/claude-code-rs",
    api_key="sk-ant-...",
))

session = client.start_session(SessionOptions(
    model="claude-sonnet-4-20250514",
    permission_mode="auto",
    max_turns=10,
    max_budget=0.50,
    working_directory="/path/to/project",
    system_prompt="You are a code reviewer. Be concise.",
    verbose=True,
))

turn = session.run("Review the last commit")
print(f"Response: {turn.final_response}")
print(f"Cost: ${turn.usage.total_cost_usd:.4f}" if turn.usage else "")
```

## Resuming Sessions

```python
client = ClaudeCode()
session = client.start_session()
turn1 = session.run("Read src/main.rs")

# Later — resume with the same session ID
resumed = client.resume_session(session.session_id)
turn2 = resumed.run("Now explain the bootstrap phase")
```

## Error Handling

```python
from claude_code_rs import (
    ClaudeCode, CcRustError, BinaryNotFoundError,
    TurnExecutionError, ProcessError,
)

try:
    client = ClaudeCode()
    session = client.start_session()
    turn = session.run("Do something")
except BinaryNotFoundError as e:
    print(f"Install claude-code-rs first. Searched: {e.searched}")
except TurnExecutionError as e:
    print(f"Turn failed [{e.subtype}]: {e}")
except ProcessError as e:
    print(f"Process crashed (exit {e.exit_code}): {e.stderr}")
except CcRustError as e:
    print(f"SDK error: {e}")
```

## Architecture

```
ClaudeCode (client)
  -> Session (session management)
    -> ClaudeCodeExec (subprocess spawn + JSONL readline)
      -> spawn('claude-code-rs --output-format json -p')
        -> stdin: prompt
        <- stdout: JSONL lines
      -> transform_raw_event()
      -> yield SessionEvent
```

The SDK mirrors the [TypeScript SDK](../typescript/) and follows the same event model.

## Event Types

| Event | Description |
|-------|-------------|
| `SessionStartedEvent` | Session initialized (tools, model, permission mode) |
| `ItemCompletedEvent` | An item was completed (agent message, tool summary, etc.) |
| `StreamDeltaEvent` | Real-time streaming content delta |
| `TurnCompletedEvent` | Turn finished successfully (usage, result) |
| `TurnFailedEvent` | Turn finished with an error |
| `SessionErrorEvent` | Retryable API error |

## Item Types

| Item | Source |
|------|--------|
| `AgentMessageItem` | Assistant response with text + content blocks |
| `ToolUseSummaryItem` | Consolidated tool execution summary |
| `CompactBoundaryItem` | Context compression marker |
| `UserReplayItem` | User message replay |
| `ErrorItem` | Error message |
