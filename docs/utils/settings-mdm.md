# 设置系统与 MDM 移植缺口

对应: `claude-code-bun/src/utils/settings/`  
Rust 对应: `cc-config/`

## Bun 端规模

| 文件 | 行数 | 说明 |
|------|------|------|
| settings.ts | 1,003 | 核心设置管理 |
| types.ts | 1,196 | Zod 类型定义 |
| constants.ts | 202 | 来源定义 |
| changeDetector.ts | 488 | 变更检测 |
| validation.ts | 267 | 通用验证 |
| permissionValidation.ts | 411 | 权限规则验证 |
| validationTips.ts | 164 | 验证建议 |
| settingsCache.ts | 80 | 内存缓存 |
| applySettingsChange.ts | 92 | 变更应用 |
| internalWrites.ts | 37 | 内部写入检测 |
| managedPath.ts | 34 | 路径解析 |
| toolValidationConfig.ts | 103 | 工具验证配置 |
| validateEditTool.ts | 45 | 编辑工具验证 |
| schemaOutput.ts | 8 | Schema 输出 |
| pluginOnlyPolicy.ts | 60 | 插件策略 |
| allErrors.ts | 30 | 错误聚合 |
| **mdm/** | | |
| mdm/settings.ts | 316 | MDM 企业设置 |
| mdm/rawRead.ts | 129 | 原始策略读取 |
| mdm/constants.ts | 81 | MDM 常量 |
| **合计** | **~4,521** | |

## Rust 已实现

### cc-config/ (设置主干已成型，MDM/raw-read/watch 仍缺)
- `crates/cc-config/src/settings.rs`: `RawSettings` / `EffectiveSettings`、source map、JSON schema、atomic write + backup、多源加载。
- `crates/cc-config/src/settings.rs`: 已有 `SettingsSource::Managed`，启动加载顺序包含 managed/user/project/local/env；managed 文件路径支持 `CC_RUST_MANAGED_SETTINGS`、Windows `%ProgramData%\cc-rust\settings.json`、非 Windows `/etc/cc-rust/managed-settings.json`。
- `crates/cc-config/src/settings.rs`: 已有 `permissions.*`、`sandbox.*`、`allowManagedReadPathsOnly`、`allowManagedDomainsOnly`、`defaultModel` / `fallbackModel` / `fastModel` 等字段；未知字段通过 `extra` 保留。注意 managed-only 字段目前只是解析/schema 层存在，强制语义尚未接入 policy builder。
- `crates/cc-config/src/runtime_settings.rs`: `SettingsJson` 作为运行时 settings 投影，`crates/claude-code-rs/src/main.rs` 启动时从 `load_effective()` 填充。
- `crates/cc-config/src/validation.rs`: 只覆盖模型、backend、theme、permission mode、editor mode、language、output style、effort、sandbox mode 等基础警告。
- `crates/cc-config/src/features.rs`: 功能开关。
- `crates/cc-config/src/claude_md.rs`: CLAUDE.md 配置。
- `crates/cc-config/src/constants.rs`: 常量。
- `crates/cc-config/src/paths.rs`: `~/.cc-rust/` / `$CC_RUST_HOME` 路径隔离。
- `crates/cc-config/src/user_agent.rs`: User Agent。

### 权限与 sandbox runtime (部分已实现)
- `crates/start-up/src/runtime_config.rs`: 启动时把 `LoadedSettings` 中的 managed/user/project/local 权限层转成 `ToolPermissionContext`，并保留来源标签。
- `crates/cc-types/src/permissions.rs`: 定义 `PermissionMode`、`ToolPermissionContext`、按来源分组的规则类型。
- `crates/cc-permissions/src/rules.rs`: deny > ask > allow > mode fallback 的规则匹配，包含 Bash / Read / Edit 等 specifier 匹配。
- `crates/cc-permissions/src/decision.rs`: 非测试 runtime 使用的完整权限决策流，包含 hook、session grant、auto classifier adapter、mode fallback。
- `crates/cc-permissions/src/shadowed_rules.rs`: 已有 unreachable allow rule 检测和修复建议，但目前没有发现非测试 runtime call site。
- `crates/cc-permissions/src/permission_update.rs`: 已有内存态权限更新类型和应用函数，但目前没有发现非测试 runtime call site。
- `crates/cc-sandbox/src/policy.rs`: 读取 `SandboxSettings`，实现 `allowedCommands` argv-aware fail-closed 匹配、`excludedCommands`、filesystem/network policy。
- `crates/cc-engine/src/lifecycle/deps.rs`: 运行时权限决策中调用 `sandbox_allowed_command_applies()`，仅在 workspace sandbox 可用时用 `allowedCommands` 预批准命令。

### 命令层写入与诊断 (部分已实现)
- `crates/cc-commands/src/config_cmd.rs`: `/config set` 能即时修改一部分 scalar settings 并持久化到 user/project/local 文件。
- `crates/cc-commands/src/permissions_cmd.rs`: `/permissions allow|ask|deny` 能把规则写回 user/project/local 或 session。
- `crates/cc-commands/src/config_cmd/show.rs`: `/config show --raw` 能展示 managed/user/project/local 原始层。
- `crates/cc-commands/src/doctor.rs`: `/doctor` 调用 `cc_config::validation::validate_settings()` 显示设置警告。
- `crates/cc-commands/src/logout.rs`: logout 会探测 managed settings 文件并明确不触碰该 policy 层。

## Rust 缺失的主要功能

### 1. MDM 企业设置 (316+129+81 行) — 文件型 managed 层已实现，平台 MDM/raw-read 缺失
- 已有: `managed_settings_path()` + `load_effective()` 会读取 managed settings JSON，并把来源标记为 `managed`。
- 已有: `sandbox.allowManagedReadPathsOnly`、`sandbox.allowManagedDomainsOnly` 字段存在于 `SandboxSettings` 和 schema。
- 缺失: 没有 `mdm/` 模块，没有 macOS 配置描述文件 / Windows policy store / 设备管理原始策略读取。
- 缺失: 没有 MDM 常量、原始 payload 到 `RawSettings` 的规范化、MDM 诊断和企业部署错误说明。
- 缺失: `allowManagedReadPathsOnly` / `allowManagedDomainsOnly` 没有在 `crates/cc-sandbox/src/policy.rs` 中强制“只采纳 managed 来源”的路径/域名集合。
- 注意: 当前 managed 文件在优先级上低于 user/project/local/env/cli；如果上游 MDM 语义要求 managed policy 不可被用户覆盖，需要重新核对并调整 merge 规则。

### 2. 设置变更检测 (488 行) — 缺失
- 缺失: 没有 settings 专用 watcher、启动文件扫描缓存、mtime/hash change detector、防抖重载和 merge cache invalidation。
- 已核查: `notify` crate 未用于 settings；仓库只有 `notify-rust` 桌面通知依赖。
- 相关但不等价: `crates/cc-keybindings/src/registry.rs` 有 keybindings mtime 轮询热重载；`crates/cc-engine/src/hooks/file_watcher.rs` 是 hooks FileChanged watcher 结构性 stub，不是 settings watcher。

### 3. 权限验证 (411 行) — 部分实现，缺少 settings permissionValidation 等价层
- 已有: `cc-permissions` 的运行时匹配与决策能处理 allow/ask/deny、来源、Bash compound command、auto/bypass gating。
- 已有: `cc-permissions/src/shadowed_rules.rs` 能发现部分 unreachable allow rule 并给出 fix suggestion，但未接入 `/doctor`、`/permissions` 或启动诊断。
- 缺失: 没有针对 settings 文件内容的权限规则语法验证、跨来源冲突诊断、managed/project/local/user 一致性检查、工具名/规则 specifier 合法性批量校验。

### 4. 验证建议 (164 行) — 缺失
- 已有: `cc-config/src/validation.rs` 直接返回 warning/error 文本，`doctor` 能展示。
- 缺失: 没有独立的 validation tips 模块，没有按字段生成“如何修复”的结构化建议，也没有常见配置问题的分级提示库。

### 5. 工具验证配置 (103+45 行) — 工具各自 validate_input 已有，集中配置缺失
- 已有: 多数工具通过 `Tool::validate_input()` 做输入边界验证，`crates/cc-tools/src/fs/file_edit.rs` 已包含 Edit 工具的特殊校验。
- 缺失: 没有类似 `toolValidationConfig.ts` 的集中式 tool validation config，没有 settings 驱动的工具验证策略，也没有单独的 `validateEditTool` 配置适配层。

### 6. 变更应用 (92 行) — 部分实现，通用 applySettingsChange 缺失
- 已有: `/config set` 对 scalar keys 做内存态更新 + 持久化，`/permissions` 对 allow/ask/deny 规则做增量写回。
- 缺失: 没有覆盖全部 settings key 的通用变更应用入口，没有 nested object/array patch、source-aware reload、失败回滚和 watcher 联动。

### 7. 内部写入检测 (37 行) — 缺失
- 已有: settings 写入使用 atomic rename + backup，但不会标记“这次写入来自本进程”。
- 缺失: 没有 internal write token / timestamp / path marker；未来实现 watcher 后会缺少避免自触发反馈循环的机制。

### 8. 插件专用策略 (60 行) — 未核查到 settings 等价实现
- `pluginOnlyPolicy.ts` 在 Rust settings 层没有同名或等价模块。
- 插件系统有自己的 reload / drift 提示路径，但不等价于 settings MDM/policy 层的 plugin-only policy。

## 关键差异总结

| 功能 | Bun | Rust | 差距 |
|------|-----|------|------|
| 多源设置 | 完整 (5 来源) | `crates/cc-config/src/settings.rs` | ✅ 已移植，并包含 managed/user/project/local/env/cli |
| 设置类型 | Zod schemas | Rust structs + hand schema | ✅ 已移植主干，未知字段 passthrough |
| Feature flags | features.rs | ✅ 已移植 |
| CLAUDE.md | claude_md.rs | ✅ 已移植 |
| 路径管理 | paths.rs | ✅ 已移植 |
| 运行时设置 | runtime_settings.rs | ✅ 已移植 |
| managed policy 文件层 | mdm/settings + managedPath | `settings.rs` | ✅ 部分移植；仅 JSON 文件层 |
| **MDM 原始策略读取** | **mdm/rawRead.ts + constants** | **无** | **企业缺口** |
| **变更检测** | **changeDetector.ts (488 行)** | **无 settings watcher** | **UX 降级** |
| **权限验证** | **permissionValidation.ts (411 行)** | **运行时决策已实现，settings 批量验证缺失** | **安全/诊断缺口** |
| 验证建议 | validationTips.ts | 基础 warning 文本 | UX 降级 |
| 工具验证配置 | toolValidationConfig + validateEditTool | `Tool::validate_input()` 分散实现 | 集中策略缺口 |
| 变更应用 | applySettingsChange.ts | `/config set` + `/permissions` 局部实现 | 通用 patch/reload 缺口 |
| 内部写入检测 | internalWrites.ts | 无 | watcher 前置缺口 |

## 主要缺失功能的 Rust 落点

| 缺失功能 | 修改/新增位置 | 说明 |
|----------|---------------|------|
| MDM 常量与平台路径 | `crates/cc-config/src/mdm/constants.rs` (新增), `crates/cc-config/src/mdm/mod.rs` (新增), `crates/cc-config/src/lib.rs` | 新增 `mdm` 模块并导出；保留 cc-rust 路径隔离，不使用 `~/.Codex/`。 |
| MDM 原始策略读取 | `crates/cc-config/src/mdm/raw_read.rs` (新增) | 实现 macOS configuration profile、Windows policy store、环境变量/文件 fallback 的原始读取；目前 `settings.rs` 只读 JSON 文件。 |
| MDM settings 规范化 | `crates/cc-config/src/mdm/settings.rs` (新增), `crates/cc-config/src/settings.rs` | 把 raw policy payload 规范化为 `RawSettings`，并接入 `load_effective()` 的 managed 层。 |
| managed 不可覆盖语义 | `crates/cc-config/src/settings.rs`, `crates/start-up/src/runtime_config.rs`, `crates/cc-sandbox/src/policy.rs` | 若上游语义要求 managed policy 强制优先，需要调整 `SettingsSource::rank()`、merge 顺序、权限/sandbox 字段的来源保留逻辑。 |
| settings watcher / change detector | `crates/cc-config/src/change_detector.rs` (新增), `crates/cc-config/src/lib.rs`, `crates/claude-code-rs/src/main.rs` | 新增 settings 文件扫描、mtime/hash、debounce 和 reload 入口；启动后注册 watcher。 |
| settings reload 应用 | `crates/cc-config/src/apply_settings_change.rs` (新增), `crates/cc-commands/src/config_cmd.rs`, `crates/cc-engine/src/types/app_state.rs`, `crates/claude-code-rs/src/ui/tui.rs` | 抽出 `/config set` 的局部逻辑，覆盖全部 settings key，并把变更同步到 AppState/TUI/runtime。 |
| 内部写入检测 | `crates/cc-config/src/internal_writes.rs` (新增), `crates/cc-config/src/settings.rs`, `crates/cc-config/src/change_detector.rs` (新增) | `write_settings_file()` 写入前后登记 path/token，watcher 收到事件时过滤本进程写入。 |
| permissionValidation 等价层 | `crates/cc-config/src/permission_validation.rs` (新增), `crates/cc-config/src/validation.rs`, `crates/cc-commands/src/doctor.rs`, `crates/cc-commands/src/permissions_cmd.rs` | 对 `RawSettings.permissions` / `allowedTools` / sandbox 权限字段做语法、工具名、specifier、跨来源冲突和 managed 覆盖诊断。 |
| shadowed rule runtime 接入 | `crates/cc-commands/src/doctor.rs`, `crates/cc-commands/src/permissions_cmd.rs`, `crates/start-up/src/runtime_config.rs` | 复用 `crates/cc-permissions/src/shadowed_rules.rs`，把 unreachable allow rule 提示显示给用户。 |
| validation tips | `crates/cc-config/src/validation_tips.rs` (新增), `crates/cc-config/src/validation.rs`, `crates/cc-commands/src/doctor.rs` | 给 `ValidationWarning` 增加修复建议/文档锚点/示例值，避免只输出错误文本。 |
| tool validation config | `crates/cc-tools/src/validation_config.rs` (新增), `crates/cc-tools/src/tool.rs`, `crates/cc-tools/src/fs/file_edit.rs`, `crates/cc-tools/src/registry.rs` | 建立集中式工具验证配置，再让各工具的 `validate_input()` 读取共享策略。 |
| validateEditTool 等价层 | `crates/cc-tools/src/fs/validate_edit_tool.rs` (新增), `crates/cc-tools/src/fs/file_edit.rs` | 从 `FileEditTool::validate_input()` 拆出可测试、可配置的 Edit 专用校验。 |
| plugin-only policy | `crates/cc-config/src/plugin_only_policy.rs` (新增), `crates/cc-plugins/src/loader.rs`, `crates/cc-plugins/src/manifest.rs`, `crates/cc-plugins/src/tools.rs`, `crates/cc-commands/src/plugin_cmd.rs` | 如果需要对齐 Bun 的 plugin-only policy，应放在 settings/policy 解析层，再由插件 runtime 消费。 |
