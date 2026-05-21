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

### cc-plugins/ (完整度 ~60%)
- **manifest**: 完整 `plugin.json` 模式 (`PluginManifest`)
- **loader**: 从 `installed_plugins.json` 加载 (V1/V2 兼容)
- **refresh**: 热重载插件注册表，事件驱动
- **tools**: `PluginToolWrapper` — 插件工具包装器 (子进程执行)
- **installation**: 插件安装、更新、卸载（支持 marketplace/URL/GitHub/npm/file 来源）
- **marketplace**: 多来源市场管理、本地缓存、市场发现
- **validation**: 插件清单和目录结构验证
- **lsp**: LSP 服务器声明收集
- **blocklist/policy**: 插件块列表和策略管理
- **commands/agents/hooks/output_styles**: 声明式命令、Agent、Hook、输出样式加载
- **mcpb**: MCP Bundle 二进制插件格式支持
- **dependency_resolver/reconciler/autoupdate/versioning/zip_cache**: 依赖解析、状态同步、自动更新、版本管理、ZIP 缓存
- 事件系统: `PluginSubsystemEvent` (Reloaded, RefreshNeeded, StatusChanged, Installed, Updated, Uninstalled, ValidationFailed, ConfigChanged)

### IPC 集成 (Phase 1+2)
- `PluginSubsystemEvent` → IPC `PluginEvent` 映射（全 8 变体）
- `PluginCommandRuntime` 扩展了 `install_plugin`/`list_marketplace`/`refresh_marketplace_cache`/`update_plugin`/`validate_plugin`/`get_plugin_info`
- 插件命令注册到 `DynamicRegistry`（优先级 40）
- LSP 推荐 "yes" 决策触发真实插件安装

## Rust 缺失的主要功能

### 1. MCPB Handler (966 行) — 部分实现
- MCP Bundle 二进制插件格式
- 下载、解压、清单提取
- 用户配置输入处理
- 内容哈希和缓存

### 2. LSP 插件集成和推荐 — 部分实现
- `lspPluginIntegration.ts`: LSP 服务器配置（`lsp.rs` 已实现）
- `lspRecommendation.ts`: 基于使用的推荐（IPC `LspRecommendations` 已添加，推荐安装逻辑已接入）

### 3. 插件生命周期功能 — 部分实现
- `pluginAutoupdate.ts`: 自动更新调度和检查
- `pluginVersioning.ts`: 版本兼容性检查
- `zipCache.ts`: ZIP 下载缓存
- `reconciler.ts`: 状态同步

### 4. 遥测和统计 — 缺失
- `installCounts.ts`: 安装计数
- `fetchTelemetry.ts`: 遥测获取
- `hintRecommendation.ts`: 推荐提示

## 关键差异总结

| 功能 | Bun | Rust | 差距 |
|------|-----|------|------|
| Plugin manifest schema | Zod schemas (1,683 行) | PluginManifest | ✅ 已移植 |
| 插件加载 | pluginLoader.ts (3,305 行) | loader.rs | ✅ 已移植 |
| 热重载 | refresh.ts | refresh.rs | ✅ 已移植 |
| 工具包装器 | 直接执行 JS | PluginToolWrapper | ⚠️ 仅子进程 |
| **Marketplace** | **完整市场 (2,643 行)** | **marketplace.rs** | **✅ 已移植** |
| **插件安装/更新** | **pluginInstallationHelpers** | **installation.rs** | **✅ 已移植** |
| **插件验证** | **validatePlugin (903 行)** | **validation.rs** | **✅ 已移植** |
| **命令/Agent/Hook 加载** | **3 文件 ~1,600 行** | **commands.rs/agents.rs/hooks.rs** | **✅ 已移植** |
| **策略/块列表** | **~5 文件** | **blocklist.rs/policy.rs** | **✅ 已移植** |
| **MCPB Handler** | **完整处理 (966 行)** | **mcpb.rs** | **⚠️ 部分实现** |
| **MCP 插件集成** | **mcpPluginIntegration (634 行)** | **mcpb.rs** | **⚠️ 部分实现** |
| 自动更新 | pluginAutoupdate.ts | autoupdate.rs | ⚠️ 部分实现 |
| LSP 集成 | lspPluginIntegration.ts | lsp.rs | ✅ 已移植 |
| 遥测/统计 | 3 文件 ~500 行 | 无 | 缺失 |

## 插件运行时差异

Bun 版本可以直接运行 JavaScript 插件，因为 Bun 原生支持 JS/TS。  
Rust 版本只能通过子进程执行插件工具，需要已安装的二进制文件。
