use std::collections::BTreeMap;

#[cfg(not(test))]
use zellij_tile::shim::next_swap_layout;

use super::{PluginState, Widget};

/// Displays the active swap layout name with click-to-cycle support.
///
/// Config keys:
/// - `swap_layout_format` — format string with `{name}` placeholder
///   (default: `"{name}"`)
/// - `swap_layout_hide_if_empty` — `"true"` to hide when no layout is active
///   (default: `"true"`)
pub struct SwapLayoutWidget {
    format: String,
    hide_if_empty: bool,
}

impl SwapLayoutWidget {
    pub fn new(config: &BTreeMap<String, String>) -> Self {
        let format = config
            .get("swap_layout_format")
            .cloned()
            .unwrap_or_else(|| "{name}".to_string());

        let hide_if_empty = config
            .get("swap_layout_hide_if_empty")
            .map(|v| v != "false")
            .unwrap_or(true);

        Self {
            format,
            hide_if_empty,
        }
    }
}

impl Widget for SwapLayoutWidget {
    fn process(&self, _name: &str, state: &PluginState<'_>) -> String {
        let layout_name = state
            .tabs
            .iter()
            .find(|t| t.active)
            .and_then(|t| t.active_swap_layout_name.clone())
            .unwrap_or_default();

        if layout_name.is_empty() && self.hide_if_empty {
            return String::new();
        }

        self.format.replace("{name}", &layout_name)
    }

    fn process_click(&self, _name: &str, _state: &PluginState<'_>, _col: usize) {
        // next_swap_layout() is a WASM host function unavailable in native tests.
        #[cfg(not(test))]
        next_swap_layout();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::PluginConfig;
    use crate::notify::tracker::NotificationTracker;
    use zellij_tile::prelude::{ModeInfo, PaneManifest, TabInfo};

    fn make_state(
        layout_name: Option<&str>,
    ) -> (
        Vec<TabInfo>,
        ModeInfo,
        PaneManifest,
        PluginConfig,
        NotificationTracker,
        BTreeMap<String, crate::widgets::command::CommandResult>,
        BTreeMap<String, String>,
    ) {
        let tabs = vec![TabInfo {
            active: true,
            active_swap_layout_name: layout_name.map(String::from),
            ..Default::default()
        }];
        let mode = ModeInfo::default();
        let panes = PaneManifest::default();
        let config = PluginConfig::from_configuration(std::collections::BTreeMap::new()).unwrap();
        let notifications = NotificationTracker::default();
        let command_results = BTreeMap::new();
        let pipe_data = BTreeMap::new();
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
    fn shows_layout_name() {
        let (tabs, mode, panes, config, notifications, cmd, pipe) = make_state(Some("compact"));
        let state = PluginState {
            tabs: &tabs,
            panes: &panes,
            mode: &mode,
            config: &config,
            notifications: &notifications,
            command_results: &cmd,
            pipe_data: &pipe,
        };
        let w = SwapLayoutWidget::new(&BTreeMap::new());
        assert_eq!(w.process("swap_layout", &state), "compact");
    }

    #[test]
    fn hides_when_empty_by_default() {
        let (tabs, mode, panes, config, notifications, cmd, pipe) = make_state(None);
        let state = PluginState {
            tabs: &tabs,
            panes: &panes,
            mode: &mode,
            config: &config,
            notifications: &notifications,
            command_results: &cmd,
            pipe_data: &pipe,
        };
        let w = SwapLayoutWidget::new(&BTreeMap::new());
        assert_eq!(w.process("swap_layout", &state), "");
    }

    #[test]
    fn shows_empty_when_hide_disabled() {
        let (tabs, mode, panes, config, notifications, cmd, pipe) = make_state(None);
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
            BTreeMap::from([("swap_layout_hide_if_empty".to_string(), "false".to_string())]);
        let w = SwapLayoutWidget::new(&widget_config);
        // Should return the format with empty name substituted
        assert_eq!(w.process("swap_layout", &state), "");
    }

    #[test]
    fn custom_format() {
        let (tabs, mode, panes, config, notifications, cmd, pipe) = make_state(Some("wide"));
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
            BTreeMap::from([("swap_layout_format".to_string(), "[{name}]".to_string())]);
        let w = SwapLayoutWidget::new(&widget_config);
        assert_eq!(w.process("swap_layout", &state), "[wide]");
    }

    #[test]
    fn no_active_tab_returns_empty() {
        let mode = ModeInfo::default();
        let panes = PaneManifest::default();
        let config = PluginConfig::from_configuration(std::collections::BTreeMap::new()).unwrap();
        let notifications = NotificationTracker::default();
        let cmd = BTreeMap::new();
        let pipe = BTreeMap::new();
        let tabs: Vec<TabInfo> = vec![]; // no tabs
        let state = PluginState {
            tabs: &tabs,
            panes: &panes,
            mode: &mode,
            config: &config,
            notifications: &notifications,
            command_results: &cmd,
            pipe_data: &pipe,
        };
        let w = SwapLayoutWidget::new(&BTreeMap::new());
        assert_eq!(w.process("swap_layout", &state), "");
    }
}
