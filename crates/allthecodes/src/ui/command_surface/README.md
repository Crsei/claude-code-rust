## 执行计划

### 入生产路径分析

| # | 项 | 生产入口判断 | 操作 |
|---|---|---|---|
| 1 | `adapters/hooks.rs::hook_summary()` | ❌ **保留** — `HookConfigSummary` 仅用于快照测试展示Hooks配置摘要 | 保持 `#[cfg(test)]` |
| 2 | `surfaces/remote.rs` 测试辅助函数/结构体 | ❌ **保留** — `key()`, `snapshot()`, `EnvGuard`, `request()` 都是测试基础设施 | 保持 `#[cfg(test)]` |
| 3 | `tests.rs` 辅助函数 | ❌ **保留** — 整个文件是测试模块 | 保持 `#[cfg(test)]` |

### 需要修改的生产文件
无 — 全部保留。
