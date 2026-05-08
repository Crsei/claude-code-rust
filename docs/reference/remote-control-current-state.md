# Remote Control Gateway Current State

Updated: 2026-05-08

This document freezes the Phase 0 boundary for the remote-control gateway work. It records what exists today and what later sessions are allowed to build against.

## Current Runtime Surfaces

| Surface | Current role | Remote-control boundary |
| --- | --- | --- |
| `crates/gateway` | Not implemented yet. Planned as the durable remote-control control plane. | Owns remote source/session/run models, gateway HTTP API, auth policy, adapter registry, durable run events, delivery, recovery, and redaction. It must not depend on `claude-code-rs` internals. |
| `crates/claude-code-rs/src/daemon` | Existing background HTTP/SSE service and worker/supervisor host. | Execution host only. It bridges gateway commands/events to the existing worker protocol and `QueryEngine`; it must not absorb remote-control policy, adapter logic, or durable run-store ownership. |
| `crates/claude-code-rs/src/ipc` | Existing local headless JSONL bridge for a frontend process. | Local UI bridge only. Gateway must not reuse `ipc::headless::run_headless`, must not expose `FrontendMessage` as a remote API, and must not model Telegram/Lark events as IPC messages. Shared protocol types should move to `cc-types` or a small protocol crate if needed later. |
| `/api/*` daemon routes | Existing local daemon control API for submit/status/history/permission and related operations. | Not a public remote-control API. These routes remain local/daemon compatibility endpoints and must not be documented or treated as the stable external control surface. |
| `/remote-control/v1/**` | Not implemented yet. | Planned stable external gateway API for capabilities, runs, events, approval/ask-user, webhooks, and adapter status/connect/test-message. |
| `/remote` slash command | Not registered yet. | Planned local user operation surface implemented in `claude-code-rs`, backed by the loopback gateway API/local client. Mutating actions must not write worker command files directly. |
| TUI `RemoteSurface` and status indicator | Not implemented yet. | Planned local discoverability and status UI implemented in `claude-code-rs`. Render paths must use cached snapshots and must not perform blocking network IO. |

## Existing Daemon Baseline

- The daemon starts behind the `FEATURE_KAIROS=1` gate and defaults to loopback binding.
- Current daemon HTTP routes include `/api/*`, `/webhook/*`, `/events`, and `/health`.
- Mutating daemon routes already require the local daemon control token, but this token and route family are not sufficient to make `/api/*` safe as a public remote-control API.
- Current `/api/submit` still has legacy supervisor execution ownership that later Phase 3 work must resolve before gateway runs can claim a single worker-owned lifecycle.
- Current SSE replay is a daemon event convenience, not the durable run event log required by the gateway.

## Boundary Decisions

- `crates/gateway` is the control plane. It owns transport-neutral remote identity, run identity, busy policy, persistent run state, remote API shape, adapter status, delivery, and security diagnostics.
- The daemon is the execution host. It owns process supervision, worker command/event files, and the bridge from gateway commands to `QueryEngine`.
- `ipc` is the local headless UI bridge. It remains separate from gateway transport and recovery semantics.
- `/remote` and TUI surfaces are local user entry points. They belong in `claude-code-rs` and call the gateway through a local client; they do not belong in `crates/gateway`.
- `/api/*` is not the public remote-control API. The stable external surface is `/remote-control/v1/**`.
- Telegram and Lark are first-wave gateway adapters. Their first version is limited to connection, health checks, provider-neutral status diagnostics, and test-message sending. Inbound conversation control and full remote session parity are follow-up work.

## Phase 0 Gaps To Preserve

- No `crates/gateway` crate exists yet.
- No `/remote-control/v1/**` routes exist yet.
- No `/remote` slash command exists yet.
- No TUI `RemoteSurface` exists yet.
- No remote status indicator exists yet.
- Telegram/Lark remote session control is intentionally out of the first adapter slice; only adapter connectivity and diagnostics are in scope for the first version.

## Verification Anchor

Use this document with:

```powershell
rg -n "remote-control|Telegram|Lark|ipc|/remote|gateway" docs/reference docs/IMPLEMENTATION_GAPS.md docs/plan
```
