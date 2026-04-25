#[allow(dead_code)]
pub mod app;
pub mod browser;
pub mod command_palette;
#[allow(dead_code)]
pub mod diff;
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
pub mod spinner;
// `status_line` moved to `cc-engine` in Phase 6 (issue #75). Downstream
// consumers should import from `cc_engine::status_line` directly.
pub use cc_engine::status_line;
pub mod status_line_resolver;
pub mod terminal_env;
#[allow(dead_code)]
pub mod theme;
pub mod transcript;
pub mod tui;
#[allow(dead_code)]
pub mod vim;
pub mod virtual_scroll;
pub mod welcome;
