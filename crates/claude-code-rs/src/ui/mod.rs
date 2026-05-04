// Entry and feature-domain facades stay at the top level. Most leaf UI files
// live in responsibility folders below, while their public module names remain
// `crate::ui::<name>` for compatibility.

#[allow(dead_code)]
pub mod agents;
#[allow(dead_code)]
pub mod app;
#[allow(dead_code)]
pub mod diff;
#[allow(dead_code)]
pub mod hooks;
#[allow(dead_code)]
pub mod lsp_recommendation;
#[allow(dead_code)]
pub mod mcp;
#[allow(dead_code)]
pub mod memory;
#[allow(dead_code)]
pub mod messages;
#[allow(dead_code)]
pub mod permissions;
#[allow(dead_code)]
pub mod skills;
#[allow(dead_code)]
pub mod tasks;
#[allow(dead_code)]
pub mod teams;
pub mod tui;

// Components: interactive widgets and modal surfaces owned or orchestrated by
// App.
#[allow(dead_code)]
#[path = "components/approval_overlay.rs"]
pub mod approval_overlay;
#[allow(dead_code)]
#[path = "components/bottom_pane.rs"]
pub mod bottom_pane;
#[allow(dead_code)]
#[path = "components/chat_composer.rs"]
pub mod chat_composer;
#[allow(dead_code)]
#[path = "components/chatwidget.rs"]
pub mod chatwidget;
#[path = "components/command_palette/mod.rs"]
pub mod command_palette;
#[path = "components/command_surface/mod.rs"]
pub mod command_surface;
#[allow(dead_code)]
#[path = "components/cwd_prompt.rs"]
pub mod cwd_prompt;
#[allow(dead_code)]
#[path = "components/feature_panels.rs"]
pub mod feature_panels;
#[allow(dead_code)]
#[path = "components/fuzzy_match.rs"]
pub mod fuzzy_match;
#[allow(dead_code)]
#[path = "components/history_search_dialog.rs"]
pub mod history_search_dialog;
#[allow(dead_code)]
#[path = "components/pager_overlay.rs"]
pub mod pager_overlay;
#[allow(dead_code)]
#[path = "components/prompt_input.rs"]
pub mod prompt_input;
#[allow(dead_code)]
#[path = "components/resume_picker.rs"]
pub mod resume_picker;
#[allow(dead_code)]
#[path = "components/search_box.rs"]
pub mod search_box;
#[allow(dead_code)]
#[path = "components/selection_surface.rs"]
pub mod selection_surface;
#[allow(dead_code)]
#[path = "components/status_widget.rs"]
pub mod status_widget;
#[allow(dead_code)]
#[path = "components/tooltops.rs"]
pub mod tooltops;
#[path = "components/welcome.rs"]
pub mod welcome;

// Input: keyboard editing, command parsing, mentions, clipboard, and picker
// navigation helpers.
#[allow(dead_code)]
#[path = "input/clipboard_paste.rs"]
pub mod clipboard_paste;
#[allow(dead_code)]
#[path = "input/clipboard_text.rs"]
pub mod clipboard_text;
#[allow(dead_code)]
#[path = "input/file_search.rs"]
pub mod file_search;
#[path = "input/form_navigation.rs"]
pub mod form_navigation;
#[allow(dead_code)]
#[path = "input/insert_history.rs"]
pub mod insert_history;
#[allow(dead_code)]
#[path = "input/keybindings.rs"]
pub mod keybindings;
#[allow(dead_code)]
#[path = "input/mention_codec.rs"]
pub mod mention_codec;
#[allow(dead_code)]
#[path = "input/slash_command.rs"]
pub mod slash_command;
#[allow(dead_code)]
#[path = "input/vim.rs"]
pub mod vim;

// Rendering: shared text shaping, visual effects, styles, scroll calculation,
// and message/tool presentation helpers.
#[allow(dead_code)]
#[path = "rendering/get_git_diff.rs"]
pub mod get_git_diff;
#[allow(dead_code)]
#[path = "rendering/history_cell.rs"]
pub mod history_cell;
#[allow(dead_code)]
#[path = "rendering/markdown.rs"]
pub mod markdown;
#[allow(dead_code)]
#[path = "rendering/markdown_render.rs"]
pub mod markdown_render;
#[allow(dead_code)]
#[path = "rendering/markdown_stream.rs"]
pub mod markdown_stream;
#[allow(dead_code)]
#[path = "rendering/progress_bar.rs"]
pub mod progress_bar;
#[allow(dead_code)]
#[path = "rendering/shimmer.rs"]
pub mod shimmer;
#[allow(dead_code)]
#[path = "rendering/spinner.rs"]
pub mod spinner;
#[allow(dead_code)]
#[path = "rendering/theme.rs"]
pub mod theme;
#[allow(dead_code)]
#[path = "rendering/tool_activity.rs"]
pub mod tool_activity;
#[path = "rendering/virtual_scroll.rs"]
pub mod virtual_scroll;

// Runtime: non-visual state machines, event routing, streaming control, logs,
// transcript state, and visual regression fixtures.
#[allow(dead_code)]
#[path = "runtime/capability_contract.rs"]
pub mod capability_contract;
#[allow(dead_code)]
#[path = "runtime/event_router.rs"]
pub mod event_router;
#[allow(dead_code)]
#[path = "runtime/frame_requester.rs"]
pub mod frame_requester;
#[allow(dead_code)]
#[path = "runtime/session_log.rs"]
pub mod session_log;
#[allow(dead_code)]
#[path = "runtime/streaming_controller.rs"]
pub mod streaming_controller;
#[path = "runtime/transcript.rs"]
pub mod transcript;
#[allow(dead_code)]
#[path = "runtime/visual_regression.rs"]
pub mod visual_regression;

// Platform: terminal/browser/audio/environment adapters.
#[allow(dead_code)]
#[path = "platform/audio_device.rs"]
pub mod audio_device;
#[path = "platform/browser.rs"]
pub mod browser;
#[allow(dead_code)]
#[path = "platform/custom_terminal.rs"]
pub mod custom_terminal;
#[allow(dead_code)]
#[path = "platform/debug_config.rs"]
pub mod debug_config;
#[path = "platform/terminal_env.rs"]
pub mod terminal_env;
#[allow(dead_code)]
#[path = "platform/terminal_integration.rs"]
pub mod terminal_integration;

// Helpers and cross-crate status-line integration.
#[allow(dead_code)]
#[path = "helpers/skills_helpers.rs"]
pub mod skills_helpers;
// `status_line` moved to `cc-engine` in Phase 6 (issue #75). Downstream
// consumers should import from `cc_engine::status_line` directly.
pub use cc_engine::status_line;
#[path = "status/status_line_resolver.rs"]
pub mod status_line_resolver;
