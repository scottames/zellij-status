use std::collections::BTreeMap;

use zellij_tile::prelude::InputMode;

use crate::render::format::parse_format_string;

use super::{PluginState, Widget};

/// Displays the current Zellij input mode with per-mode styling.
///
/// Config keys: `mode_normal`, `mode_locked`, `mode_pane`, `mode_tab`, etc.
/// Each value is a format string (e.g., `"#[fg=red,bold]LOCKED"`).
pub struct ModeWidget {
    /// Per-mode format strings keyed by lowercase mode name.
    formats: BTreeMap<String, String>,
}

impl ModeWidget {
    pub fn new(config: &BTreeMap<String, String>) -> Self {
        let formats = config
            .iter()
            .filter_map(|(k, v)| {
                k.strip_prefix("mode_")
                    .map(|name| (name.to_string(), v.clone()))
            })
            .collect();

        Self { formats }
    }

    /// Get the format string for a mode, falling back to uppercase mode name.
    fn format_for_mode(&self, mode: &InputMode) -> String {
        let key = mode_config_key(mode);
        self.formats
            .get(key)
            .cloned()
            .unwrap_or_else(|| default_mode_text(mode).to_string())
    }
}

impl Widget for ModeWidget {
    fn process(&self, _name: &str, state: &PluginState<'_>) -> String {
        let format_str = self.format_for_mode(&state.mode.mode);
        let aliases = &state.config.color_aliases;
        let parts = parse_format_string(&format_str, aliases);
        parts.iter().map(|p| p.render_content()).collect()
    }

    fn process_click(&self, _name: &str, _state: &PluginState<'_>, _col: usize) {
        // No click action for mode widget.
    }

    fn fill_part(
        &self,
        _name: &str,
        state: &PluginState<'_>,
    ) -> Option<crate::render::format::FormattedPart> {
        let format_str = self.format_for_mode(&state.mode.mode);
        let aliases = &state.config.color_aliases;
        parse_format_string(&format_str, aliases)
            .into_iter()
            .find(|part| part.fill)
    }
}

/// Map an `InputMode` to its config key suffix (e.g., `Normal` → `"normal"`).
fn mode_config_key(mode: &InputMode) -> &'static str {
    match mode {
        InputMode::Normal => "normal",
        InputMode::Locked => "locked",
        InputMode::Pane => "pane",
        InputMode::Tab => "tab",
        InputMode::Resize => "resize",
        InputMode::Move => "move",
        InputMode::Scroll => "scroll",
        InputMode::EnterSearch => "enter_search",
        InputMode::Search => "search",
        InputMode::Session => "session",
        InputMode::Tmux => "tmux",
        InputMode::Prompt => "prompt",
        InputMode::RenameTab => "rename_tab",
        InputMode::RenamePane => "rename_pane",
    }
}

/// Default display text when no format string is configured for a mode.
fn default_mode_text(mode: &InputMode) -> &'static str {
    match mode {
        InputMode::Normal => "NORMAL",
        InputMode::Locked => "LOCKED",
        InputMode::Pane => "PANE",
        InputMode::Tab => "TAB",
        InputMode::Resize => "RESIZE",
        InputMode::Move => "MOVE",
        InputMode::Scroll => "SCROLL",
        InputMode::EnterSearch => "ENTERSEARCH",
        InputMode::Search => "SEARCH",
        InputMode::Session => "SESSION",
        InputMode::Tmux => "TMUX",
        InputMode::Prompt => "PROMPT",
        InputMode::RenameTab => "RENAMETAB",
        InputMode::RenamePane => "RENAMEPANE",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mode_config_key_mapping() {
        assert_eq!(mode_config_key(&InputMode::Normal), "normal");
        assert_eq!(mode_config_key(&InputMode::Locked), "locked");
        assert_eq!(mode_config_key(&InputMode::RenameTab), "rename_tab");
    }

    #[test]
    fn default_text_for_unconfigured_mode() {
        let w = ModeWidget::new(&BTreeMap::new());
        assert_eq!(w.format_for_mode(&InputMode::Normal), "NORMAL");
        assert_eq!(w.format_for_mode(&InputMode::Locked), "LOCKED");
    }

    #[test]
    fn custom_format_for_mode() {
        let config = BTreeMap::from([
            ("mode_normal".to_string(), "NRM".to_string()),
            ("mode_locked".to_string(), "#[fg=red]LCK".to_string()),
        ]);
        let w = ModeWidget::new(&config);
        assert_eq!(w.format_for_mode(&InputMode::Normal), "NRM");
        assert_eq!(w.format_for_mode(&InputMode::Locked), "#[fg=red]LCK");
    }

    #[test]
    fn extracts_only_mode_prefixed_keys() {
        let config = BTreeMap::from([
            ("mode_normal".to_string(), "N".to_string()),
            ("tab_normal".to_string(), "T".to_string()),
            ("color_fg".to_string(), "#fff".to_string()),
        ]);
        let w = ModeWidget::new(&config);
        assert_eq!(w.formats.len(), 1);
        assert!(w.formats.contains_key("normal"));
    }
}
