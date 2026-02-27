use std::sync::Arc;

use zellij_tile::prelude::{ModeInfo, PaneManifest, TabInfo};

use crate::config::PluginConfig;
use crate::notify::tracker::NotificationTracker;

pub mod tabs;

/// A read-only view of plugin state passed to widgets for rendering.
pub struct PluginState<'a> {
    pub tabs: &'a [TabInfo],
    pub panes: &'a PaneManifest,
    pub mode: &'a ModeInfo,
    pub config: &'a PluginConfig,
    pub notifications: &'a NotificationTracker,
}

/// A widget that can render a string value and handle click events.
pub trait Widget: Send + Sync {
    /// Render the widget to a string given current plugin state.
    fn process(&self, name: &str, state: &PluginState<'_>) -> String;

    /// Handle a click at `col` offset (horizontal position within the widget).
    fn process_click(&self, name: &str, state: &PluginState<'_>, col: usize);
}

/// Register all built-in widgets, returning them keyed by name.
pub fn register_widgets(
    config: &PluginConfig,
) -> std::collections::BTreeMap<String, Arc<dyn Widget>> {
    let mut map: std::collections::BTreeMap<String, Arc<dyn Widget>> =
        std::collections::BTreeMap::new();

    map.insert(
        "tabs".to_string(),
        Arc::new(tabs::TabsWidget::new(&config.raw)),
    );

    map
}
