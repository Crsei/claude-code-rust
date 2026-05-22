# UI 面板截断 / 省略展示汇总

> 记录时间：2026-05-22  
> 来源分析：Rust TUI (claude-code-rs/src/ui/) 代码审计

本文档列出所有 UI 面板中因空间限制而截断/省略展示的内容，按渲染位置分类。

---

## 一、对话记录（消息区域，内联展示）

这些截断发生在主消息流中，直接可见于聊天历史。

### 1.1 助手思考预览

| 项目 | 说明 |
|------|------|
| 文件 | `crates/claude-code-rs/src/ui/messages/assistant_thinking_message.rs:37` |
| 截断方式 | `.chars().take(120)`，仅显示前 120 字符 |
| 展示位置 | 助手消息的思考过程块内 |

### 1.2 助手文本（API 错误）

| 项目 | 说明 |
|------|------|
| 文件 | `crates/claude-code-rs/src/ui/messages/assistant_text_message.rs:166-168` |
| 截断方式 | 手动 `&s[..997]` + `"..."`，上限 1000 字符 |
| 展示位置 | API 错误文本块内 |

### 1.3 工具调用结果

| 项目 | 说明 |
|------|------|
| 文件 | `crates/claude-code-rs/src/ui/messages/render.rs:1851-1871` |
| 截断方式 | `.take(5)`，仅显示前 5 行，剩余显示 `"... {} more lines"` |
| 展示位置 | 工具返回内容块内 |

### 1.4 工具输入 JSON

| 项目 | 说明 |
|------|------|
| 文件 | `crates/claude-code-rs/src/ui/messages/render.rs:2120-2141` |
| 截断方式 | `abbreviate_json()` — 递归截断至 `max_chars`，末尾加 `...` |
| 展示位置 | 工具调用行摘要中（如 `"● Bash(cargo test)"` 旁） |

### 1.5 Bash 输出行

| 项目 | 说明 |
|------|------|
| 文件 | `crates/claude-code-rs/src/ui/messages/user_bash_output_message.rs:71, 160-176` |
| 截断方式 | `truncate_to_width()` — 按 unicode 显示宽度 silent 截断，不加省略号 |
| 展示位置 | bash 输出消息块内 |

### 1.6 附件文件

| 项目 | 说明 |
|------|------|
| 文件 | `crates/claude-code-rs/src/ui/messages/attachment_message.rs:19, 359-364` |
| 截断方式 | 显示 `(truncated)` 标记 |
| 展示位置 | 文件附件块内 |

### 1.7 工具活动摘要

| 项目 | 说明 |
|------|------|
| 文件 | `crates/claude-code-rs/src/ui/rendering/tool_activity.rs:311, 328, 334` |
| 截断方式 | 工具输入摘要截断至 96 字符；JSON 值截断至 48 字符 |
| 展示位置 | 助手消息中合并后的工具活动行 |

### 1.8 欢迎界面（消息区域空状态）

| 项目 | 说明 |
|------|------|
| 文件 | `crates/claude-code-rs/src/ui/components/welcome.rs:62-63, 107-135` |
| 截断方式 | CWD 路径从开头截断（`truncate_start`）；提示文字末尾截断（`truncate_str`） |
| 展示位置 | 无对话时覆盖消息区域 |

### 1.9 转录模式消息限制

| 项目 | 说明 |
|------|------|
| 文件 | `crates/claude-code-rs/src/ui/messages/render.rs:645-658` |
| 截断方式 | `truncate_transcript_messages()` — 仅保留最近 30 条消息 |
| 展示位置 | Transcript view 模式下（`Ctrl+O` 切换） |

---

## 二、专门面板（覆盖层 / 独立视图）

这些截断发生在用户主动打开的面板或系统自动弹出的对话框中。

### 2.1 Diff 面板

| 项目 | 说明 |
|------|------|
| 触发 | `/diff` 命令 |
| 文件 | `crates/claude-code-rs/src/ui/diff/diff_detail_view.rs:10, 23, 34-35, 69-70` |
| 截断方式 | 超过 400 行截断并显示 `(diff truncated (exceeded 400 line limit))`；每行按宽度 silent 截断 |
| 文件 | `crates/claude-code-rs/src/ui/diff/diff_file_list.rs:41-45, 68` |
| 截断方式 | 文件路径从开头截断（`truncate_start_to_width`）；截断文件显示 `(truncated)` |
| 文件 | `crates/claude-code-rs/src/ui/diff/structured_diff.rs:257-476` |
| 截断方式 | 样式 spans 按显示宽度截断；超出 max_lines 的行丢弃 |

### 2.2 命令面板

| 项目 | 说明 |
|------|------|
| 触发 | `/` 快捷键 |
| 文件 | `crates/claude-code-rs/src/ui/command_palette/render.rs:85, 110, 210, 220, 228, 297-306, 346` |
| 截断方式 | 命令描述、标签、用法文本、示例均按可用宽度截断（`truncate()` + `...`） |

### 2.3 权限对话框

| 项目 | 说明 |
|------|------|
| 触发 | 工具需用户审批时自动弹出 |
| 文件 | `crates/claude-code-rs/src/ui/permissions/dialog_overlay.rs:253-326, 794, 924-935` |
| 截断方式 | 反馈文本、提示文本、正文行、警告 spans、按钮文字均用 `truncate_str()` 截断 |
| 文件 | `crates/claude-code-rs/src/ui/permissions/utils.rs:176, 185, 269-286, 305, 331` |
| 截断方式 | 请求摘要截断至 100 字符（`truncate_middle` head...tail）；路径截断至 100 字符；每行截断至 120 字符 |
| 文件 | `crates/claude-code-rs/src/ui/permissions/question_dialog.rs:104-167, 244-256` |
| 截断方式 | 问题文本、答案预览、对话框行均按字符截断 |

### 2.4 审批覆盖层

| 项目 | 说明 |
|------|------|
| 触发 | 工具执行审批流程 |
| 文件 | `crates/claude-code-rs/src/ui/components/approval_overlay.rs:137-145` |
| 截断方式 | 长文本行 `.take(width - 1)` + 单个省略号 `\u{2026}` |

### 2.5 历史搜索对话框

| 项目 | 说明 |
|------|------|
| 触发 | `Ctrl+R` |
| 文件 | `crates/claude-code-rs/src/ui/components/history_search_dialog.rs:169, 175, 213, 218, 249, 382-404` |
| 截断方式 | prompt、来源、首行按 unicode 宽度截断，过长时加 `...` |

### 2.6 命令表面系列（Command Surface 覆盖层）

所有 `/xxx` 命令均打开居中覆盖层，覆盖整个终端。

#### 2.6.1 Resume（恢复会话）

| 项目 | 说明 |
|------|------|
| 触发 | `/resume` |
| 文件 | `crates/claude-code-rs/src/ui/command_surface/surfaces/resume.rs:82, 130-137` |
| 截断方式 | 会话标题截断至 72 字符 |

#### 2.6.2 Remote（远程运行）

| 项目 | 说明 |
|------|------|
| 触发 | `/remote` |
| 文件 | `crates/claude-code-rs/src/ui/command_surface/surfaces/remote.rs:111` |
| 截断方式 | 运行 ID 截断至 26 字符（`truncate_middle` head...tail） |

#### 2.6.3 Agents（代理管理）

| 项目 | 说明 |
|------|------|
| 触发 | `/agents` |
| 文件 | `crates/claude-code-rs/src/ui/agents/utils.rs:118-139` |
| 截断方式 | 代理名称/值用 `truncate_middle()` head...tail 截断 |

### 2.7 状态栏

| 项目 | 说明 |
|------|------|
| 位置 | 底部固定区域（始终可见） |
| 文件 | `crates/claude-code-rs/src/ui/app/render.rs:1019` |
| 截断方式 | 标签文本截断至 28 字符 |

### 2.8 输入框（Chat Composer）

| 项目 | 说明 |
|------|------|
| 位置 | 底部输入区域 |
| 文件 | `crates/claude-code-rs/src/ui/components/chat_composer.rs:115-121` |
| 截断方式 | 提示行过长时 `fit_line()` 截断，加 `\u{2026}` 省略号 |

### 2.9 搜索框

| 项目 | 说明 |
|------|------|
| 位置 | 命令面板/对话框中的搜索输入 |
| 文件 | `crates/claude-code-rs/src/ui/components/search_box.rs:79, 127-139` |
| 截断方式 | 搜索文本按 unicode 宽度 silent 截断（`truncate_by_width`） |

---

## 三、工具函数清单

以下工具函数被多处复用，供参考：

| 函数 | 位置 | 行为 |
|------|------|------|
| `truncate_text(text, max_len)` | `crates/cc-utils/src/messages.rs:149-163` | 字节安全截断 + `...` |
| `truncate_start_to_width(text, max_width)` | `crates/claude-code-rs/src/ui/diff.rs:110-130` | 从开头截断，保留尾部 |
| `truncate_by_width(text, max_width)` | `crates/claude-code-rs/src/ui/diff.rs:110-130` | 按显示宽度 silent 截断 |
| `truncate_middle(value, max_chars)` | `crates/claude-code-rs/src/ui/agents/utils.rs:118-139` | head...tail 截断 |
| `truncate_middle(input, max_chars)` | `crates/claude-code-rs/src/ui/permissions/utils.rs:269-286` | head...tail 截断 |
| `truncate_str(s, max_chars)` | `crates/claude-code-rs/src/ui/permissions/dialog_overlay.rs:925-935` | 字符截断 + `...` |
| `truncate_to_width(text, max_width)` | `crates/claude-code-rs/src/ui/components/history_search_dialog.rs:382-404` | unicode 宽度截断 + `...` |
| `truncate_to_width(text, max_width)` | `crates/claude-code-rs/src/ui/messages/user_bash_output_message.rs:160-176` | unicode 宽度 silent 截断 |
| `abbreviate_json(value, max_chars)` | `crates/claude-code-rs/src/ui/messages/render.rs:2120-2141` | JSON 递归截断 |

---

## 四、观察

- **对话记录**中最明显的省略是：工具调用结果仅显示前 5 行、Bash 输出按宽度 silent 截断。
- **专门面板**中省略最突出的是：Diff 面板 400 行上限、权限对话框的 head...tail 摘要截断。
- silent 截断（不加省略号）可能让用户察觉不到内容被截断，尤其是 `user_bash_output_message.rs` 和 `diff.rs` 中的 `truncate_by_width`。
