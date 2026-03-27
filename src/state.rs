use std::cmp::{max, min};
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::PathBuf;

use zellij_tile::prelude::*;
#[cfg(target_arch = "wasm32")]
use zellij_tile::shim::highlight_and_unhighlight_panes;
#[cfg(target_arch = "wasm32")]
use zellij_tile::shim::switch_tab_to;
#[cfg(target_arch = "wasm32")]
use zellij_tile::shim::unblock_cli_pipe_input;

#[cfg(not(target_arch = "wasm32"))]
fn highlight_and_unhighlight_panes(
    _pane_ids_to_highlight: Vec<PaneId>,
    _pane_ids_to_unhighlight: Vec<PaneId>,
) {
}

#[cfg(not(target_arch = "wasm32"))]
fn switch_tab_to(_tab_index: u32) {}

#[cfg(not(target_arch = "wasm32"))]
fn unblock_cli_pipe_input(_pipe_name: &str) {}

use crate::config::{LayoutMode, PluginConfig};
use crate::notify::protocol::{parse_pipe_data, parse_pipe_message};
use crate::notify::tracker::NotificationTracker;
use crate::render::bar::render_bar;
use crate::render::vertical::{render_vertical, tab_at_row};
use crate::widgets::command::CommandResult;
use crate::widgets::{register_widgets, tabs::TabsWidget, PluginState};

/// Main plugin state implementing ZellijPlugin.
#[derive(Default)]
pub struct State {
    /// Whether permissions have been granted by Zellij.
    permissions_granted: bool,

    /// Events received before permissions were granted.
    pending_events: Vec<Event>,

    /// Parsed plugin configuration.
    config: Option<PluginConfig>,

    /// Current tab information.
    tabs: Vec<TabInfo>,

    /// Current pane manifest.
    panes: PaneManifest,

    /// Current Zellij mode.
    mode: ModeInfo,

    /// Current session info.
    sessions: Vec<SessionInfo>,

    /// Cached number of rows from last render (needed for click handling).
    last_rows: usize,

    /// Per-pane notification tracker (Phase 3).
    notifications: NotificationTracker,

    /// Cached command widget results keyed by widget name (Phase 5).
    command_results: BTreeMap<String, CommandResult>,

    /// Pipe widget data keyed by widget name (Phase 5).
    pipe_data: BTreeMap<String, String>,

    /// Pane highlights currently applied for notification state.
    highlighted_notification_panes: BTreeSet<PaneId>,
}

impl ZellijPlugin for State {
    fn load(&mut self, configuration: BTreeMap<String, String>) {
        subscribe(&[
            EventType::Mouse,
            EventType::ModeUpdate,
            EventType::TabUpdate,
            EventType::PaneUpdate,
            EventType::PaneClosed,
            EventType::PluginConfigurationChanged,
            EventType::PermissionRequestResult,
            EventType::SessionUpdate,
            EventType::RunCommandResult,
        ]);

        request_permission(&[
            PermissionType::ReadApplicationState,
            PermissionType::ChangeApplicationState,
            PermissionType::ReadCliPipes,
            PermissionType::RunCommands,
        ]);

        match PluginConfig::from_configuration(configuration) {
            Ok(config) => self.config = Some(config),
            Err(e) => eprintln!("zellij-status: config error: {e}"),
        }
    }

    fn update(&mut self, event: Event) -> bool {
        if let Event::PermissionRequestResult(PermissionStatus::Granted) = &event {
            self.permissions_granted = true;
            set_selectable(false);

            // Replay events that arrived before permissions were granted.
            let pending = std::mem::take(&mut self.pending_events);
            for e in pending {
                self.handle_event(e);
            }
            self.sync_notification_pane_highlights();
            return true;
        }

        if !self.permissions_granted {
            self.pending_events.push(event);
            return false;
        }

        self.handle_event(event)
    }

    fn pipe(&mut self, pipe_message: PipeMessage) -> bool {
        let payload_ref = pipe_message.payload.as_deref();

        // Try notification protocol first.
        if let Some(notification) = parse_pipe_message(&pipe_message.name, payload_ref) {
            let enabled = self
                .config
                .as_ref()
                .map(|c| c.notifications.enabled)
                .unwrap_or(true);

            if enabled {
                self.notifications
                    .add(notification.pane_id, notification.notification_type);
                self.persist_notifications_to_store();
                self.sync_notification_pane_highlights();
            }

            unblock_cli_pipe_input(&pipe_message.name);
            return enabled;
        }

        // Try pipe widget data protocol.
        if let Some(data) = parse_pipe_data(&pipe_message.name, payload_ref) {
            self.pipe_data.insert(data.key, data.value);
            unblock_cli_pipe_input(&pipe_message.name);
            return true; // re-render with new data
        }

        // Not our message — unblock and ignore.
        unblock_cli_pipe_input(&pipe_message.name);
        false
    }

    fn render(&mut self, rows: usize, cols: usize) {
        self.last_rows = rows;

        if let Some(message) = self.permission_status_message(cols) {
            print!("{message}");
            return;
        }

        if self.tabs.is_empty() {
            return;
        }

        let Some(config) = &self.config else {
            print!("zellij-status: no valid config");
            return;
        };

        let state = PluginState {
            tabs: &self.tabs,
            panes: &self.panes,
            mode: &self.mode,
            config,
            notifications: &self.notifications,
            command_results: &self.command_results,
            pipe_data: &self.pipe_data,
        };

        let widgets = register_widgets(config);

        match config.layout_mode {
            LayoutMode::Vertical => {
                let tabs_widget = TabsWidget::new(&config.raw);
                render_vertical(&tabs_widget, &widgets, &state, rows, cols);
            }
            LayoutMode::Horizontal => {
                render_bar(&widgets, &state, rows, cols);
            }
        }
    }
}

impl State {
    fn permission_status_message(&self, cols: usize) -> Option<String> {
        if self.permissions_granted {
            return None;
        }

        Some(
            "zellij-status: grant plugin permissions"
                .chars()
                .take(cols)
                .collect(),
        )
    }

    fn merged_runtime_config(
        &self,
        updates: BTreeMap<String, String>,
    ) -> Result<BTreeMap<String, String>, String> {
        if let Some(layout_mode) = updates.get("layout_mode") {
            let normalized = layout_mode.to_ascii_lowercase();
            if normalized != "horizontal" && normalized != "vertical" {
                return Err(format!("invalid layout_mode: {layout_mode}"));
            }
        }

        let mut merged = self
            .config
            .as_ref()
            .map(|config| config.raw.clone())
            .unwrap_or_default();
        merged.extend(updates);
        Ok(merged)
    }

    fn notification_store_path(&self) -> Option<PathBuf> {
        let session_name = self.mode.session_name.as_deref()?;
        let safe_name: String = session_name
            .chars()
            .map(|ch| {
                if ch.is_ascii_alphanumeric() || matches!(ch, '-' | '_') {
                    ch
                } else {
                    '_'
                }
            })
            .collect();
        Some(PathBuf::from(format!(
            "/tmp/zellij-status-notifications-{safe_name}.json"
        )))
    }

    fn hydrate_notifications_from_store(&mut self) -> bool {
        if !self.notifications.is_empty() {
            return false;
        }

        let Some(path) = self.notification_store_path() else {
            return false;
        };

        let Ok(contents) = fs::read_to_string(path) else {
            return false;
        };

        let Ok(restored) = serde_json::from_str::<NotificationTracker>(&contents) else {
            return false;
        };

        if restored.is_empty() {
            return false;
        }

        self.notifications = restored;
        true
    }

    fn persist_notifications_to_store(&self) {
        let Some(path) = self.notification_store_path() else {
            return;
        };

        if self.notifications.is_empty() {
            let _ = fs::remove_file(path);
            return;
        }

        let Ok(serialized) = serde_json::to_string(&self.notifications) else {
            return;
        };

        let _ = fs::write(path, serialized);
    }

    fn notification_pane_highlighting_enabled(&self) -> bool {
        self.config.as_ref().is_some_and(|config| {
            config.notifications.enabled && config.notifications.pane_highlight_enabled
        })
    }

    fn sync_notification_pane_highlights(&mut self) {
        if !self.permissions_granted {
            return;
        }

        let desired: BTreeSet<PaneId> = if self.notification_pane_highlighting_enabled() {
            self.notifications
                .highlighted_panes(&self.tabs, &self.panes)
                .into_iter()
                .collect()
        } else {
            BTreeSet::new()
        };

        let panes_to_highlight: Vec<PaneId> = desired
            .difference(&self.highlighted_notification_panes)
            .copied()
            .collect();
        let panes_to_unhighlight: Vec<PaneId> = self
            .highlighted_notification_panes
            .difference(&desired)
            .copied()
            .collect();

        if panes_to_highlight.is_empty() && panes_to_unhighlight.is_empty() {
            return;
        }

        highlight_and_unhighlight_panes(panes_to_highlight, panes_to_unhighlight);
        self.highlighted_notification_panes = desired;
    }

    fn handle_closed_pane(&mut self, pane_id: PaneId) -> bool {
        let PaneId::Terminal(pane_id) = pane_id else {
            return false;
        };

        let had_notification = self.notifications.total_count();
        self.notifications.clear_pane(pane_id);
        let notification_removed = self.notifications.total_count() != had_notification;

        if notification_removed {
            self.persist_notifications_to_store();
        }

        let pane_id = PaneId::Terminal(pane_id);
        let highlight_removed = self.highlighted_notification_panes.remove(&pane_id);

        if highlight_removed {
            highlight_and_unhighlight_panes(Vec::new(), vec![pane_id]);
        }

        notification_removed || highlight_removed
    }

    /// Handle a single event after permissions are granted.
    fn handle_event(&mut self, event: Event) -> bool {
        match event {
            Event::TabUpdate(tabs) => {
                self.tabs = tabs;
                // Clear notifications for the focused pane (focus-clears behavior)
                let cleared = self.notifications.clear_focused(&self.tabs, &self.panes);
                let cleaned = self.notifications.clean_stale(&self.panes);
                if !cleared && !cleaned {
                    self.hydrate_notifications_from_store();
                } else {
                    self.persist_notifications_to_store();
                }
                self.sync_notification_pane_highlights();
                // Always re-render on tab update; cleared/cleaned just confirm side effects
                let _ = (cleared, cleaned);
                true
            }
            Event::PaneUpdate(panes) => {
                self.panes = panes;
                // Clear focused pane notifications + clean stale entries
                let cleared = self.notifications.clear_focused(&self.tabs, &self.panes);
                let cleaned = self.notifications.clean_stale(&self.panes);
                if !cleared && !cleaned {
                    self.hydrate_notifications_from_store();
                } else {
                    self.persist_notifications_to_store();
                }
                self.sync_notification_pane_highlights();
                true
            }
            Event::PaneClosed(pane_id) => self.handle_closed_pane(pane_id),
            Event::ModeUpdate(mode) => {
                self.mode = mode;
                self.hydrate_notifications_from_store();
                self.sync_notification_pane_highlights();
                true
            }
            Event::PluginConfigurationChanged(configuration) => {
                match self
                    .merged_runtime_config(configuration)
                    .and_then(|config| {
                        PluginConfig::from_configuration(config).map_err(|e| e.to_string())
                    }) {
                    Ok(config) => {
                        self.config = Some(config);
                        self.sync_notification_pane_highlights();
                        true
                    }
                    Err(e) => {
                        eprintln!("zellij-status: config update error: {e}");
                        false
                    }
                }
            }
            Event::SessionUpdate(sessions, _) => {
                self.sessions = sessions;
                true
            }
            Event::RunCommandResult(exit_code, stdout, stderr, context) => {
                if let Some(name) = context.get("name").cloned() {
                    self.command_results.insert(
                        name,
                        CommandResult {
                            exit_code,
                            stdout: String::from_utf8_lossy(&stdout).to_string(),
                            stderr: String::from_utf8_lossy(&stderr).to_string(),
                            context,
                        },
                    );
                }
                true
            }
            Event::Mouse(me) => self.handle_mouse(me),
            _ => false,
        }
    }

    fn handle_mouse(&mut self, me: Mouse) -> bool {
        let tab_count = self.tabs.len();
        if tab_count == 0 {
            return false;
        }

        let active_index = self.tabs.iter().position(|t| t.active).unwrap_or(0);

        match me {
            Mouse::LeftClick(row, _col) => {
                let padding_top = self
                    .config
                    .as_ref()
                    .map(|c| c.tabs.padding_top)
                    .unwrap_or(0);
                let available = self.last_rows.saturating_sub(padding_top);
                let row_in_content = (row as usize).saturating_sub(padding_top);

                if let Some(idx) = tab_at_row(row_in_content, tab_count, available, active_index) {
                    switch_tab_to(idx as u32);
                    return true;
                }
                false
            }
            Mouse::ScrollUp(_) => {
                let prev = max(active_index.saturating_sub(1), 0) + 1;
                switch_tab_to(prev as u32);
                true
            }
            Mouse::ScrollDown(_) => {
                let next = min(active_index + 2, tab_count); // +2: active_index is 0-based, switch_tab_to is 1-based
                switch_tab_to(next as u32);
                true
            }
            _ => false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{LayoutMode, PluginConfig};
    use crate::notify::NotificationType;

    fn make_tab(position: usize, active: bool) -> TabInfo {
        TabInfo {
            position,
            active,
            ..Default::default()
        }
    }

    fn make_pane(id: u32, is_focused: bool) -> PaneInfo {
        PaneInfo {
            id,
            is_focused,
            ..Default::default()
        }
    }

    fn state_with_session(session_name: &str) -> State {
        State {
            mode: ModeInfo {
                session_name: Some(session_name.to_string()),
                ..Default::default()
            },
            ..Default::default()
        }
    }

    fn remove_store_file(state: &State) {
        if let Some(path) = state.notification_store_path() {
            let _ = fs::remove_file(path);
        }
    }

    fn configured_state(session_name: &str, raw: BTreeMap<String, String>) -> State {
        let mut state = state_with_session(session_name);
        state.permissions_granted = true;
        state.config = Some(PluginConfig::from_configuration(raw).unwrap());
        state
    }

    #[test]
    fn persist_and_hydrate_notifications_for_same_session() {
        let session_name = format!(
            "state-test-persist-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        );
        let mut writer = state_with_session(&session_name);
        remove_store_file(&writer);

        writer.notifications.add(42, NotificationType::Waiting);
        writer.persist_notifications_to_store();

        let mut reader = state_with_session(&session_name);
        assert!(reader.hydrate_notifications_from_store());
        assert_eq!(reader.notifications.total_count(), 1);

        remove_store_file(&reader);
    }

    #[test]
    fn persist_notifications_removes_file_when_tracker_empty() {
        let session_name = format!(
            "state-test-empty-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        );
        let mut state = state_with_session(&session_name);
        remove_store_file(&state);

        state.notifications.add(7, NotificationType::Completed);
        state.persist_notifications_to_store();
        let path = state.notification_store_path().expect("session path");
        assert!(path.exists());

        state.notifications.clear_pane(7);
        state.persist_notifications_to_store();
        assert!(!path.exists());
    }

    #[test]
    fn mode_update_hydrates_notifications_from_store() {
        let session_name = format!(
            "state-test-mode-update-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        );
        let mut writer = state_with_session(&session_name);
        remove_store_file(&writer);

        writer.notifications.add(99, NotificationType::InProgress);
        writer.persist_notifications_to_store();

        let mut reader = State::default();
        let changed = reader.handle_event(Event::ModeUpdate(ModeInfo {
            session_name: Some(session_name.clone()),
            ..Default::default()
        }));

        assert!(changed);
        assert_eq!(reader.notifications.total_count(), 1);

        remove_store_file(&writer);
    }

    #[test]
    fn pipe_notification_persists_to_store() {
        let session_name = format!(
            "state-test-pipe-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        );
        let mut state = state_with_session(&session_name);
        remove_store_file(&state);

        let should_render = state.pipe(PipeMessage::new(
            PipeSource::Cli("test-pipe".to_string()),
            "zellij-status::waiting::123",
            &None,
            &None,
            false,
        ));

        assert!(should_render);

        let mut hydrated = state_with_session(&session_name);
        assert!(hydrated.hydrate_notifications_from_store());
        assert_eq!(hydrated.notifications.total_count(), 1);

        remove_store_file(&hydrated);
    }

    #[test]
    fn sync_notification_pane_highlights_tracks_notified_inactive_panes() {
        let mut state = configured_state("highlight-session", BTreeMap::new());
        state.tabs = vec![make_tab(0, true), make_tab(1, false)];
        state.panes = PaneManifest {
            panes: [
                (0, vec![make_pane(1, false), make_pane(2, true)]),
                (1, vec![make_pane(3, false)]),
            ]
            .into_iter()
            .collect(),
        };
        state.notifications.add(1, NotificationType::Waiting);
        state.notifications.add(2, NotificationType::InProgress);
        state.notifications.add(3, NotificationType::Completed);

        state.sync_notification_pane_highlights();

        assert_eq!(
            state.highlighted_notification_panes,
            BTreeSet::from([PaneId::Terminal(1), PaneId::Terminal(3)])
        );
    }

    #[test]
    fn sync_notification_pane_highlights_is_disabled_with_notification_flag() {
        let mut state = configured_state(
            "highlight-opt-out",
            BTreeMap::from([(
                "notification_pane_highlight_enabled".to_string(),
                "false".to_string(),
            )]),
        );
        state.tabs = vec![make_tab(0, true)];
        state.panes = PaneManifest {
            panes: [(0, vec![make_pane(1, false)])].into_iter().collect(),
        };
        state.notifications.add(1, NotificationType::Waiting);

        state.sync_notification_pane_highlights();

        assert!(state.highlighted_notification_panes.is_empty());
    }

    #[test]
    fn sync_notification_pane_highlights_is_disabled_when_notifications_are_off() {
        let mut state = configured_state(
            "highlight-disabled-notifications",
            BTreeMap::from([("notification_enabled".to_string(), "false".to_string())]),
        );
        state.highlighted_notification_panes = BTreeSet::from([PaneId::Terminal(9)]);
        state.tabs = vec![make_tab(0, true)];
        state.panes = PaneManifest {
            panes: [(0, vec![make_pane(1, false)])].into_iter().collect(),
        };
        state.notifications.add(1, NotificationType::Waiting);

        state.sync_notification_pane_highlights();

        assert!(state.highlighted_notification_panes.is_empty());
    }

    #[test]
    fn pane_closed_clears_notification_and_cached_highlight() {
        let session_name = format!(
            "state-test-pane-closed-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        );
        let mut state = configured_state(&session_name, BTreeMap::new());
        remove_store_file(&state);
        state.notifications.add(42, NotificationType::Waiting);
        state.persist_notifications_to_store();
        state.highlighted_notification_panes = BTreeSet::from([PaneId::Terminal(42)]);

        assert!(state.handle_closed_pane(PaneId::Terminal(42)));
        assert!(state.notifications.is_empty());
        assert!(state.highlighted_notification_panes.is_empty());
        assert!(!state
            .notification_store_path()
            .expect("session path")
            .exists());
    }

    #[test]
    fn pane_closed_ignores_plugin_pane_ids() {
        let mut state = configured_state("state-test-pane-closed-plugin", BTreeMap::new());
        state.notifications.add(42, NotificationType::Waiting);
        state.highlighted_notification_panes = BTreeSet::from([PaneId::Terminal(42)]);

        assert!(!state.handle_closed_pane(PaneId::Plugin(5)));
        assert_eq!(state.notifications.total_count(), 1);
        assert_eq!(
            state.highlighted_notification_panes,
            BTreeSet::from([PaneId::Terminal(42)])
        );
    }

    #[test]
    fn plugin_configuration_changed_reloads_config() {
        let mut state = State {
            config: Some(PluginConfig::from_configuration(BTreeMap::new()).unwrap()),
            ..Default::default()
        };

        let mut updated = BTreeMap::new();
        updated.insert("layout_mode".to_string(), "vertical".to_string());

        let changed = state.handle_event(Event::PluginConfigurationChanged(updated));

        assert!(changed);
        assert_eq!(
            state.config.as_ref().map(|c| c.layout_mode),
            Some(LayoutMode::Vertical)
        );
    }

    #[test]
    fn plugin_configuration_changed_preserves_previous_config_on_invalid_layout_mode() {
        let mut state = State {
            config: Some(
                PluginConfig::from_configuration(BTreeMap::from([(
                    "layout_mode".to_string(),
                    "vertical".to_string(),
                )]))
                .unwrap(),
            ),
            ..Default::default()
        };

        let mut updated = BTreeMap::new();
        updated.insert("layout_mode".to_string(), "diagonal".to_string());

        let changed = state.handle_event(Event::PluginConfigurationChanged(updated));

        assert!(!changed);
        assert_eq!(
            state.config.as_ref().map(|c| c.layout_mode),
            Some(LayoutMode::Vertical)
        );
    }

    #[test]
    fn permission_status_message_is_shown_before_permissions_granted() {
        let state = State::default();

        assert_eq!(
            state.permission_status_message(80).as_deref(),
            Some("zellij-status: grant plugin permissions")
        );
    }

    #[test]
    fn permission_status_message_is_truncated_to_available_width() {
        let state = State::default();

        assert_eq!(
            state.permission_status_message(12).as_deref(),
            Some("zellij-statu")
        );
    }
}
