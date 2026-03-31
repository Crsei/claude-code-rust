use ratatui::style::{Color, Modifier, Style};

/// Color theme for the terminal UI.
///
/// All styles are pre-composed so widgets can simply clone the style they need
/// without having to compose foreground/background/modifiers at render time.
pub struct Theme {
    /// Style for the "Claude:" prefix on assistant messages.
    pub assistant_name: Style,
    /// Style for the "You:" prefix on user messages.
    pub user_name: Style,
    /// Style for system messages.
    pub system_name: Style,
    /// Style for tool names shown in tool_use blocks.
    pub tool_name: Style,
    /// Style for tool result content.
    pub tool_result: Style,
    /// Style for error text.
    pub error: Style,
    /// Style for warning text.
    pub warning: Style,
    /// Style for informational text.
    pub info: Style,
    /// Style for the input prompt symbol (e.g. "> ").
    pub prompt: Style,
    /// Style for borders and separators.
    pub border: Style,
    /// Style for inline code and code blocks.
    pub code: Style,
    /// Style for thinking blocks.
    pub thinking: Style,
    /// Style for dimmed / secondary text.
    pub dim: Style,
    /// Style for headings in markdown.
    pub heading: Style,
    /// Style for bold text.
    pub bold: Style,
    /// Style for italic text.
    pub italic: Style,
    /// Style for links in markdown.
    pub link: Style,
    /// Style for diff additions.
    pub diff_add: Style,
    /// Style for diff deletions.
    pub diff_remove: Style,
    /// Style for diff context (unchanged) lines.
    pub diff_context: Style,
    /// Style for the diff header/separator.
    pub diff_header: Style,
    /// Style for selected item in dialogs.
    pub selected: Style,
    /// Style for unselected items in dialogs.
    pub unselected: Style,
}

impl Default for Theme {
    fn default() -> Self {
        Self {
            assistant_name: Style::default()
                .fg(Color::Rgb(190, 140, 255))
                .add_modifier(Modifier::BOLD),
            user_name: Style::default()
                .fg(Color::Rgb(100, 200, 255))
                .add_modifier(Modifier::BOLD),
            system_name: Style::default()
                .fg(Color::Rgb(180, 180, 180))
                .add_modifier(Modifier::ITALIC),
            tool_name: Style::default()
                .fg(Color::Rgb(255, 200, 100))
                .add_modifier(Modifier::BOLD),
            tool_result: Style::default().fg(Color::Rgb(160, 160, 160)),
            error: Style::default()
                .fg(Color::Rgb(255, 100, 100))
                .add_modifier(Modifier::BOLD),
            warning: Style::default().fg(Color::Rgb(255, 200, 80)),
            info: Style::default().fg(Color::Rgb(130, 200, 255)),
            prompt: Style::default()
                .fg(Color::Rgb(100, 200, 255))
                .add_modifier(Modifier::BOLD),
            border: Style::default().fg(Color::Rgb(80, 80, 80)),
            code: Style::default()
                .fg(Color::Rgb(220, 220, 180))
                .bg(Color::Rgb(40, 40, 40)),
            thinking: Style::default()
                .fg(Color::Rgb(120, 120, 120))
                .add_modifier(Modifier::ITALIC),
            dim: Style::default().fg(Color::Rgb(100, 100, 100)),
            heading: Style::default()
                .fg(Color::Rgb(255, 255, 255))
                .add_modifier(Modifier::BOLD | Modifier::UNDERLINED),
            bold: Style::default().add_modifier(Modifier::BOLD),
            italic: Style::default().add_modifier(Modifier::ITALIC),
            link: Style::default()
                .fg(Color::Rgb(100, 180, 255))
                .add_modifier(Modifier::UNDERLINED),
            diff_add: Style::default().fg(Color::Rgb(100, 220, 100)),
            diff_remove: Style::default().fg(Color::Rgb(255, 100, 100)),
            diff_context: Style::default().fg(Color::Rgb(180, 180, 180)),
            diff_header: Style::default()
                .fg(Color::Rgb(130, 170, 255))
                .add_modifier(Modifier::BOLD),
            selected: Style::default()
                .fg(Color::Rgb(0, 0, 0))
                .bg(Color::Rgb(100, 200, 255))
                .add_modifier(Modifier::BOLD),
            unselected: Style::default().fg(Color::Rgb(200, 200, 200)),
        }
    }
}

impl Theme {
    /// Create a new theme with default colors.
    pub fn new() -> Self {
        Self::default()
    }
}
