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

### cc-config/ (完整度 ~90%)
- `settings.rs`: 多源设置 (user/project/local/flag/policy)
- `features.rs`: 功能开关
- `claude_md.rs`: CLAUDE.md 配置
- `constants.rs`: 常量和路径
- `paths.rs`: 路径解析
- `runtime_settings.rs`: 运行时设置
- `user_agent.rs`: User Agent
- `validation.rs`: 验证

## Rust 缺失的主要功能

### 1. MDM 企业设置 (316+129+81 行) — 完全缺失
- MDM 策略管理和读取
- macOS 配置描述文件的策略读取 (`mdm/rawRead.ts`)
- 企业部署支持
- **Rust 缺少对管理设备的 MDM 策略集成**

### 2. 变更检测 (488 行) — 缺失
- 使用 chokidar 的文件监控
- 防抖重载
- 设置合并失效
- 启动文件扫描

### 3. 权限验证 (411 行) — 缺失
- 设置中的权限规则验证
- 跨来源一致性检查

### 4. 验证建议 (164 行) — 缺失
- 格式良好验证提示
- 常见问题解决建议

### 5. 工具验证配置 (103+45 行) — 缺失
- 每个工具的独立验证配置
- 编辑工具特殊处理

### 6. 变更应用 (92 行) — 缺失
- 单个设置变更的应用逻辑

### 7. 内部写入检测 (37 行) — 缺失
- 避免反馈循环的内部写检测

## 关键差异总结

| 功能 | Bun | Rust | 差距 |
|------|-----|------|------|
| 多源设置 | 完整 (5 来源) | settings.rs | ✅ 已移植 |
| 设置类型 | Zod schemas | Rust structs | ✅ 已移植 |
| Feature flags | features.rs | ✅ 已移植 |
| CLAUDE.md | claude_md.rs | ✅ 已移植 |
| 路径管理 | paths.rs | ✅ 已移植 |
| 运行时设置 | runtime_settings.rs | ✅ 已移植 |
| 验证 | validation.rs | ✅ 部分移植 |
| **MDM 企业设置** | **mdm/ (526 行)** | **无** | **企业缺口** |
| **变更检测** | **changeDetector.ts (488 行)** | **无** | **UX 降级** |
| **权限验证** | **permissionValidation.ts (411 行)** | **无** | **安全缺口** |
| 验证建议 | validationTips.ts | 无 | UX 降级 |
