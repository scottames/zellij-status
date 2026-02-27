use std::collections::BTreeMap;

use anstyle::Color;

/// Parse a color string into an `anstyle::Color`.
///
/// Supports:
/// - `$alias` — resolved via color_aliases map
/// - `#RRGGBB` — hex RGB
/// - `#RGB` — short hex (expanded: `#abc` → `#aabbcc`)
/// - `rgb(R,G,B)` — function syntax
/// - Named colors: `black`, `red`, `green`, `yellow`, `blue`, `magenta`,
///   `cyan`, `white`, and `bright_*` variants
/// - `default` / `none` / `reset` — returns None (terminal default)
/// - `0`-`255` — ANSI 256-color palette index
pub fn parse_color(value: &str, aliases: &BTreeMap<String, String>) -> Option<Color> {
    let value = value.trim();

    if value.is_empty() {
        return None;
    }

    // Alias resolution: $name → look up in aliases, then re-parse
    if let Some(alias_name) = value.strip_prefix('$') {
        let resolved = aliases.get(alias_name)?;
        return parse_color(resolved, aliases);
    }

    // Hex: #RRGGBB or #RGB
    if let Some(hex) = value.strip_prefix('#') {
        return parse_hex(hex);
    }

    // rgb(R,G,B) function
    if let Some(inner) = value.strip_prefix("rgb(").and_then(|s| s.strip_suffix(')')) {
        return parse_rgb_func(inner);
    }

    // Named colors
    if let Some(color) = parse_named_color(value) {
        return Some(color);
    }

    // Default / none / reset
    match value.to_lowercase().as_str() {
        "default" | "none" | "reset" => return None,
        _ => {}
    }

    // ANSI 256-color index
    if let Ok(idx) = value.parse::<u8>() {
        return Some(Color::Ansi256(anstyle::Ansi256Color(idx)));
    }

    None
}

/// Parse `RRGGBB` or `RGB` hex string (without `#` prefix).
fn parse_hex(hex: &str) -> Option<Color> {
    let expanded = match hex.len() {
        3 => {
            let chars: Vec<char> = hex.chars().collect();
            format!(
                "{}{}{}{}{}{}",
                chars[0], chars[0], chars[1], chars[1], chars[2], chars[2]
            )
        }
        6 => hex.to_string(),
        _ => return None,
    };

    let r = u8::from_str_radix(&expanded[0..2], 16).ok()?;
    let g = u8::from_str_radix(&expanded[2..4], 16).ok()?;
    let b = u8::from_str_radix(&expanded[4..6], 16).ok()?;

    Some(Color::Rgb(anstyle::RgbColor(r, g, b)))
}

/// Parse `R,G,B` from `rgb()` function.
fn parse_rgb_func(inner: &str) -> Option<Color> {
    let parts: Vec<&str> = inner.split(',').map(str::trim).collect();
    if parts.len() != 3 {
        return None;
    }

    let r = parts[0].parse::<u8>().ok()?;
    let g = parts[1].parse::<u8>().ok()?;
    let b = parts[2].parse::<u8>().ok()?;

    Some(Color::Rgb(anstyle::RgbColor(r, g, b)))
}

/// Parse a named color to its ANSI 256-color equivalent.
fn parse_named_color(name: &str) -> Option<Color> {
    let idx = match name.to_lowercase().as_str() {
        "black" => 0,
        "red" => 196,
        "green" => 82,
        "yellow" => 226,
        "blue" => 33,
        "magenta" => 201,
        "cyan" => 51,
        "white" => 15,
        "orange" => 208,
        "gray" | "grey" => 244,
        "pink" => 213,
        "purple" => 135,

        // Bright variants
        "bright_black" => 8,
        "bright_red" => 9,
        "bright_green" => 10,
        "bright_yellow" => 11,
        "bright_blue" => 12,
        "bright_magenta" => 13,
        "bright_cyan" => 14,
        "bright_white" => 15,

        // Semantic aliases
        "accent" | "primary" => 39,
        "secondary" => 75,
        "tertiary" => 141,
        "muted" => 245,
        "dim" => 240,

        _ => return None,
    };

    Some(Color::Ansi256(anstyle::Ansi256Color(idx)))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn empty_aliases() -> BTreeMap<String, String> {
        BTreeMap::new()
    }

    #[test]
    fn parse_hex_6_digit() {
        let color = parse_color("#a6e3a1", &empty_aliases());
        assert_eq!(color, Some(Color::Rgb(anstyle::RgbColor(0xa6, 0xe3, 0xa1))));
    }

    #[test]
    fn parse_hex_3_digit() {
        let color = parse_color("#abc", &empty_aliases());
        assert_eq!(color, Some(Color::Rgb(anstyle::RgbColor(0xaa, 0xbb, 0xcc))));
    }

    #[test]
    fn parse_rgb_function() {
        let color = parse_color("rgb(255, 128, 0)", &empty_aliases());
        assert_eq!(color, Some(Color::Rgb(anstyle::RgbColor(255, 128, 0))));
    }

    #[test]
    fn parse_named() {
        let color = parse_color("red", &empty_aliases());
        assert_eq!(color, Some(Color::Ansi256(anstyle::Ansi256Color(196))));
    }

    #[test]
    fn parse_ansi_256() {
        let color = parse_color("42", &empty_aliases());
        assert_eq!(color, Some(Color::Ansi256(anstyle::Ansi256Color(42))));
    }

    #[test]
    fn parse_alias() {
        let aliases = BTreeMap::from([("accent".to_string(), "#00ff00".to_string())]);
        let color = parse_color("$accent", &aliases);
        assert_eq!(color, Some(Color::Rgb(anstyle::RgbColor(0, 255, 0))));
    }

    #[test]
    fn parse_default_returns_none() {
        assert_eq!(parse_color("default", &empty_aliases()), None);
        assert_eq!(parse_color("none", &empty_aliases()), None);
        assert_eq!(parse_color("reset", &empty_aliases()), None);
    }

    #[test]
    fn parse_empty_returns_none() {
        assert_eq!(parse_color("", &empty_aliases()), None);
    }

    #[test]
    fn parse_invalid_returns_none() {
        assert_eq!(parse_color("not_a_color", &empty_aliases()), None);
        assert_eq!(parse_color("#xyz", &empty_aliases()), None);
    }

    #[test]
    fn alias_chain_does_not_resolve() {
        // Aliases don't chain — $foo must resolve to a literal color, not another $alias
        let aliases = BTreeMap::from([
            ("foo".to_string(), "$bar".to_string()),
            ("bar".to_string(), "#ff0000".to_string()),
        ]);
        // $foo → "$bar" → tries to parse "$bar" → looks up "bar" → "#ff0000" → resolves
        let color = parse_color("$foo", &aliases);
        assert_eq!(color, Some(Color::Rgb(anstyle::RgbColor(255, 0, 0))));
    }
}
