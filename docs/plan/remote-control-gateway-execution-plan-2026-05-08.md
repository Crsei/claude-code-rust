# Remote Control Gateway 执行计划

日期：2026-05-08

参考来源：`F:/AIclassmanager/cc/codex/docs/reference/remote-control/hermes-gateway-design.md`

范围：新增 `crates/gateway/**`，并在 `crates/claude-code-rs/src/daemon/**`、`crates/cc-config/src/paths.rs`、必要的 CLI / docs / tests 中接线。`crates/gateway` 不能反向依赖 `claude-code-rs` 内部模块；真实 `QueryEngine` / `SdkMessage` / daemon event 的映射放在 `claude-code-rs` 接线层。

目标：在 cc-rust 中建立一个可恢复、可审计、可安全暴露的远程控制网关。它不复制 Hermes 的 Python gateway，也不直接把当前 daemon HTTP API 当公网 API 使用；它吸收 Hermes 的控制面方法，把外部输入归一化为稳定的 source/session/run 模型，再通过现有 daemon worker command/event 和 `QueryEngine` 执行。

## 1. 结论

推荐路线是“薄 remote-control gateway + daemon supervisor/worker 执行核心”：

```text
HTTP / webhook / future WS / remote channel input
  -> crates/gateway adapter
  -> RemoteSource + RunRequest
  -> GatewayRunner
  -> daemon command/event protocol
  -> assistant-session worker / QueryEngine
  -> RunEvent fan-out + delivery router
```

不建议把远程控制逻辑继续塞进 `daemon::routes`。当前 daemon 已有 HTTP/SSE、control token、worker command/event 基座，但职责仍偏本地控制面；远程控制需要额外处理 source identity、run_id、忙碌策略、webhook 安全、事件回放、结果投递和重启恢复。

## 2. 当前基线

### 2.1 已有能力

- CLI 已有 `--headless`、`--daemon`、`--daemon-worker`、`--web` 等运行面，见 `crates/claude-code-rs/src/cli.rs:78`、`:86`、`:91`、`:139`。
- 主入口已经按 daemon / web / headless / TUI 分流，daemon 需要 `FEATURE_KAIROS=1`，见 `crates/claude-code-rs/src/main.rs:880`、`:956`。
- daemon HTTP server 默认绑定 `127.0.0.1:{port}`，并挂载 `/api/*`、`/webhook/*`、`/events` 和 `/health`，见 `crates/claude-code-rs/src/daemon/server.rs:20`-`:30`。
- daemon API 已有 `submit`、`abort`、`permission`、`status`、`attach`、`detach`、`history` 等接口，见 `crates/claude-code-rs/src/daemon/routes.rs:194`-`:204`。
- mutating daemon endpoint 已要求本地 control token，支持 `x-cc-rust-daemon-token` 和 `Authorization: Bearer`，见 `crates/claude-code-rs/src/daemon/routes.rs:207`-`:235`。
- worker command/event 协议已经有 `submit`、`abort`、`permission_response`、`ask_user_response`、`shutdown`、`reload_config`，并带 `idempotency_key`，见 `crates/claude-code-rs/src/daemon/protocol.rs:23`-`:71`、`:109`。
- `/events` SSE 支持 `last_event_id` 和 worker event log replay，见 `crates/claude-code-rs/src/daemon/sse.rs:25`-`:85`。
- `ChannelManager` 已有外部消息 allowlist 和 `ChannelEvent`，但只覆盖 MCP/webhook 到 XML 包装的轻量路径，见 `crates/claude-code-rs/src/daemon/channels.rs:18`-`:61`。
- 所有运行时数据已有 `CC_RUST_HOME` / `~/.cc-rust` 路径隔离基础，见 `crates/cc-config/src/paths.rs:20`-`:55`、`:80`。
- TaskTools 已有 remote task metadata、recoverable 和 remote review timeout 基础，见 `crates/claude-code-rs/src/tools/tasks/store.rs:32`-`:34`、`:790`-`:819`。

### 2.2 明确缺口

- `docs/IMPLEMENTATION_GAPS.md:52` 把远程控制、多端集成、`bridge/`、`remote/` 标为未完成方向。
- `docs/plan/daemon-usability-plan.md:424`-`:435` 记录已完成 remote-control 前置安全边界，但真正 bridge worker 注册、远程 submit/abort/permission 映射和结果回传仍未完成。
- 当前 `/api/submit` 仍在 HTTP supervisor 路径中直接调用 `QueryEngine::submit_message`，同时额外投递 command；真实执行所有权尚未完全迁入 assistant worker，见 `crates/claude-code-rs/src/daemon/routes.rs:251`-`:329`。
- `/api/permission` 目前只投递 command，尚未闭环到运行中的 permission callback，见 `crates/claude-code-rs/src/daemon/routes.rs:447`-`:479`。
- `/api/resize` 是 no-op，`/api/history` 返回空 history 加 SSE/daemon events，见 `crates/claude-code-rs/src/daemon/routes.rs:545`-`:563`。
- daemon 和 web server 当前都使用 permissive CORS，虽然默认 loopback，但不能直接升级为远程控制公网入口，见 `crates/claude-code-rs/src/daemon/server.rs:25` 和 `crates/claude-code-rs/src/web/mod.rs:42`。
- `QueryEngine` 有 abort/reset/submit/session 切换能力，但未看到 mid-turn `steer` 语义；`steer` 应作为协议保留项或后续增强，不应在第一阶段伪实现。

## 3. 设计原则

1. 执行核心不分叉：远程控制最终走 daemon worker command/event 与 `QueryEngine`，不新增第二套模型执行引擎。
2. 传输入口可插拔：HTTP、webhook、未来 WebSocket、Telegram/Lark 等都只是 gateway adapter，不能各自实现 session、busy、approval 策略。
3. source identity 是一等对象：不能用 TCP 连接或 bearer token 直接当 session key。
4. run 是外部 API 的最小资源：外部客户端看到 `run_id`、状态、事件、停止、审批，不需要理解内部 `SdkMessage` 全量细节。
5. 安全默认关闭：loopback 可低摩擦；任何非 loopback、webhook 或浏览器入口必须显式 token/secret/origin 策略。
6. 忙碌语义必须命名：`queue`、`interrupt`、`reject` 先落地，`steer` 仅在底层支持后启用。
7. 事件可恢复：SSE ring buffer 只是热路径，持久化 run event 才是恢复和审计依据。
8. 所有持久化继续写入 `~/.cc-rust`，不读写上游 Claude/Codex 路径。

## 4. 目标架构

### 4.1 新增模块

建议新增独立 workspace crate。当前 `Cargo.toml` 使用 `members = ["crates/*"]`，因此目录放在 `crates/gateway` 后会自然成为 workspace member；主程序 `claude-code-rs` 只负责把 gateway router、worker bridge 和配置接入 daemon。

依赖方向必须保持单向：

```text
claude-code-rs
  -> gateway
  -> cc-config / cc-types / cc-utils / cc-observability
```

`gateway` 不引用 `crate::engine`、`crate::daemon`、`crate::ipc` 或 `SdkMessage`。这些运行时细节由 `claude-code-rs/src/daemon/gateway_bridge.rs` 或等价接线模块转换成 `gateway::RunEvent` / `gateway::GatewayCommand`。

```text
crates/gateway/
├── Cargo.toml
├── src/
│   ├── lib.rs
│   ├── source.rs
│   ├── session_key.rs
│   ├── run.rs
│   ├── runner.rs
│   ├── policy.rs
│   ├── api.rs
│   ├── events.rs
│   ├── delivery.rs
│   ├── webhook.rs
│   ├── auth.rs
│   ├── config.rs
│   ├── store.rs
│   └── adapters/
│       ├── mod.rs
│       ├── telegram.rs
│       └── lark.rs
└── tests/
    ├── api.rs
    ├── adapters.rs
    └── store.rs
```

模块职责：

- `source.rs`：定义 `RemoteSource`，保留 transport、tenant、workspace、client、user、thread、message id、origin metadata。
- `session_key.rs`：从 `RemoteSource` 和 policy 生成确定性 session key。
- `run.rs`：定义 `RunId`、`RunStatus`、`RunRequest`、`RunPolicy`、`RunEvent`、`ApprovalRequest`。
- `runner.rs`：gateway 策略边界，处理 auth 后的 run 创建、busy policy、command 投递、event fan-out、恢复。
- `policy.rs`：queue/interrupt/reject/steer support matrix、并发限制、payload 限制、工具/权限策略。
- `api.rs`：HTTP API router，不直接持有业务逻辑。
- `events.rs`：定义 gateway-neutral `RunEvent`、event replay 和序列化规则；不直接依赖 `SdkMessage`。
- `delivery.rs`：结果投递路由，支持 origin、local、callback、channel 等目标。
- `webhook.rs`：声明式 webhook route、HMAC、rate limit、idempotency、event filter、prompt rendering。
- `auth.rs`：remote token、loopback policy、origin policy、非 loopback 强制认证。
- `config.rs`：配置解析和诊断。
- `store.rs`：`~/.cc-rust/gateway/**` 的 run/session/source/index 持久化。
- `adapters/mod.rs`：定义 Hermes-style adapter trait；adapter 只负责连接、鉴权、收发、归一化事件，不负责 session/run/busy/approval 策略。
- `adapters/telegram.rs`：Telegram 连接 adapter。第一版只要求配置 token、`getMe` 健康检查、可选 `getUpdates` 探测和 `sendMessage` 测试发送。
- `adapters/lark.rs`：Lark/Feishu 连接 adapter。第一版只要求 app credentials / webhook 配置校验、token 获取或 webhook 连通性检测、状态诊断；是否启用事件订阅或长连接由后续阶段决定。

### 4.2 新增路径

在 `crates/cc-config/src/paths.rs` 中增加：

```rust
pub fn gateway_dir() -> PathBuf {
    data_root().join("gateway")
}

pub fn gateway_runs_dir() -> PathBuf {
    gateway_dir().join("runs")
}

pub fn gateway_adapters_dir() -> PathBuf {
    gateway_dir().join("adapters")
}

pub fn gateway_webhooks_dir() -> PathBuf {
    gateway_dir().join("webhooks")
}
```

目标布局：

```text
~/.cc-rust/gateway/
├── config.json
├── tokens.json
├── adapters/
│   ├── telegram.json
│   └── lark.json
├── sessions/
│   └── <session-key>.json
├── runs/
│   └── <run-id>/
│       ├── meta.json
│       ├── events.ndjson
│       └── delivery.ndjson
├── idempotency/
│   └── <hash>.json
└── webhooks/
    └── <route-id>.json
```

## 5. 外部 API 草案

第一阶段优先 HTTP + SSE，不做公网 WebSocket。

```text
GET  /remote-control/v1/capabilities
POST /remote-control/v1/runs
GET  /remote-control/v1/runs/{run_id}
GET  /remote-control/v1/runs/{run_id}/events
POST /remote-control/v1/runs/{run_id}/stop
POST /remote-control/v1/runs/{run_id}/approval
POST /remote-control/v1/runs/{run_id}/ask-user
POST /remote-control/v1/webhooks/{route_id}
GET  /remote-control/v1/adapters
POST /remote-control/v1/adapters/{provider}/connect
POST /remote-control/v1/adapters/{provider}/test-message
```

### 5.1 `GET /capabilities`

返回：

- server version；
- enabled transports；
- auth mode；
- loopback/non-loopback 状态；
- busy policies；
- 是否支持 `steer`；
- 最大 payload、最大并发 run、事件 replay 窗口；
- endpoint paths。

### 5.2 `POST /runs`

输入：

```json
{
  "prompt": "string",
  "source": {
    "transport": "http",
    "tenant": "local",
    "workspace": "F:/AIclassmanager/cc/rust",
    "client_id": "dashboard-1",
    "user_id": "86186",
    "thread_id": "default"
  },
  "policy": {
    "busy": "queue",
    "permission_mode": "ask",
    "delivery": ["origin", "local"]
  },
  "idempotency_key": "optional-provider-delivery-id"
}
```

输出立即返回：

```json
{
  "run_id": "run_...",
  "session_key": "remote:http:local:...:default",
  "status": "queued",
  "events_url": "/remote-control/v1/runs/run_.../events"
}
```

### 5.3 busy policy

- `reject`：assistant busy 时返回 `409 busy`，不创建 pending run。
- `queue`：创建 `queued` run，写入 durable queue，当前 run 完成后自动启动。
- `interrupt`：先投递 abort command，再以同一 source/session 创建新 run。
- `steer`：第一版返回 `501 unsupported`，直到 `QueryEngine` 有明确 mid-turn steering 能力。

### 5.4 approval / ask-user

远程 approval 和 ask-user 必须走 run-scoped response：

- `POST /runs/{run_id}/approval` 投递 `DaemonCommandKind::PermissionResponse`。
- `POST /runs/{run_id}/ask-user` 投递 `DaemonCommandKind::AskUserResponse`。
- response 必须校验 `run_id`、`question_id` 或 `tool_use_id` 是否仍 pending；过期时返回 `409 stale_response`。

### 5.5 adapter 连接 API

Telegram/Lark 第一版只要求连接能力，不要求完整远程会话闭环：

- `GET /adapters` 返回 provider、配置状态、auth 状态、最近连接错误、最近健康检查时间。
- `POST /adapters/telegram/connect` 执行 Telegram `getMe`，确认 token 有效；如果配置允许，执行一次 bounded `getUpdates` 探测。
- `POST /adapters/telegram/test-message` 向 allowlist 中的测试 chat 发送一条短消息，证明 outbound 可用。
- `POST /adapters/lark/connect` 校验 Lark app/webhook 配置，获取 app token 或验证 webhook endpoint；如果缺少必要凭据，返回精确 blocked reason。
- `POST /adapters/lark/test-message` 只在配置了 outbound webhook 或可发送应用消息时启用。

这些 endpoint 仍走 gateway auth / source redaction / durable status，不把 provider token 暴露给 caller。

## 6. 与 `crates/claude-code-rs/src/ipc` 的关系

`ipc` 和 `gateway` 有概念重叠，但职责不应重叠。

`crates/claude-code-rs/src/ipc` 是本地 headless UI bridge：

- 传输是 stdin/stdout JSONL。
- 宿主是当前 `claude-code-rs --headless` 进程。
- 主要服务本地 UI/TUI 前端，把 `FrontendMessage::SubmitPrompt`、`AbortQuery`、`PermissionResponse`、`SlashCommand`、`Resize` 等映射到 `QueryEngine`。
- 它维护的是本进程交互状态，例如 pending permission、pending AskUserQuestion、前端文件搜索和子系统命令。

`crates/gateway` 是长期运行的远程控制控制面：

- 传输是 HTTP/SSE、webhook、Telegram/Lark adapter，未来可加 WebSocket。
- 宿主是 daemon supervisor/worker。
- 它维护 `RemoteSource`、session key、run_id、busy policy、auth、idempotency、delivery、恢复和审计。
- 它不应该直接复用 stdin/stdout headless loop，也不应该把 Telegram/Lark 事件伪装成 `FrontendMessage`。

实际关系：

- submit / abort / permission / ask-user / stream event 是共享语义，不是共享传输。
- 第一版 gateway 应该通过 daemon command/event 协议接入执行核心，而不是调用 `ipc::headless::run_headless`。
- 如果未来发现 `ipc`、daemon、gateway 都需要相同的协议类型，应把共享类型抽到 `cc-types` 或独立 protocol crate；不要让 gateway 依赖 `ipc` 的 UI JSONL 枚举。
- `ipc` 可以继续作为本地前端协议；gateway 是外部控制面。二者并行存在，边界是 transport/session ownership/recovery/auth。

## 7. 阶段拆解

### Phase 0：冻结现状和边界

目标：先把现有 daemon/headless/web 的职责写清楚，避免远程控制计划与已有计划冲突。

改动：

- 更新或新增 `docs/reference/remote-control-current-state.md`。
- 交叉引用：
  - `docs/plan/daemon-usability-plan.md`
  - `docs/plan/remote-channel-phase1-telegram-lark-plan-2026-05-08.md`
  - `docs/reference/DAEMON_OPERATIONS.md`
  - 本计划
- 在 `docs/IMPLEMENTATION_GAPS.md` 中把 “gateway 控制面” 与 “Telegram/Lark adapter 连通性” 区分开。

验收：

- 文档明确：`crates/gateway` 是控制面；Telegram/Lark 是首批 adapter；daemon 是执行宿主；`ipc` 是本地 headless UI bridge。
- 没有声称现有 `/api/*` 已可作为安全远程公网 API。

验证：

```bash
rg -n "remote-control|remote channel|daemon" docs/IMPLEMENTATION_GAPS.md docs/plan docs/reference
```

### Phase 1：RemoteSource / session key / run schema

目标：实现 Hermes 风格的 canonical source 和确定性 session identity。

改动：

- 新增 `crates/gateway/src/source.rs`：
  - `RemoteTransport`：`Http`、`Webhook`、`Mcp`、`Channel`、`WebSocketReserved`。
  - `RemoteSource`：`transport`、`tenant`、`workspace`、`client_id`、`user_id`、`thread_id`、`message_id`、`metadata`。
  - source metadata redaction 方法，避免敏感字段进入 prompt/log。
- 新增 `crates/gateway/src/session_key.rs`：
  - `build_session_key(source, policy) -> RemoteSessionKey`。
  - 稳定字符串格式，例如 `remote:<transport>:<tenant>:<workspace-hash>:<client-or-user>:<thread>`。
  - 禁止空 workspace/client/thread 直接落盘；必须填默认值并可追踪。
- 新增 `crates/gateway/src/run.rs`：
  - `RunStatus`：`queued`、`running`、`waiting_approval`、`waiting_user`、`completed`、`failed`、`cancelled`、`interrupted`、`recoverable`。
  - `RunPolicy`：busy policy、permission mode、delivery targets、timeout、tool restrictions。
  - `RunEvent`：`created`、`started`、`stream_delta`、`tool_use`、`permission_request`、`approval_response`、`ask_user_request`、`completed`、`failed`、`delivery_*`。
- 新增 `crates/gateway/src/store.rs`：
  - `RemoteRunStore` 负责 `meta.json`、`events.ndjson`、idempotency index。
  - 使用 atomic write；NDJSON append 失败必须返回可见错误。
- 在 `crates/cc-config/src/paths.rs` 增加 gateway 路径函数和测试。

验收：

- 相同 `RemoteSource + policy` 生成相同 session key。
- 不同 user/thread/workspace 不会误共享 session key。
- run metadata 可 roundtrip。
- idempotency key 重复提交返回同一个 run，不重复写危险 command。

验证：

```bash
cargo test -p cc-config partition_functions_all_root_under_data_root --lib
cargo test -p gateway source
cargo test -p gateway run
cargo test -p gateway store
```

### Phase 2：Telegram/Lark adapter 连通性 MVP

目标：先证明 gateway 可以按 Hermes 的 adapter 方法连接 Telegram/Lark。此阶段只做连接、健康检查、状态诊断和可选测试发送，不做完整对话 run。

改动：

- 新增 `crates/gateway/src/adapters/mod.rs`：
  - 定义 `GatewayAdapter` trait：
    - `provider_id()`
    - `connect()`
    - `disconnect()`
    - `health_check()`
    - `poll_once()` 或 `receive_once()`
    - `send_text()`
    - `status()`
  - 定义 provider-neutral `GatewayInboundEvent`、`GatewayOutboundReceipt`、`GatewayTarget`。
  - adapter 返回 canonical event；不得直接调用 `QueryEngine`、daemon command、run queue。
- 新增 `crates/gateway/src/adapters/telegram.rs`：
  - 从 `~/.cc-rust/gateway/adapters/telegram.json` 或环境变量读取 bot token。
  - `connect()` 调用 `getMe`。
  - `health_check()` 返回 bot username、allowed update mode、最近错误。
  - 可选 `poll_once()` 调用 `getUpdates`，但只归一化事件并写 adapter status，不自动启动 run。
  - `send_text()` 只允许发送到 allowlist chat。
- 新增 `crates/gateway/src/adapters/lark.rs`：
  - 支持两种明确模式：`outbound_webhook` 和 `app_credentials`。
  - `connect()` 对 outbound webhook 做配置校验；对 app credentials 获取 tenant/app token。
  - 如果用户没有配置事件订阅/长连接，不报“已支持 inbound”，而是返回 `connected_outbound_only`。
  - `send_text()` 仅在当前模式具备发送能力时启用。
- 新增 `GatewayAdapterRegistry`：
  - 注册 Telegram/Lark；
  - 持久化 adapter status；
  - 提供 `/adapters` API 查询。

验收：

- Telegram token 有效时，`connect` 返回 bot 信息并写 status。
- Telegram token 错误时，错误可见且不写入 token 原文。
- Telegram 测试消息只允许发送到 allowlist chat。
- Lark 缺少必要 app credentials 或 webhook URL 时，返回精确 blocked reason。
- Lark outbound webhook 可配置时，测试消息能发出或返回 provider 错误。
- adapter 事件只进入 gateway event/store，不绕过 runner 直接触发模型。

验证：

```bash
cargo test -p gateway adapters
cargo test -p gateway adapters::telegram
cargo test -p gateway adapters::lark
```

### Phase 3：GatewayRunner 与 daemon command/event 桥接

目标：把 run lifecycle 接到现有 daemon command/event 协议，但不直接在 HTTP route 里执行模型。

改动：

- 新增 `crates/gateway/src/runner.rs`：
  - `create_run(request) -> RunHandle`
  - `start_next_for_session(session_key)`
  - `stop_run(run_id)`
  - `resolve_permission(run_id, tool_use_id, decision)`
  - `resolve_question(run_id, question_id, text)`
- 新增 `crates/gateway/src/policy.rs`：
  - busy 状态读取来自 daemon/runner 的 run index，不只读 `is_query_running`。
  - `queue` 只对同一 session 串行；不同 session 可以按配置并发。
  - `interrupt` 必须先发 abort event，再启动新 run。
  - `steer` 第一版显式 unsupported。
- 扩展 `daemon::protocol::DaemonCommandKind`：
  - 保留现有 kind。
  - payload 增加 `run_id`、`session_key`、`remote_source`、`delivery_targets`。
  - 不破坏现有 CLI `daemon submit`。
- 新增 `crates/claude-code-rs/src/daemon/gateway_bridge.rs` 或等价模块：
  - 把 `SdkMessage` 映射成 `gateway::RunEvent`。
  - 把 daemon worker event 映射成 gateway run lifecycle event。
  - 把 gateway command 投递到现有 daemon command/event 文件协议。
  - 保持所有 `QueryEngine` / callback / abort 细节留在 `claude-code-rs`，不进入 `crates/gateway`。
- 调整 assistant worker：
  - 当前 worker 对 `submit` 只 `command_deferred`，后续应真正拥有 `QueryEngine::submit_message`。
  - 该迁移可与 `docs/plan/daemon-usability-plan.md` 的 Phase 4 合并执行。
- `runner` 监听 daemon event / SdkMessage 映射后写 run events。

验收：

- `POST /runs` 创建 run 时，command 文件中能看到 `run_id/session_key/source`。
- worker ack 后 run 从 `queued` 进入 `running` 或 `recoverable`。
- abort 后 run 进入 `interrupted` 或 `cancelled`，并写终态事件。
- permission response 只影响对应 run 的 pending permission。

验证：

```bash
cargo test -p claude-code-rs daemon::protocol
cargo test -p claude-code-rs daemon::supervisor
cargo test -p gateway runner
cargo test -p gateway policy
```

### Phase 4：HTTP/SSE gateway API

目标：提供外部 UI 可用的 run resource，而不是暴露内部 daemon `/api/*`。

改动：

- 新增 `crates/gateway/src/api.rs`，返回一个 `Router<GatewayState>`。
- 在 daemon server 中挂载：
  - `/remote-control/v1/capabilities`
  - `/remote-control/v1/runs`
  - `/remote-control/v1/runs/{run_id}`
  - `/remote-control/v1/runs/{run_id}/events`
  - `/remote-control/v1/runs/{run_id}/stop`
  - `/remote-control/v1/runs/{run_id}/approval`
  - `/remote-control/v1/runs/{run_id}/ask-user`
- SSE 事件源从 run store replay，再订阅热事件。
- `capabilities` 必须报告 `steer=false`，直到底层支持。
- API 不直接复用 daemon control token 文件；remote token 可以从 daemon token 派生，但需要独立 auth mode 和配置。

验收：

- 无 token 调用 mutating remote-control endpoint 返回 `401`。
- `GET /capabilities` 在未认证时只返回非敏感信息。
- `POST /runs` 返回 `run_id`，`GET /runs/{run_id}` 可轮询状态。
- `GET /runs/{run_id}/events?last_event_id=...` 能 replay durable events。
- `/api/*` 保持兼容，不因新增 `/remote-control/*` 改变现有行为。

验证：

```bash
cargo test -p gateway api
cargo test -p claude-code-rs daemon::routes
cargo test -p claude-code-rs daemon::sse
```

手工 smoke：

```bash
$env:FEATURE_KAIROS="1"
$env:CC_RUST_HOME="$env:TEMP\\cc-rust-remote-control-smoke"
cargo run -p claude-code-rs -- daemon start --port 21990
$TOKEN = cargo run -p claude-code-rs -- daemon token
curl.exe -H "Authorization: Bearer $TOKEN" http://127.0.0.1:21990/remote-control/v1/capabilities
curl.exe -H "Authorization: Bearer $TOKEN" -H "content-type: application/json" -d "{\"prompt\":\"ping\",\"source\":{\"transport\":\"http\",\"tenant\":\"local\",\"workspace\":\"F:/AIclassmanager/cc/rust\",\"client_id\":\"smoke\",\"thread_id\":\"default\"},\"policy\":{\"busy\":\"reject\"}}" http://127.0.0.1:21990/remote-control/v1/runs
cargo run -p claude-code-rs -- daemon stop
```

### Phase 5：安全硬化

目标：把 Hermes 的安全默认值落成代码约束，而不是部署建议。

改动：

- `auth.rs`：
  - loopback-only 默认允许使用本地 daemon token。
  - 非 loopback bind 必须配置 remote token 或 signed token，否则启动失败。
  - token 永不写入日志、SSE event、run metadata。
- CORS / Origin：
  - 替换 remote-control route 上的 permissive CORS。
  - browser client 必须匹配 allowlist origin。
  - Webhook route 不使用 browser CORS。
- payload 限制：
  - run prompt、metadata、webhook body 设置最大字节数。
  - 超限返回 `413 payload_too_large`。
- rate limit：
  - 按 token/source/route 维度限流。
  - 限流事件写入 audit，但不写敏感 body。
- webhook HMAC：
  - 每个 route 独立 secret。
  - secret 缺失时 route 不启用。
  - GitHub/Slack 现有签名函数可复用，但要收敛到 declarative route。
- idempotency：
  - `Idempotency-Key` header 和 provider delivery id 都进入 idempotency index。
- redaction：
  - source metadata、headers、webhook payload 进入 prompt/log 前统一 redaction。

验收：

- 监听 `0.0.0.0` 且无 remote token 时启动失败。
- 错误 token、跨 origin、超大 payload、重复 idempotency 都有稳定错误码。
- webhook secret 未配置时 route 返回 disabled，而不是弱认证接收。
- 日志和 run meta 中不出现 Authorization、secret、raw token。

验证：

```bash
cargo test -p gateway auth
cargo test -p gateway webhook
cargo test -p claude-code-rs daemon::webhook
rg -n "Authorization|Bearer|token" $env:CC_RUST_HOME\\gateway
```

### Phase 6：Webhook 声明式入口

目标：把现有 `/webhook/github`、`/webhook/slack`、`/webhook/generic` 从 hardcoded stub 推进为声明式 remote input。

改动：

- `crates/gateway/src/webhook.rs` 定义：
  - route id；
  - provider；
  - accepted events；
  - secret env 或 secret file ref；
  - prompt template；
  - default source fields；
  - busy policy；
  - delivery target；
  - optional `deliver_only`；
  - idempotency key extraction；
  - body limit。
- `daemon::routes::webhook_routes()` 保留兼容路径，但内部转发到 `gateway::webhook`。
- GitHub PR activity 现有 team mailbox 路由保持兼容；不要破坏 `tools::pr_activity`。
- 增加 route reload：配置变更后不需要重启 daemon，至少支持下一次请求读取新配置。

验收：

- GitHub webhook 带有效 HMAC 能创建 run 或 deliver-only event。
- 重复 `x-github-delivery` 不重复创建 run。
- event filter 不匹配时返回 `ignored`，不写 prompt。
- `deliver_only` route 只投递 delivery event，不启动模型。

验证：

```bash
cargo test -p claude-code-rs daemon::webhook
cargo test -p gateway webhook
cargo test -p claude-code-rs tools::pr_activity
```

### Phase 7：DeliveryRouter

目标：分离“从哪里触发 run”和“结果送到哪里”。

改动：

- 新增 `delivery.rs`：
  - `DeliveryTarget::Origin`
  - `DeliveryTarget::Local`
  - `DeliveryTarget::Callback { url }`
  - `DeliveryTarget::Channel { provider, target, thread }`
  - `DeliveryTarget::SseSubscriber { client_id }`
- local delivery 写入 run `delivery.ndjson`。
- callback delivery 只允许 HTTPS 或 loopback HTTP；禁止跳转到私网/metadata 地址。
- channel delivery 第一版对接 gateway adapter registry。Telegram/Lark 仅保证连接状态和测试发送；完整 inbound conversational control 后续再启用。
- delivery failures 不改变 run completion，但必须写 `delivery_failed` event。

验收：

- run 从 webhook 触发时，结果可以投递到 local + origin/callback。
- callback 失败有重试上限和最终失败事件。
- 未配置 channel provider 时返回可见 `delivery_unsupported`。

验证：

```bash
cargo test -p gateway delivery
cargo test -p claude-code-rs daemon::channels
```

### Phase 8：恢复、队列和长期运行

目标：让 remote-control 可以跨 daemon 重启恢复状态。

改动：

- `store.rs` 启动恢复：
  - `running` -> `recoverable` 或 `failed`，取决于 worker event 是否有终态。
  - `waiting_approval` / `waiting_user` 保留 pending，但必须带过期时间。
  - `queued` run 重新入队。
- stale lock 自愈：
  - session active lock 带 owner pid / heartbeat。
  - stale 后允许 takeover，并写 `session_lock_recovered` event。
- bounded queue：
  - 每 session 最大 queued run 数；
  - 全局最大 running run 数；
  - 超限返回 `429 queue_full`。
- event replay：
  - `events.ndjson` 是完整 replay；
  - SSE buffer 只作为热连接优化。
- 30 分钟 daemon soak：
  - 多 run；
  - abort；
  - permission timeout；
  - daemon restart；
  - event replay。

验收：

- daemon 重启后 pending run 不会静默丢失。
- 重启后重复 webhook delivery 不重复执行。
- stale lock 会恢复，并留下审计事件。
- queue 满时 fail closed。

验证：

```bash
cargo test -p gateway store
cargo test -p gateway runner
cargo test -p claude-code-rs daemon::process_state
cargo test -p claude-code-rs daemon::supervisor
```

新增 e2e：

```text
crates/claude-code-rs/tests/e2e_gateway.rs
```

覆盖：

- start daemon -> create run -> read events -> stop；
- create queued run -> restart daemon -> run recoverable/queued 可见；
- bad token / bad origin / duplicate idempotency；
- stop run 后不再产生 assistant stream event。

### Phase 9：文档和发布门槛

目标：让能力边界、命令、API、风险和验证都可维护。

改动：

- 新增 `docs/reference/REMOTE_CONTROL_GATEWAY.md`：
  - API endpoint；
  - auth；
  - source/session key；
  - busy policy；
  - webhook route；
  - delivery targets；
  - recovery semantics。
- 更新 `docs/CLI_REFERENCE.md`：
  - remote-control feature gate；
  - daemon token；
  - smoke commands。
- 更新 `docs/FINAL_RELEASE_PLAN.md`：
  - remote-control gateway 发布门槛；
  - 明确非目标：公网托管、多租户 SaaS、完整 Telegram/Lark 远程会话 parity、WebSocket parity。
- 更新 `docs/KNOWN_ISSUES.md`：
  - 若 `steer` 仍 unsupported，记录为非阻塞限制。

验收：

- 文档里的每个 endpoint 都有示例请求和错误码。
- 发布计划明确 remote-control 的测试门槛。
- 文档明确 Telegram/Lark 当前只承诺连接、健康检查、状态诊断和测试发送；完整远程会话控制是后续增强。

验证：

```bash
rg -n "gateway|RemoteSource|run_id|busy|webhook|delivery|Telegram|Lark|ipc" docs crates/gateway crates/claude-code-rs/src/daemon
```

## 8. 测试矩阵

### 单元测试

- `gateway::source`：source redaction、session key 分离。
- `gateway::run`：状态机合法转移。
- `gateway::store`：meta/events/idempotency 持久化。
- `gateway::policy`：queue/interrupt/reject/unsupported steer。
- `gateway::auth`：loopback、non-loopback、bad token、origin。
- `gateway::webhook`：HMAC、rate limit、idempotency、event filter。
- `gateway::delivery`：target parse、callback allowlist、失败事件。
- `gateway::adapters`：Telegram/Lark 连接、健康检查、状态诊断、token redaction。

### 集成测试

- `daemon::protocol`：新增 payload 字段向后兼容。
- `daemon::routes`：旧 `/api/*` 行为不变。
- `daemon::sse`：旧 SSE replay 不变。
- `daemon::supervisor`：worker restart 后 remote run 恢复。
- `ipc`：headless protocol 不被 gateway 修改影响。

### E2E / smoke

- loopback daemon + gateway/remote-control token + run create + event stream。
- Telegram/Lark adapter connect endpoint 返回稳定状态。
- duplicate idempotency 不重复执行。
- busy reject 返回 409。
- busy queue 产生 ordered events。
- interrupt 会终止当前 run 并启动新 run。
- permission request -> approval response -> run resumes。
- webhook HMAC valid/invalid。
- daemon restart 后 `GET /runs/{run_id}` 可见 recoverable/terminal 状态。

## 9. 风险与缓解

| 风险 | 影响 | 缓解 |
| --- | --- | --- |
| daemon `/api/submit` 真实执行所有权仍在 HTTP supervisor | gateway run 可能绕过 worker 生命周期 | Phase 3 与 daemon-usability Phase 4 合并，先迁移 assistant worker ownership |
| Telegram/Lark 从“连接证明”膨胀成完整聊天控制 | 范围膨胀、接口不稳 | Phase 2 只做 adapter connect/health/test-message；inbound conversational control 后续单独开阶段 |
| CORS permissive 被误用于非 loopback | CSRF / 浏览器跨站风险 | remote-control route 独立 CORS/origin；非 loopback 无 token 启动失败 |
| permission/ask-user 没有 run-scoped pending map | 错误审批可能作用到其他 run | pending response 必须校验 run_id + tool_use_id/question_id |
| `steer` 被客户端当成已支持 | 行为不一致 | capabilities 明确 `steer=false`，endpoint 返回 501 |
| run event 与 daemon event 双写漂移 | 恢复和排障困难 | 定义唯一映射层 `events.rs`，测试覆盖 daemon event -> run event |
| 敏感 source metadata 入日志或 prompt | token/PII 泄漏 | redaction 在 `RemoteSource` 序列化前执行，日志只写 hash/route id |
| 队列无限增长 | 长期 daemon 内存/磁盘膨胀 | 每 session/global queue 上限，过期清理和可见 `queue_full` |

## 10. 非目标

第一版不做：

- 公网多租户 remote-control 服务。
- 完整 WebSocket 双向协议。
- Telegram/Lark/Feishu 完整远程会话 provider 生产实现；第一版只做连接、健康检查、状态诊断和测试发送。
- remote desktop / GUI 控制。
- mid-turn `steer` 真实注入。
- 替换 headless JSONL 协议。
- 替换现有 Web UI。

## 11. 推荐实施顺序

1. Phase 0 文档边界，避免计划冲突。
2. Phase 1 schema/store/path，先锁数据模型。
3. Phase 2 Telegram/Lark adapter 连通性，证明 Hermes-style adapter 边界。
4. Phase 3 runner + daemon worker ownership，解决执行边界。
5. Phase 4 HTTP/SSE run API，给外部 UI 最小可用控制面。
6. Phase 5 安全硬化，必须在任何非 loopback 使用前完成。
7. Phase 6 webhook declarative routes。
8. Phase 7 delivery router。
9. Phase 8 恢复和长期运行。
10. Phase 9 docs/release gate。

最小可交付切片建议到 Phase 5 为止：

- loopback remote-control API；
- token auth；
- run create/status/events/stop/approval；
- Telegram/Lark connect/status/test-message；
- queue/reject/interrupt；
- durable run events；
- 明确 `steer=false`；
- 安全测试通过。

## 12. 完成定义

remote-control gateway 进入可用状态必须满足：

- 所有 gateway 状态写入 `~/.cc-rust/gateway` 或既有 `~/.cc-rust/daemon`，不写上游路径。
- 外部客户端只依赖 `run_id` 和 `/remote-control/v1/**`，不需要直接操作 daemon command 文件。
- Telegram/Lark adapter 可连接并输出状态，但不会绕过 gateway runner 直接触发模型。
- daemon 重启后 run 状态可解释：completed、failed、cancelled、recoverable 或 queued。
- 无 token / bad token / bad origin / bad HMAC / duplicate idempotency / queue full 都有稳定响应。
- permission 和 ask-user response 不会串到其他 run。
- `cargo test -p gateway` 通过。
- `cargo test -p claude-code-rs daemon::protocol daemon::routes daemon::sse daemon::supervisor` 通过。
- 至少一个 loopback smoke 证明 start -> create run -> events -> stop 闭环。
