use std::collections::BTreeMap;

use super::{PluginState, Widget};

/// Displays the current Zellij session name.
///
/// No configuration needed — the session name comes from `ModeInfo`.
pub struct SessionWidget;

impl SessionWidget {
    pub fn new(_config: &BTreeMap<String, String>) -> Self {
        Self
    }
}

impl Widget for SessionWidget {
    fn process(&self, _name: &str, state: &PluginState<'_>) -> String {
        state.mode.session_name.clone().unwrap_or_default()
    }

    fn process_click(&self, _name: &str, _state: &PluginState<'_>, _col: usize) {
        // No click action for session widget.
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::PluginConfig;
    use crate::notify::tracker::NotificationTracker;
    use zellij_tile::prelude::{ModeInfo, PaneManifest};

    fn make_state(
        session_name: &str,
    ) -> (
        Vec<zellij_tile::prelude::TabInfo>,
        ModeInfo,
        PaneManifest,
        PluginConfig,
        NotificationTracker,
        BTreeMap<String, crate::widgets::command::CommandResult>,
        BTreeMap<String, String>,
    ) {
        let tabs = vec![];
        let mut mode = ModeInfo::default();
        mode.session_name = Some(session_name.to_string());
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
    fn returns_session_name() {
        let (tabs, mode, panes, config, notifications, cmd, pipe) = make_state("my-session");
        let state = PluginState {
            tabs: &tabs,
            panes: &panes,
            mode: &mode,
            config: &config,
            notifications: &notifications,
            command_results: &cmd,
            pipe_data: &pipe,
        };
        let w = SessionWidget::new(&BTreeMap::new());
        assert_eq!(w.process("session", &state), "my-session");
    }

    #[test]
    fn returns_empty_when_no_session_name() {
        let (tabs, mode, panes, config, notifications, cmd, pipe) = make_state("");
        let state = PluginState {
            tabs: &tabs,
            panes: &panes,
            mode: &mode,
            config: &config,
            notifications: &notifications,
            command_results: &cmd,
            pipe_data: &pipe,
        };
        let w = SessionWidget::new(&BTreeMap::new());
        assert_eq!(w.process("session", &state), "");
    }
}
