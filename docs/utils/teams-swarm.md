# Team/Swarm 系统移植缺口

对应: `claude-code-bun/src/utils/swarm/`  
Rust 对应: `cc-teams/`, `cc-engine/teams/` (空目录)

## Bun 端规模

| 文件 | 行数 | 说明 |
|------|------|------|
| inProcessRunner.ts | 1,650 | 进程内队友运行器 |
| teamHelpers.ts | 683 | 团队文件管理 |
| permissionSync.ts | 928 | 权限同步 |
| spawnInProcess.ts | 383 | 进程内生成 |
| spawnUtils.ts | 165 | 生成工具 |
| teammateInit.ts | 129 | 队友初始化 |
| teammateLayoutManager.ts | 107 | 布局管理 |
| teammateModel.ts | 10 | 模型类型 |
| teammatePromptAddendum.ts | 18 | Prompt 补充 |
| leaderPermissionBridge.ts | 54 | 领导权限桥接 |
| reconnection.ts | 119 | 重连处理 |
| constants.ts | 33 | 常量 |
| **Backends:** | | |
| TmuxBackend.ts | 800 | Tmux 后端 |
| ITermBackend.ts | 370 | iTerm2 集成 |
| InProcessBackend.ts | 345 | 进程内后端 |
| PaneBackendExecutor.ts | 407 | 窗格执行器 |
| WindowsTerminalBackend.ts | 237 | Windows 终端 |
| registry.ts | 585 | 后端注册表 |
| types.ts | 349 | 类型定义 |
| detection.ts | 158 | 后端检测 |
| **合计** | **~7,862** | |

## Rust 已实现

### cc-teams/ (完整度 ~50%)
- **in_process.rs**: 进程内队友执行器
- **mailbox.rs**: 邮箱 IPC (文件锁)
- **runner.rs**: 队友运行循环
- **send_message.rs**: SendMessage 工具
- **team_spawn.rs**: TeamSpawn 工具
- **identity.rs**: 身份系统 (tokio::task_local!)
- **coordinator.rs**: 协调器模式
- **context.rs**: 上下文传播
- **protocol.rs**: 协议定义
- **types.rs**: 类型定义
- **helpers.rs**: 共享工具
- **constants.rs**: 常量
- **pr_activity.rs**: PR 活动工具
- **backend.rs**: 后端抽象 (TeammateExecutor trait)

### cc-engine/teams/ — 空目录
- 作为未来提取的占位，尚无代码

## Rust 缺失的主要功能

### 1. Tmux 后端 (800 行) — 完全缺失
- Bun 版本的功能齐全的 tmux 后端
- 窗格创建/管理/终止
- Tmux socket 通信
- **Rust 已故意裁剪** (cc-teams 代码注释说明)

### 2. iTerm2 后端 (370 行) — 完全缺失
- iTerm2 Python API 集成
- 窗格分割和管理
- 被故意裁剪

### 3. Windows Terminal 后端 (237 行) — 完全缺失
- Windows Terminal 集成
- 被故意裁剪

### 4. 窗格执行器 (407 行) — 完全缺失
- 多窗格命令执行
- 窗格布局管理

### 5. 后端注册表和检测 (585+158 行) — 完全缺失
- 自动后端检测
- 平台能力检测
- 后端选择策略

### 6. 布局管理 (107 行) — 缺失
- 队友 UI 布局管理
- SwarmView 渲染

### 7. 重连处理 (119 行) — 缺失
- Swarm 断线重连逻辑
- 会话恢复

## 设计决策差异

Bun 版本支持多后端 (tmux, iTerm2, InProcess, Windows Terminal)，提供灵活的团队执行模式。  
Rust 版本有意识地**只保留了 InProcess 后端**，裁剪了 tmux/iTerm2/Windows Terminal 支持。

裁剪原因 (根据 cc-teams 代码注释):
- "tmux 和 iTerm2 窗格被故意裁剪" — 可能是为了简化架构
- Rust 的团队系统专注于进程内并发 (tokio tasks)，而非终端多路复用

## 关键差异总结

| 功能 | Bun | Rust | 差距 |
|------|-----|------|------|
| 进程内队友 | 完整 (1,650 行) | in_process.rs | ✅ 已移植 |
| 邮箱 IPC | 完整 | mailbox.rs (文件锁) | ✅ 已移植 |
| SendMessage 工具 | 完整 | send_message.rs | ✅ 已移植 |
| TeamSpawn 工具 | 完整 | team_spawn.rs | ✅ 已移植 |
| 协调器模式 | 完整 | coordinator.rs | ✅ 已移植 |
| **Tmux 后端** | **800 行** | **被裁剪** | **设计决策** |
| **iTerm2 后端** | **370 行** | **被裁剪** | **设计决策** |
| **Window Terminal** | **237 行** | **被裁剪** | **设计决策** |
| **窗格管理** | **~1,200 行** | **无** | **设计决策** |
| **后端注册表/检测** | **~740 行** | **无** | **设计决策** |
| 重连处理 | reconnection.ts | 无 | 功能缺口 |
