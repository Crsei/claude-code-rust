# cc-rust 当前问题汇总

> 更新日期: 2026-05-17

本文是当前开放问题、代码审查发现和文档状态问题的唯一活跃入口。已修复、已失效或只具历史价值的问题已迁移到：

- [archive/resolved-known-issues-2026-05-07.md](archive/resolved-known-issues-2026-05-07.md)
- [archive/issues/](archive/issues/)

## 1. 构建与实现阻塞

| ID | 严重度 | 状态 | 范围 | 摘要 | 详情 |
| --- | --- | --- | --- | --- | --- |
| CLIPPY-001 | 高 | Open | clippy warning gate | `cargo clippy -p claude-code-rs --all-targets -- -D warnings` 仍被跨模块 lint 阻塞；已清理本轮发现的低风险机械项，剩余项需单独计划处理。 | [2026-05-07 编译记录](archive/resolved-known-issues-2026-05-07.md#2026-05-07-编译过程记录) |
| WORKTREE-001 | 中 | Open | worktree hooks | `WorktreeRemove` 可能收到子目录而非仓库根；路径边界校验未处理 symlink / Windows junction 逃逸。 | [2026-05-07 review](archive/issues/2026-05-07-code-review-findings.md) §二 |

## 2. 安全与权限

| ID | 严重度 | 状态 | 范围 | 摘要 | 详情 |
| --- | --- | --- | --- | --- | --- |
| SAFETY-001 | 高 | Fixed | Auto mode | `permissions.enableAutoMode=false` 现在由统一的 permission transition helper 强制执行，启动配置、Web、`/permissions`、`/config` 与子上下文不能绕过进入 Auto。 | [2026-05-07 review](archive/issues/2026-05-07-code-review-findings.md) §四 |
| SAFETY-002 | 高 | Fixed | Plan `allowedPrompts` | Auto -> Plan -> ExitPlanMode 追加的 `allowedPrompts` 规则会在恢复 Auto mode 后立即复用危险 allow 规则剥离逻辑，宽泛/解释器/package runner Bash 规则进入 Auto mode stripped side buffer。 | 同上 |
| SAFETY-003 | 高 | Fixed | Sandbox `allowedCommands` | `allowedCommands` 仅在 workspace sandbox 且 OS-level sandbox 可用时预批准；匹配改为 argv 结构化检查，链式/管道命令中未显式允许的子命令不会搭车放行。 | 同上 |
| SAFETY-004 | 高 | Fixed | classifier redaction | classifier redaction 覆盖 JSON/object-like secret 字段，包括 `password`、`apiKey`、`api_key`、`token`、`accessToken`、`refreshToken`、`secret` 等。 | 同上 |
| SAFETY-005 | 中 | Fixed | Plan approval UI | ExitPlanMode 审批提示现在列出将写入的去重后 transient allowed prompt rules，而不是只显示数量。 | 同上 |

## 3. 模型与 provider 文档/兼容性

| ID | 严重度 | 状态 | 范围 | 摘要 | 详情 |
| --- | --- | --- | --- | --- | --- |
| MODEL-001 | 高 | Open | Bedrock model mapping | `claude-opus-4-7` Bedrock 映射缺日期与 `:0` 后缀，可能路由到不存在的模型 ID。 | [2026-05-07 review](archive/issues/2026-05-07-code-review-findings.md) §一 |
| MODEL-002 | 高 | Open | model aliases | `opus`、`sonnet`、`haiku` 兼容别名被移除但文档/配置仍描述为有效。 | 同上 |
| DOC-001 | 中 | Open | command/provider docs | `/model` 命令参考、CLI 用法和配置文档需要随别名策略同步。 | 同上 |

## 4. Context / compact

| ID | 严重度 | 状态 | 范围 | 摘要 | 详情 |
| --- | --- | --- | --- | --- | --- |
| CONTEXT-001 | 高 | Open | auto compact | 阈值判断重复扣减本地释放 token，可能错误跳过 auto-compact。 | [2026-05-07 review](archive/issues/2026-05-07-code-review-findings.md) §三 |
| CONTEXT-002 | 高 | Open | exact count preflight | exact-count 预检漏掉 system prompt 和 tools，可能低估真实模型请求大小。 | 同上 |

## 5. UI / runtime residuals

| ID | 严重度 | 状态 | 范围 | 摘要 | 详情 |
| --- | --- | --- | --- | --- | --- |
| UI-001 | 中 | Open | TS/OpenTUI resize | Rust TUI resize 已收口；TS/OpenTUI 在 Windows maximize/fullscreen 后仍可能留下白色横行或未完整 repaint。 | 历史 #1/#17 |
| UI-002 | 中 | Open | Rust TUI shell output | shell output renderer 已支持 expanded/detail view，但最新 shell 输出尚未由 runtime context 自动展开。 | 历史 #21 |
| UI-003 | 中 | Open | Rust TUI Ctrl+R | Ctrl+R 历史搜索只搜索当前 session，尚未接跨会话持久 prompt history reader。 | 历史 #22 |
| UI-004 | 中 | Open | Browser MCP | Browser MCP 截图、console/network 结构化结果和权限文案尚未对真实第三方 server 做端到端验证。 | 历史 Browser MCP |

## 6. 文档状态问题

| ID | 严重度 | 状态 | 范围 | 摘要 | 详情 |
| --- | --- | --- | --- | --- | --- |
| DOC-002 | 中 | Open | Extensibility implementation map | Phase 5/6 closure 与旧“部分实现”状态冲突；Phase 5 实施记录、future fields、WebSocket/out-of-scope 口径需收口。 | [2026-05-07 review](archive/issues/2026-05-07-code-review-findings.md) §五 |
| DOC-003 | 中 | Open | stale Lite wording | 部分历史/概览文档仍使用 Lite 口径或旧路径表述；触及时应改成 Full Build 语义或移入 archive。 | 本轮文档清理发现 |

## 7. 更新规则

1. 新增开放问题写入本文，不再新增 `docs/issues*.md` 或 `docs/issues/` 下的活跃问题文档。
2. 只读审查原文、长日志和历史复盘放入 [archive/issues/](archive/issues/)。
3. 修复完成后，把问题从本文移到 [archive/resolved-known-issues-2026-05-07.md](archive/resolved-known-issues-2026-05-07.md) 或后续同类 resolved archive。
4. 文档中若只有“已实现/已修复”历史，不应留在活跃入口；迁入 `docs/archive/`。

## 8. Remote-Control Gateway Residuals (2026-05-08)

| ID | Severity | Status | Scope | Summary | Detail |
| --- | --- | --- | --- | --- | --- |
| REMOTE-001 | Medium | Open | Gateway busy policy | Mid-turn `steer` is intentionally unsupported. | `GatewayPolicy::supports_steer` defaults to false and `BusyPolicy::Steer` returns `501 unsupported`. Capabilities must not imply live steering until `QueryEngine` has explicit mid-turn injection semantics. |
| REMOTE-002 | Medium | Open | Telegram/Lark adapters | Telegram and Lark are outbound diagnostics only. | The first gateway release supports adapter configuration status, connect/health checks, and allowlisted test messages. Full inbound conversational remote control remains follow-up work. |
| REMOTE-003 | Medium | Open | Webhook configuration | Declarative webhook routes are code-backed but not yet backed by a full admin CRUD surface. | Built-in route ids resolve secrets from environment variables and use generic defaults for unknown route ids. A durable route-management UX/API is still needed before broad operator use. |
| REMOTE-004 | Medium | Open | Release verification | Session 16 performed docs-gate verification, not full remote-control code verification. | The next release step must run the Session 17 command set before claiming the gateway implementation is fully green. |
| REMOTE-005 | Low | Open | Public exposure | Public hosted gateway and multi-tenant SaaS are non-goals for this release. | Non-loopback use requires explicit remote-token policy and origin controls; production hosting design remains out of scope. |

## 9. Ratatui UI parity OMX verification residuals (2026-05-10)

| ID | Severity | Status | Scope | Summary | Detail |
| --- | --- | --- | --- | --- | --- |
| UI-005 | Low | Open | line endings | `git diff --check` passes but reports CRLF-to-LF normalization warnings for several touched files. | The warnings are not whitespace errors, but commit packaging should expect Git normalization on touched Rust/docs files. |
