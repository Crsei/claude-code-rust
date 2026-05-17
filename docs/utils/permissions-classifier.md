# 权限系统与 LLM 分类器移植缺口

对应: `claude-code-bun/src/utils/permissions/`  
Rust 对应: `cc-permissions/`, `cc-safety/`

## Bun 端规模

| 文件 | 行数 | 说明 |
|------|------|------|
| permissions.ts | 1,507 | 核心权限逻辑 |
| permissionSetup.ts | 1,521 | 权限基础设施 |
| yoloClassifier.ts | 1,510 | 自动模式 LLM 分类器 |
| filesystem.ts | 1,782 | 文件权限检查 |
| pathValidation.ts | 494 | 路径验证 |
| PermissionMode.ts | 136 | 权限模式定义 |
| PermissionRule.ts | 40 | 权限规则类型 |
| PermissionResult.ts | 35 | 权限决策结果 |
| PermissionUpdate.ts | 389 | 权限更新管理 |
| PermissionUpdateSchema.ts | 78 | 更新请求模式 |
| PermissionPromptToolResultSchema.ts | 127 | 工具结果模式 |
| permissionRuleParser.ts | 198 | 规则字符串解析 |
| permissionsLoader.ts | 296 | 磁盘持久化 |
| dangerousPatterns.ts | 80 | 危险命令模式 |
| bashClassifier.ts | 61 | Bash 分类器 (存根) |
| classifierDecision.ts | 98 | 分类器决策类型 |
| shadowedRuleDetection.ts | 234 | 规则遮蔽检测 |
| shellRuleMatching.ts | 228 | Shell 规则匹配 |
| bypassPermissionsKillswitch.ts | 124 | 绕过权限开关 |
| denialTracking.ts | 45 | 拒绝频率追踪 |
| **合计** | **~9,378** | |

## Rust 已实现

### cc-permissions/ (完整度 ~80%)
- **rules.rs**: 完整的规则引擎，支持 Managed/Project/Local/User/CLI/Session 来源
- **decision.rs**: 完整的决策状态机 — 拒绝→询问→允许流程
- **dangerous.rs**: 危险命令检测 (rm -rf, git push --force 等)
- **bash_matcher.rs**: Bash 专用模式匹配
- **path_validation.rs**: 路径验证 (规范化、路径遍历保护)

### cc-safety/ (完整度 ~20%)
- **classifier.rs**: 类型定义和提示构建完成
  - `SafetyClassifierRequest` 结构体
  - `SafetyClassifierPurpose` (AutoModeToolUse, PlanAllowedPrompt)
  - 系统提示构建辅助函数
  - **实际 LLM 推理/分类器调用未实现!**

## Rust 缺失的主要功能

### 1. YOLO/自动模式 LLM 分类器 (1,510 行) — 完全缺失
- 这是自动模式的核心组件
- 使用 Claude API 对权限决策进行分类
- 包含 YOLO prompt、system/deny/allow prompts
- 分类器响应解析
- 权限描述生成
- 拒绝缓存 + CLAUDE.md 重新评估
- **Rust 中的自动模式只能回退到基于规则的决策，没有 LLM 辅助判断**

### 2. 文件系统权限检查 (1,782 行) — 缺失
- 路径模式匹配 (glob patterns)
- 项目目录检测
- 危险文件列表
- 工作路径验证
- Glob 模式解析
- MCP 路径工具

### 3. 完整权限流程协调 (1,507+1,521 行) — 部分缺失
- `checkPermissions()` 对所有工具类型 (bash, file, agent, MCP 等) 的集成
- `applyPermissionUpdate()` 权限更新应用
- `submitPermissionPrompt()` 权限提示提交流程
- 权限提示渲染和规则匹配集成
- **Rust 中有 decision.rs 和 rules.rs，但缺少将所有这些组件串联起来的高层协调逻辑**

### 4. 权限设置管理 (389+296 行) — 缺失
- `PermissionUpdate`: 规则建议、持久化、模式切换、目录管理
- `permissionsLoader`: 跨设置源的规则读写
- 管理规则支持

### 5. 权限更新 Schema 和工具结果 Schema — 缺失
- JSON schema 验证
- 工具结果格式化

### 6. 规则解析器 (198 行) — 可移植
- `Tool(content)` 格式解析
- 转义处理
- 传统工具名规范化

### 7. 规则遮蔽检测 (234 行) — 缺失
- 检测被覆盖/隐藏的权限规则
- 在 UI 中显示警告

### 8. 拒绝追踪 (45 行) — 缺失
- 权限拒绝频率追踪
- 用于调整自动模式行为

### 9. 绕过权限 Killswitch (124 行) — 缺失
- bypass-permissions 模式的紧急停止开关

## 关键差异总结

| 功能 | Bun | Rust | 差距 |
|------|-----|------|------|
| 规则引擎 | 完整 | 完整 (rules.rs) | ✅ 已移植 |
| 决策状态机 | 完整 | 完整 (decision.rs) | ✅ 已移植 |
| 危险命令检测 | 完整 | 完整 (dangerous.rs) | ✅ 已移植 |
| 路径验证 | 完整 | 完整 (path_validation.rs) | ✅ 已移植 |
| Bash 模式匹配 | 完整 | 完整 (bash_matcher.rs) | ✅ 已移植 |
| **YOLO 分类器** | **完整 (1,510 行)** | **缺失** | **核心功能缺口** |
| 文件权限检查 | 完整 (1,782 行) | 缺失 | **功能缺口** |
| 核心权限流程 | checkPermissions() | partial | 高层协调器 |
| 权限持久化 | permissionsLoader.ts | 缺失 | 功能缺口 |
| 规则遮蔽检测 | shadowedRuleDetection.ts | 缺失 | UX 降级 |
| Schema 验证 | PermissionUpdateSchema | 缺失 | 安全缺口 |

## 安全影响

缺少 LLM 分类器意味着自动模式在 Rust 版本中的行为不同：
- Bun: 在自动模式下，每个工具调用可以被 LLM 分类器智能评估，允许安全操作自动执行
- Rust: 自动模式回退到基于规则的匹配，对未明确允许的操作会更多地提示用户
- 这可能导致自动模式的用户体验较差，但并不会降低安全性（默认拒绝）
