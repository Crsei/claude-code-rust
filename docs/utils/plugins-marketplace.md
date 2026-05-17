# 插件系统移植缺口

对应: `claude-code-bun/src/utils/plugins/`  
Rust 对应: `cc-plugins/`

## Bun 端规模

| 文件 | 行数 | 说明 |
|------|------|------|
| pluginLoader.ts | 3,305 | 核心插件加载器 |
| marketplaceManager.ts | 2,643 | 市场管理 |
| installedPluginsManager.ts | 1,268 | 安装元数据管理 |
| schemas.ts | 1,683 | Zod 模式定义 |
| validatePlugin.ts | 903 | 插件验证 |
| mcpbHandler.ts | 966 | MCPB 二进制插件格式 |
| loadPluginCommands.ts | 946 | 命令加载 |
| loadPluginAgents.ts | 348 | Agent 定义加载 |
| loadPluginHooks.ts | 287 | Hook 加载 |
| loadPluginOutputStyles.ts | 178 | 输出样式加载 |
| marketplaceHelpers.ts | 592 | 市场工具函数 |
| mcpPluginIntegration.ts | 634 | MCP 服务器集成 |
| pluginInstallationHelpers.ts | 600 | 安装工具 |
| dependencyResolver.ts | 305 | 依赖解析 |
| reconciler.ts | 265 | 状态同步 |
| refresh.ts | 223 | 刷新逻辑 |
| pluginDirectories.ts | 178 | 路径解析 |
| pluginIdentifier.ts | 123 | 标识符解析 |
| zipCache.ts + zipCacheAdapters.ts | 406+ | ZIP 缓存 |
| pluginAutoupdate.ts | 278 | 自动更新 |
| ...其他 15+ 文件 | ~2,000+ | 策略、块列表、遥测等 |
| **合计** | **~20,468** | |

## Rust 已实现

### cc-plugins/ (完整度 ~40%)
- **manifest**: 完整 `plugin.json` 模式 (`PluginManifest`)
- **loader**: 从 `installed_plugins.json` 加载 (V1/V2 兼容)
- **refresh**: 热重载插件注册表，事件驱动
- **tools**: `PluginToolWrapper` — 插件工具包装器 (子进程执行)
- 事件系统: `PluginSubsystemEvent` (Reloaded, RefreshNeeded, StatusChanged)

## Rust 缺失的主要功能

### 1. Marketplace 管理 (2,643 行) — 完全缺失
- 多来源市场: URL、GitHub、npm、file
- 本地缓存管理
- 安装/更新/删除
- 自动更新
- 策略、块列表/允许列表
- GCS 启动检查
- 市场发现和浏览

### 2. MCPB Handler (966 行) — 完全缺失
- MCP Bundle 二进制插件格式
- 下载、解压、清单提取
- 用户配置输入处理
- 内容哈希和缓存

### 3. 命令/Agent/Hook/输出样式加载 (~1,800 行) — 完全缺失
- `loadPluginCommands.ts`: 插件命令加载、Markdown frontmatter 解析
- `loadPluginAgents.ts`: Agent 定义加载
- `loadPluginHooks.ts`: Hook 配置加载
- `loadPluginOutputStyles.ts`: 输出样式加载

### 4. 插件验证 (903 行) — 缺失
- 清单字段验证
- 目录结构检查
- 文件类型验证
- YAML frontmatter 验证

### 5. 安装管理 (1,268 行) — 缺失
- `installed_plugins.json` V1→V2 迁移
- 安装追踪 (全局)
- 作用域管理 (user/project/local)
- Git 感知的检出路径检测

### 6. MCPB 插件集成 (634 行) — 缺失
- MCP 服务器配置
- 集成到 MCP 管理器

### 7. LSP 插件集成和推荐 — 缺失
- `lspPluginIntegration.ts`: LSP 服务器配置
- `lspRecommendation.ts`: 基于使用的推荐

### 8. 插件生命周期功能 — 缺失
- `pluginAutoupdate.ts`: 自动更新调度和检查
- `pluginVersioning.ts`: 版本兼容性检查
- `zipCache.ts`: ZIP 下载缓存
- `reconciler.ts`: 状态同步

### 9. 策略和安全 — 缺失
- `pluginBlocklist.ts`: 插件块列表
- `pluginFlagging.ts`: 插件标记
- `pluginPolicy.ts`: 策略管理
- `orphanedPluginFilter.ts`: 孤立插件过滤

### 10. 遥测和统计 — 缺失
- `installCounts.ts`: 安装计数
- `fetchTelemetry.ts`: 遥测获取
- `hintRecommendation.ts`: 推荐提示

## 关键差异总结

| 功能 | Bun | Rust | 差距 |
|------|-----|------|------|
| Plugin manifest schema | Zod schemas (1,683 行) | PluginManifest | ✅ 已移植 |
| 插件加载 | pluginLoader.ts (3,305 行) | loader.rs | ✅ 部分移植 |
| 热重载 | refresh.ts | refresh.rs | ✅ 已移植 |
| 工具包装器 | 直接执行 JS | PluginToolWrapper | ⚠️ 仅子进程 |
| **Marketplace** | **完整市场 (2,643 行)** | **无** | **核心缺口** |
| **MCPB Handler** | **完整处理 (966 行)** | **无** | **核心缺口** |
| **命令加载** | **loadPluginCommands (946 行)** | **无** | **功能缺口** |
| **Agent 加载** | **loadPluginAgents (348 行)** | **无** | **功能缺口** |
| **插件验证** | **validatePlugin (903 行)** | **无** | **功能缺口** |
| **安装管理** | **~1,200 行** | **无** | **功能缺口** |
| 自动更新 | pluginAutoupdate.ts | 无 | 功能缺口 |
| LSP 集成 | lspPluginIntegration.ts | 无 | 功能缺口 |
| 策略/块列表 | ~5 文件 | 无 | 安全缺口 |

## 插件运行时差异

Bun 版本可以直接运行 JavaScript 插件，因为 Bun 原生支持 JS/TS。  
Rust 版本只能通过子进程执行插件工具，需要已安装的二进制文件。
