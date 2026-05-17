# Hook 系统移植缺口

对应: `claude-code-bun/src/utils/hooks/`  
Rust 对应: `cc-tools/hooks/`, `cc-types/hooks.rs`, `cc-engine/worktree_hooks.rs`

## Bun 端规模

| 文件 | 行数 | 说明 |
|------|------|------|
| hooksConfigManager.ts | 400 | Hook 配置管理 |
| AsyncHookRegistry.ts | 309 | 异步 Hook 生命周期 |
| sessionHooks.ts | 447 | 会话级函数 Hook |
| hookEvents.ts | 192 | 通用 Hook 事件系统 |
| hooksSettings.ts | 271 | Hook 设置 |
| execAgentHook.ts | 341 | Agent Hook 执行 |
| execHttpHook.ts | 243 | HTTP Hook 执行 |
| execPromptHook.ts | 214 | Prompt Hook 执行 |
| fileChangedWatcher.ts | 191 | 文件变更检测 |
| skillImprovement.ts | 299 | 技能改进 Hook |
| ssrfGuard.ts | 294 | SSRF 防护 |
| postSamplingHooks.ts | 70 | 采样后 Hook |
| registerFrontmatterHooks.ts | 67 | Frontmatter Hook 注册 |
| registerSkillHooks.ts | 64 | Skill Hook 注册 |
| hooksConfigSnapshot.ts | 133 | 配置快照 |
| hookHelpers.ts | 83 | 共享工具 |
| apiQueryHookHelper.ts | 144 | API 查询 Hook |
| **合计** | **~4,051** | |

## Rust 已实现

### cc-tools/hooks/
- `pre_tool.rs` / `post_tool.rs`: 工具执行前后的 Hook 调用
- `execution.rs`: Hook 执行基础框架
- `mod.rs`: Hook 模块组织

### cc-types/hooks.rs
- Hook 相关类型定义

### cc-engine/worktree_hooks.rs
- WorktreeCreate/Remove 钩子集成

## Rust 缺失的主要功能

### 1. Hook 配置管理 (400 行) — 缺失
- Hook 配置的加载/保存/更新
- Hook 变量解析
- 事件元数据描述

### 2. 异步 Hook 注册表 (309 行) — 缺失
- 待处理 Hook 注册表
- 超时处理
- 响应收集
- 进度轮询间隔
- 进程追踪

### 3. 会话级函数 Hook (447 行) — 缺失
- `addFunctionHook()` / `removeFunctionHook()`
- 运行时注册的回调函数
- 按 Agent 生命周期的临时 Hook

### 4. Hook 事件系统 (192 行) — 缺失
- `registerHookEventHandler()`
- `emitHookStarted()`, `emitHookProgress()`, `emitHookResponse()`
- 带定期轮询的进度间隔
- 待处理事件队列

### 5. Hook 执行后端 — 部分缺失
- **execAgentHook.ts** (341 行): Agent 事件 Hook 执行 — 缺失
- **execHttpHook.ts** (243 行): WebHook 执行 — 缺失
- **execPromptHook.ts** (214 行): Prompt Hook 执行 — 缺失

### 6. SSRF 防护 (294 行) — 缺失
- WebHook 的 SSRF 保护
- URL 验证和限制

### 7. 注册 Hook — 缺失
- `registerFrontmatterHooks.ts`: Frontmatter 钩子注册
- `registerSkillHooks.ts`: Skill 钩子注册

### 8. Hook 快照和比较 (133 行) — 缺失
- `hooksConfigSnapshot.ts`
- 配置变更检测

### 9. 技能改进集成 (299 行) — 缺失
- `skillImprovement.ts`
- 基于 Hook 的技能改进反馈

### 10. 文件变更检测 (191 行) — 缺失
- Hook 配置的 chokidar 文件监控

## 关键差异总结

| 功能 | Bun | Rust | 差距 |
|------|-----|------|------|
| Pre/Post Hook 执行 | execAgentHook.ts | pre_tool.rs / post_tool.rs | ✅ 部分移植 |
| Hook 类型定义 | hooksSettings.ts + types | hooks.rs | ✅ 已移植 |
| **Hook 配置管理** | **hooksConfigManager.ts (400 行)** | **无** | **功能缺口** |
| **异步注册表** | **AsyncHookRegistry.ts (309 行)** | **无** | **功能缺口** |
| **会话 Hook** | **sessionHooks.ts (447 行)** | **无** | **功能缺口** |
| **HTTP WebHook** | **execHttpHook.ts (243 行)** | **无** | **功能缺口** |
| **SSRF 防护** | **ssrfGuard.ts (294 行)** | **无** | **安全缺口** |
| **事件系统** | **hookEvents.ts (192 行)** | **无** | **功能缺口** |
| **Frontmatter/Skill** | **register*.ts** | **无** | **功能缺口** |
| 快照管理 | hooksConfigSnapshot.ts | 无 | 功能缺口 |
