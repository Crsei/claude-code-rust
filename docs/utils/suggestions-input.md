# 建议系统与输入处理移植缺口

对应: `claude-code-bun/src/utils/suggestions/` + `processUserInput/`  
Rust 对应: 无 (完全缺失)

## Bun 端规模

### suggestions/ (完全缺失)
| 文件 | 行数 | 说明 |
|------|------|------|
| commandSuggestions.ts | 576 | Fuse.js 模糊搜索命令补全 |
| directoryCompletion.ts | 263 | 目录/路径补全 |
| shellHistoryCompletion.ts | 119 | Shell 历史补全 |
| slackChannelSuggestions.ts | 209 | Slack 频道补全 |
| skillUsageTracking.ts | 55 | 技能使用追踪用于排序 |
| **合计** | **~1,222** | |

### processUserInput/ (完全缺失)
| 文件 | 行数 | 说明 |
|------|------|------|
| processUserInput.ts | 620 | 中央输入路由 |
| processSlashCommand.tsx | 1,209 | 斜杠命令处理 |
| processBashCommand.tsx | 182 | Bash 命令处理 |
| processTextPrompt.ts | 100 | 文本提示处理 |
| **合计** | **~2,201** | |

## Rust 实现状态: 无对应 crate

Rust 中没有 `cc-suggestions` 或类似的 crate。  
`cc-engine/src/input_processing.rs` 只有基础的斜杠命令检测，远不完整。

### IPC 协议扩展 (Phase 2)
- 新增 `FrontendMessage::RequestCompletions { input, cursor_pos, request_id }` 和 `BackendMessage::Completions { items, request_id }`，为前端输入补全提供协议层支持
- 新增 `FrontendMessage::AcceptCompletion { request_id, index }`，支持补全项选择
- 新增 `FrontendMessage::RequestLspRecommendations { language }` 和 `BackendMessage::LspRecommendations { recommendations }`，支持 LSP 推荐查询

## 建议系统缺失细节

### 1. 命令补全 (576 行)
- **Fuse.js** 模糊搜索索引
- 按构建/运行频率和使用频率排序
- 部分匹配、命令别名、快捷键建议
- **Rust 替代**: `fuzzy-matcher` 或 `skim` crate

### 2. 目录/路径补全 (263 行)
- LRU 缓存的目录扫描
- 路径前缀补全
- Git 感知的目录检查
- 隐藏文件过滤
- **Rust 替代**: 标准库 `std::fs::read_dir` + `lru` crate

### 3. Shell 历史补全 (119 行)
- 缓存的历史查找
- 后缀幽灵文本建议
- **Rust 替代**: 读取 shell 历史文件

### 4. Slack 频道补全 (209 行)
- 从缓存或 API 获取频道列表
- 前缀匹配和排序

### 5. 技能使用追踪 (55 行)
- 追踪技能使用频率
- 用于提升高使用率技能的排序权重

## 输入处理缺失细节

### 1. 中央输入路由 (620 行)
- 输入类型检测: `!` → bash, `/` → 命令, 其他 → 文本
- Attachment 解析
- 图片处理
- IDE 选择集成
- 权限模式路由
- 用户提示提交 Hook
- OTEL 分析事件

### 2. 斜杠命令处理 (1,209 行)
- `/command [args]` 解析
- 内置命令 vs 插件命令路由
- Agent 路由命令 (/, /research, /test)
- 内联插件
- Frontmatter hooks
- Skill hooks
- 进度渲染
- Agent 工具执行

### 3. Bash 命令处理 (182 行)
- Shell 路由 (bash vs PowerShell)
- BashTool 执行
- 进度渲染

### 4. 文本提示处理 (100 行)
- 用户消息创建
- Prompt ID 处理
- OTEL 事件发射
- 否定/继续关键词检测

## 移植建议

建议系统是一个相对独立的模块，优先级中等：
1. **命令补全** — `/commands` 列表的模糊搜索，依赖少
2. **路径补全** — 纯文件系统操作，无外部依赖
3. **Slack 补全** — 需要 Slack API 集成，优先级低

输入处理依赖于引擎架构，建议在引擎抽象稳定后移植。

## Rust 替代方案

| Bun 模块 | Rust 替代 |
|----------|-----------|
| Fuse.js | `fuzzy-matcher` 或 `skim` |
| LRU Cache | `lru` crate |
| chokidar (文件监控) | `notify` crate |
| shell-quote | `shell-words` crate (已使用) |
| tree-sitter | `tree-sitter` crate |
