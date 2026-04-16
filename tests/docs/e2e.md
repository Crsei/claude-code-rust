# E2E 黑盒测试

通过 `assert_cmd` 启动编译后的二进制，以黑盒方式验证 CLI 行为、环境配置、工具注册、上下文压缩、数据导出和 API 调用。

## 运行方式

```bash
# 所有 offline E2E 测试
cargo test --test e2e_cli --test e2e_env --test e2e_tools --test e2e_compact --test e2e_audit_export --test e2e_session_export

# 单个文件
cargo test --test e2e_cli

# live 测试 (需要 .env 中的 API key)
cargo test --test e2e_live_api -- --ignored
cargo test --test e2e_terminal -- --ignored
```

## 依赖

```toml
[dev-dependencies]
assert_cmd = "2"        # 启动二进制、断言退出码
predicates = "3"        # 输出内容断言
assert_fs = "1"         # 临时目录/文件断言
tempfile = "3"          # 临时文件
serde_json = "1"        # JSON 解析
sha2 = "0.10"           # 哈希验证 (审计)
hex = "0.4"             # hex 编码 (审计)
```

---

## e2e_cli.rs — CLI 参数解析 (16 tests)

验证 CLI flag 的接受、拒绝、组合行为。

| 测试 | 说明 |
|------|------|
| `version_flag_prints_version_and_exits` | `-V` 输出版本号并退出 |
| `version_long_flag` | `--version` 同上 |
| `init_only_exits_successfully` | `--init-only` 初始化后立即退出 |
| `dump_system_prompt_outputs_prompt_and_exits` | `--dump-system-prompt` 输出提示词 |
| `cwd_flag_accepts_valid_directory` | `-C` 接受有效目录 |
| `cwd_flag_rejects_nonexistent_directory` | `-C` 拒绝不存在的路径 |
| `cwd_short_flag_works` | `-C` 短 flag 等效 |
| `print_mode_without_prompt_fails` | `-p` 无 prompt 时失败 |
| `print_mode_no_api_key_reports_error` | `-p` 无 API key 报错 |
| `model_flag_accepted` | `-m` 指定模型 |
| `dump_system_prompt_with_model_override` | `--dump-system-prompt -m` 组合 |
| `verbose_flag_accepted` | `-v` verbose 模式 |
| `custom_system_prompt_in_dump` | `--system-prompt` 自定义提示词 |
| `append_system_prompt_in_dump` | `--append-system-prompt` 追加提示词 |
| `permission_mode_auto_accepted` | `--permission-mode auto` |
| `permission_mode_bypass_accepted` | `--permission-mode bypass` |
| `max_budget_flag_accepted` | `--max-budget` 预算上限 |
| `max_turns_flag_accepted` | `--max-turns` 轮数上限 |
| `unknown_flag_fails` | 未知 flag 报 clap 错误 |
| `max_turns_requires_value` | `--max-turns` 需要值 |

---

## e2e_env.rs — 环境与认证 (13 tests)

验证 `.env` 文件加载、环境变量检测、provider 优先级、CLI flag 覆盖。

| 测试 | 说明 |
|------|------|
| `no_api_key_shows_warning_on_init` | 无 key 时输出警告 |
| `env_file_in_cwd_is_loaded` | 当前目录的 .env 被加载 |
| `claude_model_env_var_overrides_default` | `CLAUDE_MODEL` 环境变量覆盖默认模型 |
| `cli_model_flag_overrides_env_var` | `-m` flag 优先于环境变量 |
| `anthropic_key_env_detected_no_warning` | 检测到 Anthropic key 无警告 |
| `openai_key_env_detected_no_warning` | 检测到 OpenAI key 无警告 |
| `azure_key_env_detected_no_warning` | 检测到 Azure key 无警告 |
| `env_file_in_workspace_cwd_loaded` | 工作区 .env 加载 |
| `permission_mode_env_var` | 环境变量设置权限模式 |
| `verbose_env_var` | 环境变量启用 verbose |
| `workspace_f_temp_exists_and_usable` | F:\temp 工作区可用 |
| `cwd_env_overrides_global_env` | 目录级环境变量覆盖全局 |

---

## e2e_tools.rs — 工具注册 (31 tests)

验证 `--dump-system-prompt` 输出的系统提示词包含所有已注册工具的描述和 JSON schema。

| 测试 | 说明 |
|------|------|
| `system_prompt_contains_bash_tool` | Bash 工具 |
| `system_prompt_contains_read_tool` | Read 工具 |
| `system_prompt_contains_write_tool` | Write 工具 |
| `system_prompt_contains_edit_tool` | Edit 工具 |
| `system_prompt_contains_glob_tool` | Glob 工具 |
| `system_prompt_contains_grep_tool` | Grep 工具 |
| `system_prompt_contains_askuser_tool` | AskUser 工具 |
| `system_prompt_contains_skill_tool` | Skill 工具 |
| `system_prompt_contains_agent_tool` | Agent 工具 |
| `system_prompt_contains_agent_schema` | Agent JSON schema |
| `system_prompt_contains_webfetch_tool` | WebFetch 工具 |
| `system_prompt_contains_websearch_tool` | WebSearch 工具 |
| `system_prompt_contains_enter_plan_mode_tool` | EnterPlanMode |
| `system_prompt_contains_exit_plan_mode_tool` | ExitPlanMode |
| `system_prompt_contains_task_create_tool` | TaskCreate |
| `system_prompt_contains_task_update_tool` | TaskUpdate |
| `system_prompt_contains_task_list_tool` | TaskList |
| `system_prompt_contains_enter_worktree_tool` | EnterWorktree |
| `system_prompt_contains_exit_worktree_tool` | ExitWorktree |
| `system_prompt_contains_lsp_tool` | LSP 工具 |
| `system_prompt_contains_lsp_schema` | LSP JSON schema |
| `system_prompt_omits_send_message_by_default` | SendMessage 默认不出现 |
| `system_prompt_contains_environment_section` | 环境信息段落 |
| `system_prompt_contains_cwd_path` | 当前工作目录 |
| `system_prompt_mentions_permission_model` | 权限模型说明 |
| `print_mode_bash_tool_no_crash_without_api` | -p Bash 不崩溃 |
| `print_mode_read_tool_no_crash_without_api` | -p Read 不崩溃 |
| `dump_system_prompt_with_verbose_and_model` | verbose + model 组合 |
| `init_only_with_cwd_and_permission_mode` | init-only + cwd + permission 组合 |

---

## e2e_compact.rs — 上下文压缩 (10 tests)

验证 compact 管道的阈值计算、微压缩策略、snip 裁剪和工具结果预算。

| 测试 | 说明 |
|------|------|
| `binary_starts_with_version` | 二进制启动正常 |
| `system_prompt_generated_successfully` | 提示词生成正常 |
| `compact_module_compiles` | compact 模块编译通过 |
| `print_mode_does_not_crash_without_api_key` | -p 无 key 不崩溃 |
| `tool_result_temp_dir_is_writable` | 临时目录可写 |
| `tool_result_path_isolation` | 路径隔离 (.cc-rust) |
| `four_chars_per_token_heuristic` | 4字符/token 启发式 |
| `auto_compact_threshold_calculation` | 自动压缩阈值 |
| `microcompact_threshold` | 微压缩阈值 |
| `snip_default_max_turns` | snip 默认最大轮数 |

---

## e2e_audit_export.rs — 审计导出 (6 tests)

验证审计记录的序列化、哈希链完整性和篡改检测。

| 测试 | 说明 |
|------|------|
| `init_succeeds_with_audit_export` | 审计模块初始化 |
| `system_prompt_unaffected_by_audit_export` | 不影响系统提示词 |
| `audit_record_roundtrip_and_verify` | 记录序列化往返 |
| `tamper_detection_catches_modified_entry` | 检测修改篡改 |
| `tamper_detection_catches_inserted_entry` | 检测插入篡改 |
| `audit_record_schema_has_required_fields` | schema 字段完整 |

---

## e2e_session_export.rs — 会话导出 (9 tests)

验证会话导出的 JSON 结构、工具调用时间线重建、compact 边界提取。

| 测试 | 说明 |
|------|------|
| `init_succeeds_with_session_export` | 导出模块初始化 |
| `system_prompt_unaffected_by_session_export` | 不影响系统提示词 |
| `session_export_roundtrip` | JSON 往返 |
| `tool_timeline_reconstruction` | 工具时间线重建 |
| `compact_boundary_extraction` | compact 边界提取 |
| `content_replacement_detection` | 内容替换检测 |
| `microcompact_detection` | 微压缩检测 |
| `context_snapshot_schema` | 上下文快照 schema |

---

## e2e_services.rs — 服务集成 (12 tests)

验证 tool_use_summary 和 prompt_suggestion 服务的模块链接和 live 回归。

| 测试 | 说明 |
|------|------|
| `init_succeeds_with_summary_integration` | summary 集成初始化 |
| `system_prompt_unaffected_by_summary_integration` | 不影响提示词 |
| `print_mode_graceful_without_api` | -p 无 key 优雅降级 |
| `init_succeeds_with_suggestion_integration` | suggestion 集成初始化 |
| `combined_flags_work_with_suggestions_field` | 组合 flag |
| `live_summary_single_bash_tool` | (live) 单工具摘要 |
| `live_summary_multi_tool_no_corruption` | (live) 多工具不损坏 |
| `live_summary_injected_across_turns` | (live) 跨轮注入 |
| `live_suggestions_no_crash_after_tool_use` | (live) 工具后不崩溃 |
| `live_suggestions_no_crash_chat_only` | (live) 纯聊天不崩溃 |

---

## e2e_live_api.rs — 真实 API 调用 (17 tests, 全部 `#[ignore]`)

分两层：Tier 1 (纯聊天) 和 Tier 2 (工具调用)。需要 `.env` 中配置 API key。

### Tier 1 — Chat

| 测试 | 说明 |
|------|------|
| `t1_simple_question_returns_answer` | 简单问答 |
| `t1_simple_chinese_question` | 中文问答 |
| `t1_say_exact_phrase` | 精确复述 |
| `t1_custom_system_prompt` | 自定义系统提示词 |
| `t1_append_system_prompt` | 追加系统提示词 |
| `t1_max_turns_one` | 单轮限制 |
| `t1_print_mode_clean_text` | -p 输出干净文本 |
| `t1_env_file_provides_working_credentials` | .env 凭证可用 |

### Tier 2 — Tool Use

| 测试 | 说明 |
|------|------|
| `t2_bash_echo` | Bash echo |
| `t2_bash_pwd_shows_workspace` | Bash pwd |
| `t2_read_file` | 读文件 |
| `t2_write_file` | 写文件 |
| `t2_edit_file` | 编辑文件 |
| `t2_glob_finds_files` | Glob 搜索 |
| `t2_grep_searches_content` | Grep 搜索 |
| `t2_multi_tool_write_read_edit` | 多工具组合 |
| `t2_read_nonexistent_file_graceful` | 读不存在文件优雅降级 |

---

## e2e_terminal.rs — Headless IPC 协议 (14 tests)

通过 `--headless` 模式 spawn 二进制，验证 JSONL IPC 协议的消息格式和生命周期。共享 helper 位于 `e2e_terminal/helpers.rs`。

| 测试 | 说明 |
|------|------|
| `headless_emits_ready_on_start` | 启动后发送 ready 消息 |
| `headless_quit_exits_cleanly` | quit 消息正常退出 |
| `headless_invalid_json_returns_error` | 无效 JSON 返回可恢复错误 |
| `headless_unknown_message_type_returns_error` | 未知消息类型返回可恢复错误 |
| `headless_resize_accepted` | resize 消息静默接受 |
| `headless_slash_command_returns_warning` | slash_command 返回 "not yet supported" 警告 |
| `headless_submit_prompt_no_api_key_returns_error` | 无 API key 提交 prompt 报错 |
| `headless_multiple_messages_in_sequence` | resize → slash_command → bad json 连续处理 |
| `headless_cwd_in_ready_message` | ready 消息包含 `-C` 指定的 cwd |
| `headless_model_override_in_ready` | ready 消息包含 `-m` 指定的 model |
| `headless_stdin_close_causes_exit` | 关闭 stdin 导致进程退出 |
| `headless_permission_response_no_pending` | 无 pending 的权限回复不崩溃 |
| `live_headless_simple_chat` | (live) stream_start → delta* → stream_end → assistant_message |
| `live_headless_tool_use_bash` | (live) tool_use + tool_result 消息 |
| `live_headless_two_prompts` | (live) 同一会话两轮对话 |
| `live_headless_abort_during_stream` | (live) 流中中断后恢复 |
