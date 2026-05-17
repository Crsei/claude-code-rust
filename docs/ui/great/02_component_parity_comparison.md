# 组件对等性对比：Rust vs TypeScript UI

**日期**：2026-05-17  
**范围**：Ratatui 后端（`rust/crates/claude-code-rs/src/ui/`）、OpenTUI 前端（`rust/ui/`）、TypeScript 设计系统（`src/components/design-system/`）以及下游 Ink 主题组件（`claude-code-bun/packages/@ant/ink/src/theme/`）

---

## 汇总表

| 组件 | Rust ratatui | Rust OpenTUI | TS design-system | @ant/ink theme | 评级 |
|-----------|:----------:|:----------:|:--------------:|:-------------:|:------:|
| Dialog | 缺失 | 缺失 | 完整 | 完整 | 1/5 |
| Divider | 缺失 | 缺失 | 完整 | 完整 | 1/5 |
| Pane | 缺失 | 缺失 | 完整 | 完整 | 1/5 |
| ThemedBox | 缺失 | 缺失 | 完整 | 完整 | 1/5 |
| ThemedText | 缺失 | 缺失 | 完整 | 完整 | 1/5 |
| ThemeProvider | 缺失 | 缺失 | 完整 | 完整 | 1/5 |
| KeyboardShortcutHint | 缺失 | 缺失 | 完整 | 完整 | 1/5 |
| ListItem | 缺失 | 缺失 | 完整 | 完整 | 1/5 |
| ProgressBar | 缺失 | 部分 | 完整 | 完整 | 2/5 |
| Byline | 缺失 | 缺失 | 完整 | 完整 | 1/5 |
| LoadingState | 缺失 | 缺失 | 完整 | 完整 | 1/5 |
| FuzzyPicker | 缺失 | 缺失 | 完整 | 完整 | 1/5 |
| Ratchet | 缺失 | 缺失 | 完整 | 完整 | 1/5 |
| StatusIcon | 1/5 | 部分 | 完整 | 完整 | 2/5 |
| Tabs | 1/5 | 部分 | 完整 | 完整 | 3/5 |
| Spinner | 缺失 | 3/5 | 缺失 | 完整 | 不适用 |
| SearchBox | 4/5 | 缺失 | 缺失 | 完整 | 不适用 |
| 欢迎屏幕 | 4/5 | 完整 | 缺失 | 缺失 | 不适用 |
| 状态组件/状态行 | 3/5 | 完整 | 缺失 | 缺失 | 不适用 |
| 批准/权限 UI | 3/5 | 完整 | 缺失 | 缺失 | 不适用 |
| 底部面板堆栈 | 1/5 | 缺失 | 缺失 | 缺失 | 不适用 |
| 聊天组件 | 2/5 | 完整 | 缺失 | 缺失 | 不适用 |
| 分页器/滚动覆盖层 | 4/5 | 缺失 | 缺失 | 缺失 | 不适用 |
| 功能面板 | 2/5 | 完整 | 缺失 | 缺失 | 不适用 |
| 会话恢复选择器 | 2/5 | 缺失 | 缺失 | 缺失 | 不适用 |

**图例**：1=桩/缺失，2=部分状态，3=功能可用但有限，4=接近完整，5=功能完整

**补齐计划**：详见 [`plans/plan-02-design-system.md`](plans/plan-02-design-system.md)——16 个组件分 3 个冲刺实现，总工作量约 17-21 天。

---

## 第1章：设计系统原语

这些组件构成了 TypeScript 代码库中的原子 UI 词汇。它们是 Rust 实现中最基础的缺口。

### 组件：Dialog

**Rust ratatui 路径**：缺失  
**Rust OpenTUI 路径**：缺失  
**TS design-system 路径**：`src/components/design-system/Dialog.tsx`  
**Rust 完整度**：1/5

#### Rust 实现状态
两个 Rust UI 后端均不存在 Dialog 组件。最接近的类似物是批准覆盖层（`approval_overlay.rs`）中使用的 `Pane` 包装器，但它缺乏完整的 Dialog 语义。

#### 缺失功能
- 支持主题颜色的标题/副标题渲染
- 退出时 Ctrl+C/D 行为与待处理状态显示（"再按一次 X 退出"）
- 用于取消的 `confirm:no`（Esc/n）快捷键集成
- 可配置的输入指南（byline 风格的键盘提示）
- 可选边框切换（`hideBorder`）
- 用于边框显示的 `Pane` 包装
- 嵌入文本字段兼容性的 `isCancelActive` 属性

#### 影响
用户看不到一致统一的对话框容器。权限提示、设置屏幕和确认对话框在没有共享框架、键盘提示和"再按一次退出"安全网的情况下临时拼凑渲染。

> **补齐计划**: [plan-02 冲刺 2](plans/plan-02-design-system.md#冲刺-2-week-3-4-复合组件--核心增强)

---

### 组件：Divider

**Rust ratatui 路径**：缺失  
**Rust OpenTUI 路径**：缺失  
**TS design-system 路径**：`src/components/design-system/Divider.tsx`  
**Rust 完整度**：1/5

#### Rust 实现状态
任何 Rust UI 代码库中均不存在 Divider 组件。水平分隔线在需要时手动编写。

#### 缺失功能
- 主题感知颜色解析（接受 `keyof Theme`）
- 可配置宽度（默认终端宽度），填充支持
- 支持 ANSI 的居中对齐标题（`<Ansi>{title}</Ansi>`）
- 自定义重复字符（默认 `─`）
- 用于全宽计算的 `useTerminalSize()` 集成
- 针对多宽 Unicode 字符的 `stringWidth()` 正确测量

#### 影响
缺乏一致的视觉分隔符。功能面板、状态显示和设置屏幕中的区域边界缺少专业的间距。

---

### 组件：Pane

**Rust ratatui 路径**：缺失  
**Rust OpenTUI 路径**：缺失  
**TS design-system 路径**：`src/components/design-system/Pane.tsx`  
**Rust 完整度**：1/5

#### Rust 实现状态
不存在 Pane 组件。`approval_overlay.rs` 调用了 `BetterViewPanel::new()`，这是一个本地的 ratatui 实现，但不是通用 Pane。

#### 缺失功能
- 主题着色顶部分隔线（委托给 `<Divider>`）
- 模态框感知渲染（在模态框内部时跳过分隔线以避免双重边框）
- 水平填充（`paddingX=2`）
- 顶部边距（`paddingTop=1`）
- 模态框插槽内的 `flexShrink=0` 以确保高度稳定

#### 影响
斜杠命令屏幕（`/config`、`/help`、`/plugins`、`/sandbox`、`/stats`、`/permissions`）在渲染时缺少标准的彩色顶部边框和一致的填充。

---

### 组件：ThemeProvider / ThemedBox / ThemedText

**Rust ratatui 路径**：缺失  
**Rust OpenTUI 路径**：缺失（使用静态 `theme.ts`，包含硬编码的十六进制颜色）  
**TS design-system 路径**：`src/components/design-system/ThemeProvider.tsx`、`ThemedBox.tsx`、`ThemedText.tsx`  
**Rust 完整度**：1/5

#### Rust 实现状态
OpenTUI 前端使用 `rust/ui/src/theme.ts`——一个静态调色板对象（`c`），包含硬编码的十六进制值和一个旧版 `theme` 对象。没有运行时主题切换，没有基于终端主题检测的 `auto` 模式，没有 `ThemeProvider` React 上下文，也没有主题键到颜色的解析层。

ratatui 后端各处使用内联硬编码的 ANSI/RGB 颜色（例如 `welcome.rs` 中的 `Color::Rgb(190, 140, 255)`、`ACCENT_DIM`、`MUTED`、`LIGHT` 常量）。

#### 缺失功能
- 包含 `useTheme()` / `useThemeSetting()` / `usePreviewTheme()` 钩子的 `ThemeProvider` 上下文
- 主题设置持久化（`saveGlobalConfig`）
- 带有 `$COLORFGBG` 种子和 OSC 11 实时监视器的 `auto` 模式
- 主题选择器的预览/取消工作流
- `ThemedBox`——将 `keyof Theme` 的边框/背景颜色解析为原始颜色
- `ThemedText`——解析 `keyof Theme` 的文本颜色，支持 `TextHoverColorContext`
- 处理 `rgb()`、`#hex`、`ansi256()`、`ansi:` 原始格式的 `resolveColor()` 辅助函数
- 使用主题的 `inactive` 颜色的 `dimColor`（与粗体兼容，不同于 ANSI 的 dim）
- `claude-code-bun/packages/@ant/ink/src/theme/theme-types.ts` 用于 TypeScript 主题类型安全
- 用于实时终端背景检测的 `systemTheme.ts` / `systemThemeWatcher.ts`

#### 影响
Rust UI **完全没有主题系统**。每种颜色都是硬编码的。用户无法切换浅色/深色主题、设置 `auto` 模式或自定义颜色。OpenTUI 前端的 `theme.ts` 调色板（`c`）使用任意十六进制值（强调色使用洋红 `#CC00CC`，用户色使用青色 `#55FFFF`），与上游 Claude Code 配色方案不匹配。

---

### 组件：KeyboardShortcutHint

**Rust ratatui 路径**：缺失  
**Rust OpenTUI 路径**：缺失  
**TS design-system 路径**：`src/components/design-system/KeyboardShortcutHint.tsx`  
**Rust 完整度**：1/5

#### Rust 实现状态
没有等价组件。键盘提示作为原始字符串嵌入在页脚文本中（例如 `approval_overlay.rs` 中的 `"Up/Down decision | Enter confirm | Esc deny"`）。

#### 缺失功能
- 结构化 `shortcut` + `action` 显示（"ctrl+o 展开"）
- 可选括号包裹
- 可选粗体快捷键文本
- 通过 `<Byline>` 共享，使用中间点分隔符实现多提示显示
- 通过 `<ConfigurableShortcutHint>` 支持用户可自定义快捷键

#### 影响
键盘提示不一致且不可自定义。使用自定义快捷键的用户会看到错误的快捷键标签。

---

### 组件：ListItem

**Rust ratatui 路径**：缺失  
**Rust OpenTUI 路径**：缺失（使用临时 `<text>` 元素）  
**TS design-system 路径**：`src/components/design-system/ListItem.tsx`  
**Rust 完整度**：1/5

#### Rust 实现状态
没有通用的 ListItem 组件。功能面板（`feature_panels.rs`）、会话选择器（`resume_picker.rs`）和搜索结果中的选择列表使用每个组件自定义的格式渲染。

#### 缺失功能
- 焦点指示器（`figures.pointer` = ">"），使用 `suggestion` 颜色
- 选择复选标记（`figures.tick` = 勾号），使用 `success` 颜色
- 用于截断列表的上/下滚动指示器
- 主要内容下方的描述文本
- 带有暗淡文本且无指示器的 `disabled` 状态
- 用于自定义子元素样式的 `styled` 模式切换
- 支持屏幕阅读器终端光标定位的 `declareCursor`
- 基于状态（聚焦/选中/禁用）的自动文本颜色分配

#### 影响
所有选择 UI 的外观各不相同。缺乏一致的"已选中"/"已聚焦"/"已禁用"视觉语言。

---

### 组件：ProgressBar

**Rust ratatui 路径**：缺失  
**Rust OpenTUI 路径**：部分（后台任务中的内联渲染）  
**TS design-system 路径**：`src/components/design-system/ProgressBar.tsx`  
**Rust 完整度**：2/5

#### Rust 实现状态
OpenTUI 前端在后台任务中内联显示进度（例如 `rust/ui/src/components/tasks/` 中的 `renderToolActivity`），但没有可复用的 ProgressBar 组件。

#### 缺失功能
- Unicode ⅛ 块精度（字符 `▏▎▍▌▋▊▉█`）
- 从主题获取的填充/空颜色支持
- 自动比例 clamping
- 带有标准 API 的专用组件

#### 影响
进度显示是临时拼凑的，与上游在视觉上不一致。

---

## 第2章：状态与图标组件

### 组件：StatusIcon

**Rust ratatui 路径**：`rust/crates/claude-code-rs/src/ui/components/status_icon.rs`  
**Rust OpenTUI 路径**：部分（状态行中的内联 `<text fg={c.dim}>`）  
**TS design-system 路径**：`src/components/design-system/StatusIcon.tsx`  
**@ant/ink theme 路径**：`claude-code-bun/packages/@ant/ink/src/theme/StatusIcon.tsx`  
**Rust 完整度**：2/5

#### Rust 实现状态
- **Ratatui**：`status_icon.rs` 组件是一个最小枚举，包含四个变体（`Ok`、`Warning`、`Error`、`Attention`）和纯文本 `label()` 字符串（"ok"、"warn"、"error"、"attention"）。它不提供 Unicode 图标、颜色、渲染——它纯粹是基于文本的 `status_widget.rs` 的标签提供者。
- **OpenTUI**：在状态行组件中使用 figlet 字符和内联颜色，但没有可复用的 StatusIcon 组件。

#### 缺失功能
- 通过 `figures` 库提供的 Unicode 图标字形（勾号、叉号、警告、信息、圆圈、省略号）
- 命名语义颜色：`success`（绿色）、`error`（红色）、`warning`（黄色）、`suggestion`（蓝色）
- 图标后尾随空格的 `withSpace` 属性
- 六种语义状态：`success`、`error`、`warning`、`info`、`pending`、`loading`
- 未分配颜色时的 `dimColor` 回退（pending/loading 状态）

#### 影响
Rust 后端中的状态图标仅限于文本标签。上游所见勾号、叉号和警告符号的视觉丰富性缺失。

---

## 第3章：标签页与导航

### 组件：Tabs

**Rust ratatui 路径**：`rust/crates/claude-code-rs/src/ui/components/tabs.rs`  
**Rust OpenTUI 路径**：`rust/ui/src/components/TagTabs.tsx`  
**TS design-system 路径**：`src/components/design-system/Tabs.tsx`  
**@ant/ink theme 路径**：`claude-code-bun/packages/@ant/ink/src/theme/Tabs.tsx`  
**Rust 完整度**：3/5

#### Rust 实现状态
- **Ratatui**：最小的 `render_tabs()` 函数——生成一个空格分隔的字符串，活动标签显示 `[selected]`，非活动标签显示 ` label `。没有键盘处理，没有样式，没有颜色。一个 5 行函数。
- **OpenTUI**：`TagTabs.tsx` 组件明显更复杂。其特性包括：
  - 水平滚动标签条，带溢出提示（`← N` / `→N (tab to cycle)`）
  - 导出的 `planTagTabs()` 纯函数，用于可单元测试的窗口计算
  - `All` 标签特殊处理
  - 选中标签高亮显示，使用强调色 + 反色文本
  - 隐藏标签计数溢出指示器
  - 长标签名的截断逻辑
  - 哈希前缀显示（`#tagname`）

#### 缺失功能（对比 TS design-system）
- 标签页头部焦点状态管理（`headerFocused` / `blurHeader` / `focusHeader`）
- 子组件协调键盘交互的 `useTabHeaderFocus()` 钩子
- 受控模式（`selectedTab` + `onTabChange` 属性）
- 用于 `tabs:next` / `tabs:previous` 动作的 `useKeybindings` 集成
- 模态框滚动引用集成（`useModalScrollRef`）
- 固定高度标签内容的 `contentHeight` 属性（防止布局偏移）
- 从聚焦内容使用 Tab/Left/Right 导航的 `navFromContent` 属性
- 标签头部下方附加内容的 `banner` 属性
- 终端宽度感知内容区域的 `useFullWidth`
- 与子箭头键处理器冲突解决的 `disableNavigation` 属性
- 支持主题颜色的标签标题

#### 影响
ratatui 后端基本没有标签系统。OpenTUI 的 `TagTabs` 对于标签式导航是功能性的，但缺乏斜杠命令屏幕（如 `/config`、`/help` 和 `/permissions`）所需的完整交互式标签面板语义（焦点管理、键盘导航、模态框协调）。

---

## 第4章：欢迎屏幕与状态

### 组件：欢迎屏幕

**Rust ratatui 路径**：`rust/crates/claude-code-rs/src/ui/components/welcome.rs`  
**Rust OpenTUI 路径**：`rust/ui/src/components/WelcomeScreen.tsx`  
**TS design-system 路径**：缺失（无专用组件——嵌入在 `LogoV2/` 中）  
**Rust 完整度**：4/5

#### Rust 实现状态
- **Ratatui**：功能完整的欢迎面板渲染器，具有：
  - 带边框面板和强调色标题
  - 版本、模型、会话 ID（截断）、CWD 显示
  - 工具提示行（"Enter to send, /help for commands"）
  - 窄终端紧凑模式（单行回退）
  - 用于布局计算的 `welcome_height_for()` 集成
  - Unicode 安全字符串截断
  - 全面的测试覆盖率（6 个测试）
  - 无 ASCII 标志（出于紧凑性的设计选择）

- **OpenTUI**：ASCII 标志版本，具有：
  - 强调色大型 ASCII 艺术字 "Claude Code" 标志
  - 模型名称、CWD、会话 ID 显示
  - 尚未连接到后端时的 `Spinner` 组件
  - 可点击 CWD 的 `FilePathLink`

#### 缺失功能
- 主题感知颜色（目前在 ratatui 中硬编码为 RGB 常量，或在 OpenTUI 中硬编码为 `c.accent`/`c.dim`）
- 动画标志（见于 TS `LogoV2/` 组件）
- 基于用户上下文的提示/入门建议
- 键盘快捷键提示集成
- 来自最近历史的会话恢复建议

#### 影响
两个 Rust 实现都是功能性的，与上游欢迎显示相当。主题和动画方面存在微小差距。

---

### 组件：状态组件 / 状态行

**Rust ratatui 路径**：`rust/crates/claude-code-rs/src/ui/components/status_widget.rs`  
**Rust OpenTUI 路径**：`rust/ui/src/components/StatusLine/CustomStatusLine.tsx`、`SubsystemStatus.tsx`  
**TS design-system 路径**：缺失（嵌入在应用级组件中）  
**Rust 完整度**：3/5

#### Rust 实现状态
- **Ratatui**：基于文本的状态行，具有：
  - `StatusSnapshot` 数据模型：模型、CWD、权限模式、沙箱、费用、运行中的工具、活动代理、子系统状态
  - 带有标签/值/严重度三元组的 `StatusIndicator`，用于可扩展指示器
  - `render_line()` 单行紧凑格式（竖线分隔）
  - `render_details()` 多行展开格式
  - 与 `StatusIcon` 的严重度标签集成
  - 良好的测试覆盖率，使用 `insta` 快照

- **OpenTUI**：`CustomStatusLine` 渲染用户配置的状态行输出。`SubsystemStatus` 显示 LSP、MCP、插件、技能状态。关注点分离。

#### 缺失功能（ratatui）
- 无实际的终端渲染——仅为下游显示生成字符串
- 输出中无颜色/样式（全部为纯文本）
- 详情视图无 `BetterViewPanel` 集成
- 不支持自定义状态行命令

#### 影响
ratatui 状态组件提供了坚实的数据模型，但无法直接渲染带样式的输出。

---

## 第5章：权限与批准 UI

### 组件：批准覆盖层

**Rust ratatui 路径**：`rust/crates/claude-code-rs/src/ui/components/approval_overlay.rs`  
**Rust OpenTUI 路径**：`rust/ui/src/components/permissions/PermissionRequestDialog.tsx`（+ 变体）  
**TS design-system 路径**：嵌入在 `src/components/permissions/` 中  
**Rust 完整度**：3/5

#### Rust 实现状态
- **Ratatui**：结构良好的批准覆盖层，具有：
  - `ApprovalKind` 枚举，覆盖 Bash、FileEdit、WebFetch、MCP、UserInput、Fallback
  - `ApprovalChoice` 枚举：AllowOnce、AllowAlways、Deny、EditRequest
  - 通过 `BetterViewPanel` 的基于网格的布局，包含请求详情、决策列表和页脚
  - 风险分类（`approval_risk()` 返回人类可读的风险标签）
  - 默认 `fail_closed`（出错时拒绝）
  - 长命令字符串的 `fit_line()` 截断

- **OpenTUI**：类别感知权限对话框系统，具有：
  - `PermissionRequestDialog`，分发到类别特定的主体变体
  - `BashPermissionRequest`、`FileEditPermissionRequest`、`FileWritePermissionRequest`、`WebFetchPermissionRequest`、`FallbackPermissionRequest`
  - 用于共享框架的 `PermissionDialogFrame`
  - 用于一致按钮/键盘布局的 `PermissionPromptOptions`
  - 通过 `useBackend()` 的 IPC 集成，用于发送 `permission_response`
  - 通过每个选项的 `hotkey` 属性的热键支持（y/n/a）

#### 缺失功能（ratatui）
- EditRequest 支持（变体存在但与 Deny 没有视觉区别）
- 默认选项中未包含 `Always Allow` 选择
- 无文件编辑差异的视觉预览
- 待处理状态无动画指示器
- 无内联显示的键盘热键字母

#### 影响
ratatui 批准覆盖层覆盖了基本批准流程。OpenTUI 实现具有类别特定布局和 IPC 集成，明显更完整。

---

## 第6章：输入与搜索

### 组件：SearchBox

**Rust ratatui 路径**：`rust/crates/claude-code-rs/src/ui/components/search_box.rs`  
**@ant/ink theme 路径**：`claude-code-bun/packages/@ant/ink/src/theme/SearchBox.tsx`  
**Rust 完整度**：4/5

#### Rust 实现状态
ratatui 后端的实现出人意料地完善：
- 构建器模式，流畅 API（`.placeholder()`、`.focused()`、`.prefix()`、`.cursor_offset()`、`.borderless()`、`.width()`）
- 在正确偏移处插入可视光标字符（`|`）
- 通过 `UnicodeWidthChar` 的 Unicode 感知宽度截断
- `is_terminal_focused` 状态区分终端/失焦光标与输入聚焦光标
- 查询为空时显示占位符文本
- 使用 `insta` 快照的相当全面的测试覆盖率（5 个场景：聚焦带占位符、聚焦带光标、失焦、无边框、宽度截断）

#### 缺失功能
- 主题感知边框颜色（目前默认为 ANSI 样式）
- 加载状态无 `Spinner` 集成
- 无历史搜索集成
- 无文件路径自动完成支持
- 无 `/` 斜杠命令补全提示

#### 影响
SearchBox 是 ratatui 后端中最完整的组件之一。对于基本搜索/过滤操作是功能性的。

---

## 第7章：分页器与覆盖层

### 组件：分页器覆盖层

**Rust ratatui 路径**：`rust/crates/claude-code-rs/src/ui/components/pager_overlay.rs`  
**Rust 完整度**：4/5

#### Rust 实现状态
实现完善的可滚动分页器：
- `StaticOverlay` 用于静态文本内容，具有完整的键盘导航：
  - Up/Down/k/j 单行滚动
  - PageUp/PageDown/space 翻页滚动
  - Home/End 跳转
  - Esc/q 关闭
- 包装 `StaticOverlay` 的 `TranscriptOverlay`，用于聊天记录显示
- 调度到任一变体的 `Overlay` 枚举
- 滚动边界保护
- 用于视口切片的 `render_offset_content()` 工具函数
- PageDown 行为的测试覆盖

#### 缺失功能
- 分页内容内无搜索/查找
- 无进度指示器（"第 45/200 行"）
- 无鼠标滚轮支持
- 宽内容无水平滚动
- ratatui `Line` 显示中无 ANSI 转义渲染

#### 影响
一个坚实、功能完善的分页器。与 `less` 风格的基本功能相当。缺少搜索和进度显示。

---

## 第8章：功能面板

### 组件：功能面板

**Rust ratatui 路径**：`rust/crates/claude-code-rs/src/ui/components/feature_panels.rs`  
**Rust OpenTUI 路径**：`rust/ui/src/components/panels/`（McpServerCard、PluginRow、TeamMemberCard + 状态颜色）  
**TS design-system 路径**：嵌入在斜杠命令屏幕中  
**Rust 完整度**：2/5

#### Rust 实现状态
- **Ratatui**：功能面板导航的标签化表单状态：
  - `FeaturePanelKind` 枚举，包含 9 种面板类型（MCP、Agents、Teams、LSP、Settings、Sandbox、Plugins、Skills、Tasks）
  - `PanelState`：Ready、NeedsConfig、Running、Error
  - 带有选项和描述的 `FormTab` 系统
  - 徽章计数显示
  - 带有 `TabbedFormEvent` 响应的按键事件处理
  - 用于扁平文本面板列表的 `render_panel_index()`

- **OpenTUI**：单独的卡片组件（`McpServerCard`、`PluginRow`、`TeamMemberCard`），具有基于状态的着色和结构化布局。

#### 缺失功能（ratatui）
- 无视觉渲染——一切通过 `render_lines()` 以纯文本形式呈现
- 无内联状态指示器（彩色圆点、状态徽章）
- 无配置操作表单
- 无连接状态指示器（在线/离线/错误）
- 无可滚动面板内容

#### 影响
ratatui 功能面板仅是一个状态管理层。OpenTUI 有结构化的卡片组件，但没有统一的标签化面板导航。

---

## 第9章：聊天与编辑器

### 组件：聊天组件

**Rust ratatui 路径**：`rust/crates/claude-code-rs/src/ui/components/chatwidget.rs`  
**Rust OpenTUI 路径**：`rust/ui/src/components/PromptInput/`、`rust/ui/src/components/messages/`  
**Rust 完整度**：4/5

#### Rust 实现状态
- **Ratatui**：包装 `App` 结构的薄适配器：
  - 用于初始化参数的 `ChatWidgetInit`（模型、后端、会话、CWD、语音设置）
  - 委托调用的 `add_message()`、`messages()`、`clear_messages()`、`set_streaming()`
  - 转发到 `App` 的 `handle_key_event()`
  - 用于 App 生命周期管理的 `from_app()` / `into_app()`

- **OpenTUI**：功能完整的聊天系统：
  - `PromptInput/`，包含编辑器缓冲区、斜杠命令提示、模式指示器、排队提交、截断
  - `messages/`，包含 `SystemMessage`、`ToolGroupMessage`、`ToolResultOrphanMessage`、`CompactBoundaryMessage`
  - `tasks/`，包含 `BackgroundTask`、`BackgroundTaskStatus`、`ShellProgress`
  - `agent-settings/`，包含完整的代理创建向导（11 个向导步骤）

#### 缺失功能（ratatui）
- ratatui 的 `ChatWidget` 不是 UI 组件——它是一个状态管理边界
- 没有消息、编辑器或对话历史的实际渲染
- 所有聊天渲染必须通过下游的 `App` 方法进行

#### 影响
ratatui 聊天组件的范围正确，作为一个适配器。OpenTUI 前端承担了完整的聊天 UI 重量，拥有丰富的组件层次结构。

---

## 第10章：会话与恢复

### 组件：恢复选择器

**Rust ratatui 路径**：`rust/crates/claude-code-rs/src/ui/components/resume_picker.rs`  
**Rust 完整度**：2/5

#### Rust 实现状态
会话恢复的数据模型：
- 包含 session_id、title、CWD、last_modified、message_count 的 `SessionTarget`
- 按最新优先排序的会话列表的 `ResumePicker`
- `SessionPickerAction` 枚举：MoveUp/Down、PageUp/Down、Select、Cancel
- `apply()` 方法返回 `Option<SessionSelection>`（终端动作返回 Some）
- 与 `cc_session::storage::list_sessions()` 集成的 `load_resume_targets()`

#### 缺失功能
- 无视觉渲染
- 无搜索/过滤
- 无删除会话功能
- 无会话预览（最后一条消息片段）
- 无日期格式化（"2 小时前"）
- 无键盘提示页脚
- 长列表无滚动指示器

#### 影响
一个正确但最小的数据模型。没有用户可交互的实际 UI。

---

## 第11章：底部面板

### 组件：底部面板

**Rust ratatui 路径**：`rust/crates/claude-code-rs/src/ui/components/bottom_pane.rs`  
**Rust 完整度**：1/5

#### Rust 实现状态
视图堆栈状态机：
- `BottomPaneView` 枚举：Composer、Approval、Selection、Status
- `name()` 字符串标签
- 基于堆栈的 push/pop 导航
- `focused_view()` 查看堆栈顶部
- `render_lines()` 生成文本字符串（非渲染）

#### 缺失功能
- 无实际终端渲染
- 视图之间无视觉分隔
- 视图切换无动画
- 无高度计算
- 无聊天编辑器渲染集成
- 视图之间无焦点管理

#### 影响
底部面板是一个概念草图——仅有状态管理而无 UI。实际底部栏布局必须在其他地方实现。

---

## 主要发现

### 1. 根本差距：Rust 中无设计系统

TypeScript 代码库有 16 个可复用的设计系统组件（`Dialog`、`Divider`、`Pane`、`ThemedBox`、`ThemedText`、`ThemeProvider`、`KeyboardShortcutHint`、`ListItem`、`ProgressBar`、`StatusIcon`、`Tabs`、`Byline`、`Ratchet`、`FuzzyPicker`、`LoadingState`、`color`），具有一致的主题、键盘处理和无障碍支持。**Rust 代码库中这些组件数量为零。** OpenTUI 前端有一个静态调色板（`theme.ts`）和临时组件样式，但没有共享的设计词汇表。

### 2. Ratatui 后端：状态重、渲染轻

ratatui 组件（`claude-code-rs/src/ui/components/`）主要是状态管理层——它们拥有数据模型、事件处理和纯文本输出，但将实际的终端渲染委托给调用者。`search_box.rs` 和 `pager_overlay.rs` 是明显的例外，它们具有实际的渲染逻辑。

### 3. OpenTUI 前端：正在成长但不一致

OpenTUI 前端（`rust/ui/src/components/`）有更完整的组件实现（权限对话框、欢迎屏幕、消息气泡、后台任务、代理设置向导），但它们是独立构建的，没有共享组件库。每个组件直接从静态 `theme.ts` 导入颜色，并使用内联的 OpenTUI 原语（`<box>`、`<text>`、`<span>`），没有将它们包装在可复用的抽象层中。

### 4. 主题系统差距是影响最大的缺失功能

缺少运行时主题系统影响每个组件。没有它：
- 无法切换浅色/深色模式
- 无法基于终端背景检测实现 `auto` 模式
- 无法自定义颜色
- 无法进行无障碍颜色对比度调整
- 所有颜色都是硬编码的，必须逐个更新

### 5. Rust 中存在但 TypeScript 中不存在的组件

一些组件仅存在于 Rust 代码库中：
- `SearchBox`（ratatui）：构建器模式的搜索框，带光标定位
- `PagerOverlay`（ratatui）：可滚动文本分页器，带键盘导航
- `SubsystemStatus`（OpenTUI）：结构化子系统概览（MCP、LSP、插件、技能）
- `BackgroundTask` / `BackgroundTaskStatus`（OpenTUI）：任务进度显示
- `AgentSettings` 向导（OpenTUI）：11 步代理创建流程
- `TagTabs`（OpenTUI）：水平滚动的标签页，带溢出处理

这些代表 Rust 优先的特性，可以为 TypeScript 的改进提供参考。

### 6. 测试覆盖率的差异

ratatui 组件有合理的测试覆盖率（尤其是 `welcome.rs` 有 6 个以上测试、`search_box.rs` 有快照测试、`status_widget.rs` 有 insta 快照）。OpenTUI 前端的测试覆盖率稀疏（只有 `tag-tabs`、`validation-errors-list`、`server-list-editor`、`file-path-link`、`frame-clear`、`string-width`、`prompt-hotkey`、`prompt-state`、`status-line-state`、`hunks`、`state-colors`、`team-summary`、`paste-display` 有测试）。
