## 执行计划

### 入生产路径分析

| # | 项 | 生产入口判断 | 操作 |
|---|---|---|---|
| 1 | `terminal_env.rs::DISABLE_MOUSE_RUNTIME_SUPPORTED` 常量 | ❌ **保留** — 测试中禁用的运行时标志 | 保持 `#[cfg(test)]` |
| 2 | `terminal_env.rs::EditorCommand` 结构体+impl | ❌ **保留** — 编辑器命令解析在测试中模拟 | 保持 `#[cfg(test)]` |
| 3 | `terminal_env.rs::parse_editor_command()` | ❌ **保留** — 测试辅助 | 保持 `#[cfg(test)]` |

### 需要修改的生产文件
无 — 全部保留。
