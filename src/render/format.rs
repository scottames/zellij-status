use std::collections::BTreeMap;

use anstyle::{Color, Effects, Style};

use super::color::parse_color;

/// A styled segment of text parsed from a format string.
///
/// Format strings use `#[fg=color,bg=color,bold,dim,fill]content` syntax.
/// Multiple segments are created by splitting on `#[`.
#[derive(Debug, Clone)]
pub struct FormattedPart {
    /// Foreground color.
    pub fg: Option<Color>,
    /// Background color.
    pub bg: Option<Color>,
    /// Text effects (bold, italic, dim, etc.).
    pub effects: Effects,
    /// Whether this segment should fill the remaining row width (vertical mode).
    pub fill: bool,
    /// Text content with `{widget}` placeholders.
    pub content: String,
}

impl FormattedPart {
    /// Parse a single format directive + content string.
    ///
    /// Input is the text after `#[` has been stripped, e.g.:
    /// `"fg=red,bold]hello world"` or just `"plain text"` (no directive).
    pub fn from_format_string(input: &str, aliases: &BTreeMap<String, String>) -> Self {
        // Split on first `]` to separate style directive from content
        let (directive, content) = match input.find(']') {
            Some(pos) => (&input[..pos], &input[pos + 1..]),
            None => {
                // No directive — entire string is content
                return Self {
                    fg: None,
                    bg: None,
                    effects: Effects::new(),
                    fill: false,
                    content: input.to_string(),
                };
            }
        };

        let mut fg = None;
        let mut bg = None;
        let mut effects = Effects::new();
        let mut fill = false;

        for part in directive.split(',') {
            let part = part.trim();

            if let Some(color_str) = part.strip_prefix("fg=") {
                fg = parse_color(color_str, aliases);
            } else if let Some(color_str) = part.strip_prefix("bg=") {
                bg = parse_color(color_str, aliases);
            } else {
                match part.to_lowercase().as_str() {
                    "bold" => effects = effects | Effects::BOLD,
                    "dim" | "dimmed" => effects = effects | Effects::DIMMED,
                    "italic" | "italics" => effects = effects | Effects::ITALIC,
                    "underscore" | "underline" => {
                        effects = effects | Effects::UNDERLINE;
                    }
                    "blink" => effects = effects | Effects::BLINK,
                    "reverse" => effects = effects | Effects::INVERT,
                    "hidden" => effects = effects | Effects::HIDDEN,
                    "strikethrough" => effects = effects | Effects::STRIKETHROUGH,
                    "fill" => fill = true,
                    _ => {} // Unknown directives are silently ignored
                }
            }
        }

        Self {
            fg,
            bg,
            effects,
            fill,
            content: content.to_string(),
        }
    }

    /// Render the content with ANSI escape codes applied.
    pub fn render(&self, text: &str) -> String {
        let style = self.to_style();
        format!(
            "{}{}{}{}",
            style.render_reset(),
            style.render(),
            text,
            style.render_reset(),
        )
    }

    /// Render this part's content (without widget substitution) with styling.
    pub fn render_content(&self) -> String {
        self.render(&self.content)
    }

    /// Build an `anstyle::Style` from this part's colors and effects.
    fn to_style(&self) -> Style {
        let mut style = Style::new();
        style = style.fg_color(self.fg);
        style = style.bg_color(self.bg);
        style = style.effects(self.effects);
        style
    }
}

/// Parse a complete format string into multiple `FormattedPart` segments.
///
/// The format string is split on `#[` — each segment becomes a `FormattedPart`.
///
/// # Examples
///
/// ```text
/// "#[fg=red,bold]hello#[fg=blue]world"
/// → [FormattedPart { fg=red, bold, content="hello" },
///    FormattedPart { fg=blue, content="world" }]
///
/// "plain text"
/// → [FormattedPart { no style, content="plain text" }]
/// ```
pub fn parse_format_string(format: &str, aliases: &BTreeMap<String, String>) -> Vec<FormattedPart> {
    if format.is_empty() {
        return Vec::new();
    }

    let mut parts = Vec::new();

    // Split on `#[` — first segment may have no directive
    let segments: Vec<&str> = format.split("#[").collect();

    for (i, segment) in segments.iter().enumerate() {
        if segment.is_empty() {
            continue;
        }

        if i == 0 && !format.starts_with("#[") {
            // First segment without a `#[` prefix — plain text
            parts.push(FormattedPart {
                fg: None,
                bg: None,
                effects: Effects::new(),
                fill: false,
                content: segment.to_string(),
            });
        } else {
            parts.push(FormattedPart::from_format_string(segment, aliases));
        }
    }

    parts
}

#[cfg(test)]
mod tests {
    use super::*;

    fn empty_aliases() -> BTreeMap<String, String> {
        BTreeMap::new()
    }

    #[test]
    fn parse_plain_text() {
        let parts = parse_format_string("hello world", &empty_aliases());
        assert_eq!(parts.len(), 1);
        assert_eq!(parts[0].content, "hello world");
        assert!(parts[0].fg.is_none());
        assert!(parts[0].bg.is_none());
    }

    #[test]
    fn parse_single_styled_part() {
        let parts = parse_format_string("#[fg=red,bold]hello", &empty_aliases());
        assert_eq!(parts.len(), 1);
        assert_eq!(parts[0].content, "hello");
        assert!(parts[0].fg.is_some());
        assert!(parts[0].effects.contains(Effects::BOLD));
    }

    #[test]
    fn parse_multiple_styled_parts() {
        let parts = parse_format_string("#[fg=red]hello#[fg=blue]world", &empty_aliases());
        assert_eq!(parts.len(), 2);
        assert_eq!(parts[0].content, "hello");
        assert_eq!(parts[1].content, "world");
    }

    #[test]
    fn parse_mixed_plain_and_styled() {
        let parts = parse_format_string("prefix#[fg=green]styled", &empty_aliases());
        assert_eq!(parts.len(), 2);
        assert_eq!(parts[0].content, "prefix");
        assert!(parts[0].fg.is_none());
        assert_eq!(parts[1].content, "styled");
        assert!(parts[1].fg.is_some());
    }

    #[test]
    fn parse_fill_attribute() {
        let parts = parse_format_string("#[bg=blue,fill]content", &empty_aliases());
        assert_eq!(parts.len(), 1);
        assert!(parts[0].fill);
        assert!(parts[0].bg.is_some());
    }

    #[test]
    fn parse_all_effects() {
        let parts = parse_format_string(
            "#[bold,dim,italic,underline,blink,reverse,hidden,strikethrough]x",
            &empty_aliases(),
        );
        assert_eq!(parts.len(), 1);
        let e = parts[0].effects;
        assert!(e.contains(Effects::BOLD));
        assert!(e.contains(Effects::DIMMED));
        assert!(e.contains(Effects::ITALIC));
        assert!(e.contains(Effects::UNDERLINE));
        assert!(e.contains(Effects::BLINK));
        assert!(e.contains(Effects::INVERT));
        assert!(e.contains(Effects::HIDDEN));
        assert!(e.contains(Effects::STRIKETHROUGH));
    }

    #[test]
    fn parse_with_color_alias() {
        let aliases = BTreeMap::from([("accent".to_string(), "#a6e3a1".to_string())]);
        let parts = parse_format_string("#[fg=$accent]text", &aliases);
        assert_eq!(parts.len(), 1);
        assert_eq!(
            parts[0].fg,
            Some(Color::Rgb(anstyle::RgbColor(0xa6, 0xe3, 0xa1)))
        );
    }

    #[test]
    fn parse_empty_string() {
        let parts = parse_format_string("", &empty_aliases());
        assert!(parts.is_empty());
    }

    #[test]
    fn parse_fg_and_bg() {
        let parts = parse_format_string("#[fg=#ff0000,bg=#0000ff]colored", &empty_aliases());
        assert_eq!(parts.len(), 1);
        assert_eq!(parts[0].fg, Some(Color::Rgb(anstyle::RgbColor(255, 0, 0))));
        assert_eq!(parts[0].bg, Some(Color::Rgb(anstyle::RgbColor(0, 0, 255))));
        assert_eq!(parts[0].content, "colored");
    }

    #[test]
    fn render_produces_ansi_output() {
        let parts = parse_format_string("#[fg=red,bold]test", &empty_aliases());
        let rendered = parts[0].render_content();
        // Should contain ANSI escape codes and the text
        assert!(rendered.contains("test"));
        assert!(rendered.contains('\x1b'));
    }

    #[test]
    fn content_with_widget_placeholders() {
        let parts = parse_format_string("#[fg=green]{mode} | {tabs}", &empty_aliases());
        assert_eq!(parts.len(), 1);
        assert_eq!(parts[0].content, "{mode} | {tabs}");
    }
}
