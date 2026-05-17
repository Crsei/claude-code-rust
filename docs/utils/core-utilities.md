# 通用工具函数移植缺口

对应: `claude-code-bun/src/utils/` 下大量顶级 .ts 文件
Rust 对应: `cc-utils/` (仅 9 个模块)

## Rust cc-utils 已实现 (9 模块)

| 模块 | 对应 Bun 文件 | 功能 |
|------|-------------|------|
| abort.rs | abortController.ts | 中止/取消协调 |
| bash.rs | bash/commands.ts, bash/shellQuote.ts (部分) | Bash 命令分割和引用 |
| cwd.rs | cwd.ts | 工作目录 |
| file_state_cache.rs | fileStateCache.ts | 文件状态缓存 |
| git.rs | git.ts | Git 仓库检测/状态 |
| git_operation_tracking.rs | — | Git 操作追踪 |
| messages.rs | messages.ts (部分) | 消息工具 |
| shell.rs | shell/ 部分 | Shell 转义/控制台输出 |
| tokens.rs | tokens.ts | Token 估算 |

## 未移植的通用工具函数 (~190 文件缺失)

### 数据结构/算法 (Rust 标准库已有替代)
- `array.ts` — 数组操作工具 (可用 Rust slices/iterators)
- `set.ts` — Set 操作 (可用 Rust HashSet)
- `CircularBuffer.ts` — 环形缓冲区 (可用 `ringbuf` crate)
- `stream.ts` — 流处理工具
- `semaphore.ts` — 信号量 (可用 tokio::sync::Semaphore)
- `sequential.ts` — 顺序执行器
- `memoize.ts` — 记忆化 (可用 `cached` crate)
- `withResolvers.ts` — Promise withResolvers (Rust 用 oneshot channel)
- `semanticBoolean.ts` — 语义布尔值判断
- `semanticNumber.ts` — 语义数值判断

### 字符串/文本处理
- `stringUtils.ts` — 字符串工具函数
- `words.ts` — 单词处理
- `truncate.ts` — 字符串截断
- `treeify.ts` — 树状格式化输出
- `diff.ts` — 文本差异对比 (可用 `similar` crate)
- `markdown.ts` — Markdown 处理
- `markdownConfigLoader.ts` — Markdown 配置加载
- `frontmatterParser.ts` — Frontmatter 解析器
- `textHighlighting.ts` — 文本高亮
- `hyperlink.ts` — 超链接处理
- `format.ts` — 格式化工具
- `formatBriefTimestamp.ts` — 简短时间戳格式化
- `json.ts` — JSON 工具
- `jsonRead.ts` — JSON 读取
- `lazySchema.ts` — 懒加载 Schema
- `contentArray.ts` — 内容数组处理

### 加密/哈希
- `hash.ts` — 哈希函数
- `crypto.ts` — 加密工具
- `uuid.ts` — UUID 生成

### 文件系统
- `file.ts` — 文件操作
- `fileRead.ts` — 文件读取
- `fileReadCache.ts` — 文件读取缓存
- `fsOperations.ts` — 文件系统操作
- `glob.ts` — Glob 匹配
- `tempfile.ts` — 临时文件
- `lockfile.ts` — 文件锁
- `findExecutable.ts` — 可执行文件查找
- `which.ts` — which 命令封装
- `binaryCheck.ts` — 二进制文件检查

### 环境/路径
- `env.ts` — 环境变量
- `envUtils.ts` — 环境工具
- `envDynamic.ts` — 动态环境
- `envValidation.ts` — 环境验证
- `systemDirectories.ts` — 系统目录
- `windowsPaths.ts` — Windows 路径处理
- `subprocessEnv.ts` — 子进程环境
- `cachePaths.ts` — 缓存路径

### 错误/调试
- `errors.ts` — 错误类型
- `errorLogSink.ts` — 错误日志接收
- `debug.ts` — 调试工具
- `debugFilter.ts` — 调试过滤
- `warningHandler.ts` — 警告处理
- `sentry.ts` — Sentry 集成 (空实现)

### HTTP/网络
- `http.ts` — HTTP 客户端
- `api.ts` — API 客户端
- `apiPreconnect.ts` — API 预连接
- `browser.ts` — 浏览器检测
- `userAgent.ts` — User Agent

### 进程/执行
- `execFileNoThrow.ts` — 不抛异常的文件执行
- `execFileNoThrowPortable.ts` — 便携版
- `execSyncWrapper.ts` — 同步执行包装
- `genericProcessUtils.ts` — 进程工具
- `signal.ts` — 信号处理
- `sleep.ts` — 延迟
- `timeouts.ts` — 超时处理
- `gracefulShutdown.ts` — 优雅关闭
- `cleanup.ts` — 清理
- `cleanupRegistry.ts` — 清理注册表

### 终端/渲染
- `ansiToPng.ts` — ANSI 转 PNG
- `ansiToSvg.ts` — ANSI 转 SVG
- `sliceAnsi.ts` — ANSI 切片
- `terminal.ts` — 终端操作
- `claudeDesktop.ts` — Claude Desktop 集成
- `fullscreen.ts` — 全屏模式
- `ink.ts` — Ink 渲染器
- `staticRender.tsx` — 静态渲染
- `exportRenderer.tsx` — 导出渲染器
- `highlightMatch.tsx` — 匹配高亮

### 系统/平台
- `systemPrompt.ts` — 系统提示
- `systemPromptType.ts` — 系统提示类型
- `systemTheme.ts` — 系统主题
- `cwd.ts` — 工作目录
- `editor.ts` — 编辑器检测
- `ide.ts` — IDE 集成
- `idePathConversion.ts` — IDE 路径转换
- `jetbrains.ts` — JetBrains 集成
- `intl.ts` — 国际化
- `language.ts` — 语言检测

### 会话/状态
- `sessionActivity.ts` — 会话活动
- `sessionDataUploader.ts` — 会话数据上传
- `sessionEnvironment.ts` — 会话环境
- `sessionEnvVars.ts` — 会话环境变量
- `sessionFileAccessHooks.ts` — 会话文件访问钩子
- `sessionIngressAuth.ts` — 会话入口认证
- `sessionRestore.ts` — 会话恢复
- `sessionStart.ts` — 会话启动
- `sessionState.ts` — 会话状态
- `sessionStorage.ts` — 会话存储
- `sessionStoragePortable.ts` — 便携会话存储
- `sessionTitle.ts` — 会话标题
- `sessionUrl.ts` — 会话 URL

### 其他工具
- `stats.ts` — 统计
- `statsCache.ts` — 统计缓存
- `fpsTracker.ts` — FPS 追踪
- `startupProfiler.ts` — 启动性能分析
- `headlessProfiler.ts` — Headless 性能分析
- `heapDumpService.ts` — 堆转储
- `eventLoopStallDetector.ts` — 事件循环卡顿检测
- `concurrentSessions.ts` — 并发会话
- `crossProjectResume.ts` — 跨项目恢复
- `ccshareResume.ts` — CC Share 恢复
- `cliArgs.ts` — CLI 参数
- `cliHighlight.ts` — CLI 高亮
- `cliLaunch.ts` — CLI 启动
- `commandLifecycle.ts` — 命令生命周期
- `handlePromptSubmit.ts` — 提示提交处理
- `immediateCommand.ts` — 立即命令
- `billing.ts` — 计费
- `extraUsage.ts` — 额外用量
- `tokenBudget.ts` — Token 预算
- `thinking.ts` — 思考显示
- `fastMode.ts` — 快速模式
- `effort.ts` — 努力程度
- `theme.ts` — 主题
- `keyboardShortcuts.ts` — 快捷键
- `combinedAbortSignal.ts` — 合并中止信号
- `status.tsx` — 状态组件
- `statusNoticeDefinitions.tsx` — 状态通知定义
- `statusNoticeHelpers.ts` — 状态通知帮助
- `autoModeDenials.ts` — 自动模式拒绝
- `autonomy*.ts` — 自治功能系列文件
- `collaps*.ts` — 折叠功能系列文件
- `taggedId.ts` — 标签 ID
- `hotkeys.ts` — 热键
- `mcpInstructionsDelta.ts` — MCP 指令增量
- `mcpOutputStorage.ts` — MCP 输出存储
- `mcpValidation.ts` — MCP 验证
- `mcpWebSocketTransport.ts` — MCP WebSocket 传输
- `claudeCodeHints.ts` — Claude Code 提示
- `codeIndexing.ts` — 代码索引
- `completionCache.ts` — 完成缓存
- `contextAnalysis.ts` — 上下文分析
- `contextSuggestions.ts` — 上下文建议
- `conversationRecovery.ts` — 会话恢复
- `cron*.ts` — 计划任务系列
- `deepLink/**` — Deep Link 处理
- `desktopDeepLink.ts` — 桌面 Deep Link
- `detectRepository.ts` — 仓库检测
- `diagLogs.ts` — 诊断日志
- `displayTags.ts` — 显示标签
- `doctor*.ts` — 诊断功能
- `dxt/**` — DXT 工具
- `embeddedTools.ts` — 嵌入式工具
- `exampleCommands.ts` — 示例命令
- `fileHistory.ts` — 文件历史
- `fileOperationAnalytics.ts` — 文件操作分析
- `filePersistence/**` — 文件持久化
- `fingerprint.ts` — 指纹识别
- `forkedAgent.ts` — Forked Agent
- `generatedFiles.ts` — 生成的文件
- `generators.ts` — 生成器
- `getWorktreePaths*.ts` — 工作树路径
- `ghPrStatus.ts` — GitHub PR 状态
- `git/**` — Git 相关工具
- `gitDiff.ts` — Git 差异
- `github/**` — GitHub 工具
- `githubRepoPathMapping.ts` — GitHub 仓库路径映射
- `gitSettings.ts` — Git 设置
- `groupToolUses.ts` — 工具使用分组
- `heatmap.ts` — 热力图
- `hooks.ts` — 钩子
- `horizontalScroll.ts` — 水平滚动
- `image*.ts` — 图片处理系列
- `inProcessTeammateHelpers.ts` — InProcess 队友助手
- `iTermBackup.ts` — iTerm 备份
- `appleTerminalBackup.ts` — Apple 终端备份
- `lanBeacon.ts` — LAN 信标
- `listSessionsImpl.ts` — 会话列表实现
- `localInstaller.ts` — 本地安装器
- `localValidate.ts` — 本地验证
- `logoV2Utils.ts` — Logo 工具
- `mailbox.ts` — 邮箱
- `managedEnv*.ts` — 托管环境
- `memory/**` — 记忆系统
- `memoryFileDetection.ts` — 记忆文件检测
- `messagePredicates.ts` — 消息谓词
- `messageQueueManager.ts` — 消息队列管理
- `messages/**` — 消息映射
- `nativeInstaller/**` — 原生安装器
- `powershell/**` — PowerShell 工具
- `sandbox/**` — 沙箱 (已有 cc-sandbox)
- `secureStorage/**` — 安全存储
- `sessionState.ts` — 会话状态
- `settings/**` — 设置 (已有 cc-config)
- `shell/readOnlyCommandValidation.ts` — 只读命令验证
- `shell/prefix.ts` — 命令前缀提取
- `sinks.ts` — 接收器
- `slashCommandParsing.ts` — 斜杠命令解析
- `slowOperations.ts` — 慢操作检测
- `standaloneAgent.ts` — 独立 Agent
- `streamJsonStdoutGuard.ts` — Stream JSON stdout 保护
- `streamlinedTransform.ts` — 流式转换
- `suggestions/**` — 建议系统 (完全缺失)
- `swarm/**` — Swarm 系统 (部分移植)
- `task/**` — 任务系统 (已有 cc-tasks)
- `taskStateMessage.ts` — 任务状态消息
- `taskSummary.ts` — 任务摘要
- `team*.ts` — 团队相关
- `telemetry/**` — 遥测 (部分移植)
- `telemetryAttributes.ts` — 遥测属性
- `teleport/**` — Teleport 远程执行
- `terminalPanel.ts` — 终端面板
- `todo/**` — TODO 类型
- `toolErrors.ts` — 工具错误
- `toolPool.ts` — 工具池
- `toolResultStorage.ts` — 工具结果存储
- `toolSchemaCache.ts` — 工具 Schema 缓存
- `transcriptSearch.ts` — 转录搜索
- `tmuxSocket.ts` — Tmux Socket
- `uds*.ts` — Unix Domain Socket 系列
- `ultraplan/**` — Ultraplan
- `unaryLogging.ts` — 一元日志
- `undercover.ts` — 隐藏模式
- `user.ts` — 用户
- `userAgent.ts` — User Agent
- `userPromptKeywords.ts` — 用户提示关键词
- `atomic.js` — 原子操作

## 移植优先级建议

**P0 (核心功能依赖):**
- errors.ts, debug.ts, hash.ts, uuid.ts 等基础工具 (几乎所有模块依赖)
- file.ts, fileRead.ts, glob.ts (文件操作工具链)
- env.ts, envUtils.ts (环境变量)

**P1 (用户体验):**
- format.ts, stringUtils.ts, markdown.ts
- terminal.ts, ansiToPng.ts (终端渲染)
- sleep.ts, signal.ts (进程控制)

**P2 (补充完善):**
- stream.ts, json.ts (数据处理)
- http.ts, api.ts (网络请求)
- 其余工具函数

**待定 (Rust 生态已有更好替代):**
- semaphore.ts → tokio::sync::Semaphore
- CircularBuffer.ts → ringbuf crate
- memoize.ts → cached crate
- diff.ts → similar crate
