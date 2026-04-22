# 终端前端演进说明

更新日期: 2026-04-22

## 状态

`ink-terminal` / `ink-ui` 路线已退役，不再维护。

当前唯一维护中的终端前端是：
- `ui/`：基于 `@opentui/core` + `@opentui/react` 的 OpenTUI 前端

## 保留本文件的原因

本文件不再描述当前主线架构，而是作为历史演进说明保留，供以下场景参考：
- 回溯此前的 headless IPC 设计思路
- 理解从 `ink-terminal` 迁移到 OpenTUI 的背景
- 查阅旧文档中的历史引用

## 当前结论

- `ui/src/main.tsx` 是当前终端前端入口
- `ink-ui/` 不再是支持矩阵的一部分
- `ui/ink-terminal` 不再是当前仓库主线依赖
- 新的终端前端改动应围绕 OpenTUI 路线展开

## 兼容边界

headless IPC 仍然是前后端解耦的基础：
- Rust 后端：`src/ipc/*`
- OpenTUI 前端：`ui/src/ipc/*`, `ui/src/main.tsx`

旧的 `ink-terminal` 相关内容应视为历史资料，而不是当前实现约束。
