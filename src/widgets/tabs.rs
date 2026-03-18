use std::collections::BTreeMap;

use anstyle::Effects;
use unicode_width::UnicodeWidthStr;
use zellij_tile::prelude::{InputMode, PaneInfo, TabInfo};
#[cfg(target_arch = "wasm32")]
use zellij_tile::shim::switch_tab_to;

#[cfg(not(target_arch = "wasm32"))]
fn switch_tab_to(_tab_index: u32) {}

use crate::notify::NotificationType;
use crate::render::format::{parse_format_string, FormattedPart};

use super::{PluginState, Widget};

/// Renders the tab list.
///
/// In vertical mode, the vertical renderer drives iteration and calls
/// `render_tab` directly. In horizontal mode, `process` concatenates tabs
/// with the configured separator.
pub struct TabsWidget {
    /// Pre-parsed format strings keyed by tab state.
    formats: TabFormats,
    /// Separator string rendered between tabs in horizontal mode.
    separator: String,
}

/// Pre-parsed format strings for each tab state.
struct TabFormats {
    normal: String,
    active: String,
    normal_fullscreen: String,
    active_fullscreen: String,
    normal_sync: String,
    active_sync: String,
    rename: String,
}

impl TabsWidget {
    pub fn new(config: &BTreeMap<String, String>) -> Self {
        let normal = config
            .get("tab_normal")
            .cloned()
            .unwrap_or_else(|| "{index}:{name}".to_string());

        let active = config
            .get("tab_active")
            .cloned()
            .unwrap_or_else(|| normal.clone());

        Self {
            formats: TabFormats {
                normal_fullscreen: config
                    .get("tab_normal_fullscreen")
                    .cloned()
                    .unwrap_or_else(|| normal.clone()),
                active_fullscreen: config
                    .get("tab_active_fullscreen")
                    .cloned()
                    .unwrap_or_else(|| active.clone()),
                normal_sync: config
                    .get("tab_normal_sync")
                    .cloned()
                    .unwrap_or_else(|| normal.clone()),
                active_sync: config
                    .get("tab_active_sync")
                    .cloned()
                    .unwrap_or_else(|| active.clone()),
                rename: config
                    .get("tab_rename")
                    .cloned()
                    .unwrap_or_else(|| active.clone()),
                normal,
                active,
            },
            separator: config.get("tab_separator").cloned().unwrap_or_default(),
        }
    }

    /// Select the correct format string for a tab based on its state.
    ///
    /// Priority (highest first):
    /// 1. Active + RenameTab mode
    /// 2. Active + fullscreen
    /// 3. Active + sync
    /// 4. Active
    /// 5. Inactive + fullscreen
    /// 6. Inactive + sync
    /// 7. Inactive (default)
    pub fn select_format<'a>(&'a self, tab: &TabInfo, mode: &InputMode) -> &'a str {
        if tab.active && *mode == InputMode::RenameTab {
            return &self.formats.rename;
        }
        if tab.active && tab.is_fullscreen_active {
            return &self.formats.active_fullscreen;
        }
        if tab.active && tab.is_sync_panes_active {
            return &self.formats.active_sync;
        }
        if tab.active {
            return &self.formats.active;
        }
        if tab.is_fullscreen_active {
            return &self.formats.normal_fullscreen;
        }
        if tab.is_sync_panes_active {
            return &self.formats.normal_sync;
        }
        &self.formats.normal
    }

    /// Expand a format string for a single tab, substituting variables.
    ///
    /// Supported variables:
    /// - `{index}` — tab position (1-based by default, controlled by `start_index`, right-aligned to the widest current index)
    /// - `{name}` — tab name (shows "Enter name..." when renaming and name is empty)
    /// - `{sync_indicator}` — icon from `tab_indicator_sync` (shown only when sync active)
    /// - `{fullscreen_indicator}` — icon from `tab_indicator_fullscreen` (shown only when fullscreen active)
    /// - `{floating_indicator}` — icon from `tab_indicator_floating` (shown only when floating panes visible)
    /// - `{notification}` — notification icon for the tab (or empty if none)
    pub fn render_tab(
        &self,
        tab: &TabInfo,
        state: &PluginState<'_>,
        display_index: usize,
    ) -> String {
        let max_name = state.config.tabs.max_name_length;

        let name = resolve_tab_name(tab, &state.mode.mode);
        let name_truncated = truncate_str(&name, max_name);

        let notification_fragment = resolve_notification_icon(tab, state);
        let sync_val =
            resolve_indicator(&state.config.tabs.indicator_sync, tab.is_sync_panes_active);
        let fs_val = resolve_indicator(
            &state.config.tabs.indicator_fullscreen,
            tab.is_fullscreen_active,
        );
        let float_val = resolve_indicator(
            &state.config.tabs.indicator_floating,
            tab.are_floating_panes_visible,
        );
        let padded_index = format_display_index(display_index, state);
        let parts = self.parts_for_tab(tab, state);
        let mut out = String::new();

        for part in &parts {
            let mut content = part.content.clone();
            if content.contains("{index}") {
                content = content.replace("{index}", &padded_index);
            }
            if content.contains("{name}") {
                content = content.replace("{name}", &name_truncated);
            }
            if content.contains("{sync_indicator}") {
                content = content.replace("{sync_indicator}", &sync_val);
            }
            if content.contains("{fullscreen_indicator}") {
                content = content.replace("{fullscreen_indicator}", &fs_val);
            }
            if content.contains("{floating_indicator}") {
                content = content.replace("{floating_indicator}", &float_val);
            }

            if content.contains("{notification}") {
                let segments: Vec<&str> = content.split("{notification}").collect();
                for (i, segment) in segments.iter().enumerate() {
                    if !segment.is_empty() {
                        out.push_str(&part.render(segment));
                    }
                    if i + 1 < segments.len() {
                        out.push_str(&render_notification_fragment(
                            &notification_fragment,
                            part,
                            &state.config.color_aliases,
                        ));
                    }
                }
            } else {
                out.push_str(&part.render(&content));
            }
        }

        out
    }

    /// Try to split a tab's format string at the fill segment.
    ///
    /// When a `fill` segment sits between rendered content on both sides, this
    /// returns the left half, right half, and the fill part (for gap styling).
    /// Returns `None` when fill is absent or at an edge — caller should fall
    /// back to `render_tab()` + single-block alignment.
    pub fn render_tab_halves(
        &self,
        tab: &TabInfo,
        state: &PluginState<'_>,
        display_index: usize,
    ) -> Option<(String, String, FormattedPart)> {
        let max_name = state.config.tabs.max_name_length;
        let name = resolve_tab_name(tab, &state.mode.mode);
        let name_truncated = truncate_str(&name, max_name);
        let notification_fragment = resolve_notification_icon(tab, state);
        let sync_val =
            resolve_indicator(&state.config.tabs.indicator_sync, tab.is_sync_panes_active);
        let fs_val = resolve_indicator(
            &state.config.tabs.indicator_fullscreen,
            tab.is_fullscreen_active,
        );
        let float_val = resolve_indicator(
            &state.config.tabs.indicator_floating,
            tab.are_floating_panes_visible,
        );
        let padded_index = format_display_index(display_index, state);

        let parts = self.parts_for_tab(tab, state);
        let fill_idx = parts.iter().position(|p| p.fill)?;

        // A fill part with substantive content (variables like {name}) is a
        // "fill this segment's background" directive, not a split marker.
        // Only treat it as a split point when its content is empty/whitespace.
        let fill_content = &parts[fill_idx].content;
        if !fill_content.trim().is_empty() {
            return None;
        }

        // Fill must sit between content on both sides to act as a split point.
        let has_left = parts[..fill_idx].iter().any(|p| !p.content.is_empty());
        let has_right = parts[fill_idx + 1..].iter().any(|p| !p.content.is_empty());
        if !has_left || !has_right {
            return None;
        }

        let render_half = |range: &[FormattedPart]| -> String {
            let mut out = String::new();
            for part in range {
                let mut content = part.content.clone();
                if content.contains("{index}") {
                    content = content.replace("{index}", &padded_index);
                }
                if content.contains("{name}") {
                    content = content.replace("{name}", &name_truncated);
                }
                if content.contains("{sync_indicator}") {
                    content = content.replace("{sync_indicator}", &sync_val);
                }
                if content.contains("{fullscreen_indicator}") {
                    content = content.replace("{fullscreen_indicator}", &fs_val);
                }
                if content.contains("{floating_indicator}") {
                    content = content.replace("{floating_indicator}", &float_val);
                }
                if content.contains("{notification}") {
                    let segments: Vec<&str> = content.split("{notification}").collect();
                    for (i, segment) in segments.iter().enumerate() {
                        if !segment.is_empty() {
                            out.push_str(&part.render(segment));
                        }
                        if i + 1 < segments.len() {
                            out.push_str(&render_notification_fragment(
                                &notification_fragment,
                                part,
                                &state.config.color_aliases,
                            ));
                        }
                    }
                } else {
                    out.push_str(&part.render(&content));
                }
            }
            out
        };

        let left = render_half(&parts[..fill_idx]);
        let right = render_half(&parts[fill_idx + 1..]);

        Some((left, right, parts[fill_idx].clone()))
    }

    fn parts_for_tab(&self, tab: &TabInfo, state: &PluginState<'_>) -> Vec<FormattedPart> {
        let format_str = self.select_format(tab, &state.mode.mode);
        let mut parts = parse_format_string(format_str, &state.config.color_aliases);
        apply_notification_tab_style(tab, state, &mut parts);
        parts
    }

    /// Horizontal-mode rendering: all tabs joined with separator.
    fn render_inline(&self, state: &PluginState<'_>) -> String {
        let start = state.config.tabs.start_index;
        let mut out = String::new();

        for (i, tab) in state.tabs.iter().enumerate() {
            if i > 0 && !self.separator.is_empty() {
                // Render separator as a plain styled segment (no variable expansion)
                let sep_parts = parse_format_string(&self.separator, &state.config.color_aliases);
                for p in &sep_parts {
                    out.push_str(&p.render_content());
                }
            }
            out.push_str(&self.render_tab(tab, state, i + start));
        }

        out
    }
}

impl Widget for TabsWidget {
    fn process(&self, _name: &str, state: &PluginState<'_>) -> String {
        // In vertical mode the renderer calls render_tab directly per-row;
        // in horizontal mode (or as fallback) we produce a flat string.
        self.render_inline(state)
    }

    fn process_click(&self, _name: &str, state: &PluginState<'_>, col: usize) {
        let start = state.config.tabs.start_index;
        let mut offset = 0;

        for (i, tab) in state.tabs.iter().enumerate() {
            let rendered = self.render_tab(tab, state, i + start);
            // Measure visible width (strip ANSI before measuring)
            let visible_width = strip_ansi_width(&rendered);

            if col >= offset && col < offset + visible_width {
                // Zellij tab positions are 0-based; switch_tab_to is 1-based
                switch_tab_to(tab.position as u32 + 1);
                return;
            }

            offset += visible_width;

            // Separator width
            if !self.separator.is_empty() && i + 1 < state.tabs.len() {
                offset += self.separator.width();
            }
        }
    }

    fn fill_part(
        &self,
        _name: &str,
        state: &PluginState<'_>,
    ) -> Option<crate::render::format::FormattedPart> {
        let active = state.tabs.iter().find(|tab| tab.active)?;
        self.parts_for_tab(active, state)
            .into_iter()
            .find(|part| part.fill)
    }
}

fn resolve_notification_state(tab: &TabInfo, state: &PluginState<'_>) -> Option<NotificationType> {
    if !state.config.notifications.enabled {
        return None;
    }

    state
        .notifications
        .get_tab_notification(tab.position, state.panes)
}

/// Resolve the rendered notification content for a tab based on tracker state
/// and config.
///
/// Returns the formatted notification fragment, or an empty string if no
/// notification is active or notifications are disabled.
fn resolve_notification_icon(tab: &TabInfo, state: &PluginState<'_>) -> String {
    let notify_config = &state.config.notifications;
    let (icon, format_template) = match resolve_notification_state(tab, state) {
        Some(NotificationType::Waiting) => {
            (&notify_config.waiting_icon, &notify_config.waiting_format)
        }
        Some(NotificationType::InProgress) => (
            &notify_config.in_progress_icon,
            &notify_config.in_progress_format,
        ),
        Some(NotificationType::Completed) => (
            &notify_config.completed_icon,
            &notify_config.completed_format,
        ),
        None => return String::new(),
    };

    format_template.replace("{icon}", icon)
}

fn apply_notification_tab_style(
    tab: &TabInfo,
    state: &PluginState<'_>,
    parts: &mut [FormattedPart],
) {
    let Some(overlay) = resolve_notification_tab_style(tab, state) else {
        return;
    };

    let base_bg = parts
        .iter()
        .find_map(|part| if part.fill { part.bg } else { None })
        .or_else(|| parts.iter().find_map(|part| part.bg));

    for part in parts {
        let cap_only = is_cap_only_part(part);

        if let Some(overlay_bg) = overlay.bg {
            if cap_only {
                if part.bg == base_bg {
                    part.bg = Some(overlay_bg);
                }
                if part.fg == base_bg {
                    part.fg = Some(overlay_bg);
                }
            } else {
                part.bg = Some(overlay_bg);
            }
        }

        if !cap_only {
            if let Some(overlay_fg) = overlay.fg {
                part.fg = Some(overlay_fg);
            }
            part.effects |= overlay.effects;
        }
    }
}

fn resolve_notification_tab_style(tab: &TabInfo, state: &PluginState<'_>) -> Option<FormattedPart> {
    let notify_config = &state.config.notifications;
    if tab.active && !notify_config.tab_style_apply_to_active {
        return None;
    }

    let template = match resolve_notification_state(tab, state)? {
        NotificationType::Waiting => &notify_config.waiting_tab_style,
        NotificationType::InProgress => &notify_config.in_progress_tab_style,
        NotificationType::Completed => &notify_config.completed_tab_style,
    };

    let overlay = parse_format_string(template, &state.config.color_aliases)
        .into_iter()
        .next()?;

    if overlay.fg.is_none() && overlay.bg.is_none() && overlay.effects == Effects::new() {
        return None;
    }

    Some(FormattedPart {
        content: String::new(),
        fill: false,
        ..overlay
    })
}

fn is_cap_only_part(part: &FormattedPart) -> bool {
    let trimmed = part.content.trim();
    !trimmed.is_empty() && trimmed.chars().all(|ch| matches!(ch, '' | ''))
}

fn render_notification_fragment(
    fragment: &str,
    host: &FormattedPart,
    aliases: &BTreeMap<String, String>,
) -> String {
    if fragment.is_empty() {
        return String::new();
    }

    if !fragment.contains("#[") {
        return host.render(fragment);
    }

    let mut out = String::new();
    for mut part in parse_format_string(fragment, aliases) {
        if part.fg.is_none() {
            part.fg = host.fg;
        }
        if part.bg.is_none() {
            part.bg = host.bg;
        }
        out.push_str(&part.render_content());
    }
    out
}

/// Resolve a tab state indicator: returns the configured icon when the state
/// is active, empty string otherwise.
fn resolve_indicator(icon: &str, active: bool) -> String {
    if active {
        icon.to_string()
    } else {
        String::new()
    }
}

fn format_display_index(display_index: usize, state: &PluginState<'_>) -> String {
    let max_display_index = state
        .config
        .tabs
        .start_index
        .saturating_add(state.tabs.len().saturating_sub(1));
    let width = max_display_index.max(1).to_string().len();

    format!("{display_index:>width$}")
}

/// Resolve what name to display for a tab.
fn resolve_tab_name(tab: &TabInfo, mode: &InputMode) -> String {
    if *mode == InputMode::RenameTab && tab.active {
        if tab.name.is_empty() {
            return "Enter name...".to_string();
        }
        return tab.name.clone();
    }
    tab.name.clone()
}

/// Truncate a string to `max_width` display columns, appending "…" if cut.
pub fn truncate_str(s: &str, max_width: usize) -> String {
    if s.width() <= max_width {
        return s.to_string();
    }
    if max_width == 0 {
        return String::new();
    }
    // Reserve 1 column for ellipsis if max_width > 1
    let target = if max_width > 1 {
        max_width - 1
    } else {
        max_width
    };
    let mut out = String::new();
    let mut w = 0;
    for ch in s.chars() {
        let cw = ch.to_string().width();
        if w + cw > target {
            break;
        }
        out.push(ch);
        w += cw;
    }
    if max_width > 1 {
        out.push('…');
    }
    out
}

/// Measure the visible display width of a string, ignoring ANSI escape sequences.
pub fn strip_ansi_width(s: &str) -> usize {
    let mut width = 0;
    let mut in_escape = false;

    for ch in s.chars() {
        if ch == '\x1b' {
            in_escape = true;
            continue;
        }
        if in_escape {
            // CSI ends on a letter (A-Z or a-z)
            if ch.is_ascii_alphabetic() {
                in_escape = false;
            }
            continue;
        }
        width += ch.to_string().width();
    }

    width
}

/// Returns panes for a tab position (non-plugin panes only).
pub fn terminal_panes_for_tab(
    panes: &zellij_tile::prelude::PaneManifest,
    tab_position: usize,
) -> Vec<&PaneInfo> {
    panes
        .panes
        .get(&tab_position)
        .map(|v| v.iter().filter(|p| !p.is_plugin).collect())
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::PluginConfig;
    use crate::notify::tracker::NotificationTracker;
    use zellij_tile::prelude::{ModeInfo, PaneInfo, PaneManifest};

    fn make_tab(position: usize, name: &str, active: bool) -> TabInfo {
        TabInfo {
            position,
            name: name.to_string(),
            active,
            ..Default::default()
        }
    }

    fn make_widget(config: &BTreeMap<String, String>) -> TabsWidget {
        TabsWidget::new(config)
    }

    fn make_pane_manifest(entries: Vec<(usize, Vec<PaneInfo>)>) -> PaneManifest {
        PaneManifest {
            panes: entries.into_iter().collect(),
        }
    }

    fn make_plugin_state_for_notification_test<'a>(
        tabs: &'a [TabInfo],
        panes: &'a PaneManifest,
        config: &'a PluginConfig,
        notifications: &'a NotificationTracker,
        mode: &'a ModeInfo,
        command_results: &'a BTreeMap<String, crate::widgets::command::CommandResult>,
        pipe_data: &'a BTreeMap<String, String>,
    ) -> PluginState<'a> {
        PluginState {
            tabs,
            panes,
            mode,
            config,
            notifications,
            command_results,
            pipe_data,
        }
    }

    // ---- tab state selection ----

    #[test]
    fn select_format_active_tab() {
        let config = BTreeMap::from([
            ("tab_normal".to_string(), "N".to_string()),
            ("tab_active".to_string(), "A".to_string()),
        ]);
        let w = make_widget(&config);
        let tab = make_tab(0, "foo", true);
        assert_eq!(w.select_format(&tab, &InputMode::Normal), "A");
    }

    #[test]
    fn select_format_normal_tab() {
        let config = BTreeMap::from([
            ("tab_normal".to_string(), "N".to_string()),
            ("tab_active".to_string(), "A".to_string()),
        ]);
        let w = make_widget(&config);
        let tab = make_tab(0, "foo", false);
        assert_eq!(w.select_format(&tab, &InputMode::Normal), "N");
    }

    #[test]
    fn select_format_active_fullscreen() {
        let config = BTreeMap::from([
            ("tab_active".to_string(), "A".to_string()),
            ("tab_active_fullscreen".to_string(), "AF".to_string()),
        ]);
        let w = make_widget(&config);
        let mut tab = make_tab(0, "foo", true);
        tab.is_fullscreen_active = true;
        assert_eq!(w.select_format(&tab, &InputMode::Normal), "AF");
    }

    #[test]
    fn select_format_active_sync() {
        let config = BTreeMap::from([
            ("tab_active".to_string(), "A".to_string()),
            ("tab_active_sync".to_string(), "AS".to_string()),
        ]);
        let w = make_widget(&config);
        let mut tab = make_tab(0, "foo", true);
        tab.is_sync_panes_active = true;
        assert_eq!(w.select_format(&tab, &InputMode::Normal), "AS");
    }

    #[test]
    fn select_format_rename_mode() {
        let config = BTreeMap::from([
            ("tab_active".to_string(), "A".to_string()),
            ("tab_rename".to_string(), "R".to_string()),
        ]);
        let w = make_widget(&config);
        let tab = make_tab(0, "foo", true);
        assert_eq!(w.select_format(&tab, &InputMode::RenameTab), "R");
    }

    #[test]
    fn select_format_inactive_fullscreen() {
        let config = BTreeMap::from([
            ("tab_normal".to_string(), "N".to_string()),
            ("tab_normal_fullscreen".to_string(), "NF".to_string()),
        ]);
        let w = make_widget(&config);
        let mut tab = make_tab(0, "foo", false);
        tab.is_fullscreen_active = true;
        assert_eq!(w.select_format(&tab, &InputMode::Normal), "NF");
    }

    // ---- format variable expansion ----

    #[test]
    fn expand_index_variable() {
        let config = BTreeMap::from([("tab_normal".to_string(), "{index}".to_string())]);
        let w = make_widget(&config);
        let tabs = vec![
            make_tab(0, "one", true),
            make_tab(1, "two", false),
            make_tab(2, "three", false),
        ];
        let panes = PaneManifest::default();
        let parsed = PluginConfig::from_configuration(config).unwrap();
        let notifications = NotificationTracker::default();
        let mode = ModeInfo::default();
        let command_results = BTreeMap::new();
        let pipe_data = BTreeMap::new();
        let state = make_plugin_state_for_notification_test(
            &tabs,
            &panes,
            &parsed,
            &notifications,
            &mode,
            &command_results,
            &pipe_data,
        );

        assert_eq!(w.render_tab(&tabs[2], &state, 3), "3");
    }

    #[test]
    fn expand_index_variable_pads_to_widest_tab_index() {
        let config = BTreeMap::from([("tab_normal".to_string(), "{index}>".to_string())]);
        let w = make_widget(&config);
        let parsed = PluginConfig::from_configuration(config).unwrap();
        let tabs: Vec<TabInfo> = (0..11)
            .map(|position| make_tab(position, &format!("tab-{position}"), position == 0))
            .collect();
        let panes = PaneManifest::default();
        let notifications = NotificationTracker::default();
        let mode = ModeInfo::default();
        let command_results = BTreeMap::new();
        let pipe_data = BTreeMap::new();
        let state = make_plugin_state_for_notification_test(
            &tabs,
            &panes,
            &parsed,
            &notifications,
            &mode,
            &command_results,
            &pipe_data,
        );

        assert_eq!(w.render_tab(&tabs[0], &state, 1), " 1>");
        assert_eq!(w.render_tab(&tabs[9], &state, 10), "10>");
    }

    #[test]
    fn truncate_str_no_truncation() {
        assert_eq!(truncate_str("hello", 10), "hello");
    }

    #[test]
    fn truncate_str_exact_fit() {
        assert_eq!(truncate_str("hello", 5), "hello");
    }

    #[test]
    fn truncate_str_truncates_with_ellipsis() {
        let result = truncate_str("hello world", 7);
        assert!(result.ends_with('…'));
        assert!(result.width() <= 7);
    }

    #[test]
    fn truncate_str_zero_width() {
        assert_eq!(truncate_str("abc", 0), "");
    }

    #[test]
    fn strip_ansi_width_plain() {
        assert_eq!(strip_ansi_width("hello"), 5);
    }

    #[test]
    fn strip_ansi_width_with_escapes() {
        // "\x1b[1mhello\x1b[0m" — bold hello, width should be 5
        let s = "\x1b[1mhello\x1b[0m";
        assert_eq!(strip_ansi_width(s), 5);
    }

    #[test]
    fn resolve_tab_name_normal() {
        let tab = make_tab(0, "myname", false);
        assert_eq!(resolve_tab_name(&tab, &InputMode::Normal), "myname");
    }

    #[test]
    fn resolve_tab_name_rename_empty() {
        let tab = make_tab(0, "", true);
        assert_eq!(
            resolve_tab_name(&tab, &InputMode::RenameTab),
            "Enter name..."
        );
    }

    #[test]
    fn resolve_tab_name_rename_has_value() {
        let tab = make_tab(0, "partial", true);
        assert_eq!(resolve_tab_name(&tab, &InputMode::RenameTab), "partial");
    }

    #[test]
    fn resolve_notification_icon_in_progress() {
        let tab = make_tab(0, "work", true);
        let tabs = vec![tab.clone()];
        let mode = ModeInfo::default();
        let panes = make_pane_manifest(vec![(
            0,
            vec![PaneInfo {
                id: 42,
                is_plugin: false,
                ..Default::default()
            }],
        )]);

        let raw = BTreeMap::from([(
            "notification_indicator_in_progress".to_string(),
            "RUN".to_string(),
        )]);
        let config = PluginConfig::from_configuration(raw).unwrap();

        let mut notifications = NotificationTracker::default();
        notifications.add(42, NotificationType::InProgress);
        let command_results = BTreeMap::new();
        let pipe_data = BTreeMap::new();

        let state = make_plugin_state_for_notification_test(
            &tabs,
            &panes,
            &config,
            &notifications,
            &mode,
            &command_results,
            &pipe_data,
        );

        assert_eq!(resolve_notification_icon(&tab, &state), "RUN");
    }

    #[test]
    fn resolve_notification_icon_uses_state_specific_format() {
        let tab = make_tab(0, "work", true);
        let tabs = vec![tab.clone()];
        let mode = ModeInfo::default();
        let panes = make_pane_manifest(vec![(
            0,
            vec![PaneInfo {
                id: 99,
                is_plugin: false,
                ..Default::default()
            }],
        )]);

        let raw = BTreeMap::from([
            (
                "notification_indicator_waiting".to_string(),
                "WAIT".to_string(),
            ),
            (
                "notification_format_waiting".to_string(),
                "#[fg=green,bold]{icon}".to_string(),
            ),
        ]);
        let config = PluginConfig::from_configuration(raw).unwrap();

        let mut notifications = NotificationTracker::default();
        notifications.add(99, NotificationType::Waiting);
        let command_results = BTreeMap::new();
        let pipe_data = BTreeMap::new();

        let state = make_plugin_state_for_notification_test(
            &tabs,
            &panes,
            &config,
            &notifications,
            &mode,
            &command_results,
            &pipe_data,
        );

        let rendered = resolve_notification_icon(&tab, &state);
        assert!(rendered.contains("WAIT"));
        assert!(rendered.contains("#[fg=green,bold]"));
    }

    #[test]
    fn resolve_notification_icon_falls_back_to_tab_format() {
        let tab = make_tab(0, "work", true);
        let tabs = vec![tab.clone()];
        let mode = ModeInfo::default();
        let panes = make_pane_manifest(vec![(
            0,
            vec![PaneInfo {
                id: 77,
                is_plugin: false,
                ..Default::default()
            }],
        )]);

        let raw = BTreeMap::from([
            (
                "notification_indicator_completed".to_string(),
                "DONE".to_string(),
            ),
            (
                "notification_format_tab".to_string(),
                "[{icon}]".to_string(),
            ),
        ]);
        let config = PluginConfig::from_configuration(raw).unwrap();

        let mut notifications = NotificationTracker::default();
        notifications.add(77, NotificationType::Completed);
        let command_results = BTreeMap::new();
        let pipe_data = BTreeMap::new();

        let state = make_plugin_state_for_notification_test(
            &tabs,
            &panes,
            &config,
            &notifications,
            &mode,
            &command_results,
            &pipe_data,
        );

        assert_eq!(resolve_notification_icon(&tab, &state), "[DONE]");
    }

    #[test]
    fn render_tab_applies_notification_tab_style_to_inactive_tabs() {
        let tab = make_tab(0, "work", false);
        let tabs = vec![tab.clone()];
        let mode = ModeInfo::default();
        let panes = make_pane_manifest(vec![(
            0,
            vec![PaneInfo {
                id: 11,
                is_plugin: false,
                ..Default::default()
            }],
        )]);

        let raw = BTreeMap::from([
            (
                "tab_normal".to_string(),
                "#[bg=black,fg=white] {name} #[bg=default,fg=black]".to_string(),
            ),
            (
                "notification_tab_style_waiting".to_string(),
                "#[bg=yellow,fg=black,bold]".to_string(),
            ),
        ]);
        let widget = make_widget(&raw);
        let config = PluginConfig::from_configuration(raw).unwrap();

        let mut notifications = NotificationTracker::default();
        notifications.add(11, NotificationType::Waiting);
        let command_results = BTreeMap::new();
        let pipe_data = BTreeMap::new();
        let state = make_plugin_state_for_notification_test(
            &tabs,
            &panes,
            &config,
            &notifications,
            &mode,
            &command_results,
            &pipe_data,
        );

        let parts = widget.parts_for_tab(&tab, &state);
        assert_eq!(parts.len(), 2);
        assert_eq!(parts[0].content, " {name} ");
        assert_eq!(parts[1].content, "");
        assert!(parts[0].bg.is_some());
        assert_eq!(parts[0].bg, parts[1].fg);
        assert!(parts[0].effects.contains(Effects::BOLD));
    }

    #[test]
    fn render_tab_keeps_active_style_by_default() {
        let tab = make_tab(0, "focus", true);
        let tabs = vec![tab.clone()];
        let mode = ModeInfo::default();
        let panes = make_pane_manifest(vec![(
            0,
            vec![PaneInfo {
                id: 12,
                is_plugin: false,
                ..Default::default()
            }],
        )]);

        let raw = BTreeMap::from([
            (
                "tab_active".to_string(),
                "#[bg=green,fg=black] {name} ".to_string(),
            ),
            (
                "notification_tab_style_waiting".to_string(),
                "#[bg=yellow,fg=black,bold]".to_string(),
            ),
        ]);
        let widget = make_widget(&raw);
        let config = PluginConfig::from_configuration(raw).unwrap();

        let mut notifications = NotificationTracker::default();
        notifications.add(12, NotificationType::Waiting);
        let command_results = BTreeMap::new();
        let pipe_data = BTreeMap::new();
        let state = make_plugin_state_for_notification_test(
            &tabs,
            &panes,
            &config,
            &notifications,
            &mode,
            &command_results,
            &pipe_data,
        );

        let parts = widget.parts_for_tab(&tab, &state);
        assert_eq!(parts.len(), 1);
        assert_eq!(parts[0].content, " {name} ");
        assert!(!parts[0].effects.contains(Effects::BOLD));
    }

    #[test]
    fn render_tab_updates_powerline_cap_transition_colors() {
        let tab = make_tab(0, "power", false);
        let tabs = vec![tab.clone()];
        let mode = ModeInfo::default();
        let panes = make_pane_manifest(vec![(
            0,
            vec![PaneInfo {
                id: 13,
                is_plugin: false,
                ..Default::default()
            }],
        )]);

        let raw = BTreeMap::from([
            (
                "tab_normal".to_string(),
                "#[bg=black,fg=white]#[bg=black,fg=white] {name} #[bg=default,fg=black]"
                    .to_string(),
            ),
            (
                "notification_tab_style_waiting".to_string(),
                "#[bg=yellow,fg=black]".to_string(),
            ),
        ]);
        let widget = make_widget(&raw);
        let config = PluginConfig::from_configuration(raw).unwrap();

        let mut notifications = NotificationTracker::default();
        notifications.add(13, NotificationType::Waiting);
        let command_results = BTreeMap::new();
        let pipe_data = BTreeMap::new();
        let state = make_plugin_state_for_notification_test(
            &tabs,
            &panes,
            &config,
            &notifications,
            &mode,
            &command_results,
            &pipe_data,
        );

        let parts = widget.parts_for_tab(&tab, &state);
        assert_eq!(parts.len(), 3);
        assert_eq!(parts[0].content, "");
        assert_eq!(parts[1].content, " {name} ");
        assert_eq!(parts[2].content, "");
        assert_eq!(parts[0].bg, parts[1].bg);
        assert_eq!(parts[2].fg, parts[1].bg);
    }
}
