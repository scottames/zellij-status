use std::cmp::{max, min};
use std::collections::BTreeMap;
use std::fs;
use std::path::PathBuf;

use zellij_tile::prelude::*;
#[cfg(target_arch = "wasm32")]
use zellij_tile::shim::switch_tab_to;
#[cfg(target_arch = "wasm32")]
use zellij_tile::shim::unblock_cli_pipe_input;

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
}

impl ZellijPlugin for State {
    fn load(&mut self, configuration: BTreeMap<String, String>) {
        request_permission(&[
            PermissionType::ReadApplicationState,
            PermissionType::ChangeApplicationState,
            PermissionType::ReadCliPipes,
            PermissionType::RunCommands,
        ]);

        subscribe(&[
            EventType::Mouse,
            EventType::ModeUpdate,
            EventType::TabUpdate,
            EventType::PaneUpdate,
            EventType::PermissionRequestResult,
            EventType::SessionUpdate,
            EventType::RunCommandResult,
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

        if !self.permissions_granted || self.tabs.is_empty() {
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
                true
            }
            Event::ModeUpdate(mode) => {
                self.mode = mode;
                self.hydrate_notifications_from_store();
                true
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
    use crate::notify::NotificationType;

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
}
