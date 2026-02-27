use std::collections::BTreeMap;

/// Layout rendering mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum LayoutMode {
    /// Sections map to top/middle(tabs)/bottom.
    Vertical,
    /// Sections map to left/center/right.
    #[default]
    Horizontal,
}

impl LayoutMode {
    fn parse(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "vertical" => Some(Self::Vertical),
            "horizontal" => Some(Self::Horizontal),
            _ => None,
        }
    }
}

/// Parsed plugin configuration.
#[derive(Debug, Clone)]
pub struct PluginConfig {
    /// Layout rendering mode.
    pub layout_mode: LayoutMode,

    /// Color aliases: name → color value (e.g., "bg" → "#1e1e2e").
    /// Referenced in format strings as `$name`.
    pub color_aliases: BTreeMap<String, String>,

    /// Universal section format strings.
    pub format_1: String,
    pub format_2: String,
    pub format_3: String,

    /// Spacer styling for horizontal mode.
    pub format_space: String,

    /// Section priority for overlength trimming (e.g., "132").
    pub format_precedence: String,

    /// Whether to hide lower-priority sections on overlength.
    pub hide_on_overlength: bool,
}

impl PluginConfig {
    /// Parse configuration from Zellij's flat key-value map.
    pub fn from_configuration(config: &BTreeMap<String, String>) -> anyhow::Result<Self> {
        let layout_mode = config
            .get("layout_mode")
            .and_then(|v| LayoutMode::parse(v))
            .unwrap_or_default();

        let color_aliases = config
            .iter()
            .filter_map(|(k, v)| {
                k.strip_prefix("color_")
                    .map(|name| (name.to_string(), v.clone()))
            })
            .collect();

        let format_1 = config.get("format_1").cloned().unwrap_or_default();
        let format_2 = config.get("format_2").cloned().unwrap_or_default();
        let format_3 = config.get("format_3").cloned().unwrap_or_default();
        let format_space = config.get("format_space").cloned().unwrap_or_default();
        let format_precedence = config
            .get("format_precedence")
            .cloned()
            .unwrap_or_else(|| "123".to_string());
        let hide_on_overlength = config
            .get("format_hide_on_overlength")
            .is_some_and(|v| v == "true");

        Ok(Self {
            layout_mode,
            color_aliases,
            format_1,
            format_2,
            format_3,
            format_space,
            format_precedence,
            hide_on_overlength,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_layout_mode_defaults_to_horizontal() {
        let config = BTreeMap::new();
        let parsed = PluginConfig::from_configuration(&config).unwrap();
        assert_eq!(parsed.layout_mode, LayoutMode::Horizontal);
    }

    #[test]
    fn parse_layout_mode_vertical() {
        let config = BTreeMap::from([("layout_mode".to_string(), "vertical".to_string())]);
        let parsed = PluginConfig::from_configuration(&config).unwrap();
        assert_eq!(parsed.layout_mode, LayoutMode::Vertical);
    }

    #[test]
    fn parse_color_aliases() {
        let config = BTreeMap::from([
            ("color_bg".to_string(), "#1e1e2e".to_string()),
            ("color_accent".to_string(), "#a6e3a1".to_string()),
            ("unrelated_key".to_string(), "value".to_string()),
        ]);
        let parsed = PluginConfig::from_configuration(&config).unwrap();
        assert_eq!(parsed.color_aliases.len(), 2);
        assert_eq!(parsed.color_aliases["bg"], "#1e1e2e");
        assert_eq!(parsed.color_aliases["accent"], "#a6e3a1");
    }

    #[test]
    fn parse_format_sections() {
        let config = BTreeMap::from([
            ("format_1".to_string(), "{mode}".to_string()),
            ("format_2".to_string(), "{tabs}".to_string()),
            ("format_3".to_string(), "{datetime}".to_string()),
        ]);
        let parsed = PluginConfig::from_configuration(&config).unwrap();
        assert_eq!(parsed.format_1, "{mode}");
        assert_eq!(parsed.format_2, "{tabs}");
        assert_eq!(parsed.format_3, "{datetime}");
    }

    #[test]
    fn parse_overlength_config() {
        let config = BTreeMap::from([
            ("format_hide_on_overlength".to_string(), "true".to_string()),
            ("format_precedence".to_string(), "321".to_string()),
        ]);
        let parsed = PluginConfig::from_configuration(&config).unwrap();
        assert!(parsed.hide_on_overlength);
        assert_eq!(parsed.format_precedence, "321");
    }
}
