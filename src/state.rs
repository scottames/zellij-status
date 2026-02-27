use std::cmp::{max, min};
use std::collections::BTreeMap;

use zellij_tile::prelude::*;
use zellij_tile::shim::{switch_tab_to, unblock_cli_pipe_input};

use crate::config::{LayoutMode, PluginConfig};
use crate::notify::protocol::parse_pipe_message;
use crate::notify::tracker::NotificationTracker;
use crate::render::bar::render_bar;
use crate::render::vertical::{render_vertical, tab_at_row};
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
}

impl ZellijPlugin for State {
    fn load(&mut self, configuration: BTreeMap<String, String>) {
        request_permission(&[
            PermissionType::ReadApplicationState,
            PermissionType::ChangeApplicationState,
            PermissionType::ReadCliPipes,
        ]);

        subscribe(&[
            EventType::Mouse,
            EventType::ModeUpdate,
            EventType::TabUpdate,
            EventType::PaneUpdate,
            EventType::PermissionRequestResult,
            EventType::SessionUpdate,
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

        if let Some(notification) = parse_pipe_message(&pipe_message.name, payload_ref) {
            let enabled = self
                .config
                .as_ref()
                .map(|c| c.notifications.enabled)
                .unwrap_or(true);

            if enabled {
                self.notifications
                    .add(notification.pane_id, notification.notification_type);
            }

            // Always unblock the pipe so the CLI command returns
            unblock_cli_pipe_input(&pipe_message.name);
            return enabled; // re-render only if notifications are enabled
        }

        // Not our message — unblock and ignore
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
    /// Handle a single event after permissions are granted.
    fn handle_event(&mut self, event: Event) -> bool {
        match event {
            Event::TabUpdate(tabs) => {
                self.tabs = tabs;
                // Clear notifications for the focused pane (focus-clears behavior)
                let cleared = self.notifications.clear_focused(&self.tabs, &self.panes);
                let cleaned = self.notifications.clean_stale(&self.panes);
                // Always re-render on tab update; cleared/cleaned just confirm side effects
                let _ = (cleared, cleaned);
                true
            }
            Event::PaneUpdate(panes) => {
                self.panes = panes;
                // Clear focused pane notifications + clean stale entries
                self.notifications.clear_focused(&self.tabs, &self.panes);
                self.notifications.clean_stale(&self.panes);
                true
            }
            Event::ModeUpdate(mode) => {
                self.mode = mode;
                true
            }
            Event::SessionUpdate(sessions, _) => {
                self.sessions = sessions;
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
