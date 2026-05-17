# Rust UI 奇偶性分析 — Great 目录

> **目的**: 逐项对比 Rust (ratatui/OpenTUI) 与 TypeScript (Ink/React) 的 UI 实现，
> 识别薄弱环节与待改进项，为功能补齐提供 roadmap。
>
> 生成日期: 2026-05-17

## 文档索引

| # | 文档 | 行数 | 覆盖范围 |
|---|------|------|----------|
| 01 | [消息渲染对比](01_message_rendering_comparison.md) | 872 | ~35 个消息渲染器 (`render_*`)，从 `user_agent_notification_message`（2 行 stub）到 `user_bash_output_message`（完整 5/5） |
| 02 | [组件奇偶性对比](02_component_parity_comparison.md) | 600 | 25 个 UI 组件对比（Design System、Dialog、StatusIcon、Tabs、Welcome、Approval 等） |
| 03 | [渲染子系统对比](03_rendering_subsystem_comparison.md) | 371 | Markdown、Theme、ProgressBar、Spinner、Shimmer、Diff、Syntax Highlighting、Virtual Scroll |
| 04 | [权限与输入对比](04_permission_input_comparison.md) | 631 | Permission 系统、Dialog/Overlay、Input、Vim、Command Palette、Keybinding、Event Router |
| 05 | [应用壳对比](05_app_shell_comparison.md) | 501 | 主循环、Status Line、Voice、Agent Navigation、Notification、Transcript、Scroll、Frame Scheduling |

## 整体评分

| 子系统 | Rust 完整度 | 关键缺失 |
|--------|:----------:|----------|
| **消息渲染** | 2.1/5 | 31/35 渲染器是纯文本 `format!()` stub，忽略 theme 参数 |
| **UI 组件** | 1.8/5 | 设计系统全缺失（Dialog/Divider/Pane/ThemedBox/FuzzyPicker 等共 25 个） |
| **渲染子系统** | 3.2/5 | 无语法高亮（0/5）、Markdown 缺表格（2/5）、主题静态无切换 |
| **权限与输入** | 2.7/5 | 单一权限弹窗 vs TS 15+ 专用组件；输入框仅单行；对话系统全缺失 |
| **应用壳** | 3.5/5 | 主循环/事件/框架调度强，但 Agent 导航未接线、无终端内通知队列 |

## 优先改进项 (P0)

1. **语法高亮 (0/5)** — 代码块纯文本展示，缺少 WASM Shiki 支持。影响所有含代码的消息
2. **Markdown 表格 (2/5)** — LLM 频繁输出表格，Rust 无表格渲染（宽度计算、对齐、ANSI-aware 换行）
3. **Dialog/Overlay 系统 (1/5)** — TS 有 40+ 对话框组件，Rust 只有一个 permission overlay
4. **设计系统组件 (1/5)** — ThemedBox、ThemedText、Dialog、Pane 等基础原子组件全部缺失
5. **骨架消息渲染器 (1/5)** — 31 个 `render_*` 函数仅输出 `format!("{type}: {content}")`，未使用 theme 参数
6. **多行输入 (2/5)** — Rust PromptInput 仅单行，TS 约 3000 行支持多行/图片粘贴/撤销/高亮/补全
7. **按工具分类的权限弹窗 (2/5)** — Rust 一个通用弹窗 vs TS 15+ 工具专用权限请求组件
8. **终端内通知队列 (1/5)** — Rust 无通知队列/优先级系统/专属通知区域
