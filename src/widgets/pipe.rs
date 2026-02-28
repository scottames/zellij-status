use std::collections::BTreeMap;

use super::{PluginState, Widget};

/// Configuration for a single pipe widget instance.
struct PipeConfig {
    /// Format string with `{output}` placeholder.
    format: String,
}

/// Displays arbitrary data received via pipe messages.
///
/// Multiple pipe widgets can be configured with different names. Each
/// reads from `PluginState.pipe_data` keyed by its widget name.
///
/// Config keys (one set per pipe widget):
/// - `pipe_NAME_format` — format string with `{output}` placeholder
///   (default: `"{output}"`)
///
/// Data sent via: `zellij pipe --name "zellij-status::pipe::NAME::VALUE"`
/// or: `zellij pipe --name "zellij-status::pipe::NAME" --payload "VALUE"`
pub struct PipeWidget {
    configs: BTreeMap<String, PipeConfig>,
}

impl PipeWidget {
    pub fn new(config: &BTreeMap<String, String>) -> Self {
        let mut configs = BTreeMap::new();

        for (key, value) in config {
            if let Some(name) = key.strip_suffix("_format") {
                if name.starts_with("pipe_") {
                    configs.insert(
                        name.to_string(),
                        PipeConfig {
                            format: value.clone(),
                        },
                    );
                }
            }
        }

        Self { configs }
    }

    /// Returns all widget names this widget handles (e.g., `["pipe_my_key"]`).
    pub fn names(&self) -> Vec<String> {
        self.configs.keys().cloned().collect()
    }
}

impl Widget for PipeWidget {
    fn process(&self, name: &str, state: &PluginState<'_>) -> String {
        let Some(pipe_config) = self.configs.get(name) else {
            return String::new();
        };

        let Some(value) = state.pipe_data.get(name) else {
            return String::new();
        };

        let output = value.trim_end_matches('\n');
        pipe_config.format.replace("{output}", output)
    }

    fn process_click(&self, _name: &str, _state: &PluginState<'_>, _col: usize) {
        // No click action for pipe widgets.
    }

    fn fill_part(
        &self,
        name: &str,
        state: &PluginState<'_>,
    ) -> Option<crate::render::format::FormattedPart> {
        let pipe_config = self.configs.get(name)?;
        crate::render::format::parse_format_string(&pipe_config.format, &state.config.color_aliases)
            .into_iter()
            .find(|part| part.fill)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::PluginConfig;
    use crate::notify::tracker::NotificationTracker;
    use crate::widgets::command::CommandResult;
    use zellij_tile::prelude::{ModeInfo, PaneManifest, TabInfo};

    fn make_state(
        pipe_data: BTreeMap<String, String>,
    ) -> (
        Vec<TabInfo>,
        ModeInfo,
        PaneManifest,
        PluginConfig,
        NotificationTracker,
        BTreeMap<String, CommandResult>,
        BTreeMap<String, String>,
    ) {
        let tabs = vec![];
        let mode = ModeInfo::default();
        let panes = PaneManifest::default();
        let config = PluginConfig::from_configuration(std::collections::BTreeMap::new()).unwrap();
        let notifications = NotificationTracker::default();
        let command_results = BTreeMap::new();
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
    fn parses_pipe_configs() {
        let config = BTreeMap::from([
            ("pipe_status_format".to_string(), "[{output}]".to_string()),
            ("pipe_health_format".to_string(), "{output}".to_string()),
            ("unrelated_key".to_string(), "value".to_string()),
        ]);
        let w = PipeWidget::new(&config);
        let mut names = w.names();
        names.sort();
        assert_eq!(names, vec!["pipe_health", "pipe_status"]);
    }

    #[test]
    fn returns_empty_when_no_data() {
        let (tabs, mode, panes, config, notifications, cmd, pipe) = make_state(BTreeMap::new());
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
            BTreeMap::from([("pipe_status_format".to_string(), "[{output}]".to_string())]);
        let w = PipeWidget::new(&widget_config);
        assert_eq!(w.process("pipe_status", &state), "");
    }

    #[test]
    fn shows_pipe_data() {
        let pipe_data = BTreeMap::from([("pipe_status".to_string(), "running".to_string())]);
        let (tabs, mode, panes, config, notifications, cmd, pipe) = make_state(pipe_data);
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
            BTreeMap::from([("pipe_status_format".to_string(), "[{output}]".to_string())]);
        let w = PipeWidget::new(&widget_config);
        assert_eq!(w.process("pipe_status", &state), "[running]");
    }

    #[test]
    fn strips_trailing_newline() {
        let pipe_data = BTreeMap::from([("pipe_status".to_string(), "ok\n".to_string())]);
        let (tabs, mode, panes, config, notifications, cmd, pipe) = make_state(pipe_data);
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
            BTreeMap::from([("pipe_status_format".to_string(), "{output}".to_string())]);
        let w = PipeWidget::new(&widget_config);
        assert_eq!(w.process("pipe_status", &state), "ok");
    }

    #[test]
    fn unknown_widget_name_returns_empty() {
        let pipe_data = BTreeMap::from([("pipe_status".to_string(), "running".to_string())]);
        let (tabs, mode, panes, config, notifications, cmd, pipe) = make_state(pipe_data);
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
            BTreeMap::from([("pipe_status_format".to_string(), "{output}".to_string())]);
        let w = PipeWidget::new(&widget_config);
        assert_eq!(w.process("pipe_unknown", &state), "");
    }
}
