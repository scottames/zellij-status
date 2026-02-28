use std::collections::BTreeMap;
use std::sync::Arc;

use zellij_tile::prelude::{ModeInfo, PaneManifest, TabInfo};

use crate::config::PluginConfig;
use crate::notify::tracker::NotificationTracker;
use crate::render::format::FormattedPart;

pub mod cap;
pub mod command;
pub mod datetime;
pub mod mode;
pub mod notification;
pub mod pipe;
pub mod session;
pub mod swap_layout;
pub mod tabs;

/// A read-only view of plugin state passed to widgets for rendering.
pub struct PluginState<'a> {
    pub tabs: &'a [TabInfo],
    pub panes: &'a PaneManifest,
    pub mode: &'a ModeInfo,
    pub config: &'a PluginConfig,
    pub notifications: &'a NotificationTracker,
    /// Cached command results keyed by widget name (e.g., `"command_git"`).
    pub command_results: &'a BTreeMap<String, command::CommandResult>,
    /// Pipe widget data keyed by widget name (e.g., `"pipe_status"`).
    pub pipe_data: &'a BTreeMap<String, String>,
}

/// A widget that can render a string value and handle click events.
pub trait Widget: Send + Sync {
    /// Render the widget to a string given current plugin state.
    fn process(&self, name: &str, state: &PluginState<'_>) -> String;

    /// Handle a click at `col` offset (horizontal position within the widget).
    fn process_click(&self, name: &str, state: &PluginState<'_>, col: usize);

    /// Optional fill style used by row-level alignment padding.
    fn fill_part(&self, _name: &str, _state: &PluginState<'_>) -> Option<FormattedPart> {
        None
    }
}

/// Register all built-in widgets, returning them keyed by name.
pub fn register_widgets(config: &PluginConfig) -> BTreeMap<String, Arc<dyn Widget>> {
    let mut map: BTreeMap<String, Arc<dyn Widget>> = BTreeMap::new();

    map.insert(
        "tabs".to_string(),
        Arc::new(tabs::TabsWidget::new(&config.raw)),
    );
    map.insert(
        "mode".to_string(),
        Arc::new(mode::ModeWidget::new(&config.raw)),
    );
    map.insert(
        "session".to_string(),
        Arc::new(session::SessionWidget::new(&config.raw)),
    );
    map.insert(
        "datetime".to_string(),
        Arc::new(datetime::DateTimeWidget::new(&config.raw)),
    );
    map.insert(
        "notifications".to_string(),
        Arc::new(notification::NotificationWidget::new(&config.raw)),
    );
    map.insert(
        "swap_layout".to_string(),
        Arc::new(swap_layout::SwapLayoutWidget::new(&config.raw)),
    );

    // Dynamic widgets: one entry per configured instance.
    let command_widget = Arc::new(command::CommandWidget::new(&config.raw));
    for name in command_widget.names() {
        map.insert(name, Arc::clone(&command_widget) as Arc<dyn Widget>);
    }

    let pipe_widget = Arc::new(pipe::PipeWidget::new(&config.raw));
    for name in pipe_widget.names() {
        map.insert(name, Arc::clone(&pipe_widget) as Arc<dyn Widget>);
    }

    let cap_widget = Arc::new(cap::CapWidget::new(map.clone()));
    for name in cap_widget.names() {
        map.insert(name, Arc::clone(&cap_widget) as Arc<dyn Widget>);
    }

    map
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::notify::tracker::NotificationTracker;
    use zellij_tile::prelude::{ModeInfo, PaneManifest};

    fn make_state_with_config(
        raw: BTreeMap<String, String>,
    ) -> (
        PluginConfig,
        ModeInfo,
        PaneManifest,
        NotificationTracker,
        BTreeMap<String, command::CommandResult>,
        BTreeMap<String, String>,
        Vec<TabInfo>,
    ) {
        let config = PluginConfig::from_configuration(raw).unwrap();
        let mode = ModeInfo::default();
        let panes = PaneManifest::default();
        let notifications = NotificationTracker::default();
        let command_results = BTreeMap::new();
        let pipe_data = BTreeMap::new();
        let tabs = Vec::new();
        (
            config,
            mode,
            panes,
            notifications,
            command_results,
            pipe_data,
            tabs,
        )
    }

    #[test]
    fn register_widgets_includes_mode_cap_widget() {
        let raw = BTreeMap::from([
            (
                "mode_normal".to_string(),
                "#[bg=blue,fg=black,bold,fill] NORMAL ".to_string(),
            ),
            ("cap_bg".to_string(), "black".to_string()),
        ]);
        let (config, mode, panes, notifications, command_results, pipe_data, tabs) =
            make_state_with_config(raw);
        let widgets = register_widgets(&config);
        let state = PluginState {
            tabs: &tabs,
            panes: &panes,
            mode: &mode,
            config: &config,
            notifications: &notifications,
            command_results: &command_results,
            pipe_data: &pipe_data,
        };

        let cap = widgets
            .get("mode_cap")
            .expect("mode_cap should be registered");
        let rendered = cap.process("mode_cap", &state);
        assert!(!rendered.is_empty());
        assert!(rendered.contains(""));
        assert!(rendered.contains('\x1b'));
    }
}
