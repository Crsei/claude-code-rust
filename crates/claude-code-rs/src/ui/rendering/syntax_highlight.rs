//! Syntax highlighting for code blocks.
//!
//! Uses `syntect` when the `syntect` feature is enabled, providing
//! token-level coloring for fenced code blocks.
//!
//! When the feature is disabled, all functions degrade gracefully to
//! plain-text passthrough.

use ratatui::text::Span;

use super::theme::Theme;

// ---------------------------------------------------------------------------
// Language detection helpers
// ---------------------------------------------------------------------------

/// Map pulldown_cmark language identifiers to syntect syntax names.
///
/// pulldown_cmark gives us the raw info string from ```lang. Syntect
/// uses slightly different names for some languages.
const LANG_ALIASES: &[(&str, &[&str])] = &[
    ("js", &["javascript", "js", "node"]),
    ("ts", &["typescript", "ts"]),
    ("tsx", &["tsx", "typescriptreact"]),
    ("jsx", &["jsx", "javascriptreact"]),
    ("py", &["python", "py", "python3"]),
    ("rb", &["ruby", "rb"]),
    ("rs", &["rust", "rs"]),
    ("go", &["go", "golang"]),
    ("rs", &["rust", "rs"]),
    ("sh", &["shell", "sh", "bash", "zsh"]),
    ("yml", &["yaml", "yml"]),
    ("json", &["json"]),
    ("toml", &["toml"]),
    ("md", &["markdown", "md"]),
    ("html", &["html"]),
    ("css", &["css"]),
    ("sql", &["sql"]),
    ("c", &["c"]),
    ("cpp", &["cpp", "c++", "cc"]),
    ("h", &["c", "h"]),
    ("java", &["java"]),
    ("kt", &["kotlin", "kt"]),
    ("swift", &["swift"]),
    ("dart", &["dart"]),
    ("lua", &["lua"]),
    ("php", &["php"]),
    ("r", &["r"]),
    ("scala", &["scala"]),
    ("hs", &["haskell", "hs"]),
    ("ml", &["ocaml", "ml"]),
    ("nim", &["nim"]),
    ("ps1", &["powershell", "ps1"]),
    ("dockerfile", &["dockerfile"]),
    ("makefile", &["makefile", "make"]),
    ("graphql", &["graphql", "gql"]),
    ("proto", &["protobuf", "proto"]),
    ("tex", &["latex", "tex"]),
    ("xml", &["xml"]),
    ("yaml", &["yaml"]),
    ("plaintext", &["plaintext", "text", "txt"]),
];

/// Resolve a raw language string (from fence info) to a syntect-compatible
/// syntax name, or None if unknown.
fn resolve_lang(lang: &str) -> Option<&'static str> {
    let lower = lang.to_lowercase();
    for &(canonical, aliases) in LANG_ALIASES {
        if aliases.contains(&lower.as_str()) || canonical == lower {
            return Some(canonical);
        }
    }
    None
}

// ---------------------------------------------------------------------------
// Non-syntect fallback
// ---------------------------------------------------------------------------

/// When syntect is disabled, highlight_code_block returns spans with the
/// theme's code style.
fn fallback_highlight(code: &str, theme: &Theme) -> Vec<Span<'static>> {
    code.lines()
        .flat_map(|line| {
            let mut spans = vec![Span::styled(line.to_string(), theme.code)];
            spans.push(Span::raw("\n"));
            spans
        })
        .collect()
}

// ---------------------------------------------------------------------------
// Syntect-enabled implementation
// ---------------------------------------------------------------------------

#[cfg(feature = "syntect")]
mod imp {
    use std::sync::OnceLock;

    use ratatui::style::{Color, Modifier, Style};
    use ratatui::text::Span;
    use syntect::highlighting::{FontStyle, Style as SyntectStyle, ThemeSet};
    use syntect::parsing::SyntaxSet;

    use super::resolve_lang;
    use crate::ui::theme::Theme;

    /// Lazily-loaded syntax set (cached across all calls).
    fn syntax_set() -> &'static SyntaxSet {
        static SS: OnceLock<SyntaxSet> = OnceLock::new();
        SS.get_or_init(SyntaxSet::load_defaults_newlines)
    }

    fn theme_set() -> &'static ThemeSet {
        static TS: OnceLock<ThemeSet> = OnceLock::new();
        TS.get_or_init(ThemeSet::load_defaults)
    }

    /// Convert a syntect token style to ratatui `Style`.
    fn syntect_style_to_ratatui(s: &SyntectStyle, base: Style) -> Style {
        let mut result = base;

        let c = s.foreground;
        if c.a > 0 {
            result = result.fg(Color::Rgb(c.r, c.g, c.b));
        }

        if s.font_style.contains(FontStyle::BOLD) {
            result = result.add_modifier(Modifier::BOLD);
        }
        if s.font_style.contains(FontStyle::ITALIC) {
            result = result.add_modifier(Modifier::ITALIC);
        }
        if s.font_style.contains(FontStyle::UNDERLINE) {
            result = result.add_modifier(Modifier::UNDERLINED);
        }

        result
    }

    /// Highlight code with syntect, producing ratatui spans.
    pub(crate) fn highlight(code: &str, lang: &str, theme: &Theme) -> Vec<Span<'static>> {
        let ss = syntax_set();

        // Resolve language
        let syntax = if lang.is_empty() {
            None
        } else {
            let lang = resolve_lang(lang).unwrap_or(lang);
            ss.find_syntax_by_token(lang)
        };

        let syntax = match syntax {
            Some(s) => s,
            None => {
                // Fallback: try by extension or first newline token
                ss.find_syntax_by_extension(lang)
                    .or_else(|| ss.find_syntax_by_first_line(code))
                    .unwrap_or_else(|| ss.find_syntax_plain_text())
            }
        };

        let mut highlighter =
            syntect::easy::HighlightLines::new(syntax, &theme_set().themes["base16-ocean.dark"]);

        let mut spans: Vec<Span<'static>> = Vec::new();
        for line in code.lines() {
            let Ok(ranges) = highlighter.highlight_line(line, ss) else {
                // If syntect fails on a line, fall back to theme.code
                spans.push(Span::styled(line.to_string(), theme.code));
                spans.push(Span::raw("\n"));
                continue;
            };

            for (style, text) in ranges {
                let ratatui_style = syntect_style_to_ratatui(&style, theme.code);
                spans.push(Span::styled(text.to_string(), ratatui_style));
            }
            spans.push(Span::raw("\n"));
        }

        spans
    }
}

#[cfg(not(feature = "syntect"))]
mod imp {
    use ratatui::text::Span;

    use super::fallback_highlight;
    use crate::ui::theme::Theme;

    pub(crate) fn highlight(code: &str, lang: &str, theme: &Theme) -> Vec<Span<'static>> {
        let _ = lang; // unused without syntect
        fallback_highlight(code, theme)
    }
}

// ---------------------------------------------------------------------------
// Public API
// ---------------------------------------------------------------------------

/// Highlight a code block with syntax coloring.
///
/// When the `syntect` feature is enabled, uses syntect for token-level
/// coloring based on the detected language. Falls back to `theme.code`
/// style when syntect is disabled or the language is unsupported.
pub fn highlight_code_block(code: &str, lang: &str, theme: &Theme) -> Vec<Span<'static>> {
    if code.is_empty() {
        return vec![Span::styled(String::new(), theme.code)];
    }

    // Fast path: no language = no syntax highlighting
    if lang.is_empty() {
        return fallback_highlight(code, theme);
    }

    imp::highlight(code, lang, theme)
}

/// Check whether a given language identifier is supported for highlighting.
pub fn supports_language(lang: &str) -> bool {
    if lang.is_empty() {
        return false;
    }
    resolve_lang(lang).is_some()
}

/// Return the list of all supported language identifiers.
pub fn supported_languages() -> Vec<&'static str> {
    let mut langs: Vec<&str> = LANG_ALIASES
        .iter()
        .map(|(canonical, _)| *canonical)
        .collect();
    langs.sort();
    langs.dedup();
    langs
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolve_common_languages() {
        assert_eq!(resolve_lang("rs"), Some("rs"));
        assert_eq!(resolve_lang("Rust"), Some("rs"));
        assert_eq!(resolve_lang("py"), Some("py"));
        assert_eq!(resolve_lang("javascript"), Some("js"));
        assert_eq!(resolve_lang("unknown_lang_12345"), None);
    }

    #[test]
    fn empty_code_returns_empty_span() {
        let theme = Theme::default();
        let spans = highlight_code_block("", "rs", &theme);
        assert_eq!(spans.len(), 1);
    }

    #[test]
    fn no_lang_returns_fallback() {
        let theme = Theme::default();
        let spans = highlight_code_block("fn main() {}", "", &theme);
        assert!(!spans.is_empty());
    }

    #[test]
    fn supports_detects_known_languages() {
        assert!(supports_language("rust"));
        assert!(supports_language("python"));
        assert!(!supports_language(""));
        assert!(!supports_language("foobarbaz"));
    }
}
