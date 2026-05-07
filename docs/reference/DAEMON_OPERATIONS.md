# Daemon 操作与发布检查

> 状态日期：2026-05-05
> 范围：`crates/claude-code-rs/src/daemon/**`、daemon CLI 管理命令、KAIROS HTTP/SSE 控制面。

## 当前可用能力

Rust 端 daemon 现在已经从单进程 `--daemon` HTTP demo 推进为可由外部 CLI 管理的后台进程形态：

- supervisor 状态写入 `~/.cc-rust/daemon/supervisor.json`。
- worker 状态写入 `~/.cc-rust/daemon/workers/<worker-id>.json`。
- command 文件写入 `~/.cc-rust/daemon/commands/<worker-id>/<command-id>.json`。
- worker event 写入 `~/.cc-rust/daemon/events/<worker-id>.ndjson`。
- control token 写入 `~/.cc-rust/daemon/control-token.json`，daemon stop 时清理。
- sleep state 写入 `~/.cc-rust/daemon/sleep-state.json`，过期、wake 或 stop 时清理。

所有 daemon 持久化状态必须继续留在 `~/.cc-rust/daemon/`，不能写入上游 Claude/Codex 路径。

## CLI 管理命令

daemon 管理命令是首选入口；`--daemon` 仍是隐藏运行面。

```bash
FEATURE_KAIROS=1 claude-code-rs daemon start
FEATURE_KAIROS=1 claude-code-rs daemon start --port 19837
claude-code-rs daemon status
claude-code-rs daemon token
claude-code-rs daemon submit "hello"
claude-code-rs daemon abort
claude-code-rs daemon command <command-id> [worker-id]
claude-code-rs daemon events [worker-id]
claude-code-rs daemon sleep 60 "pause proactive"
claude-code-rs daemon wake
claude-code-rs daemon stop
claude-code-rs daemon restart --port 19837
```

`daemon start` 需要 `FEATURE_KAIROS=1`。`daemon status/stop/restart` 可以从另一个 CLI 进程操作同一个后台 supervisor。

## Slash 命令

`/daemon` 是会话内的轻量入口：

- `/daemon` 或 `/daemon status`：读取跨进程 supervisor/worker 状态。
- `/daemon stop`：写入 shutdown request，让后台 supervisor 优雅退出。
- `/daemon start` 与 `/daemon restart`：只提示使用 shell 管理命令，不在当前 REPL 内 fork 后台进程。

`/sleep <seconds>` 会写入 daemon sleep state，使 proactive tick 与 `/api/status` 都能观察到同一份休眠状态。

## HTTP 控制面

daemon 默认监听 `127.0.0.1:19836`，可通过 `--port` 调整。

- `GET /health`：健康检查。
- `GET /api/status`：返回 QueryEngine、supervisor、workers、command root、event log、daemon sleep state。
- `GET /api/history`：返回当前 SSE buffer 与 daemon worker event log。
- `GET /events`：SSE stream，连接时 replay assistant worker event log。
- `POST /api/submit`：投递 submit command，并沿用现有 QueryEngine/SSE 路径执行。
- `POST /api/abort`：投递 abort command 并调用当前 engine abort。
- `POST /api/permission`：投递 permission response command。
- `POST /api/command`：执行 slash command。
- `POST /api/resize`：当前明确为 no-op。

所有 mutating endpoint 都必须带 token：

```bash
TOKEN=$(claude-code-rs daemon token)
curl -H "x-cc-rust-daemon-token: $TOKEN" \
  -H "content-type: application/json" \
  -d '{"decision":"allow","tool_use_id":"example"}' \
  http://127.0.0.1:19836/api/permission
```

也可以使用 `Authorization: Bearer <token>`。

## 发布验证命令

Phase 1-6 已经用目标测试和本地 smoke 验证过核心路径。发布前至少运行：

```bash
rustfmt --edition 2021 --check \
  crates/claude-code-rs/src/daemon/process_state.rs \
  crates/claude-code-rs/src/daemon/protocol.rs \
  crates/claude-code-rs/src/daemon/routes.rs \
  crates/claude-code-rs/src/daemon/sse.rs \
  crates/claude-code-rs/src/daemon/supervisor.rs \
  crates/claude-code-rs/src/daemon/tick.rs \
  crates/claude-code-rs/src/commands/daemon_cmd.rs \
  crates/claude-code-rs/src/commands/sleep_cmd.rs \
  crates/claude-code-rs/src/tools/exec/sleep.rs

cargo test -p cc-config partition_functions_all_root_under_data_root --lib
cargo test -p claude-code-rs daemon::process_state
cargo test -p claude-code-rs daemon::protocol
cargo test -p claude-code-rs daemon::routes
cargo test -p claude-code-rs daemon::sse
cargo test -p claude-code-rs daemon::supervisor
cargo test -p claude-code-rs commands::daemon_cmd
cargo test -p claude-code-rs tools::exec::sleep
cargo test -p claude-code-rs --test e2e_cli daemon_management_reports_stopped_state_without_running_daemon
```

仍应在干净工作区补跑完整门槛：

```bash
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

2026-05-05 当前检查结果：

- `cargo fmt --all --check` 通过。
- `cargo clippy --workspace --all-targets -- -D warnings` 被 `crates/cc-sandbox/src/runner.rs:241` 的既有 `clippy::needless_lifetimes` 阻塞。
- `cargo test --workspace` 在 daemon memory log 测试修正后仍有 6 个非 daemon 失败：config command、IDE command、teams mailbox、file edit 和 worktree tests。

不要把这些阻塞误判为 daemon 专项回归；daemon 目标测试集应以本节上方命令为准。

## 仍未完成

以下能力尚未达到完整上游 parity：

- assistant worker 尚未完全拥有 `/api/submit` 的 QueryEngine 执行所有权；当前 HTTP supervisor 路径仍负责真实模型调用。
- bridge worker 尚未接入 remote-control server 注册、远程 submit/abort/permission 映射和结果回传。
- scheduler/proactive worker 尚未独立化，cron-style task 与 daily log 结构化双轨仍未接入。
- 30 分钟以上 daemon soak、worker 崩溃自动 e2e、HTTP submit + SSE result live e2e 仍需在有模型凭据和干净工作区时执行。
