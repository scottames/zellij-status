use std::collections::BTreeMap;

use zellij_tile::prelude::*;

use crate::config::PluginConfig;

/// Main plugin state implementing ZellijPlugin.
pub struct State {
    /// Whether permissions have been granted by Zellij.
    permissions_granted: bool,

    /// Events received before permissions were granted.
    pending_events: Vec<Event>,

    /// Parsed plugin configuration.
    config: Option<PluginConfig>,

    /// Raw configuration from Zellij layout.
    raw_config: BTreeMap<String, String>,

    /// Current tab information.
    tabs: Vec<TabInfo>,

    /// Current pane manifest.
    panes: PaneManifest,

    /// Current Zellij mode.
    mode: ModeInfo,

    /// Current session info.
    sessions: Vec<SessionInfo>,
}

impl Default for State {
    fn default() -> Self {
        Self {
            permissions_granted: false,
            pending_events: Vec::new(),
            config: None,
            raw_config: BTreeMap::new(),
            tabs: Vec::new(),
            panes: PaneManifest::default(),
            mode: ModeInfo::default(),
            sessions: Vec::new(),
        }
    }
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

        match PluginConfig::from_configuration(&configuration) {
            Ok(config) => {
                self.config = Some(config);
                self.raw_config = configuration;
            }
            Err(e) => {
                eprintln!("zellij-status: config error: {e}");
                self.raw_config = configuration;
            }
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

    fn render(&mut self, _rows: usize, _cols: usize) {
        if !self.permissions_granted {
            return;
        }

        if self.config.is_none() {
            print!("zellij-status: no valid config");
            return;
        }

        // TODO: delegate to vertical or horizontal renderer
        print!("zellij-status loaded");
    }
}

impl State {
    /// Handle a single event after permissions are granted.
    fn handle_event(&mut self, event: Event) -> bool {
        match event {
            Event::TabUpdate(tabs) => {
                self.tabs = tabs;
                true
            }
            Event::PaneUpdate(panes) => {
                self.panes = panes;
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
            Event::Mouse(_) => {
                // TODO: handle click/scroll
                false
            }
            _ => false,
        }
    }
}
