# `runtime/` — cfg(test) Audit

除 test 函数以外的 `#[cfg(test)]` 使用情况：

> 本目录下所有含 `#[cfg(test)]` 的文件均在 `#[cfg(test)] mod tests` 块中包含 `use` 导入（标准 Rust 测试模块惯例），但无在 `mod tests` 之外使用 `#[cfg(test)]` 的独立项。

## 各文件 `mod tests` 内的 `use` 导入

| 文件 | 非测试项 |
|---|---|
| `transcript.rs` | `use super::*;` + 多个 `use cc_types::...` + 4 个辅助函数（`user()`, `assistant()`, `system()`, `attachment()`）|
| `persistent_history.rs` | `use super::*;` + `use cc_types::...` + `use uuid::...` |
| 其他文件 | `use super::*;` |

以上辅助函数均位于 `mod tests` 内部，属于测试模块惯例。
