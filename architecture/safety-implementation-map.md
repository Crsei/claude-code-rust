# Safety 实现映射

## 范围

本文只记录 `docs/bun-docs-documentation-plan.md` 中 `Safety` 章节需要覆盖的内容：把 Bun 上游 `docs/safety` 下的 5 篇文档映射到 cc-rust 当前实现，并只基于只读证据判断状态。

本章采用的状态只有：

| 状态 | 含义 |
| --- | --- |
| 已实现 | cc-rust 已有稳定实现，并有明确实现入口 |
| 部分实现 | 核心路径存在，但缺少 Bun 版某些行为 |
| 未实现 | 没有对应能力，或只有占位 |
| 待确认 | 证据不足，不能稳定判断 |
| 故意裁剪 | 项目明确决定不完全对齐 Bun 版 |

## 上游文档清单

| Bun 文档 | 主题 |
| --- | --- |
| `F:\AIclassmanager\cc\claude-code-bun\docs\safety\auto-mode.mdx` | Auto mode、自主执行、分类器、危险权限剥离 |
| `F:\AIclassmanager\cc\claude-code-bun\docs\safety\permission-model.mdx` | Allow / Ask / Deny 权限模型、规则来源、模式 fallback |
| `F:\AIclassmanager\cc\claude-code-bun\docs\safety\plan-mode.mdx` | 计划模式、只读探索、计划文件、审批流 |
| `F:\AIclassmanager\cc\claude-code-bun\docs\safety\sandbox.mdx` | shell 沙箱、平台差异、文件与网络边界 |
| `F:\AIclassmanager\cc\claude-code-bun\docs\safety\why-safety-matters.mdx` | 纵深防御、安全层次、为什么要把风险拆层 |

## 安全模型层次

先看 cc-rust 的安全模型层次，再看 Bun 文档逐项映射。

| 层次 | cc-rust 入口 | 状态 | 说明 |
| --- | --- | --- | --- |
| prompt | [`crates/claude-code-rs/src/engine/system_prompt.rs`](../crates/claude-code-rs/src/engine/system_prompt.rs) | 已实现 | 系统提示词里明确要求用户批准风险操作、识别 hooks 返回、避免 prompt injection，并对安全编码保持警惕，见 `system_prompt.rs:58-65,78-79,99-106`。 |
| permissions | [`crates/cc-types/src/permissions.rs`](../crates/cc-types/src/permissions.rs), [`crates/cc-permissions/src/rules.rs`](../crates/cc-permissions/src/rules.rs), [`crates/cc-permissions/src/decision.rs`](../crates/cc-permissions/src/decision.rs) | 已实现 | 具备 `default/auto/bypass/plan/acceptEdits/dontAsk` 模式、allow/ask/deny 规则匹配、hook overlay、session grant、Auto mode fallback 和 DenialTracker。 |
| hooks | [`crates/cc-types/src/hooks.rs`](../crates/cc-types/src/hooks.rs), [`crates/claude-code-rs/src/tools/hooks/mod.rs`](../crates/claude-code-rs/src/tools/hooks/mod.rs), [`crates/claude-code-rs/src/engine/lifecycle/deps.rs`](../crates/claude-code-rs/src/engine/lifecycle/deps.rs) | 已实现 | hooks 事件模型、`HookRunner` 接口、`PreToolUse` / `PostToolUse` / `PostToolUseFailure` / `PermissionRequest` / `PermissionDenied` / `Stop` 路径都已进入执行链路。 |
| sandbox | [`crates/cc-config/src/settings.rs`](../crates/cc-config/src/settings.rs), [`crates/cc-sandbox/src/lib.rs`](../crates/cc-sandbox/src/lib.rs), [`crates/cc-sandbox/src/availability.rs`](../crates/cc-sandbox/src/availability.rs), [`crates/claude-code-rs/src/tools/exec/bash.rs`](../crates/claude-code-rs/src/tools/exec/bash.rs), [`crates/claude-code-rs/src/tools/exec/powershell.rs`](../crates/claude-code-rs/src/tools/exec/powershell.rs), [`crates/claude-code-rs/src/tools/execution/security.rs`](../crates/claude-code-rs/src/tools/execution/security.rs) | 部分实现 | Linux/macOS shell 沙箱、网络/路径预检、`failIfUnavailable`、`allowUnsandboxedCommands`、`excludedCommands`、`allowedCommands` 权限桥、`/sandbox` 命令都在；Windows OS-level 沙箱按 [`docs/IMPLEMENTATION_GAPS.md`](../docs/IMPLEMENTATION_GAPS.md) §7 记为故意裁剪，除非复审触发条件成立。 |
| plan mode | [`crates/claude-code-rs/src/tools/plan_mode.rs`](../crates/claude-code-rs/src/tools/plan_mode.rs), [`crates/claude-code-rs/src/plan_workflow.rs`](../crates/claude-code-rs/src/plan_workflow.rs), [`crates/claude-code-rs/src/commands/plan.rs`](../crates/claude-code-rs/src/commands/plan.rs) | 部分实现 | 进入 / 退出 plan mode、`pre_plan_mode` 保存与恢复、`.cc-rust/plan.md` 与 `plan-workflow.json` 持久化、`/plan` 命令都存在；`allowedPrompts` 已接入 Bash pattern 与常见验证意图的确定性 session allow 规则，但 Bun 的 LLM 语义 classifier 仍未接通。 |

## 逐文档分析

### Auto Mode

结论：**部分实现**。

已实现的部分：

- `PermissionMode::Auto` 已在权限类型中存在，且解析/序列化都可用，见 [`crates/cc-types/src/permissions.rs:17-59`](../crates/cc-types/src/permissions.rs)。
- 权限引擎在 Auto mode 下默认放行，并保留拒绝计数回退到交互式确认的机制，见 [`crates/cc-permissions/src/rules.rs:138-155`](../crates/cc-permissions/src/rules.rs) 与 [`crates/cc-permissions/src/decision.rs`](../crates/cc-permissions/src/decision.rs)。
- `cc-permissions` 现在提供 `AutoClassifierDecision` / `AutoClassifierStage` / `AutoClassifierVerdict` 和 `has_permissions_to_use_tool_with_hook_and_auto_classifier()`，能接收外部 transcript classifier 的 allow / deny / ask / unavailable / transcript-too-long 结果，并在 Auto mode fallback 处转成权限决策；fast / thinking 阶段会记录在 decision reason 中，分类器不可用或 transcript 过长时降级为 `Ask`。[`decision.rs`](../crates/cc-permissions/src/decision.rs)
- `cc-permissions` 现在提供 `strip_dangerous_permissions_for_auto_mode()` / `restore_dangerous_permissions_after_auto_mode()`，会在权限规则层移除会绕过 Auto classifier 的宽泛 `Bash` / `PowerShell` / `Agent` allow 规则，以及 `python` / `node` / `npm run` / `npx` / `ssh` / `sudo` / `Invoke-Expression` / `Start-Process` / `Add-Type` 等 shell code execution 或 elevation 前缀，并覆盖 PowerShell `.exe` 形态；同时保留 `Bash(cargo test*)`、`Bash(prefix:git)`、`Read` 等窄规则，见 [`crates/cc-permissions/src/dangerous.rs`](../crates/cc-permissions/src/dangerous.rs)。
- 2026-05-07 Phase 1 已把危险 allow 规则剥离/恢复接到运行时 mode transition：进入 Auto mode 会临时移除宽泛 always/session allow 规则，退出 Auto mode 会恢复；`/permissions mode`、`/config permissionMode`、Web settings、Plan mode restore、startup、子 agent 与 read-only plugin tool 上下文都走同一安全 helper。
- 配置与运行时都保留了 Auto mode 可用性开关，见 [`crates/cc-config/src/settings.rs:120-139`](../crates/cc-config/src/settings.rs)、[`crates/claude-code-rs/src/startup/runtime_config.rs:119-122`](../crates/claude-code-rs/src/startup/runtime_config.rs)、[`crates/claude-code-rs/src/commands/permissions_cmd.rs:138-242`](../crates/claude-code-rs/src/commands/permissions_cmd.rs)。

缺口：

- Bun 文档里的 transcript classifier / two-stage 分类流水线还没有在 cc-rust 中形成端到端闭环；当前权限层已经能消费 fast / thinking classifier 结果，但还没有真正的 LLM transcript classifier runner、prompt 模板和 API 调用链。
- `crates/claude-code-rs/src/plan_workflow.rs:261-262` 明确写着“full auto-mode LLM classifier is ported”之前的保守入口仍未完成，说明自动化分类层还在未完全迁移状态。

### Permission Model

结论：**已实现**。

已实现的部分：

- `PermissionMode` 覆盖 Bun 文档里的核心模式集合：`default`、`auto`、`bypass`、`plan`、`acceptEdits`、`dontAsk`，见 [`crates/cc-types/src/permissions.rs:17-59`](../crates/cc-types/src/permissions.rs)。
- 规则引擎按 deny > ask > allow > mode fallback 的顺序裁决，见 [`crates/cc-permissions/src/rules.rs:84-155`](../crates/cc-permissions/src/rules.rs)。
- 全决策流把 hook overlay、session grant、mode fallback 串在一起，见 [`crates/cc-permissions/src/decision.rs:237-477`](../crates/cc-permissions/src/decision.rs)。
- 系统提示词层面也要求工具调用遵循用户选择的 permission mode，见 [`crates/claude-code-rs/src/engine/system_prompt.rs:58-65`](../crates/claude-code-rs/src/engine/system_prompt.rs)。

说明：

- Bun 文档里对规则来源的命名更细，cc-rust 侧把这类来源折叠进 settings merge、session grant 和 runtime gate；但最终的 allow / ask / deny 行为已经有代码路径承接。

### Plan Mode

结论：**部分实现**。

已实现的部分：

- `EnterPlanModeTool` 与 `ExitPlanModeTool` 都已落地，且进入 plan mode 后切到只读探索，见 [`crates/claude-code-rs/src/tools/plan_mode.rs:12-14,30-40,94-120,136-171,188-232`](../crates/claude-code-rs/src/tools/plan_mode.rs)。
- 计划状态会保存 `pre_plan_mode` 并在退出时恢复，见 [`crates/claude-code-rs/src/plan_workflow.rs:68-129`](../crates/claude-code-rs/src/plan_workflow.rs)。
- 计划文件与 workflow 文件都有稳定路径，且会写入 `.cc-rust/plan.md` / `.cc-rust/plan-workflow.json`，见 [`crates/cc-config/src/paths.rs:162-201`](../crates/cc-config/src/paths.rs) 与 [`crates/claude-code-rs/src/commands/plan.rs:45-90,113-192`](../crates/claude-code-rs/src/commands/plan.rs)。
- `PlanWorkflowRecord` 也在类型层保留了 approval / trace / linked tasks 的记录面，见 [`crates/cc-types/src/plan_workflow.rs:105-114,131-131`](../crates/cc-types/src/plan_workflow.rs)。

新增实现：

- `ExitPlanModeTool` 已接受 `allowedPrompts: [{ tool: "Bash", prompt: "<pattern>" }]`，并在用户批准计划后把这些条目转成 session 级 allow 规则 `Bash(<pattern>)`，见 [`crates/claude-code-rs/src/tools/plan_mode.rs`](../crates/claude-code-rs/src/tools/plan_mode.rs)。
- `allowedPrompts` 现在也能把常见自然语言验证意图映射到窄的 Cargo 规则，例如 “run tests and lint” 会生成 `Bash(cargo test*)` 与 `Bash(cargo clippy*)`；显式 `prefix:` / glob / 常见命令 pattern 仍保持直通。
- 新增单元测试覆盖 schema、非法 tool 拒绝、批准后写入 `plan_allowed_prompts` session rules、显式 pattern 直通、自然语言验证提示分类，以及后续 `Bash(command="cargo clippy --all-targets")` 命中 session allow 自动放行。

仍保留的缺口：

- Bun 文档中的 `allowedPrompts` 是完整语义 classifier 驱动。cc-rust 当前实现仍是保守的确定性分类器：已支持常见 Rust 验证意图和显式 Bash pattern，但还不是模型驱动的通用自然语言 classifier。

### Sandbox

结论：**部分实现**。

已实现的部分：

- `cc-sandbox` 已把 Linux / macOS OS-level sandbox、文件系统规则、网络规则和可用性探测拆成独立模块，见 [`crates/cc-sandbox/src/lib.rs:11-28`](../crates/cc-sandbox/src/lib.rs)、[`crates/cc-sandbox/src/availability.rs:1-128`](../crates/cc-sandbox/src/availability.rs)。
- `SandboxSettings` 已包含 `enabled`、`mode`、`failIfUnavailable`、`allowUnsandboxedCommands`、`excludedCommands`、`allowedCommands`、`filesystem`、`network` 等字段，见 [`crates/cc-config/src/settings.rs:203-239`](../crates/cc-config/src/settings.rs)。
- `allowedCommands` 已接入权限决策：workspace sandbox 中匹配的 Bash / PowerShell 命令会在 deny / ask / hook / Plan mode 门禁之后、mode fallback ask 之前被预批准，见 [`crates/claude-code-rs/src/tools/execution/security.rs`](../crates/claude-code-rs/src/tools/execution/security.rs)、[`crates/claude-code-rs/src/engine/lifecycle/deps.rs`](../crates/claude-code-rs/src/engine/lifecycle/deps.rs)、[`crates/claude-code-rs/src/tools/execution/pipeline.rs`](../crates/claude-code-rs/src/tools/execution/pipeline.rs)。
- Bash 与 PowerShell 工具都在执行前做 sandbox gate、`dangerouslyDisableSandbox` gate 和 `sandbox_blocked` 回报，见 [`crates/claude-code-rs/src/tools/exec/bash.rs:279-407`](../crates/claude-code-rs/src/tools/exec/bash.rs) 与 [`crates/claude-code-rs/src/tools/exec/powershell.rs:156-262`](../crates/claude-code-rs/src/tools/exec/powershell.rs)。
- `/sandbox` 命令能切换 on/off/mode/require/optional/no-network，并显示当前 policy，见 [`crates/claude-code-rs/src/commands/sandbox_cmd.rs:4-16,34-140,149-215`](../crates/claude-code-rs/src/commands/sandbox_cmd.rs)。

故意裁剪 / 平台边界：

- Windows 原生 OS-level sandbox 不作为默认补齐项。`cc-sandbox/src/availability.rs:120-128` 明确写明 Windows Restricted Token + Job Object support not implemented，[`crates/cc-sandbox/src/lib.rs:21-27`](../crates/cc-sandbox/src/lib.rs) 也把 Windows 归为“Rust-level policy checks only”；该边界已在 [`docs/IMPLEMENTATION_GAPS.md`](../docs/IMPLEMENTATION_GAPS.md) §7 记为故意裁剪，复审触发条件是上游发布 Windows sandbox-runtime backend、PowerShell sandbox toggle 成为产品必需项，或安全策略要求 Windows OS-level enforcement。
- 2026-05-07 Phase 0 复核：`cargo check -p claude-code-rs --message-format short` 通过，旧的 `crates/cc-compact/src/context_collapse.rs` 编译阻塞当前不再复现；`cc-sandbox` policy 层测试也通过。

### Why Safety Matters

结论：**已实现**。

理由：

- Bun 文档强调的五层安全链路，cc-rust 侧已经能找到对应入口：prompt 在 [`crates/claude-code-rs/src/engine/system_prompt.rs:58-65,78-79,99-106`](../crates/claude-code-rs/src/engine/system_prompt.rs)，permissions 在 [`crates/cc-permissions/src/decision.rs`](../crates/cc-permissions/src/decision.rs)，hooks 在 [`crates/cc-types/src/hooks.rs`](../crates/cc-types/src/hooks.rs) 与 [`crates/claude-code-rs/src/engine/lifecycle/deps.rs`](../crates/claude-code-rs/src/engine/lifecycle/deps.rs)，sandbox 在 [`crates/cc-sandbox/src/lib.rs`](../crates/cc-sandbox/src/lib.rs)，plan mode 在 [`crates/claude-code-rs/src/tools/plan_mode.rs`](../crates/claude-code-rs/src/tools/plan_mode.rs)。
- 这意味着 Bun 文档中的“纵深防御链”在 cc-rust 里不是理念层空话，而是已经拆成了可追踪的代码层。

## 实现映射表

| Bun 文档 | cc-rust 对应面 | 状态 | 主要依据 |
| --- | --- | --- | --- |
| `auto-mode.mdx` | `PermissionMode::Auto`、Auto mode fallback、classifier result adapter、危险 allow 规则剥离/恢复 helper、可用性开关 | 部分实现 | [`crates/cc-types/src/permissions.rs:17-59`](../crates/cc-types/src/permissions.rs)，[`crates/cc-permissions/src/decision.rs`](../crates/cc-permissions/src/decision.rs)，[`crates/cc-permissions/src/dangerous.rs`](../crates/cc-permissions/src/dangerous.rs)，[`crates/claude-code-rs/src/startup/runtime_config.rs:119-122`](../crates/claude-code-rs/src/startup/runtime_config.rs) |
| `permission-model.mdx` | allow / ask / deny 规则引擎、mode fallback、hook overlay、session grant | 已实现 | [`crates/cc-permissions/src/rules.rs:84-155`](../crates/cc-permissions/src/rules.rs)，[`crates/cc-permissions/src/decision.rs:237-477`](../crates/cc-permissions/src/decision.rs) |
| `plan-mode.mdx` | `EnterPlanModeTool` / `ExitPlanModeTool`、`allowedPrompts`、`/plan`、`plan.md` / `plan-workflow.json` | 部分实现 | [`crates/claude-code-rs/src/tools/plan_mode.rs`](../crates/claude-code-rs/src/tools/plan_mode.rs)，[`crates/claude-code-rs/src/commands/plan.rs:4-90,113-192`](../crates/claude-code-rs/src/commands/plan.rs) |
| `sandbox.mdx` | shell 沙箱、网络/路径预检、`/sandbox`、`failIfUnavailable`、`excludedCommands`、`allowedCommands` | 部分实现 | [`crates/cc-sandbox/src/lib.rs:11-28`](../crates/cc-sandbox/src/lib.rs)，[`crates/cc-sandbox/src/availability.rs:1-128`](../crates/cc-sandbox/src/availability.rs)，[`crates/claude-code-rs/src/tools/execution/security.rs`](../crates/claude-code-rs/src/tools/execution/security.rs)，[`crates/claude-code-rs/src/engine/lifecycle/deps.rs`](../crates/claude-code-rs/src/engine/lifecycle/deps.rs) |
| `why-safety-matters.mdx` | prompt / permissions / hooks / sandbox / plan mode 的纵深防御链 | 已实现 | [`crates/claude-code-rs/src/engine/system_prompt.rs:58-65,78-79,99-106`](../crates/claude-code-rs/src/engine/system_prompt.rs)，[`crates/cc-permissions/src/decision.rs`](../crates/cc-permissions/src/decision.rs)，[`crates/cc-types/src/hooks.rs`](../crates/cc-types/src/hooks.rs)，[`crates/cc-sandbox/src/lib.rs`](../crates/cc-sandbox/src/lib.rs)，[`crates/claude-code-rs/src/tools/plan_mode.rs`](../crates/claude-code-rs/src/tools/plan_mode.rs) |

## 已实现汇总

- Prompt 层已有明确安全提示：风险动作需确认、hooks 反馈要当作用户输入、注意 prompt injection，见 [`crates/claude-code-rs/src/engine/system_prompt.rs:58-65,78-79,99-106`](../crates/claude-code-rs/src/engine/system_prompt.rs)。
- Permission 层已经覆盖 `allow / ask / deny` 与多种 mode fallback，见 [`crates/cc-permissions/src/rules.rs:84-155`](../crates/cc-permissions/src/rules.rs)。
- Hooks 层已经接入 tool 执行主链路，见 [`crates/claude-code-rs/src/engine/lifecycle/deps.rs:487-532,578-690,960-1013`](../crates/claude-code-rs/src/engine/lifecycle/deps.rs)。
- Sandbox 层已经覆盖 Linux/macOS shell 隔离、网络检查和 shell 工具封装，见 [`crates/cc-sandbox/src/lib.rs:11-28`](../crates/cc-sandbox/src/lib.rs) 与 [`crates/claude-code-rs/src/tools/exec/bash.rs:279-407`](../crates/claude-code-rs/src/tools/exec/bash.rs)。
- Plan mode 已经能进入、退出、恢复权限并持久化计划文件，见 [`crates/claude-code-rs/src/tools/plan_mode.rs`](../crates/claude-code-rs/src/tools/plan_mode.rs) 与 [`crates/claude-code-rs/src/plan_workflow.rs`](../crates/claude-code-rs/src/plan_workflow.rs)。

## 未实现 / 部分实现 / 待确认

| 项目 | 状态 | 说明 |
| --- | --- | --- |
| Auto mode transcript classifier / 两阶段分类流水线 | 部分实现 | `PermissionMode::Auto`、回退逻辑、classifier result adapter、危险 allow 规则剥离/恢复 helper 与进入 / 退出 Auto mode 的运行时接线已存在；权限层能消费 fast / thinking 的 allow / deny / ask / unavailable 结果，也能临时移除会绕过 classifier 的宽泛 shell / Agent always/session allow 规则。但 Bun 的 LLM transcript classifier runner、prompt 模板和 API 调用链还没有在 cc-rust 中完整落地，见 [`crates/cc-permissions/src/decision.rs`](../crates/cc-permissions/src/decision.rs)、[`crates/cc-permissions/src/dangerous.rs`](../crates/cc-permissions/src/dangerous.rs) 与 [`crates/claude-code-rs/src/plan_workflow.rs:261-262`](../crates/claude-code-rs/src/plan_workflow.rs) 。 |
| Plan mode `allowedPrompts` 语义允许列表 | 部分实现 | `ExitPlanModeTool` 现在接受 `allowedPrompts` 并在计划批准后写入 transient session allow 规则；已支持常见验证提示到 Cargo allow 规则的确定性分类，仍未实现 Bun 的通用 LLM 语义 classifier，见 [`crates/claude-code-rs/src/tools/plan_mode.rs`](../crates/claude-code-rs/src/tools/plan_mode.rs) 。 |
| Windows OS-level sandbox | 故意裁剪 | Windows 分支在可用性探测里直接返回不可用，见 [`crates/cc-sandbox/src/availability.rs:120-128`](../crates/cc-sandbox/src/availability.rs)；当前按 [`docs/IMPLEMENTATION_GAPS.md`](../docs/IMPLEMENTATION_GAPS.md) §7 保留 Rust-level policy checks、`/sandbox require` fail-closed 与 unavailable 诊断，只有上游或安全策略触发复审时才单独重开。 |
| `allowedCommands` 自动放行到 permission decision | 已实现 | workspace sandbox 中匹配的 Bash / PowerShell 命令已接到 central permission 和 reference tool pipeline；deny / ask / hook / Plan mode 不会被覆盖，见 [`crates/claude-code-rs/src/tools/execution/security.rs`](../crates/claude-code-rs/src/tools/execution/security.rs)、[`crates/claude-code-rs/src/engine/lifecycle/deps.rs`](../crates/claude-code-rs/src/engine/lifecycle/deps.rs)、[`crates/claude-code-rs/src/tools/execution/pipeline.rs`](../crates/claude-code-rs/src/tools/execution/pipeline.rs) 。 |

## 后续动作

1. 默认下一步应补齐 Bun 的 Auto mode 语义：接真正的 LLM transcript classifier runner、prompt 模板和 API 调用链；权限层 classifier result adapter 与运行时 strip/restore 接线已可承接结果。
2. 如果要补齐 Bun 的 plan mode 语义，继续把 `allowedPrompts` 从常见验证提示扩展到通用 LLM 语义 classifier；当前只完成确定性 session allow bridge。
3. Windows OS-level sandbox 当前按故意裁剪处理；若上游或安全策略触发复审，再作为单独立项处理。
