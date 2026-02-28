use std::collections::BTreeMap;

use super::{PluginState, Widget};

/// Displays an aggregate notification count across all tabs.
///
/// Distinct from the per-tab `{notification}` variable in tab format strings:
/// this widget shows a global count suitable for format_1/format_3 sections.
///
/// Config keys:
/// - `notification_format` — format string with `{count}` placeholder
///   (default: `" {count}"`)
/// - `notification_show_if_empty` — `"true"` to show even when count is 0
///   (default: `"false"`)
pub struct NotificationWidget {
    format: String,
    show_if_empty: bool,
}

impl NotificationWidget {
    pub fn new(config: &BTreeMap<String, String>) -> Self {
        let format = config
            .get("notification_format")
            .cloned()
            .unwrap_or_else(|| " {count}".to_string());

        let show_if_empty = config
            .get("notification_show_if_empty")
            .is_some_and(|v| v == "true");

        Self {
            format,
            show_if_empty,
        }
    }
}

impl Widget for NotificationWidget {
    fn process(&self, _name: &str, state: &PluginState<'_>) -> String {
        if !state.config.notifications.enabled {
            return String::new();
        }

        let count = state.notifications.total_count();

        if count == 0 && !self.show_if_empty {
            return String::new();
        }

        self.format.replace("{count}", &count.to_string())
    }

    fn process_click(&self, _name: &str, _state: &PluginState<'_>, _col: usize) {
        // No click action for notification widget.
    }

    fn fill_part(
        &self,
        _name: &str,
        state: &PluginState<'_>,
    ) -> Option<crate::render::format::FormattedPart> {
        crate::render::format::parse_format_string(&self.format, &state.config.color_aliases)
            .into_iter()
            .find(|part| part.fill)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::PluginConfig;
    use crate::notify::tracker::NotificationTracker;
    use crate::notify::NotificationType;
    use zellij_tile::prelude::{ModeInfo, PaneManifest};

    fn make_state_with_notifications(
        count: usize,
    ) -> (
        Vec<zellij_tile::prelude::TabInfo>,
        ModeInfo,
        PaneManifest,
        PluginConfig,
        NotificationTracker,
        std::collections::BTreeMap<String, crate::widgets::command::CommandResult>,
        std::collections::BTreeMap<String, String>,
    ) {
        let tabs = vec![];
        let mode = ModeInfo::default();
        let panes = PaneManifest::default();
        let config = PluginConfig::from_configuration(std::collections::BTreeMap::new()).unwrap();
        let mut notifications = NotificationTracker::default();
        for i in 0..count {
            notifications.add(i as u32, NotificationType::Waiting);
        }
        let command_results = std::collections::BTreeMap::new();
        let pipe_data = std::collections::BTreeMap::new();
        (
            tabs,
            mode,
            panes,
            config,
            notifications,
            command_results,
            pipe_data,
        )
    }

    #[test]
    fn empty_when_no_notifications() {
        let (tabs, mode, panes, config, notifications, cmd, pipe) =
            make_state_with_notifications(0);
        let state = PluginState {
            tabs: &tabs,
            panes: &panes,
            mode: &mode,
            config: &config,
            notifications: &notifications,
            command_results: &cmd,
            pipe_data: &pipe,
        };
        let w = NotificationWidget::new(&BTreeMap::new());
        assert_eq!(w.process("notifications", &state), "");
    }

    #[test]
    fn shows_count_when_notifications_exist() {
        let (tabs, mode, panes, config, notifications, cmd, pipe) =
            make_state_with_notifications(3);
        let state = PluginState {
            tabs: &tabs,
            panes: &panes,
            mode: &mode,
            config: &config,
            notifications: &notifications,
            command_results: &cmd,
            pipe_data: &pipe,
        };
        let w = NotificationWidget::new(&BTreeMap::new());
        assert_eq!(w.process("notifications", &state), " 3");
    }

    #[test]
    fn custom_format() {
        let (tabs, mode, panes, config, notifications, cmd, pipe) =
            make_state_with_notifications(2);
        let state = PluginState {
            tabs: &tabs,
            panes: &panes,
            mode: &mode,
            config: &config,
            notifications: &notifications,
            command_results: &cmd,
            pipe_data: &pipe,
        };
        let widget_config =
            BTreeMap::from([("notification_format".to_string(), "({count})".to_string())]);
        let w = NotificationWidget::new(&widget_config);
        assert_eq!(w.process("notifications", &state), "(2)");
    }

    #[test]
    fn show_if_empty_option() {
        let (tabs, mode, panes, config, notifications, cmd, pipe) =
            make_state_with_notifications(0);
        let state = PluginState {
            tabs: &tabs,
            panes: &panes,
            mode: &mode,
            config: &config,
            notifications: &notifications,
            command_results: &cmd,
            pipe_data: &pipe,
        };
        let widget_config = BTreeMap::from([
            ("notification_format".to_string(), "({count})".to_string()),
            ("notification_show_if_empty".to_string(), "true".to_string()),
        ]);
        let w = NotificationWidget::new(&widget_config);
        assert_eq!(w.process("notifications", &state), "(0)");
    }
}
