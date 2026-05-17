# 渲染子系统对比：Rust (ratatui) vs TypeScript (Ink/React)

> **日期**：2026-05-17
> **范围**：Markdown、主题、进度条、旋转指示器、闪烁效果、工具活动、差异渲染、虚拟滚动、历史单元格和语法高亮

---

## 汇总表

| 领域 | Rust | TypeScript | Rust 完成度 | 主要差距 |
|------|------|------------|-------------------|----------|
| Markdown（核心解析） | `pulldown_cmark` + LRU 缓存 | `marked` + 500 条目令牌缓存 | 3/5 | 无表格、无语法高亮、无链接化 |
| Markdown 渲染 | `markdown_render.rs` 薄外观层 | `markdown.ts` 中的完整令牌格式化器 | 2/5 | 文本换行时样式丢失；未处理图片/链接/转义/定义/删除/HTML |
| 流式 Markdown | `MarkdownStreamCollector` 简单缓冲区 | 带单调边界追踪的 `StreamingMarkdown` | 2/5 | 无增量解析；每次重新解析整个缓冲区 |
| 主题 | 25 个预组合样式 | 支持按主题名称自定义的主题系统 | 4/5 | 静态，无主题切换；TS 支持多个命名主题 |
| 进度条 | 纯函数，无颜色 | 带 `fillColor`/`emptyColor` 的 React 组件 | 3/5 | 部分块无颜色/样式 |
| 旋转指示器 | Braille 动画，缓冲区渲染 | Ink 旋转指示器组件 | 4/5 | 无滴答间隔管理 |
| 闪烁效果 | 逐字符正弦波动画 | 与 `TextHighlight` 分割器集成 | 3/5 | 全局定时器，未与渲染管线集成 |
| 工具活动 | 完整状态机 + JSON 摘要 | 内联 React 渲染 | 4/5 | `compact_line()` 输出缺少颜色样式 |
| 差异（行级） | `similar` crate，带样式的缓冲区渲染 | 带 `StructuredDiff` 的 `diff` 模块 | 4/5 | 无词级差异比较 |
| 差异（结构化） | 完整代码块解析 + 行号 | `StructuredPatchHunk` 类型 | 4/5 | 结构上等价 |
| 差异（对话框/文件列表） | 带分页的列表 + 详情视图 | 完整对话框组件 + 键盘导航 | 3/5 | 缺少颜色、差异中的语法高亮、交互式选择 |
| 文件编辑差异 | 统计信息 + 结构化代码块 | 懒加载、Suspense、上下文感知的代码块调整 | 3/5 | 无异步加载、无上下文感知的代码块调整 |
| 语法高亮 | **未实现** | 基于 WASM 的 Shiki（`cliHighlight`） | 0/5 | 完全缺失 |
| 虚拟滚动 | O(log n) 二分查找，宽度感知 | Ink 虚拟列表 | 4/5 | 成熟，有测试覆盖 |
| 历史单元格 | 类型化枚举 + 提示/对话渲染 | 消息列表组件 | 3/5 | 简单字符串输出，无样式渲染 |
| Git 差异获取 | `git2` 库原生绑定 | 调用 `git` CLI | 4/5 | 功能上等价 |

---

## 领域：Markdown 渲染

### Rust 文件
- `F:\AIclassmanager\cc\rust\crates\claude-code-rs\src\ui\rendering\markdown.rs`（259 行）
- `F:\AIclassmanager\cc\rust\crates\claude-code-rs\src\ui\rendering\markdown_render.rs`（63 行）
- `F:\AIclassmanager\cc\rust\crates\claude-code-rs\src\ui\rendering\markdown_stream.rs`（74 行）

### TS 文件
- `F:\AIclassmanager\cc\src\components\Markdown.tsx`（236 行）
- `F:\AIclassmanager\cc\src\utils\markdown.ts`（382 行）
- `F:\AIclassmanager\cc\src\components\MarkdownTable.tsx`
- `F:\AIclassmanager\cc\claude-code-bun\src\utils\textHighlighting.ts`

### Rust 完成度：2/5

### 当前状态
Rust Markdown 使用 `pulldown_cmark` 配合 LRU 缓存（256 条目），支持：标题（H1 粗体+下划线，H2+ 粗体）、粗体、斜体、内联代码（`theme.code`）、围栏/缩进代码块、无序/有序列表（单层字母前缀）、链接（下划线）、块引用（暗淡 `|` 前缀）、水平分割线和软/硬换行。解析器选项中已启用删除线。

`markdown_render.rs` 外观层将输出包装为 ratatui `Text`，并提供 `wrap_lines()` 工具函数，但**所有逐区间样式在换行时被丢弃**——换行后的行仅使用 `Span::raw()`。

`MarkdownStreamCollector` 缓冲原始文本并在每一帧重新渲染整个缓冲区；它不做增量重新解析。

### 与 TS 相比缺失的功能
- **无表格支持**——TS 有 `MarkdownTable`，具备列宽计算、ANSI 感知换行和对齐。这是最大的单一渲染差距。
- **无语法高亮**——TS 为代码块使用懒加载的 WASM Shiki（`cliHighlight`）。Rust 没有等价物。
- **无超链接**（OSC 8）——TS 将链接包装为可点击的终端超链接。Rust 将其渲染为纯下划线文本。
- **无 GitHub Issue/PR 引用链接化**（`owner/repo#123`）。
- **无列表嵌套**——Rust 仅支持带 `"  "` 缩进的单层列表。
- **无嵌套列表的有序列表字母/罗马数字编号**。
- **无 `mailto:` 处理**——TS 跳过邮件链接的 OSC 8。
- **无 `def`/`del`/`html` 令牌处理**（在 TS 中返回为空，在 Rust 中未处理）。
- **无流式优化**——TS `StreamingMarkdown` 使用单调边界追踪，仅重新解析不稳定的后缀。Rust 重新解析整个缓冲区。
- **无 Markdown 语法快速路径检测**——TS 通过 `hasMarkdownSyntax()` 正则表达式完全跳过 `marked.lexer()`（约 3ms）以处理纯文本。

### 影响
用户会看到 Markdown 表格（LLM 经常输出）的无格式/降级渲染。代码块没有语法高亮。流式响应会带来不必要的完整重新解析成本。超链接不可点击。

---

## 领域：主题

### Rust 文件
- `F:\AIclassmanager\cc\rust\crates\claude-code-rs\src\ui\rendering\theme.rs`（117 行）

### TS 文件
- `F:\AIclassmanager\cc\claude-code-bun\packages\@ant\ink\src\theme\`（多个主题文件）

### Rust 完成度：4/5

### 当前状态
主题定义了 25 个预组合的 `Style` 字段，涵盖：助手/用户/系统名称、工具名称/结果、错误/警告/信息、提示、边框、代码、思考、暗淡、标题、粗体、斜体、链接、差异添加/删除/上下文/头部、已选/未选。所有颜色在 `Default::default()` 中硬编码为 RGB。`Theme::new()` 构造函数仅调用 `Default::default()`，没有任何自定义。

### 与 TS 相比缺失的功能
- **无命名主题切换**——TS 支持多个主题名称（例如 `"dark"`、`"light"`、`"tron"`），具有不同的调色板。Rust 有一个硬编码主题。
- **无带自定义颜色的 `new()`**——`new()` 方法不接受参数；自定义需要使用结构体字面量构造。
- **无 `unselected` 样式被使用**——已定义但仅在差异对话框渲染路径中出现 `selected`。
- **无来自设置/配置的颜色覆盖**——TS 主题可以通过用户设置自定义。

### 影响
视觉自定义受限。用户无法在亮/暗主题之间切换。固定的 RGB 值可能无法在所有终端配色方案上良好呈现。

---

## 领域：进度条

### Rust 文件
- `F:\AIclassmanager\cc\rust\crates\claude-code-rs\src\ui\rendering\progress_bar.rs`（53 行）

### TS 文件
- `F:\AIclassmanager\cc\src\components\design-system\ProgressBar.tsx`（86 行）
- `F:\AIclassmanager\cc\claude-code-bun\src\components\design-system\ProgressBar.tsx`（1 行，再导出）

### Rust 完成度：3/5

### 当前状态
纯函数 `render_progress_bar(ratio, width) -> String`，使用 9 个 Unicode 块字符（`" ▏▎▍▌▋▊▉█"`）。与 TS 使用相同的字符集和算法。通过钳位处理 NaN、负值和溢出比率。有边界情况的单元测试。

该函数在 `tool_activity.rs` 中用于内联进度渲染（10 个字符宽）。

### 与 TS 相比缺失的功能
- **无颜色支持**——TS `ProgressBar` 有 `fillColor` 和 `emptyColor` 属性，类型为 `keyof Theme`。Rust 返回没有样式的纯字符串。
- **不是 UI 组件**——Rust 是返回 `String` 的纯函数；TS 是与 Ink 渲染管线集成的 React 组件。
- **无可配置宽度**——宽度在调用点硬编码（工具活动中始终为 10）；TS 接受任意宽度属性。

### 影响
工具活动输出中的进度条显示为无颜色的文本，而不是视觉上带有样式的条。

---

## 领域：旋转指示器

### Rust 文件
- `F:\AIclassmanager\cc\rust\crates\claude-code-rs\src\ui\rendering\spinner.rs`（96 行）

### TS 文件
- Ink 框架 `useInput` / `packages/@ant/ink/` 中的旋转指示器组件

### Rust 完成度：4/5

### 当前状态
`SpinnerState` 结构体，使用 10 个 Braille 字符（`⠋⠙⠹⠸⠼⠴⠦⠧⠇⠏`）进行动画帧追踪，与 TS 相同。提供 `tick()`、`start()`、`stop()`、`set_message()`、`render()` 方法。渲染到 ratatui 缓冲区中，使用主题样式（帧使用 `theme.info`，消息使用 `theme.dim`）。非活跃旋转指示器不渲染任何内容。

### 与 TS 相比缺失的功能
- **无集成的滴答定时器**——`tick()` 必须在外部按间隔调用。TS 旋转指示器与 Ink 的渲染循环集成。
- **无帧率控制**——80ms 间隔在组件内不可配置。
- **无颜色自定义**——始终为帧使用 `theme.info`，为消息使用 `theme.dim`。

### 影响
基本使用在功能上等价。正常运行中没有可见的用户层面差距。

---

## 领域：闪烁文本

### Rust 文件
- `F:\AIclassmanager\cc\rust\crates\claude-code-rs\src\ui\rendering\shimmer.rs`（41 行）

### TS 文件
- `F:\AIclassmanager\cc\claude-code-bun\src\utils\textHighlighting.ts`（`TextHighlight` 类型中的闪烁效果）

### Rust 完成度：3/5

### 当前状态
`shimmer_spans(text)` 函数使用正弦波动画创建逐字符样式的区间。使用全局 `OnceLock<Instant>` 记录经过时间，这意味着动画阶段在所有闪烁实例之间共享。颜色在灰度范围（RGB 120-220）内扫描。

### 与 TS 相比缺失的功能
- **全局定时器状态**——所有闪烁实例共享相同的动画阶段，使它们全部同步脉冲。TS 按实例计时允许独立动画。
- **未与文本高亮管线集成**——TS 闪烁效果是 `TextHighlight` 段上的 `shimmerColor` 属性，可与其他高亮类型组合。Rust 闪烁效果是独立的。
- **无颜色自定义**——硬编码的灰度范围。TS 允许指定主题颜色键。
- **逐字符开销**——每个字符创建一个 `Span`，对于长字符串来说开销很大。

### 影响
闪烁效果可用但粗糙。当多个闪烁元素可见时，所有实例同步动画是明显的。

---

## 领域：工具活动

### Rust 文件
- `F:\AIclassmanager\cc\rust\crates\claude-code-rs\src\ui\rendering\tool_activity.rs`（319 行）
- `F:\AIclassmanager\cc\rust\crates\claude-code-rs\src\ui\tasks\render_tool_activity.rs`（39 行）
- `F:\AIclassmanager\cc\rust\crates\claude-code-rs\src\ui\tool_activity.rs`（遗留文件，在 `ui/` 根目录）

### TS 文件
- 在 `src/components/Messages.tsx` / `MessageRow.tsx` 中内联渲染
- 通过 `AppStateStore` 追踪工具状态

### Rust 完成度：4/5

### 当前状态
`ToolActivity` 结构体带有 5 状态机（Queued/Running/Succeeded/Failed/Cancelled）。追踪：名称、面向用户的名称、参数摘要、状态、摘要文本、错误摘要、经过时间（毫秒）、进度（可选的完成/总数）、输出行数、输出预览（前几行）。提供 `compact_line()` 用于分组显示，`transcript_block()` 用于详细视图，以及 `display_call()` 用于格式化 `ToolName(args)` 输出。

包含 `render_grouped_activity()` 用于聚合多个工具活动，以及从任务系统桥接的 `render_task_tool_activity()`。

JSON 输入摘要，带有优先键提取（`path`、`command`、`pattern`、`url` 等），并回退到前 3 个键。使用 `insta` 进行快照测试。

### 与 TS 相比缺失的功能
- **`compact_line()` 中无带样式/颜色的输出**——状态标签、名称和摘要为纯文本。TS 使用主题感知着色渲染工具调用块。
- **无丰富的输出预览**——仅将前几行存储为字符串；无工具输出的结构化渲染。
- **无流式进度更新**——页面级进度作为快照更新；TS 可以在 Read/Edit 工具中流式传输增量文件内容。
- **`render_grouped_activity` 产生纯文本**——未与 ratatui 渲染集成；输出是 `String`，不是带样式的 `Line`。

### 影响
工具活动在文本上完整，但与 TS 相比视觉上平淡。`compact_line` 中缺少样式化输出意味着对话中内联的工具进度缺乏视觉层次。

---

## 领域：差异渲染

### Rust 文件
- `F:\AIclassmanager\cc\rust\crates\claude-code-rs\src\ui\diff.rs`（261 行）——核心差异模块
- `F:\AIclassmanager\cc\rust\crates\claude-code-rs\src\ui\diff\diff_detail_view.rs`——详情视图
- `F:\AIclassmanager\cc\rust\crates\claude-code-rs\src\ui\diff\diff_dialog.rs`——对话框状态机
- `F:\AIclassmanager\cc\rust\crates\claude-code-rs\src\ui\diff\diff_file_list.rs`——文件列表
- `F:\AIclassmanager\cc\rust\crates\claude-code-rs\src\ui\diff\file_edit_diff.rs`——文件编辑统计
- `F:\AIclassmanager\cc\rust\crates\claude-code-rs\src\ui\diff\structured_diff.rs`——结构化代码块解析
- `F:\AIclassmanager\cc\rust\crates\claude-code-rs\src\ui\rendering\get_git_diff.rs`——Git 差异获取

### TS 文件
- `F:\AIclassmanager\cc\src\components\diff\DiffDetailView.tsx`
- `F:\AIclassmanager\cc\src\components\diff\DiffDialog.tsx`
- `F:\AIclassmanager\cc\src\components\diff\DiffFileList.tsx`
- `F:\AIclassmanager\cc\src\components\StructuredDiff.tsx`
- `F:\AIclassmanager\cc\src\components\StructuredDiffList.tsx`
- `F:\AIclassmanager\cc\src\components\FileEditToolDiff.tsx`

### Rust 完成度：3/5

### 当前状态
Rust 差异子系统是最完整的渲染领域，有多个反映 TS 组件结构的子模块。关键能力：

- **行级差异**：使用 `similar` crate 进行 `TextDiff::from_lines()`。渲染时带行号（旧/新）、+/- 前缀和主题着色（diff_add/diff_remove/diff_context/diff_header）。显示"N more lines..."截断提示。
- **结构化差异**：`StructuredDiffHunk` 带有头部解析、行级详情（旧/新行号、类型）和带截断的格式化输出 `render_structured_diff_hunks()`。
- **差异数据模型**：`DiffFile`、`DiffStats`、`DiffData` 类型匹配 TS `DiffData` 结构。支持 is_binary、is_large_file、is_truncated、is_untracked 标志。
- **文件列表**：带 `MAX_VISIBLE_FILES=5` 的分页渲染，文件统计显示。
- **差异对话框**：双模式（列表/详情）状态机，支持当前/轮次差异的 `DiffSource`。
- **文件编辑差异**：使用 `TextDiff` 的 `unified_hunk_lines_from_edit()`、`file_edit_diff_stats()`、`format_file_edit_summary()`。
- **Git 差异获取**：`get_git_diff()` 通过 `git2` 库支持已暂存、未暂存和组合模式。

子模块死代码状态：`diff_detail_view`、`diff_dialog`、`diff_file_list` 被标记为 `#[allow(dead_code)]`。仅有 `file_edit_diff` 和 `structured_diff` 被活跃使用。

### 与 TS 相比缺失的功能
- **无词级差异比较**——TS `StructuredDiff` 在变更行内执行词级差异。Rust 仅做行级。这意味着 `{` 变为 `{:` 显示为整行变更，而不是字符级插入。
- **差异中无语法高亮**——TS 差异视图在差异代码块内高亮语法。Rust 根本没有语法高亮。
- **无轮次差异集成**——TS `DiffDialog` 支持"轮次差异"（每轮对话差异快照）。Rust `DiffSource` 有数据模型但是死代码。
- **无交互式键盘导航**——TS 差异对话框支持箭头键、Page Up/Down 进行文件选择。Rust 对话框是无输入处理的死代码状态机。
- **无异步加载**——TS `FileEditToolDiff` 使用 `Suspense` + `use()` 进行懒加载差异计算，带占位渲染。Rust `file_edit_diff` 是同步的。
- **无上下文感知的代码块调整**——TS `adjustHunkLineNumbers()` 在文件内容自差异生成后发生变化时调整代码块上下文。Rust 没有等价物。
- **三个子模块为死代码**——`diff_detail_view`、`diff_dialog` 和 `diff_file_list` 可编译但未被使用。

### 影响
最显著的面向用户的影响是差异中**词级差异比较**和**语法高亮**的缺失。仅修改一行内几个字符的变更显示为整行的红/绿块，这更难阅读。三个死代码子模块表明基于对话框的差异浏览体验（列表视图 -> 带键盘导航的详情视图）尚未接入 UI。

---

## 领域：Git 差异获取

### Rust 文件
- `F:\AIclassmanager\cc\rust\crates\claude-code-rs\src\ui\rendering\get_git_diff.rs`（113 行）

### TS 文件
- `src/hooks/useDiffData.ts`（通过 shell 命令使用 `git` CLI）
- `src/utils/diff.ts`

### Rust 完成度：4/5

### 当前状态
使用 `git2` 库（libgit2 绑定）进行原生 Git 操作。支持三种模式：已暂存（HEAD 到索引）、未暂存（索引到工作目录）、组合（两者皆有，带章节标题）。有 `get_status_summary()` 返回格式化的 Git 状态输出，带标记（A/M/D/??/ M/ D）。

### 与 TS 相比缺失的功能
- **无 `git diff` CLI 回退**——TS 调用 `git diff` CLI，可与任何 Git 版本配合使用。Rust 依赖 `git2` 库，可能不支持所有 Git 特性。
- **检索层中无差异统计信息解析**——TS 从输出中解析差异统计信息。Rust 返回原始字符串，消费者必须自行解析。
- **差异获取中无未追踪文件检测**——TS 单独处理未追踪文件；Rust 的 `get_git_diff` 仅包含已追踪的变更。

### 影响
常见情况下功能等价。`git2` 依赖比调用 shell 更可靠，但可能滞后于 Git CLI 的特性。

---

## 领域：虚拟滚动

### Rust 文件
- `F:\AIclassmanager\cc\rust\crates\claude-code-rs\src\ui\rendering\virtual_scroll.rs`（347 行）

### TS 文件
- Ink 框架虚拟列表（在 `packages/@ant/ink/src/` 中）
- `src/components/Messages.tsx`（消息列表）

### Rust 完成度：4/5

### 当前状态
完整的虚拟滚动实现，具有：
- **每条消息高度缓存**：维护逻辑行数和宽度感知的换行后可视行数。
- **前缀和偏移数组**：两个数组（逻辑偏移、可视偏移）支持在 O(log n) 时间内对可见范围进行二分查找。
- **过扫描**：在视口上下额外渲染 40 行以实现平滑滚动。
- **宽度感知的缓存失效**：在终端大小调整时，可视高度完全重新计算，但逻辑高度保持不变。
- **通过 `partition_point()` 进行二分查找**：将滚动偏移映射到可见消息索引范围。
- **全面的测试覆盖**：宽度变更重新计算、可视范围边界测试、失效测试。

### 与 TS 相比缺失的功能
- **未直接与渲染管线集成**——`ensure_up_to_date()` 必须在渲染前手动调用。TS 虚拟滚动是一个透明管理此操作的 React 组件。
- **无焦点/选择追踪**——仅处理可见性，不处理哪个消息当前是"活跃的"。
- **逻辑高度在可视渲染中未被使用**——`total_lines()` 和 `visible_range()`（逻辑）方法被标记为 `#[allow(dead_code)]`，表明领导者侧渲染器仅使用可视偏移。

### 影响
虚拟滚动实现成熟且经过良好测试。死代码方法表明渲染管线集成仍在进行中，但核心数据结构是可靠的。

---

## 领域：历史单元格

### Rust 文件
- `F:\AIclassmanager\cc\rust\crates\claude-code-rs\src\ui\rendering\history_cell.rs`（77 行）

### TS 文件
- `src/components/MessageRow.tsx`（消息行渲染）
- `src/types/message.ts`（消息类型层次结构）

### Rust 完成度：3/5

### 当前状态
类型化的 `HistoryCell` 枚举，有 6 个变体：User、Assistant、System、Tool（名称+摘要）、Diff（路径+添加/删除）、Status。`HistoryRenderMode` 枚举有 Prompt 和 Transcript 模式。`render_history()` 将所有单元格用换行符连接。使用带有 `format!()` 的纯文本，无样式。

### 与 TS 相比缺失的功能
- **无样式渲染**——输出是纯文本，无颜色、无缩进、无主题应用。TS 消息渲染使用完整的 Ink 组件树，带有主题感知的样式。
- **无丰富的工具调用显示**——工具单元格仅存储名称+摘要字符串。TS 渲染带参数、持续时间和带样式输出预览的工具调用。
- **无代码块格式化**——助手文本原样存储；历史单元格内无 Markdown 渲染。
- **无消息元数据**（时间戳、UUID、用量信息）。

### 影响
历史单元格作为提示/对话导出的简单文本表示。它们不用于屏幕上的渲染。对于预期的用例（上下文窗口显示），这个差距是可接受的。

---

## 领域：语法高亮

### Rust 文件
- **未实现**（未找到文件）
- `crates/claude-code-rs/Cargo.toml` — `syntect` 和 `tree-sitter` 声明为 optional 依赖，**零使用**
- `crates/claude-code-rs/src/ui/rendering/markdown.rs` — `Tag::CodeBlock(_)` 使用 `_` 丢弃 `info_string`

### TS 文件
- `F:\AIclassmanager\cc\src\utils\cliHighlight.ts`（WASM Shiki 加载器）
- `F:\AIclassmanager\cc\src\components\Markdown.tsx`（Suspense + use(highlight)）
- `F:\AIclassmanager\cc\src\utils\markdown.ts`（formatToken 高亮集成）

### Rust 完成度：0/5

### 当前状态
Rust 代码库完全没有语法高亮能力。没有提及任何高亮库，没有 `syntax` 或 `highlight` 模块，也没有集成点。`markdown_to_lines_inner()` 函数没有用于代码块语言检测的 `lang` 参数——它使用单一的 `theme.code` 样式渲染所有代码块。

TS 实现通过 WASM 加载 Shiki（`cliHighlight.ts`），使用 `Suspense` 进行懒加载。在约 50ms 的加载期间，显示无高亮的回退内容。加载后，对每个代码块调用 `highlight.highlight(text, { language })`，对于不支持的语言回退到 `plaintext`。语言检测来自围栏代码块的信息字符串。

### 与 TS 相比缺失的功能
- **Markdown 或差异视图中代码块没有任何类型的语法高亮**。
- **无从围栏代码块信息字符串进行语言检测**。
- **无回退机制**（TS 回退到纯文本）。

### 补齐计划
详见 `docs/ui/great/plans/plan-06-syntax-highlighting.md`：
- 阶段 1: syntect 集成基础设施（`SyntaxHighlighter` 结构体 + 颜色转换 + Theme 集成）
- 阶段 2: 从 `Tag::CodeBlock(CodeBlockKind::Fenced(lang))` 提取语言标识符
- 阶段 3: 令牌级着色（syntect tokens → ratatui Spans，LRU 缓存）
- 阶段 4: 差异视图语法高亮（行级 +/- 着色叠加令牌高亮）
- 阶段 5: 语言选择器 UI（模糊匹配 + 覆盖层下拉选择）
- 阶段 6: 流式代码块增量高亮（逐行 `HighlightLines::highlight()`）
- 估算: ~600 行新代码（`#[cfg(feature = "syntect")]` 编译门控）

### 影响
这是最大的单一渲染差距。Markdown 输出和差异代码块中的代码显示为纯单色文本。对于一个代码是主要内容的开发者工具来说，这显著降低了阅读体验。习惯了 TS 版本彩色代码的用户会发现 Rust 版本明显平淡。

---

## 横切关注点

### 渲染管线集成
Rust 渲染模块主要是纯函数或字符串生成器，而 TS 组件是与 Ink 渲染循环集成的 React 组件。`rendering/` 中的几个 Rust 模块似乎是工具库，而不是直接渲染的 UI 元素。差异对话框子模块（`#[allow(dead_code)]`）表明 UI 组装层尚未完成。

### 测试覆盖
Rust 模块通常有单元测试（markdown、progress_bar、shimmer、tool_activity、structured_diff、virtual_scroll），有些使用 `insta` 进行快照测试。然而，与有多个快照文件的 TS 相比，没有针对组合 UI 的视觉回归测试。

### 流式支持
TS 有成熟的流式优化：
- `StreamingMarkdown` 带单调边界追踪（避免重新解析稳定的前缀）
- 流式过程中的块级滚动锚定
- 工具调用的流式进度指示器

Rust 的 `MarkdownStreamCollector` 是简单的缓冲并渲染，没有增量优化。在长流式会话中，这种差异将更加明显。

### 颜色/样式保真度
TS 使用 ANSI 转义序列（通过 `chalk`）和 Ink 的组件样式，允许逐字符粒度。Rust ratatui `Style` 在 `Span` 级别工作。`markdown_render.rs` 的换行函数在单词换行期间丢失了所有区间级样式，这是在换行上下文中带样式 Markdown 渲染的正确性缺陷。

### 依赖方法
TS 使用 WASM（Shiki）和 JavaScript 库（`marked`），它们提供丰富的功能集，但有懒加载代价。Rust 使用原生 Rust 库（`pulldown_cmark`、`similar`、`git2`），这些库更快但内置功能较少。这种权衡使 TS 在功能丰富性方面占优，而 Rust 在原始解析性能方面占优。
