#[allow(dead_code)]
pub mod app;
#[allow(dead_code)]
pub mod approval_overlay;
#[allow(dead_code)]
pub mod bottom_pane;
pub mod browser;
#[allow(dead_code)]
pub mod capability_contract;
#[allow(dead_code)]
pub mod chat_composer;
#[allow(dead_code)]
pub mod clipboard_paste;
#[allow(dead_code)]
pub mod clipboard_text;
pub mod command_palette;
#[allow(dead_code)]
pub mod diff;
#[allow(dead_code)]
pub mod event_router;
#[allow(dead_code)]
pub mod feature_panels;
#[allow(dead_code)]
pub mod frame_requester;
#[allow(dead_code)]
pub mod history_cell;
#[allow(dead_code)]
pub mod keybindings;
#[allow(dead_code)]
pub mod markdown;
#[allow(dead_code)]
pub mod messages;
#[allow(dead_code)]
pub mod permissions;
#[allow(dead_code)]
pub mod prompt_input;
#[allow(dead_code)]
pub mod selection_surface;
#[allow(dead_code)]
pub mod spinner;
// `status_line` moved to `cc-engine` in Phase 6 (issue #75). Downstream
// consumers should import from `cc_engine::status_line` directly.
pub use cc_engine::status_line;
pub mod status_line_resolver;
#[allow(dead_code)]
pub mod status_widget;
#[allow(dead_code)]
pub mod streaming_controller;
pub mod terminal_env;
#[allow(dead_code)]
pub mod terminal_integration;
#[allow(dead_code)]
pub mod theme;
#[allow(dead_code)]
pub mod tool_activity;
pub mod transcript;
pub mod tui;
#[allow(dead_code)]
pub mod vim;
pub mod virtual_scroll;
#[allow(dead_code)]
pub mod visual_regression;
pub mod welcome;
