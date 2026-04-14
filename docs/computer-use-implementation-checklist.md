# Rust 版 Computer Use 落地清单

## 1. 目标

在 `rust/` 中落地可用的 Computer Use 能力，使模型能够：

- 主动调用桌面控制相关工具
- 获取截图并将截图继续回喂给模型
- 在同一轮 agent loop 中完成“看屏幕 -> 决策 -> 点击/输入 -> 再看屏幕”
- 在 macOS / Windows / Linux 上逐步具备可验证能力

本清单以当前 Rust-lite 代码结构为准，不假设要一比一复制 TypeScript 版本的内部实现。

## 2. 当前结论

### 2.1 已有基础

- 已有 MCP 客户端、发现、连接、工具包装能力，可连接外部 MCP server。
  - 见 [src/main.rs](../src/main.rs)、[src/mcp/manager.rs](../src/mcp/manager.rs)、[src/mcp/tools.rs](../src/mcp/tools.rs)
- 已有 `ContentBlock::Image`、`ToolResultContent::Blocks` 类型定义，消息模型允许保存图片块。
  - 见 [src/types/message.rs](../src/types/message.rs)
- 已有权限模式、规则系统、交互式 permission callback。
  - 见 [src/types/tool.rs](../src/types/tool.rs)、[src/permissions/decision.rs](../src/permissions/decision.rs)

### 2.2 当前硬阻塞

- MCP 图片结果会在 wrapper 层被压扁成字符串 `"[Image: image/png]"`，图像数据没有进入消息历史。
  - 见 [src/mcp/tools.rs](../src/mcp/tools.rs)
- query loop 会把所有工具结果强制转成 `ToolResultContent::Text(...)`，不会保留 `Blocks([Image])`。
  - 见 [src/query/loop_impl.rs](../src/query/loop_impl.rs)
- 权限 matcher 只认识 `Bash / Read / Write / Edit / Glob`，对 Computer Use 工具没有专门规则入口。
  - 见 [src/permissions/decision.rs](../src/permissions/decision.rs)
- CLI 没有 `--computer-use-mcp` 快速路径；当前 `mcp/` 目录主要是客户端，不是内置 MCP server 宿主。
  - 见 [src/main.rs](../src/main.rs)、[src/mcp/](../src/mcp/)

## 3. 推荐路线

### 结论

推荐分两步走：

1. **先做“外置 Computer Use MCP + Rust 侧图像保真传输” MVP**
2. **再决定是否需要“内置 `--computer-use-mcp` + 原生平台 backend”**

原因：

- Rust 侧已经有 MCP client，最短路径不是重写一整套 darwin/win32/linux backend，而是先把现成 `computer-use-mcp` 接进来。
- 真正的关键门槛不是“有没有 MCP”，而是“截图能不能作为图片 tool_result 继续进入下一轮模型上下文”。
- 如果这条图像链路没打通，即使接上外部 Computer Use MCP，模型也只能收到字符串占位符，无法完成闭环。

## 4. 落地清单

### Phase 0：打通多模态工具结果链路

这是 **第一优先级**。不完成这一层，Computer Use 只能“调用工具”，不能“看见截图”。

- [x] 在 [src/mcp/tools.rs](../src/mcp/tools.rs) 中新增”结构化 MCP 结果转换”逻辑。
  - 目标：把 `ToolCallContent::Image { data, mime_type }` 转成 `ContentBlock::Image { source }`
  - 不再只通过 `format_tool_call_result()` 返回字符串
- [x] 改造 [src/types/tool.rs](../src/types/tool.rs) 的 `ToolResult`
  - 建议新增结构化字段，例如：
  - `model_content: Option<ToolResultContent>`
  - `display_preview: Option<String>`
  - 保留现有 `data: Value` 仅用于兼容和日志，不要再让它承担全部语义
- [x] 改造 [src/query/loop_impl.rs](../src/query/loop_impl.rs)
  - 目标：当工具返回图片块或混合块时，生成 `ContentBlock::ToolResult { content: ToolResultContent::Blocks(...) }`
  - 只有纯文本结果才走 `ToolResultContent::Text(...)`
- [ ] 为 [src/session/storage.rs](../src/session/storage.rs) 和导出模块补充回归验证
  - 确认 `Blocks([Image])` 在会话保存、恢复、导出时不丢失
- [x] 为 headless/TUI 增加最小可见性
  - [src/ipc/protocol.rs](../src/ipc/protocol.rs)：`ToolResult` 消息增加可选 `content_blocks` 和 `ToolResultContentInfo` 类型
  - [src/ui/messages.rs](../src/ui/messages.rs)：图片型 tool result 渲染为 `[image: mime_type]`
  - [src/ipc/headless.rs](../src/ipc/headless.rs)：`extract_tool_result_output()` 正确提取图片占位符

**Phase 0 验收标准**

- fake MCP server 返回图片时，query loop 生成的工具结果是 `Blocks([Image])`，不是字符串占位符
- 下一轮模型请求中能带上图片内容块
- 会话保存/恢复后图片块仍存在

### Phase 1：接通外置 Computer Use MCP MVP

这一阶段不追求内置 server，对标“先可用”。

- [ ] 增加一份 Rust 侧 Computer Use MCP 接入文档
  - 推荐新增 `docs/reference/computer-use-mcp-config.md`
  - 内容包括 `.cc-rust/settings.json` 中 `mcpServers.computer-use` 的配置示例
- [ ] 约定保留 server 名称 `computer-use`
  - 便于权限、日志、UI 文案和后续内置化保持一致
- [ ] 为 `mcp__computer-use__*` 工具加一层识别和分组
  - 可先通过名称前缀处理，不必一开始就做复杂 capability 协商
- [ ] 做一条端到端 smoke test
  - `screenshot` 工具返回图片
  - `left_click` / `type_text` / `key` / `scroll` 等至少有一组能成功 round-trip

**建议的 MVP 截止线**

- 单平台先跑通 Windows 或 macOS
- 模型可以：
  - 调 `mcp__computer-use__screenshot`
  - 收到真实图片结果
  - 再决定调用 `click/type/key`

### Phase 2：权限模型补齐

Computer Use 不能直接复用普通 Bash/File 的权限语义，需要单独加一层“桌面控制授权”。

- [x] 扩展 [src/permissions/decision.rs](../src/permissions/decision.rs) 的 matcher
  - 支持 `mcp__computer-use__screenshot`
  - 支持 `mcp__computer-use__left_click`
  - 支持 `mcp__computer-use__type_text`
  - 支持 `mcp__computer-use__key`
  - 支持 `mcp__computer-use__scroll`
  - 支持未来原生工具名
- [x] 为 Computer Use 定义单独的危险等级
  - 截图：中风险
  - 点击/输入/快捷键：高风险
  - 打开应用、切换窗口、剪贴板读写：高风险
- [x] 在 [src/types/tool.rs](../src/types/tool.rs) / permission context 中增加”会话级授权”概念
  - 避免每次点击都弹权限框
  - 建议首次调用 `request_access` 后缓存到 session 级
- [ ] 新增 OS 能力检查模块
  - 建议路径：`src/computer_use/host_adapter.rs`
  - macOS：检查 Accessibility / Screen Recording
  - Windows：先允许，但要检测必要 API/PowerShell 可用性
  - Linux：检查 `xdotool` / `scrot` / `xclip` / `wmctrl`
- [x] 权限 UI 文案要单独写
  - 不能只显示”允许调用工具”
  - 应明确说明”允许读取屏幕并控制键盘鼠标”

**Phase 2 验收标准**

- 首次 Computer Use 调用会触发明确授权
- 同会话内后续调用不重复骚扰
- 权限拒绝后能稳定回退，不进入死循环

### Phase 3：内置 `--computer-use-mcp` 快速路径（可选）

只有在外置 MCP MVP 稳定后，才建议做这一层。

原因：

- 当前 Rust 项目已有 MCP client，但没有 TypeScript 那种现成的“内置 MCP server 快速路径”
- 这一步会把范围从“接入能力”扩大到“宿主能力 + server 生命周期管理”

- [ ] 在 [src/main.rs](../src/main.rs) 增加 `--computer-use-mcp` fast path
- [ ] 新增 `src/computer_use/mcp_server.rs`
  - 承担 `tools/list`、`tools/call`、server metadata、生命周期处理
- [ ] 新增 `src/computer_use/setup.rs`
  - 用于把内置 server 动态注册为保留 MCP 服务
- [ ] 决定是否要支持与 TS 同名工具集合
  - 建议保持工具名兼容，减少 prompt 和 skill 偏差

**是否必须做**

- 如果目标只是“Rust CLI 可以使用 Computer Use”，这一步不是必须
- 如果目标是“与 TS 版功能入口一致”，这一步是必须

### Phase 4：原生平台 backend（可选，成本最高）

只有在以下场景才建议做：

- 不想依赖外部 `computer-use-mcp`
- 希望把平台差异完全收回 Rust 仓库
- 需要比现成 MCP 更细粒度的性能/权限控制

建议新增模块：

- [ ] `src/computer_use/mod.rs`
- [ ] `src/computer_use/input/mod.rs`
- [ ] `src/computer_use/input/darwin.rs`
- [ ] `src/computer_use/input/win32.rs`
- [ ] `src/computer_use/input/linux.rs`
- [ ] `src/computer_use/screenshot/mod.rs`
- [ ] `src/computer_use/screenshot/darwin.rs`
- [ ] `src/computer_use/screenshot/win32.rs`
- [ ] `src/computer_use/screenshot/linux.rs`
- [ ] `src/computer_use/executor.rs`
- [ ] `src/computer_use/drain_run_loop.rs`
- [ ] `src/computer_use/esc_hotkey.rs`

平台建议：

- macOS
  - 输入：`CGEvent` / `osascript`
  - 截图：`screencapture`
  - 权限：TCC
- Windows
  - 输入：WinAPI / PowerShell fallback
  - 截图：GDI / PowerShell
  - 应用管理：EnumWindows / SetForegroundWindow
- Linux
  - 输入：`xdotool`
  - 截图：`scrot` 或 `grim`
  - 窗口管理：`wmctrl`
  - 剪贴板：`xclip`
  - 初期先支持 X11，Wayland 单列 todo

### Phase 5：会话、导出、前端体验补齐

- [ ] [src/session/export.rs](../src/session/export.rs) / `session_export/*`
  - 为图片型 tool result 增加更好的导出文本
  - 不要只导出空白或 `[object Object]`
- [ ] [src/ipc/protocol.rs](../src/ipc/protocol.rs)
  - `ToolResult` 增加可选 `content_blocks`
  - headless 前端可以直接展示图片结果
- [ ] [src/ui/messages.rs](../src/ui/messages.rs)
  - 把 Computer Use 工具调用做更清晰的呈现
  - 区分截图、点击、输入、权限请求、失败重试
- [ ] 如果有独立前端 `ui/`
  - 增加图片型 tool result 的展示占位
  - 至少能在历史中看出“本轮看到了哪张图”

### Phase 6：测试清单

- [ ] 单元测试：MCP `image` 结果转换为 `ContentBlock::Image`
- [ ] 单元测试：query loop 对结构化 tool result 不再强制字符串化
- [ ] 单元测试：session storage/export 对图片块的保存与恢复
- [ ] 集成测试：fake MCP server 返回 screenshot 图片，模型收到图片块
- [ ] 集成测试：权限拒绝 / 允许 / session grant 三种路径
- [ ] 平台 smoke test：
  - macOS：截图 + 权限检测
  - Windows：截图 + 点击
  - Linux：截图 + `xdotool` 检测

## 5. 文件级改动建议

### 必改

- [src/mcp/tools.rs](../src/mcp/tools.rs)
- [src/types/tool.rs](../src/types/tool.rs)
- [src/query/loop_impl.rs](../src/query/loop_impl.rs)
- [src/permissions/decision.rs](../src/permissions/decision.rs)

### 高概率会改

- [src/ipc/protocol.rs](../src/ipc/protocol.rs)
- [src/ui/messages.rs](../src/ui/messages.rs)
- [src/session/storage.rs](../src/session/storage.rs)
- [src/session/export.rs](../src/session/export.rs)

### 可后置

- [src/main.rs](../src/main.rs) 的 `--computer-use-mcp` fast path
- `src/computer_use/*` 原生 backend 全家桶

## 6. 建议执行顺序

### 里程碑 A：图像 tool_result 打通

- [x] 改 `ToolResult` 结构
- [x] 改 `mcp/tools.rs`
- [x] 改 `query/loop_impl.rs`
- [x] 加 fake MCP image 测试

### 里程碑 B：外置 MCP MVP

- [ ] 写 `computer-use` MCP 配置文档
- [ ] 跑通 `screenshot -> image tool_result -> next turn`
- [ ] 跑通一个输入动作

### 里程碑 C：权限与 UX

- [x] session-level request_access
- [x] 权限 matcher / dangerous 分类
- [x] TUI / headless 显示优化

### 里程碑 D：内置化或原生化

- [ ] `--computer-use-mcp`
- [ ] 动态注册
- [ ] 原生平台 backend

## 7. 不建议一开始做的事

- 不要一上来重写完整 darwin/win32/linux backend
- 不要先做复杂前端渲染，再补模型图像链路
- 不要把 MCP 图片继续压成文本占位后再做“Computer Use 验证”
- 不要把权限复用成普通 Bash 权限，这会导致风险边界不清

## 8. MVP 定义

满足以下条件即可认为 Rust 版 Computer Use 进入 MVP：

- 模型能调用 `computer-use` 相关 MCP 工具
- `screenshot` 返回的图片能作为图片块进入下一轮上下文
- 至少一个平台上能完成：
  - 截图
  - 点击
  - 输入
  - 再截图
- 首次调用有明确授权提示
- 会话保存/恢复不会破坏图片型 tool result

## 9. 一句话决策

**先补“图片型 tool_result 贯通”再接外置 Computer Use MCP；内置 `--computer-use-mcp` 和原生平台 backend 放在第二阶段。**
