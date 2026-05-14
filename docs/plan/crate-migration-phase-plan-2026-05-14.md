# Workspace Crate Migration Phase Plan

> 日期：2026-05-14
> 范围：把 `crates/claude-code-rs/src/**` 中可复用实现迁入 workspace
> `cc-*` crates，并把 `claude-code-rs` 收敛为 thin binary。
>
> 依据：
>
> - [`docs/reference/CRATE_MIGRATION_GUIDE.md`](../reference/CRATE_MIGRATION_GUIDE.md)
> - [`docs/reference/CRATE_MIGRATION_TARGET_STATE.md`](../reference/CRATE_MIGRATION_TARGET_STATE.md)
> - [`docs/plan/workspace-decycle-plan-2026-05-14.md`](workspace-decycle-plan-2026-05-14.md)

本文是按顺序执行的完整 phase 计划。每个 phase 可以拆成多个小 PR，但不应跳过 phase
退出条件进入后续 phase。除非某一步明确要求行为调整，迁移 PR 默认只做 ownership /
dependency boundary 调整，不混入功能重写。

## 2026-05-14 执行快照

本轮已按 Phase 0-12 拆分执行并提交 implementation slices。代码层面完成了
contract / DTO 下沉、root engine 重复实现清理、`cc-daemon` protocol owner
补齐、`cc-mcp` loopback HTTP proxy 隔离、`cc-session` fork path 固定、
`cc-ui` messages 子集迁移、以及多处会互相污染的测试全局状态隔离。

已通过的 final gates：

```bash
cargo fmt --all --check
cargo check --workspace --all-targets --message-format short
cargo test --workspace
cargo build --workspace --release
```

本机仍有已知环境 warning：`npm` 未安装时 `claude-code-rs` build script 会跳过
web-ui dependency install。该 warning 不影响本轮 crate migration gate。

Phase 12 尚不能关闭为 Completed Full，因为 thin-binary / owner guard 仍未归零：

```text
path_attrs_cc_to_root 54
path_attrs_root_to_cc 7
cc_crates_root_imports 70
allow_unused_dead 474
codex_path_hits 12
```

剩余收口重点：

- Phase 10 / 11：`cc-ui` 仍通过 `#[path = "../../claude-code-rs/src/ui/..."]`
  读取 root UI source，root `ui/mod.rs` 也仍通过 `#[path]` 反向包含 `cc-ui`
  modules；需要继续把 app/tui/messages 及依赖的 UI leaf modules 迁入 `cc-ui`。
- Phase 5 / 8：`cc-engine`、`cc-ui` 等 runtime crates 仍有 root-style
  `crate::engine/tools/teams/mcp/browser/...` 边界引用；需要用目标 crate import
  或 adapter trait 替换。
- Phase 11：大量 `allow(dead_code|unused_imports|unused)` 仍来自迁移期兼容面；
  后续每个 owner 收口 slice 需要同步删除对应 allow。
- Phase 12：guard 归零前，本文档保持 active plan，不迁入 `docs/archive/`。

## 总目标

完成后：

- `claude-code-rs` 只保留 CLI、startup、runtime adapter wiring、binary-only
  entry glue、process lifecycle glue。
- `cc-*` library crates 拥有真实实现；library crate 不依赖 `claude-code-rs`
  root-private module。
- 没有跨 crate `#[path = "..."]` 迁移桥。
- IPC JSONL、session transcript、settings、credentials、daemon event、audit
  格式和 cc-rust path isolation 不发生非预期变化。
- workspace 级格式、check、test、release build 和 guard 命令通过。

## 全局执行规则

- 每个迁移 slice 先记录目标 owner、公共类型 owner、持久化/协议风险、shim 删除 guard。
- 优先移动 contract / DTO / adapter trait，再移动 runtime implementation。
- 不把 runtime 单例、文件写入、subprocess、HTTP server、terminal raw mode 放入
  `cc-types` 或 `cc-ipc-protocol`。
- 新 crate 需要 root 行为时，先加 adapter trait，由 root startup 安装实现。
- 每个 slice 至少运行目标 crate check；触碰 root glue 时再运行 root check。
- 发现 warning 时优先修真实 unused/dead code；本机 `npm` 缺失导致 web-ui build
  install skip 是已知环境 warning，可以记录但不作为迁移失败。
- 任何 intentional crop 必须写入 `IMPLEMENTATION_GAPS.md`，不能用历史
  `rust-lite` 作为理由。

## 当前基线

当前 workspace 已有目标 crate scaffold：

- Contracts / leaf / domain：`cc-types`、`cc-ipc-protocol`、`cc-models`、
  `cc-config`、`cc-auth`、`cc-skills`、`cc-observability`、`cc-api`、
  `cc-session`、`cc-compact`、`cc-permissions`、`cc-sandbox`。
- Runtime：`cc-engine`、`cc-query`、`cc-tools`、`cc-tasks`、`cc-teams`、
  `cc-daemon`、`cc-commands`、`cc-ipc`、`cc-ipc-client`、`cc-mcp`、
  `cc-browser`、`cc-lsp-service`、`cc-plugins`、`cc-services`、
  `cc-computer-use`。
- UI / binary glue：`cc-ui`、`claude-code-rs`。

当前仍需迁移或收敛的 root 目录包括：

- `commands/`、`tools/`、`daemon/`、`ipc/`、`teams/`、`engine/`、`ui/`
- `browser/`、`computer_use/`、`lsp_service/`、`mcp/`、`plugins/`、
  `services/`、`voice/`、`web/`
- loose glue：`plan_workflow.rs`、`worktree_hooks.rs`、`dashboard.rs`、
  `shutdown.rs`、`cli.rs`、`main.rs`

已知迁移桥和优先风险：

- `crates/claude-code-rs/src/engine/lifecycle/mod.rs` 通过 `#[path]` 指向
  `cc-engine`。
- `cc-ui` 仍通过大量 `#[path = "../../claude-code-rs/src/ui/..."]` 读取 root
  UI source。
- `crates/claude-code-rs/src/ui/mod.rs` 也通过 `#[path]` 指向 `cc-ui`
  modules。
- `commands/remote_cmd.rs` 通过 `#[path = "../daemon/gateway_client.rs"]`
  复用 daemon root file。
- `cc-engine` 当前仍依赖若干 runtime crates，拆环计划中已明确后续 cut。

## Phase 0：Baseline And Ownership Inventory

目标：冻结迁移基线，形成 owner matrix 和 guard matrix，避免后续 phase 只凭目录名移动。

执行内容：

- 对 root 每个一级模块记录目标 crate、保留在 root 的 binary-only glue、公共类型 owner。
- 对每个模块记录是否涉及 persistence、IPC/wire、session transcript、settings、
  credentials、daemon event、audit output、path isolation。
- 对当前 `#[path]` shim、root-private import、crate dependency edge 建立 guard。
- 记录每个目标 crate 的 focused test filter 和需要保留的 root e2e。
- 对现有 `docs/plan/workspace-decycle-plan-2026-05-14.md` 中 cut 状态做索引，
  作为 Phase 1 输入。

退出条件：

- 新增或更新 owner matrix 文档，至少覆盖 root 当前所有一级模块。
- 每个后续 phase 有明确 blocker、guard、验证命令。
- 当前 known shim 列表可由 `rg '#\[path = '` 重现。

最小验证：

```bash
rg '#\[path = ' crates/claude-code-rs/src crates/cc-* crates/gateway -g '*.rs'
rg 'crate::(engine|tools|commands|daemon|ipc|teams|plugins|mcp|lsp_service|browser|computer_use|voice)::' crates/cc-* crates/gateway -g '*.rs'
cargo check --workspace --all-targets --message-format short
```

## Phase 1：Cargo Graph Decycle And Adapter Foundations

目标：先切断会阻止迁移的 Cargo 包级环和 root-private runtime 回调，让后续移动文件时不再
需要反向依赖。

执行内容：

- 完成 `workspace-decycle-plan-2026-05-14.md` 的剩余 cuts：
  `cc-engine -> cc-ipc-client`、`engine <-> tools`、`browser <-> mcp`、
  `commands/ui/ipc`。
- 把 callback host、query-turn host、command dispatch、tool registry、
  agent tree、task store、webhook delivery、UI snapshot 需求改成 adapter trait
  或 function table。
- Adapter trait 放在最低可见 contract crate，返回 domain DTO，不返回 root-private struct。
- Root startup 负责安装 adapter；安装顺序必须早于任何 runtime 使用路径。
- 不把临时 adapter 写成全局静默 fallback；错误路径必须显式。

退出条件：

- `cc-engine/Cargo.toml` 不再依赖 `cc-ipc-client`。
- `cc-tools` 不主动查询 engine、teams、daemon、plugins、UI runtime。
- `cc-mcp` 与 `cc-browser` 之间没有双向 runtime ownership。
- `cc-commands`、`cc-ui`、`cc-ipc` 的共享数据只通过 DTO / dispatcher contract
  流转。
- `cargo tree` 不显示新增循环或意外 heavy dependency 下沉到 leaf crate。

最小验证：

```bash
cargo check -p cc-engine --message-format short
cargo check -p cc-tools --message-format short
cargo check -p cc-mcp --message-format short
cargo check -p cc-browser --message-format short
cargo check -p cc-commands --message-format short
cargo check -p cc-ui --message-format short
cargo check -p claude-code-rs --message-format short
cargo tree -p cc-engine -e normal --depth 1
cargo tree -p cc-tools -e normal --depth 1
```

## Phase 2：Contracts And Wire DTO Consolidation

目标：先把跨 crate contract 固定下来，再迁移 runtime implementation。

执行内容：

- 把跨 runtime 共享 DTO / trait 放入 `cc-types` 或 `cc-ipc-protocol`：
  permission、hook、tool callback、agent event、task event、command result、
  status-line payload、subsystem snapshot、plan workflow event。
- `cc-ipc-protocol` 只拥有 JSONL/headless/daemon wire envelopes、enum tags、
  protocol error category、roundtrip helpers。
- `cc-types` 只拥有非 wire domain DTO 和 adapter traits，不拥有文件写入、
  subprocess、HTTP server、tool execution。
- 建立 IPC JSONL、daemon event、settings/session credential schema 的 roundtrip
  或 snapshot tests。
- 明确 `SdkMessage`、`QueryParams`、`ToolUseContext`、`AppState` 的最终 owner
  和跨 crate 可见面。

退出条件：

- 新增 contract 不需要依赖 root crate。
- Wire DTO 的 serde tag / field name / error classification 有测试覆盖。
- Root modules 可以通过目标 crate import contract，不再复制 DTO。
- 不存在为了通过编译新增的 `allow(dead_code)` / `allow(unused_imports)`。

最小验证：

```bash
cargo test -p cc-types
cargo test -p cc-ipc-protocol
cargo check -p cc-engine --message-format short
cargo check -p cc-ipc --message-format short
cargo check -p claude-code-rs --message-format short
rg 'struct .*Message|enum .*Event|enum .*Command|struct .*Payload' crates/claude-code-rs/src -g '*.rs'
```

## Phase 3：Leaf And Domain Crate Ownership Closure

目标：让低层和 domain crates 先稳定，后续 runtime crate 迁移时只依赖这些 crate。

执行内容：

- `cc-config`：settings schema、validation、global/project discovery、path-isolated
  config 读取写入。
- `cc-auth`：API key、OAuth、token persistence、keychain `cc-rust` service、
  read-only upstream fallback。
- `cc-api`：provider clients、streaming decode、retry/error mapping、provider
  request/response conversion。
- `cc-session`：session persistence、resume lookup、transcript read/write、
  compatibility readers。
- `cc-compact`：compaction pipeline、token budget、compact boundary transform。
- `cc-permissions` / `cc-sandbox`：permission policy、approval decision、
  sandbox capability and failure diagnostics。
- `cc-skills`：built-in/user/project skill discovery、prompt materialization、
  package metadata。
- `cc-observability`：tracing/diagnostic setup and shutdown helpers。

退出条件：

- 对应 root modules 只剩 re-export、adapter install 或 binary-only glue。
- 低层 crate 没有 UI、HTTP server、daemon、engine、tool execution 等不该下沉的依赖。
- Path isolation tests 覆盖 `~/.cc-rust/`、`.cc-rust/`、`cc-rust` service。
- 上游兼容读取保持 read-only 且有测试。

最小验证：

```bash
cargo check -p cc-config --message-format short
cargo check -p cc-auth --message-format short
cargo check -p cc-api --message-format short
cargo check -p cc-session --message-format short
cargo check -p cc-compact --message-format short
cargo check -p cc-permissions --message-format short
cargo check -p cc-sandbox --message-format short
cargo check -p cc-skills --message-format short
cargo check -p cc-observability --message-format short
rg '~/.Codex|\\.Codex/|~/.codex|service.*Codex|service.*Claude' crates -g '*.rs'
```

## Phase 4：Tools, Tasks, Hooks, And Execution Contracts

目标：收敛工具系统和 task runtime，把 tool metadata、execution、progress、hook、
permission 的边界固定下来。

执行内容：

- `cc-tools` 保持 tool metadata、schema、registry policy、pure helper 和 tool
  contract；需要 runtime 的 execution 通过 adapter 或迁到 owning runtime crate。
- `cc-tasks` 拥有 task store、task DTO、task lifecycle、assignment state、
  persistence/locking。
- Hook execution、criticality policy、permission preflight、progress callback
  通过低层 contract 暴露给 engine/query/runtime。
- 文件工具、shell 工具、web 工具、skill tool、team/task tool 按 owner 拆分：
  pure schema 在 `cc-tools`，实际执行依赖的 runtime 留在 owner crate。
- 保留 Bash/PowerShell sandbox/path isolation 语义，不能在迁移中弱化 fail-closed 行为。

退出条件：

- Root `tools/` 不再是 canonical tool implementation owner。
- `cc-tools` 不依赖 root，不拉入 engine/daemon/UI heavy runtime。
- TaskTools remote/multi-type runtime 的仍开放 parity 和 crate migration 边界分开记录。
- Tool execution focused tests 已迁到目标 crate，root e2e 只覆盖 CLI/runtime wiring。

最小验证：

```bash
cargo check -p cc-tools --message-format short
cargo check -p cc-tasks --message-format short
cargo check -p cc-permissions --message-format short
cargo check -p cc-sandbox --message-format short
cargo test -p cc-tools
cargo test -p cc-tasks
cargo check -p claude-code-rs --message-format short
rg 'crate::tools::' crates/cc-* crates/gateway -g '*.rs'
```

## Phase 5：Engine, Query, And Agent Runtime Closure

目标：让 `cc-engine` / `cc-query` 成为 engine/query/agent 的真实 owner，并删除 root
engine implementation shim。

执行内容：

- `cc-engine` 拥有 `QueryEngine`、lifecycle、system prompt、SDK-facing output、
  agent runtime、engine adapters、model invocation orchestration。
- `cc-query` 拥有 async query loop driver、stream/event helpers、turn boundary、
  cancellation。
- 删除 root `engine/lifecycle` 的跨 crate `#[path]` shim，改成短期 re-export 或
  直接更新 call sites 使用 `cc_engine::*`。
- `CommandDispatcher`、hook runner、tool execution、agent tree、background-agent
  updates、task runtime 通过 adapter 注入。
- Langfuse/observability setup and shutdown order 保持和迁移前一致。

退出条件：

- Root `engine/` 不再包含 QueryEngine lifecycle、query loop、agent runtime 的真实实现。
- `cc-engine` 不依赖 `cc-ipc-client` 或 root-private module。
- `cc-query` 使用 explicit traits / DTO，不依赖 root module。
- Agent tree registration/update/snapshot/active-count 在非 IPC 和 IPC 场景都有测试。

最小验证：

```bash
cargo check -p cc-engine --message-format short
cargo test -p cc-engine
cargo check -p cc-query --message-format short
cargo test -p cc-query
cargo check -p claude-code-rs --message-format short
rg 'crate::engine::lifecycle|#\[path = .*cc-engine' crates/claude-code-rs/src -g '*.rs'
```

## Phase 6：Commands And Plan Workflow Migration

目标：让 `cc-commands` 拥有 slash command metadata、parse、handler contract 和可复用命令逻辑。

执行内容：

- 迁移 root `commands/` 中不依赖 binary-only state 的 command implementation。
- 把 command result、command UI payload、status-line payload、subsystem snapshot、
  plan workflow event 下沉到 `cc-types` 或 `cc-ipc-protocol`。
- Binary-only command integrations 通过 runtime adapter 注入：engine submit、
  daemon gateway、team/task runtime、plugin refresh、voice capability、browser/IDE/LSP status。
- 删除 `commands/remote_cmd.rs` 到 root daemon file 的 `#[path]` 复用，改为
  `cc-daemon`/gateway client crate owner 或 adapter。
- 保持 `/help`、`/status`、`/remote`、`/channels`、`/plan` 等 UI/CLI 输出兼容。

退出条件：

- Root `commands/` 只剩 adapter wiring 或 binary-only glue。
- `cc-commands` 不读取 root daemon/tools/engine private modules。
- Command tests 迁入 `cc-commands`；root e2e 保留用户可见 CLI 行为。
- Plan workflow 的 persisted state / approval lifecycle 有 focused tests。

最小验证：

```bash
cargo check -p cc-commands --message-format short
cargo test -p cc-commands
cargo check -p cc-daemon --message-format short
cargo check -p claude-code-rs --message-format short
rg '#\[path = "../daemon/gateway_client.rs"\]|crate::commands::' crates/claude-code-rs/src crates/cc-* -g '*.rs'
```

## Phase 7：IPC, Headless, And Client Runtime Closure

目标：让 IPC wire、client bridge、headless runtime 和 runtime orchestration 的 owner 清晰分离。

执行内容：

- `cc-ipc-protocol` 保持 wire DTO owner。
- `cc-ipc-client` 拥有 client transport、frontend sink、callback bridge helper、
  query-turn helper。
- `cc-ipc` 拥有 runtime orchestration，不拥有 engine implementation。
- Root 只做 headless binary entry、stdio setup、startup adapter install、process signal glue。
- Headless JSONL roundtrip、error classification、ordering assumption 不在迁移中改变。

退出条件：

- Root `ipc/` 不再是 protocol/client/runtime implementation owner。
- `cc-ipc` 不调用 root `crate::engine`、`crate::tools`、`crate::commands`。
- IPC protocol roundtrip tests 覆盖新增/迁移 envelope。
- TUI/headless bridge 通过 explicit client/runtime adapters 工作。

最小验证：

```bash
cargo check -p cc-ipc-protocol --message-format short
cargo test -p cc-ipc-protocol
cargo check -p cc-ipc-client --message-format short
cargo test -p cc-ipc-client
cargo check -p cc-ipc --message-format short
cargo check -p claude-code-rs --message-format short
rg 'crate::(engine|tools|commands)::' crates/cc-ipc crates/cc-ipc-client -g '*.rs'
```

## Phase 8：Teams, Plugins, MCP, Browser, LSP, Computer Use, Voice

目标：收敛横向 runtime domains，避免这些能力继续通过 root 互相调用。

执行内容：

- `cc-teams`：teammate runtime、coordinator、mailbox/protocol、in-process worker、
  team memory integration contracts。
- `cc-plugins`：manifest loading、refresh/install state、plugin tool metadata、
  plugin runtime contracts。
- `cc-mcp`：MCP discovery/transport/config contracts、MCP tool integration；
  browser-specific detection/rendering 留在 `cc-browser`。
- `cc-browser`：browser detection、browser MCP bridge、native host、screenshot/rendering
  support、diagnostics。
- `cc-lsp-service`：LSP lifecycle、transport、server info、symbol/hover data、
  LSP command/tool adapters。
- `cc-computer-use`：detection、setup、screenshot、input、platform automation。
- `voice` domain：将可复用 capability/STT/controller/audio 边界迁入目标 crate
  或明确保留 binary-only 条件；如果需要新 `cc-voice`，先写 scaffold plan。

退出条件：

- Root 对应目录只剩 startup glue、adapter install、CLI entry。
- MCP/browser 没有 runtime ownership 互相穿透。
- Team/plugin/tool interactions 使用 adapter 或 stable DTO。
- LSP/computer-use/voice 的平台特定代码不被下沉到不相关 leaf crate。

最小验证：

```bash
cargo check -p cc-teams --message-format short
cargo check -p cc-plugins --message-format short
cargo check -p cc-mcp --message-format short
cargo check -p cc-browser --message-format short
cargo check -p cc-lsp-service --message-format short
cargo check -p cc-computer-use --message-format short
cargo check -p claude-code-rs --message-format short
rg 'crate::(teams|plugins|mcp|browser|lsp_service|computer_use|voice)::' crates/cc-* crates/gateway -g '*.rs'
```

## Phase 9：Daemon, Gateway, Web, And Remote Runtime Closure

目标：让 daemon/gateway/web runtime ownership 与 root binary glue 分离。

执行内容：

- `cc-daemon` 拥有 daemon process state、worker command/event files、local daemon
  client、supervisor lifecycle、HTTP/SSE control plane、gateway routes、webhook、
  notification、tick logic。
- `gateway` 保持 remote-control gateway models/foundations；公网 control plane 与
  local daemon `/api/*` 边界按现有 remote-control docs 保持清晰。
- Root 只注入 engine submit/abort/status、commands、webhook routing、team memory、
  tools/runtime adapters。
- `web/` 中可复用 static routing/SSE/state handler 迁入 owner crate；binary-only
  port binding/startup 留 root。
- 真实 submit/abort 与 scheduler ownership 从兼容路径迁入 supervisor/worker 架构。

退出条件：

- Root `daemon/` 不再拥有 daemon process/server/worker implementation。
- `cc-daemon` 不调用 root `crate::engine`、`crate::tools`、`crate::commands`、
  `crate::plugins`、`crate::teams`、`crate::plan_workflow`。
- Worker command/event 文件格式有 compatibility tests。
- Remote/gateway docs 与实际 crate owner 一致。

最小验证：

```bash
cargo check -p cc-daemon --message-format short
cargo test -p cc-daemon
cargo check -p gateway --message-format short
cargo check -p claude-code-rs --message-format short
rg 'crate::(engine|tools|commands|plugins|teams|plan_workflow)::' crates/cc-daemon crates/gateway -g '*.rs'
```

## Phase 10：UI And TUI Boundary Closure

目标：让 `cc-ui` 拥有可复用 Rust TUI state/render/input surface，root 只保留 terminal
lifecycle glue。

执行内容：

- 把 `cc-ui` 当前通过 `#[path = "../../claude-code-rs/src/ui/..."]` 引用的 UI
  source 迁入 `cc-ui/src/**`。
- UI-facing snapshots of tasks、teams、MCP、LSP、permissions、commands 通过 DTO /
  adapter 获取，不直接读 root globals。
- Terminal raw mode、signal integration、channel wiring、engine/headless/daemon
  startup handoff 留在 root binary glue。
- Snapshot tests 和 visual/runtime regression helpers 指向 `cc-ui` crate owner。
- Root `ui/mod.rs` 不再通过跨 crate `#[path]` 反向包含 `cc-ui` source；改为
  normal crate import。

退出条件：

- `cc-ui` 不再从 `crates/claude-code-rs/src/ui/**` 读取 source。
- Root `ui/` 只剩 binary-only terminal lifecycle glue 或已删除。
- UI tests 主要在 `cc-ui`，root PTY/e2e 只覆盖 terminal process behavior。
- 所有 referenced assets/rendering helpers 使用目标 crate path。

最小验证：

```bash
cargo check -p cc-ui --message-format short
cargo test -p cc-ui
cargo check -p claude-code-rs --message-format short
rg '#\[path = ".*claude-code-rs/src/ui' crates/cc-ui -g '*.rs'
rg '#\[path = ".*cc-ui' crates/claude-code-rs/src -g '*.rs'
```

## Phase 11：Root Binary Thinning And Shim Deletion

目标：删除所有迁移 shim，使 root crate 达到 thin binary 形态。

执行内容：

- 删除 root 中已经迁出的 implementation module declarations。
- 删除或收敛 root re-export；source compatibility shim 必须有明确删除日期。
- 更新所有 call sites 使用目标 `cc-*` crate path。
- 清理 `Cargo.toml` 中 root 只因迁移残留而存在的 direct dependency。
- 清理 stale tests、stale docs path、duplicate implementation copies。
- 确认 `cli.rs`、`main.rs`、`startup/`、`shutdown.rs`、binary-only mode glue 是 root
  主要剩余面。

退出条件：

- Guard 命令不再发现跨 crate `#[path]` migration bridge。
- Library crate 没有 root dependency 或 root source import。
- Root crate 没有 engine/query/tools/commands/daemon/ipc/teams/plugins/mcp/lsp/browser/
  computer-use/voice 的真实实现。
- `allow(dead_code)` / `allow(unused_imports)` 没有被用于掩盖迁移残留。

最小验证：

```bash
rg '#\[path = ".*claude-code-rs/src' crates/cc-* crates/gateway -g '*.rs'
rg '#\[path = ".*cc-' crates/claude-code-rs/src -g '*.rs'
rg 'crate::(engine|query|tools|commands|daemon|ipc|teams|plugins|mcp|lsp_service|browser|computer_use|voice)::' crates/cc-* crates/gateway -g '*.rs'
rg 'allow\\((dead_code|unused_imports|unused)\\)' crates/cc-* crates/claude-code-rs/src -g '*.rs'
cargo check --workspace --all-targets --message-format short
```

## Phase 12：Full Verification And Documentation Closeout

目标：把 crate migration 从活跃任务关闭为完成状态。

执行内容：

- 运行 workspace final gates。
- 跑 path-isolation guards，确认没有非预期 upstream path 写入。
- 跑 dependency shape checks，确认 thin binary 和 crate ownership。
- 更新 `WORK_STATUS.md`、`IMPLEMENTATION_GAPS.md`、`docs/README.md`。
- 将已完成的活跃迁移计划移入 `docs/archive/`，保留 target state 作为未来 review
  reference。
- 将完成记录写入 `docs/archive/COMPLETED_FULL.md` 或独立 archive note。

最终验证：

```bash
export CARGO_HOME=/data2-HDD-SATA-20T/Digital_avatar/haoweiyao/.rust/cargo
export RUSTUP_HOME=/data2-HDD-SATA-20T/Digital_avatar/haoweiyao/.rust/rustup
export PATH="$CARGO_HOME/bin:$PATH"

cargo fmt --all --check
cargo check --workspace --all-targets --message-format short
cargo test --workspace
cargo build --workspace --release

cargo tree -p claude-code-rs -e normal --depth 1
cargo tree -p cc-engine -e normal --depth 1
cargo tree -p cc-query -e normal --depth 1
cargo tree -p cc-tools -e normal --depth 1
cargo tree -p cc-daemon -e normal --depth 1
cargo tree -p cc-ui -e normal --depth 1

rg '#\[path = ".*claude-code-rs/src' crates/cc-* crates/gateway -g '*.rs'
rg '#\[path = ".*cc-' crates/claude-code-rs/src -g '*.rs'
rg 'crate::(engine|query|tools|commands|daemon|ipc|teams|plugins|mcp|lsp_service|browser|computer_use|voice)::' crates/cc-* crates/gateway -g '*.rs'
rg '~/.Codex|\\.Codex/|~/.codex|service.*Codex|service.*Claude' crates -g '*.rs'
```

完成判定：

- 所有 final gates 通过，或只有已记录的环境 warning。
- Guard 命令没有 production dependency on root implementation。
- Active docs 不再把 crate migration 作为开放 TODO。
- Archive 中有可审计的 completion note，说明最终 crate ownership、验证命令和任何
  intentional residual。

## Phase 顺序摘要

| Phase | 名称 | 主要产物 |
| --- | --- | --- |
| 0 | Baseline And Ownership Inventory | owner matrix、guard matrix、baseline tests |
| 1 | Cargo Graph Decycle And Adapter Foundations | 无阻塞循环的 crate graph、adapter contracts |
| 2 | Contracts And Wire DTO Consolidation | `cc-types` / `cc-ipc-protocol` ownership 固定 |
| 3 | Leaf And Domain Crate Ownership Closure | config/auth/api/session/compact/permission/sandbox 等收口 |
| 4 | Tools, Tasks, Hooks, And Execution Contracts | tool/task/hook/permission 边界固定 |
| 5 | Engine, Query, And Agent Runtime Closure | `cc-engine` / `cc-query` 成为真实 owner |
| 6 | Commands And Plan Workflow Migration | `cc-commands` owning commands and plan workflow |
| 7 | IPC, Headless, And Client Runtime Closure | IPC protocol/client/runtime 分离完成 |
| 8 | Teams, Plugins, MCP, Browser, LSP, Computer Use, Voice | 横向 runtime domains 收口 |
| 9 | Daemon, Gateway, Web, And Remote Runtime Closure | daemon/gateway/web ownership 收口 |
| 10 | UI And TUI Boundary Closure | `cc-ui` source 独立，root 只剩 terminal glue |
| 11 | Root Binary Thinning And Shim Deletion | 删除 migration shims 和 duplicate implementation |
| 12 | Full Verification And Documentation Closeout | workspace final gates 与 docs/archive 收口 |
