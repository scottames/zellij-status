use std::collections::BTreeMap;

use crate::notify::config::NotificationConfig;

/// Layout rendering mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum LayoutMode {
    /// Sections map to top/middle(tabs)/bottom.
    Vertical,
    /// Sections map to left/center/right.
    #[default]
    Horizontal,
}

/// Primary section zone used across layout modes.
///
/// - Horizontal: start/middle/end map to left/center/right.
/// - Vertical: start/middle/end map to top/middle/bottom.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum SectionZone {
    Start,
    Middle,
    End,
}

/// Horizontal alignment of rendered text within a section row.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum TextAlign {
    #[default]
    Left,
    Center,
    Right,
}

impl TextAlign {
    fn parse(s: &str) -> Option<Self> {
        match s.to_ascii_lowercase().as_str() {
            "left" => Some(Self::Left),
            "center" | "middle" => Some(Self::Center),
            "right" => Some(Self::Right),
            _ => None,
        }
    }
}

impl SectionZone {
    fn parse(s: &str) -> Option<Self> {
        match s.to_ascii_lowercase().as_str() {
            "start" | "left" | "top" => Some(Self::Start),
            "middle" | "center" => Some(Self::Middle),
            "end" | "right" | "bottom" => Some(Self::End),
            _ => None,
        }
    }

    pub fn precedence_index(self) -> usize {
        match self {
            Self::Start => 0,
            Self::Middle => 1,
            Self::End => 2,
        }
    }
}

/// Parsed section definition from `format_<index>_<zone>[_<align>]` and
/// paired split keys `format_<index>_<zone>_left` + `format_<index>_<zone>_right`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FormatSection {
    pub index: usize,
    pub zone: SectionZone,
    pub align: TextAlign,
    pub format: String,
    pub split_left: Option<String>,
    pub split_right: Option<String>,
}

impl FormatSection {
    pub fn split_pair(&self) -> Option<(&str, &str)> {
        Some((self.split_left.as_deref()?, self.split_right.as_deref()?))
    }
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
    /// Icon/text substituted for `{sync_indicator}` in tab format strings.
    pub indicator_sync: String,
    /// Icon/text substituted for `{fullscreen_indicator}` in tab format strings.
    pub indicator_fullscreen: String,
    /// Icon/text substituted for `{floating_indicator}` in tab format strings.
    pub indicator_floating: String,
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
            .get("tab_overflow_above")
            .cloned()
            .unwrap_or_else(|| "  ^ +{count}".to_string());

        let overflow_below = raw
            .get("tab_overflow_below")
            .cloned()
            .unwrap_or_else(|| "  v +{count}".to_string());

        let max_name_length = raw
            .get("tab_max_name_length")
            .and_then(|v| v.parse().ok())
            .unwrap_or(20);

        let padding_top = raw
            .get("tab_padding_top")
            .and_then(|v| v.parse().ok())
            .unwrap_or(0);

        let border = raw.get("tab_border").cloned().unwrap_or_default();

        let start_index = raw
            .get("tab_start_index")
            .and_then(|v| v.parse().ok())
            .unwrap_or(1);

        let indicator_sync = raw.get("tab_indicator_sync").cloned().unwrap_or_default();

        let indicator_fullscreen = raw
            .get("tab_indicator_fullscreen")
            .cloned()
            .unwrap_or_default();

        let indicator_floating = raw
            .get("tab_indicator_floating")
            .cloned()
            .unwrap_or_default();

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
            indicator_sync,
            indicator_fullscreen,
            indicator_floating,
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

    /// Universal section format strings parsed from `format_<index>_<zone>[_<align>]`
    /// and split pairs `format_<index>_<zone>_left` + `format_<index>_<zone>_right`.
    pub sections: Vec<FormatSection>,

    /// Spacer styling for horizontal mode.
    pub format_space: String,

    /// Section priority for overlength trimming (e.g., "132").
    pub format_precedence: String,

    /// Whether to hide lower-priority sections on overlength.
    pub hide_on_overlength: bool,

    /// Tab rendering config (used by the `{tabs}` widget).
    pub tabs: TabConfig,

    /// Notification system configuration.
    pub notifications: NotificationConfig,

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

        let mut sections = parse_format_sections(&config);
        sections.sort_by_key(|s| (s.index, s.zone));

        let format_space = config.get("format_space").cloned().unwrap_or_default();
        let format_precedence = config
            .get("format_precedence")
            .cloned()
            .unwrap_or_else(|| "123".to_string());
        let hide_on_overlength = config
            .get("format_hide_on_overlength")
            .is_some_and(|v| v == "true");

        let tabs = TabConfig::from_raw(&config);
        let notifications = NotificationConfig::from_raw(&config);

        Ok(Self {
            layout_mode,
            color_aliases,
            sections,
            format_space,
            format_precedence,
            hide_on_overlength,
            tabs,
            notifications,
            raw: config,
        })
    }
}

fn parse_format_sections(config: &BTreeMap<String, String>) -> Vec<FormatSection> {
    #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
    enum SinglePriority {
        Base,
        Align,
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    enum SplitSide {
        Left,
        Right,
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    struct ParsedFormatKey {
        index: usize,
        zone: SectionZone,
        align: TextAlign,
        split_side: Option<SplitSide>,
        priority: SinglePriority,
    }

    let mut singles: BTreeMap<(usize, SectionZone), (SinglePriority, FormatSection)> =
        BTreeMap::new();
    let mut split_candidates: BTreeMap<(usize, SectionZone), (Option<String>, Option<String>)> =
        BTreeMap::new();

    for (key, value) in config {
        let Some((index, zone, align, split_side_flag)) = parse_format_key(key) else {
            continue;
        };

        let parsed = ParsedFormatKey {
            index,
            zone,
            align,
            split_side: split_side_flag.map(|is_left| {
                if is_left {
                    SplitSide::Left
                } else {
                    SplitSide::Right
                }
            }),
            priority: if split_side_flag.is_some() || align != TextAlign::Left {
                SinglePriority::Align
            } else {
                SinglePriority::Base
            },
        };

        let section = FormatSection {
            index: parsed.index,
            zone: parsed.zone,
            align: parsed.align,
            format: value.clone(),
            split_left: None,
            split_right: None,
        };

        let entry = singles
            .entry((parsed.index, parsed.zone))
            .or_insert((parsed.priority, section.clone()));
        if parsed.priority > entry.0 {
            *entry = (parsed.priority, section);
        }

        if let Some(side) = parsed.split_side {
            let sides = split_candidates
                .entry((parsed.index, parsed.zone))
                .or_insert((None, None));
            match side {
                SplitSide::Left => sides.0 = Some(value.clone()),
                SplitSide::Right => sides.1 = Some(value.clone()),
            }
        }
    }

    for ((index, zone), (left, right)) in split_candidates {
        if let (Some(left), Some(right)) = (left, right) {
            singles.insert(
                (index, zone),
                (
                    SinglePriority::Align,
                    FormatSection {
                        index,
                        zone,
                        align: TextAlign::Left,
                        format: left.clone(),
                        split_left: Some(left),
                        split_right: Some(right),
                    },
                ),
            );
        }
    }

    singles.into_values().map(|(_, section)| section).collect()
}

fn parse_format_key_parts(key: &str) -> Option<(usize, SectionZone, TextAlign, bool)> {
    let rest = key.strip_prefix("format_")?;
    let mut parts = rest.split('_');
    let index = parts.next()?.parse::<usize>().ok()?;
    if index == 0 {
        return None;
    }

    let zone = SectionZone::parse(parts.next()?)?;
    let suffix = parts.next();
    if parts.next().is_some() {
        return None;
    }

    let (align, is_split_side) = match suffix {
        None => (TextAlign::Left, false),
        Some("left") => (TextAlign::Left, true),
        Some("right") => (TextAlign::Right, true),
        Some(raw) => (TextAlign::parse(raw)?, false),
    };

    Some((index, zone, align, is_split_side))
}

pub(crate) fn is_valid_format_key(key: &str) -> bool {
    parse_format_key_parts(key).is_some()
}

fn parse_format_key(key: &str) -> Option<(usize, SectionZone, TextAlign, Option<bool>)> {
    let (index, zone, align, is_split_side) = parse_format_key_parts(key)?;
    let split_side = if !is_split_side {
        None
    } else if key.ends_with("_left") {
        Some(true)
    } else {
        Some(false)
    };

    Some((index, zone, align, split_side))
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
            ("format_1_left".to_string(), "{mode}".to_string()),
            ("format_2_center".to_string(), "{tabs}".to_string()),
            ("format_3_right".to_string(), "{datetime}".to_string()),
        ]);
        let parsed = PluginConfig::from_configuration(config).unwrap();
        assert_eq!(parsed.sections.len(), 3);
        assert_eq!(parsed.sections[0].index, 1);
        assert_eq!(parsed.sections[0].zone, SectionZone::Start);
        assert_eq!(parsed.sections[0].align, TextAlign::Left);
        assert_eq!(parsed.sections[1].index, 2);
        assert_eq!(parsed.sections[1].zone, SectionZone::Middle);
        assert_eq!(parsed.sections[1].align, TextAlign::Left);
        assert_eq!(parsed.sections[2].index, 3);
        assert_eq!(parsed.sections[2].zone, SectionZone::End);
        assert_eq!(parsed.sections[2].align, TextAlign::Left);
    }

    #[test]
    fn parse_format_section_aliases() {
        let config = BTreeMap::from([
            ("format_1_top".to_string(), "A".to_string()),
            ("format_2_middle".to_string(), "B".to_string()),
            ("format_3_bottom".to_string(), "C".to_string()),
        ]);
        let parsed = PluginConfig::from_configuration(config).unwrap();
        assert_eq!(parsed.sections[0].zone, SectionZone::Start);
        assert_eq!(parsed.sections[1].zone, SectionZone::Middle);
        assert_eq!(parsed.sections[2].zone, SectionZone::End);
    }

    #[test]
    fn parse_format_sections_ignores_legacy_format_keys() {
        let config = BTreeMap::from([
            ("format_1".to_string(), "{mode}".to_string()),
            ("format_2".to_string(), "{tabs}".to_string()),
            ("format_3".to_string(), "{datetime}".to_string()),
        ]);
        let parsed = PluginConfig::from_configuration(config).unwrap();
        assert!(parsed.sections.is_empty());
    }

    #[test]
    fn parse_format_sections_sorted_by_index() {
        let config = BTreeMap::from([
            ("format_9_end".to_string(), "Z".to_string()),
            ("format_2_start".to_string(), "A".to_string()),
            ("format_4_middle".to_string(), "M".to_string()),
        ]);
        let parsed = PluginConfig::from_configuration(config).unwrap();
        assert_eq!(
            parsed.sections.iter().map(|s| s.index).collect::<Vec<_>>(),
            vec![2, 4, 9]
        );
    }

    #[test]
    fn parse_format_sections_with_alignment_suffix() {
        let config = BTreeMap::from([
            ("format_1_top_right".to_string(), "A".to_string()),
            ("format_2_middle_center".to_string(), "B".to_string()),
            ("format_3_bottom_left".to_string(), "C".to_string()),
        ]);
        let parsed = PluginConfig::from_configuration(config).unwrap();
        assert_eq!(parsed.sections[0].align, TextAlign::Right);
        assert_eq!(parsed.sections[1].align, TextAlign::Center);
        assert_eq!(parsed.sections[2].align, TextAlign::Left);
    }

    #[test]
    fn parse_format_sections_invalid_alignment_is_ignored() {
        let config = BTreeMap::from([
            ("format_1_top_sideways".to_string(), "A".to_string()),
            ("format_2_middle".to_string(), "B".to_string()),
        ]);
        let parsed = PluginConfig::from_configuration(config).unwrap();
        assert_eq!(parsed.sections.len(), 1);
        assert_eq!(parsed.sections[0].zone, SectionZone::Middle);
        assert_eq!(parsed.sections[0].align, TextAlign::Left);
    }

    #[test]
    fn parse_split_section_pair_for_same_index_and_zone() {
        let config = BTreeMap::from([
            ("format_2_bottom_left".to_string(), "LEFT".to_string()),
            ("format_2_bottom_right".to_string(), "RIGHT".to_string()),
        ]);
        let parsed = PluginConfig::from_configuration(config).unwrap();
        assert_eq!(parsed.sections.len(), 1);
        assert_eq!(parsed.sections[0].zone, SectionZone::End);
        assert_eq!(parsed.sections[0].split_left.as_deref(), Some("LEFT"));
        assert_eq!(parsed.sections[0].split_right.as_deref(), Some("RIGHT"));
    }

    #[test]
    fn parse_single_right_suffix_stays_alignment_when_pair_missing() {
        let config = BTreeMap::from([("format_2_bottom_right".to_string(), "X".to_string())]);
        let parsed = PluginConfig::from_configuration(config).unwrap();
        assert_eq!(parsed.sections.len(), 1);
        assert_eq!(parsed.sections[0].align, TextAlign::Right);
        assert!(parsed.sections[0].split_left.is_none());
        assert!(parsed.sections[0].split_right.is_none());
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
        assert_eq!(parsed.tabs.indicator_sync, "");
        assert_eq!(parsed.tabs.indicator_fullscreen, "");
        assert_eq!(parsed.tabs.indicator_floating, "");
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
    fn tab_config_indicators() {
        let config = BTreeMap::from([
            ("tab_indicator_sync".to_string(), "S".to_string()),
            ("tab_indicator_fullscreen".to_string(), "F".to_string()),
            ("tab_indicator_floating".to_string(), "L".to_string()),
        ]);
        let parsed = PluginConfig::from_configuration(config).unwrap();
        assert_eq!(parsed.tabs.indicator_sync, "S");
        assert_eq!(parsed.tabs.indicator_fullscreen, "F");
        assert_eq!(parsed.tabs.indicator_floating, "L");
    }

    #[test]
    fn tab_config_explicit_values() {
        let config = BTreeMap::from([
            ("tab_normal".to_string(), "{index}:{name}".to_string()),
            (
                "tab_active".to_string(),
                "#[bold]{index}:{name}".to_string(),
            ),
            ("tab_max_name_length".to_string(), "30".to_string()),
            ("tab_start_index".to_string(), "0".to_string()),
            ("tab_overflow_above".to_string(), "^ {count}".to_string()),
        ]);
        let parsed = PluginConfig::from_configuration(config).unwrap();
        assert_eq!(parsed.tabs.tab_active, "#[bold]{index}:{name}");
        assert_eq!(parsed.tabs.max_name_length, 30);
        assert_eq!(parsed.tabs.start_index, 0);
        assert_eq!(parsed.tabs.overflow_above, "^ {count}");
    }
}
