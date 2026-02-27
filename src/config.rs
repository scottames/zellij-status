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

/// Tab-specific rendering configuration for the `{tabs}` widget.
#[derive(Debug, Clone)]
pub struct TabConfig {
    /// Format string for a normal (inactive) tab.
    pub tab_normal: String,
    /// Format string for the active tab.
    pub tab_active: String,
    /// Format string for an inactive tab in fullscreen mode.
    pub tab_normal_fullscreen: String,
    /// Format string for the active tab in fullscreen mode.
    pub tab_active_fullscreen: String,
    /// Format string for an inactive tab with sync panes active.
    pub tab_normal_sync: String,
    /// Format string for the active tab with sync panes active.
    pub tab_active_sync: String,
    /// Format string shown when the active tab is being renamed.
    pub tab_rename: String,
    /// Optional separator rendered between tabs (vertical mode: not rendered).
    pub tab_separator: String,
    /// Overflow indicator shown above visible range, with `{count}` placeholder.
    pub overflow_above: String,
    /// Overflow indicator shown below visible range, with `{count}` placeholder.
    pub overflow_below: String,
    /// Maximum display columns for tab names before truncation.
    pub max_name_length: usize,
    /// Number of blank rows to render above the tab list.
    pub padding_top: usize,
    /// Right-side border appended to each row (e.g., `"#[fg=$muted]│"`).
    pub border: String,
    /// Tab index offset (1 = first tab shown as "1").
    pub start_index: usize,
}

impl TabConfig {
    fn from_raw(raw: &BTreeMap<String, String>) -> Self {
        let tab_normal = raw
            .get("tab_normal")
            .cloned()
            .unwrap_or_else(|| "{index}:{name}".to_string());

        let tab_active = raw
            .get("tab_active")
            .cloned()
            .unwrap_or_else(|| tab_normal.clone());

        let tab_normal_fullscreen = raw
            .get("tab_normal_fullscreen")
            .cloned()
            .unwrap_or_else(|| tab_normal.clone());

        let tab_active_fullscreen = raw
            .get("tab_active_fullscreen")
            .cloned()
            .unwrap_or_else(|| tab_active.clone());

        let tab_normal_sync = raw
            .get("tab_normal_sync")
            .cloned()
            .unwrap_or_else(|| tab_normal.clone());

        let tab_active_sync = raw
            .get("tab_active_sync")
            .cloned()
            .unwrap_or_else(|| tab_active.clone());

        let tab_rename = raw
            .get("tab_rename")
            .cloned()
            .unwrap_or_else(|| tab_active.clone());

        let tab_separator = raw.get("tab_separator").cloned().unwrap_or_default();

        let overflow_above = raw
            .get("overflow_above")
            .cloned()
            .unwrap_or_else(|| "  ^ +{count}".to_string());

        let overflow_below = raw
            .get("overflow_below")
            .cloned()
            .unwrap_or_else(|| "  v +{count}".to_string());

        let max_name_length = raw
            .get("max_name_length")
            .and_then(|v| v.parse().ok())
            .unwrap_or(20);

        let padding_top = raw
            .get("padding_top")
            .and_then(|v| v.parse().ok())
            .unwrap_or(0);

        let border = raw.get("border").cloned().unwrap_or_default();

        let start_index = raw
            .get("start_index")
            .and_then(|v| v.parse().ok())
            .unwrap_or(1);

        Self {
            tab_normal,
            tab_active,
            tab_normal_fullscreen,
            tab_active_fullscreen,
            tab_normal_sync,
            tab_active_sync,
            tab_rename,
            tab_separator,
            overflow_above,
            overflow_below,
            max_name_length,
            padding_top,
            border,
            start_index,
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

    /// Tab rendering config (used by the `{tabs}` widget).
    pub tabs: TabConfig,

    /// Raw flat key-value map from Zellij — passed to widgets for flexible access.
    pub raw: BTreeMap<String, String>,
}

impl PluginConfig {
    /// Parse configuration from Zellij's flat key-value map.
    pub fn from_configuration(config: BTreeMap<String, String>) -> anyhow::Result<Self> {
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

        let tabs = TabConfig::from_raw(&config);

        Ok(Self {
            layout_mode,
            color_aliases,
            format_1,
            format_2,
            format_3,
            format_space,
            format_precedence,
            hide_on_overlength,
            tabs,
            raw: config,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_layout_mode_defaults_to_horizontal() {
        let config = BTreeMap::new();
        let parsed = PluginConfig::from_configuration(config).unwrap();
        assert_eq!(parsed.layout_mode, LayoutMode::Horizontal);
    }

    #[test]
    fn parse_layout_mode_vertical() {
        let config = BTreeMap::from([("layout_mode".to_string(), "vertical".to_string())]);
        let parsed = PluginConfig::from_configuration(config).unwrap();
        assert_eq!(parsed.layout_mode, LayoutMode::Vertical);
    }

    #[test]
    fn parse_color_aliases() {
        let config = BTreeMap::from([
            ("color_bg".to_string(), "#1e1e2e".to_string()),
            ("color_accent".to_string(), "#a6e3a1".to_string()),
            ("unrelated_key".to_string(), "value".to_string()),
        ]);
        let parsed = PluginConfig::from_configuration(config).unwrap();
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
        let parsed = PluginConfig::from_configuration(config).unwrap();
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
        let parsed = PluginConfig::from_configuration(config).unwrap();
        assert!(parsed.hide_on_overlength);
        assert_eq!(parsed.format_precedence, "321");
    }

    #[test]
    fn tab_config_defaults() {
        let config = BTreeMap::new();
        let parsed = PluginConfig::from_configuration(config).unwrap();
        assert_eq!(parsed.tabs.tab_normal, "{index}:{name}");
        assert_eq!(parsed.tabs.max_name_length, 20);
        assert_eq!(parsed.tabs.start_index, 1);
        assert_eq!(parsed.tabs.padding_top, 0);
    }

    #[test]
    fn tab_config_inherits_fallbacks() {
        // tab_active falls back to tab_normal if not set
        let config = BTreeMap::from([("tab_normal".to_string(), "N:{name}".to_string())]);
        let parsed = PluginConfig::from_configuration(config).unwrap();
        assert_eq!(parsed.tabs.tab_normal, "N:{name}");
        assert_eq!(parsed.tabs.tab_active, "N:{name}");
    }

    #[test]
    fn tab_config_explicit_values() {
        let config = BTreeMap::from([
            ("tab_normal".to_string(), "{index}:{name}".to_string()),
            (
                "tab_active".to_string(),
                "#[bold]{index}:{name}".to_string(),
            ),
            ("max_name_length".to_string(), "30".to_string()),
            ("start_index".to_string(), "0".to_string()),
            ("overflow_above".to_string(), "^ {count}".to_string()),
        ]);
        let parsed = PluginConfig::from_configuration(config).unwrap();
        assert_eq!(parsed.tabs.tab_active, "#[bold]{index}:{name}");
        assert_eq!(parsed.tabs.max_name_length, 30);
        assert_eq!(parsed.tabs.start_index, 0);
        assert_eq!(parsed.tabs.overflow_above, "^ {count}");
    }
}
