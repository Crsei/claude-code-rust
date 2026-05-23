# render.rs 重构计划

> 文件: `crates/claude-code-rs/src/ui/messages/render.rs`
> 当前行数: **2949 行**
> 生成日期: 2026-05-23

---

## 1. 文件概览

`render.rs` 是 TUI 消息渲染的核心模块，负责将 `Message` 列表转化为 ratatui 绘制元素（`Line`、`Span`）。文件包含以下内容：

| 分类 | 行号范围 | 内容 |
|------|---------|------|
| imports + 常量 | 1–45 | 外部依赖、`USER_MESSAGE_BACKGROUND` |
| `MessageRenderContext` struct + impl | 47–285 | 渲染上下文（renderable messages、lookups、virtual scroll 缓存等） |
| 缓存键计算 | 287–325 | `render_context_cache_key()` |
| `RenderableMessage` struct + impl | 327–395 | 可渲染消息抽象（包装 `Message`，附带渲染元数据） |
| 消息预处理流水线 | 397–660 | `normalize_messages_for_render`, `derive_child_uuid`, `is_not_empty_renderable_message`, `is_empty_message_text`, `strip_prompt_xml_tags`, `strip_simple_tag`, `filter_compact_boundary`, `should_show_renderable_message`, `is_null_rendering_attachment`, `user_contains_tool_result`, `reorder_messages_in_ui`, `filter_brief_messages`, `truncate_transcript_messages` |
| 消息分组逻辑 | 660–1070 | `apply_grouping`, `collapse_read_search_groups`, `CollapsibleToolInfo` struct, `add_collapsible_info`, `collapsible_tool_info`, `collapsible_info_for_tool`, `CollapsibleKind` enum, `collapsible_kind_for_tool`, `counts_for_kind`, `grouping_tool_use_key`, `is_groupable_tool`, `source_index_of`, `tool_use_id`, `tool_result_id`, `is_api_error_message`, `find_latest_bash_output_uuid`, `find_last_thinking_block_id`, `message_content_blocks`, `derive_group_uuid`, `uuid_from_parent_and_salt`, `message_type_key`, `ToolUseRenderRecord` impl |
| 主渲染入口 | 1094–1160 | `pub fn render_messages()` — 主入口函数 |
| 选中消息装饰 | 1162–1428 | `renderable_message_uses_user_background`, `user_message_uses_background`, `decorate_selected_message`, `selected_message_meta`, `concise_timestamp`, `message_detail_lines` |
| 用户消息渲染 | 1430–1719 | `render_user_message`, `render_user_image_blocks`, `render_tagged_user_text`, `extract_xmlish_body`, `render_tool_result_user_message`, `render_file_edit_preview`, `tool_result_content_text`, `styled_text_lines`, `plain_text_to_lines`, `api_error_display_text` |
| 助手消息渲染 | 1721–1980 | `render_assistant_message`, `tool_state_for_id` |
| 系统/进度/附件渲染 | 1983–2117 | `render_system_message`, `render_progress_message`, `render_attachment_message` |
| JSON 摘要辅助 | 2119–2163 | `abbreviate_json`, `tool_input_summary`, `tool_primary_input` |
| 复制文本 / 引用提取 | 2165–2348 | `message_content_copy_text`, `message_content_reference`, `content_block_copy_text`, `content_block_reference`, `extract_code_path_reference`, `clean_path_candidate`, `looks_like_code_path`, `image_reference`, `attachment_copy_text`, `attachment_reference`, `strip_system_reminders`, `compact_boundary_summary` |
| 测试模块 | 2350–2949 | `mod tests` (~600 行) |

---

## 2. 拆分方案

将 `render.rs` 拆分为目录模块 `render/`，共 **7 个文件**：

```
ui/messages/render/
├── mod.rs              # 模块声明 + pub use 重导出 + 主入口 render_messages()
├── context.rs          # MessageRenderContext, RenderableMessage, 缓存键
├── preprocessing.rs    # 消息预处理流水线（过滤、去空、重排序、截断）
├── grouping.rs         # 消息分组逻辑（tool grouping, collapsible info, GroupingToolUseKey）
├── render_user.rs      # 用户消息渲染（含附件、图片、tool result、file preview）
├── render_assistant.rs # 助手消息渲染 + 系统/进度/附件消息渲染
└── copy_text.rs        # 复制文本提取、引用路径提取、JSON 摘要
```

---

## 3. 每个新文件的包含内容

### `mod.rs` (~120 行)
- 模块声明 (`mod context; mod preprocessing; ...`)
- `pub use` 重导出（保持外部 API 不变）
- `pub fn render_messages()` (行 1094–1160) — 主入口，调用其他子模块
- `decorate_selected_message()` (行 1352–1384)
- `selected_message_meta()` (行 1386–1395)
- `concise_timestamp()` (行 1397–1408)
- `message_detail_lines()` (行 1410–1428)
- 常量 `USER_MESSAGE_BACKGROUND`

### `context.rs` (~240 行)
- `MessageRenderContext` struct + impl (行 47–285)
- `render_context_cache_key()` (行 287–325)
- `RenderableMessage` struct + impl (行 327–395)
- `ToolUseRenderRecord` impl (行 1070–1092)
- `renderable_message_uses_user_background()` (行 1162–1172)
- `user_message_uses_background()` (行 1174–1350)

### `preprocessing.rs` (~280 行)
- `normalize_messages_for_render()` (行 397–449)
- `derive_child_uuid()` (行 451–456)
- `is_not_empty_renderable_message()` (行 458–496)
- `is_empty_message_text()` (行 498–501)
- `strip_prompt_xml_tags()` (行 503–514)
- `strip_simple_tag()` (行 516–532)
- `filter_compact_boundary()` (行 534–556)
- `should_show_renderable_message()` (行 558–573)
- `is_null_rendering_attachment()` (行 575–582)
- `user_contains_tool_result()` (行 584–588)
- `reorder_messages_in_ui()` (行 590–636)
- `filter_brief_messages()` (行 638–643)
- `truncate_transcript_messages()` (行 645–658)

### `grouping.rs` (~420 行)
- `apply_grouping()` (行 660–733)
- `collapse_read_search_groups()` (行 735–819)
- `CollapsibleToolInfo` struct (行 821–827)
- `add_collapsible_info()` (行 829–849)
- `collapsible_tool_info()` (行 851–879)
- `collapsible_info_for_tool()` (行 881–896)
- `CollapsibleKind` enum (行 898–902)
- `collapsible_kind_for_tool()` (行 904–911)
- `counts_for_kind()` (行 913–919)
- `grouping_tool_use_key()` (行 921–939)
- `is_groupable_tool()` (行 941–943)
- `source_index_of()` (行 945–951)
- `tool_use_id()` (行 953–965)
- `tool_result_id()` (行 967–981)
- `is_api_error_message()` (行 983–994)
- `find_latest_bash_output_uuid()` (行 996–1015)
- `find_last_thinking_block_id()` (行 1017–1039)
- `message_content_blocks()` (行 1041–1046)
- `derive_group_uuid()` (行 1048–1050)
- `uuid_from_parent_and_salt()` (行 1052–1058)
- `message_type_key()` (行 1060–1068)

### `render_user.rs` (~300 行)
- `render_user_message()` (行 1430–1506)
- `render_user_image_blocks()` (行 1508–1522)
- `render_tagged_user_text()` (行 1524–1548)
- `extract_xmlish_body()` (行 1550–1554)
- `render_tool_result_user_message()` (行 1556–1635)
- `render_file_edit_preview()` (行 1637–1676)
- `tool_result_content_text()` (行 1678–1691)
- `styled_text_lines()` (行 1693–1700)
- `plain_text_to_lines()` (行 1702–1709)
- `api_error_display_text()` (行 1711–1719)

### `render_assistant.rs` (~400 行)
- `render_assistant_message()` (行 1721–1961)
- `tool_state_for_id()` (行 1963–1981)
- `render_system_message()` (行 1983–2060)
- `render_progress_message()` (行 2062–2083)
- `render_attachment_message()` (行 2085–2117)

### `copy_text.rs` (~240 行)
- `abbreviate_json()` (行 2119–2130)
- `tool_input_summary()` (行 2132–2138)
- `tool_primary_input()` (行 2140–2163)
- `message_content_copy_text()` (行 2165–2174)
- `message_content_reference()` (行 2176–2181)
- `content_block_copy_text()` (行 2183–2196)
- `content_block_reference()` (行 2198–2214)
- `extract_code_path_reference()` (行 2216–2221)
- `clean_path_candidate()` (行 2223–2234)
- `looks_like_code_path()` (行 2236–2279)
- `image_reference()` (行 2281–2287)
- `attachment_copy_text()` (行 2289–2302)
- `attachment_reference()` (行 2304–2315)
- `strip_system_reminders()` (行 2317–2328)
- `compact_boundary_summary()` (行 2330–2348)

---

## 4. 模块间依赖关系

```
mod.rs
 ├── context.rs          (MessageRenderContext, RenderableMessage, ToolUseRenderRecord)
 ├── preprocessing.rs    (依赖 context::RenderableMessage)
 ├── grouping.rs         (依赖 context::RenderableMessage, preprocessing 输出)
 ├── render_user.rs      (依赖 context::MessageRenderContext, cc_types::message)
 ├── render_assistant.rs (依赖 context::MessageRenderContext, render_user 的部分辅助类型)
 └── copy_text.rs        (独立，仅依赖 cc_types::message)
```

- `copy_text.rs` 是完全独立的叶子模块，无内部依赖
- `preprocessing.rs` 和 `grouping.rs` 有上下游关系（grouping 接收 preprocessing 的输出）
- `render_user.rs` 和 `render_assistant.rs` 都依赖 context，但相互之间无直接依赖
- `mod.rs` 中的 `render_messages()` 是唯一入口，串联所有子模块

---

## 5. 重构步骤（迁移顺序）

1. **创建 `render/` 目录**，将 `render.rs` 暂时改名为 `render/mod.rs`
2. **提取 `copy_text.rs`** — 最独立，提取后立即验证编译
3. **提取 `context.rs`** — 核心数据结构，所有模块的依赖基础
4. **提取 `preprocessing.rs`** — 消息预处理流水线
5. **提取 `grouping.rs`** — 分组逻辑（依赖 preprocessing 的输出类型）
6. **提取 `render_user.rs`** — 用户消息渲染
7. **提取 `render_assistant.rs`** — 助手+系统+进度+附件渲染
8. **清理 `mod.rs`** — 仅保留入口函数 `render_messages()` + 选中装饰 + re-exports
9. **运行 `cargo build --release`** 验证无编译错误
10. **运行 `cargo test`** 验证所有测试通过

---

## 6. 注意事项

1. **外部 API 不变**: `mod.rs` 通过 `pub use` 确保 `crate::ui::messages::render::render_messages` 等公共符号路径不变
2. **生命周期标注**: 大量 `fn<'a>` 返回 `Vec<Line<'a>>` 的函数，拆分时需确保 `use` 引入正确的 ratatui 类型
3. **测试迁移**: `mod tests` (~600 行) 建议保留在 `mod.rs` 中或拆到 `tests.rs`，测试函数引用多个子模块
4. **`USER_MESSAGE_BACKGROUND` 常量**: 被 `render_user_message` 和 `decorate_selected_message` 共用，放在 `mod.rs` 或 `context.rs` 中
5. **`apply_grouping` 与 `collapse_read_search_groups`**: 两者存在调用关系，必须在同一模块（`grouping.rs`）中
6. **渐进式重构**: 每步提取后立即编译验证，避免一次性大改导致调试困难
