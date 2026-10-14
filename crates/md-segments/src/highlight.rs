//! Syntax highlighting for code segments.
//!
//! Highlights with syntect and returns one [`CodeLine`] (a `Vec<ColoredSpan>`)
//! per source line — plain data, cheap to compute off the UI thread and
//! trivial to turn into any frontend's per-span model (Slint: a nested `for
//! line in code-lines: for span in line.spans: Text { color: span.color }`;
//! SwiftUI: the same shape per `RowRecord`), unlike routing colored code
//! through a markdown renderer, which would need every span escaped and
//! indentation faked to survive markdown parsing. Copy actions use the
//! original code string directly. Display spans expand tabs and normalize
//! CRLF line endings. Every span's color is a concrete `(u8, u8,
//! u8)` RGB triple — a consumer needs no syntect theme/asset data of its own.

use std::sync::OnceLock;

use syntect::easy::HighlightLines;
use syntect::highlighting::{Style, Theme, ThemeSet};
use syntect::parsing::SyntaxSet;

#[derive(Debug, Clone, PartialEq)]
pub struct ColoredSpan {
    pub text: String,
    pub color: (u8, u8, u8),
}

#[derive(Debug, Clone, PartialEq)]
pub struct CodeLine {
    pub spans: Vec<ColoredSpan>,
}

struct Assets {
    syntaxes: SyntaxSet,
    dark: Theme,
    light: Theme,
}

fn assets() -> &'static Assets {
    static ASSETS: OnceLock<Assets> = OnceLock::new();
    ASSETS.get_or_init(|| {
        let mut themes = ThemeSet::load_defaults();
        Assets {
            syntaxes: SyntaxSet::load_defaults_newlines(),
            dark: themes
                .themes
                .remove("base16-ocean.dark")
                .expect("bundled theme"),
            light: themes
                .themes
                .remove("InspiredGitHub")
                .expect("bundled theme"),
        }
    })
}

/// The background color the active syntect theme was designed against.
/// Code cards must use this so span colors keep their intended contrast.
pub fn theme_background(dark: bool) -> (u8, u8, u8) {
    let assets = assets();
    let theme = if dark { &assets.dark } else { &assets.light };
    theme
        .settings
        .background
        .map(|c| (c.r, c.g, c.b))
        .unwrap_or(if dark { (43, 48, 59) } else { (255, 255, 255) })
}

/// Highlight `code` into one [`CodeLine`] per source line.
/// Tabs expand to four spaces and CRLF becomes LF. Line endings are excluded
/// from spans. A parsing error renders that line in the theme's foreground
/// and restarts highlighting on the next line.
pub fn highlight_lines(code: &str, lang: &str, dark: bool) -> Vec<CodeLine> {
    let assets = assets();
    let syntax = assets
        .syntaxes
        .find_syntax_by_token(lang)
        .unwrap_or_else(|| assets.syntaxes.find_syntax_plain_text());
    let theme = if dark { &assets.dark } else { &assets.light };
    let mut highlighter = HighlightLines::new(syntax, theme);

    // Slint's `Text` renders tabs as tofu glyphs, not tab stops.
    let mut code = code.replace("\r\n", "\n").replace('\t', "    ");
    if !code.is_empty() && !code.ends_with('\n') {
        code.push('\n');
    }
    code.split_inclusive('\n')
        .map(|line| {
            let spans = match highlighter.highlight_line(line, &assets.syntaxes) {
                Ok(spans) => spans
                    .into_iter()
                    .map(|(style, text)| (style, text.strip_suffix('\n').unwrap_or(text)))
                    .filter(|(_, text)| !text.is_empty())
                    .map(|(style, text)| colored_span(style, text))
                    .collect(),
                Err(_) => {
                    highlighter = HighlightLines::new(syntax, theme);
                    vec![ColoredSpan {
                        text: line.trim_end_matches('\n').to_string(),
                        color: theme
                            .settings
                            .foreground
                            .map(|c| (c.r, c.g, c.b))
                            .unwrap_or(if dark { (192, 197, 206) } else { (36, 41, 46) }),
                    }]
                }
            };
            CodeLine { spans }
        })
        .collect()
}

fn colored_span(style: Style, text: &str) -> ColoredSpan {
    let c = style.foreground;
    ColoredSpan {
        text: text.to_string(),
        color: (c.r, c.g, c.b),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn comments_end_at_the_line_boundary() {
        for (lang, source) in [
            ("python", "# comment\nx = 1"),
            ("javascript", "// comment\nlet x = 1;"),
            ("rust", "// comment\nlet x = 1;"),
        ] {
            let lines = highlight_lines(source, lang, true);
            let comment = lines[0].spans[0].color;
            assert!(lines[1].spans.iter().any(|s| s.color != comment), "{lang}");
            assert_eq!(lines, highlight_lines(&format!("{source}\n"), lang, true));
        }
    }

    #[test]
    fn multiline_strings_close_and_line_endings_preserve_display_text() {
        let source = "x = \"\"\"first\r\nsecond\r\n\"\"\"\r\nx = 1\r\n\r\n\tend";
        let lines = highlight_lines(source, "python", false);
        let rebuilt: Vec<String> = lines
            .iter()
            .map(|l| l.spans.iter().map(|s| s.text.as_str()).collect())
            .collect();
        assert_eq!(
            rebuilt,
            [
                "x = \"\"\"first",
                "second",
                "\"\"\"",
                "x = 1",
                "",
                "    end"
            ]
        );
        assert_eq!(lines[3], highlight_lines("x = 1", "python", false)[0]);
        assert!(lines[4].spans.is_empty());
        assert!(highlight_lines("", "python", false).is_empty());
    }

    #[test]
    fn rust_code_produces_colored_spans() {
        let lines = highlight_lines("fn main() { let x = 1; }", "rust", true);
        assert_eq!(lines.len(), 1);
        assert!(!lines[0].spans.is_empty());
        // Highlighted code isn't a single uniformly-colored run.
        let colors: std::collections::HashSet<_> = lines[0].spans.iter().map(|s| s.color).collect();
        assert!(colors.len() > 1);
    }

    #[test]
    fn span_text_concatenates_back_to_the_source_line() {
        let code = "a = '**not bold**' # <tag> `tick`";
        let lines = highlight_lines(code, "python", false);
        let rebuilt: String = lines[0].spans.iter().map(|s| s.text.as_str()).collect();
        assert_eq!(rebuilt, code);
    }

    #[test]
    fn unknown_language_falls_back_to_plain_syntax() {
        let lines = highlight_lines("whatever ^^ !!", "nosuchlang", true);
        assert_eq!(lines.len(), 1);
        let rebuilt: String = lines[0].spans.iter().map(|s| s.text.as_str()).collect();
        assert_eq!(rebuilt, "whatever ^^ !!");
    }

    #[test]
    fn theme_backgrounds_differ_by_scheme() {
        let dark = theme_background(true);
        let light = theme_background(false);
        assert_ne!(dark, light);
        // Dark theme background must actually be dark, light light.
        assert!((dark.0 as u16 + dark.1 as u16 + dark.2 as u16) < 300);
        assert!((light.0 as u16 + light.1 as u16 + light.2 as u16) > 600);
    }

    #[test]
    fn multiline_code_produces_one_code_line_per_source_line() {
        let lines = highlight_lines("a\nb", "", true);
        assert_eq!(lines.len(), 2);
    }

    #[test]
    fn tabs_are_expanded_to_spaces() {
        let lines = highlight_lines("\tindented", "", true);
        let rebuilt: String = lines[0].spans.iter().map(|s| s.text.as_str()).collect();
        assert_eq!(rebuilt, "    indented");
    }
}
