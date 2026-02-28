use std::collections::BTreeMap;
use std::sync::Arc;

use anstyle::Effects;

use crate::render::color::parse_color;
use crate::render::format::FormattedPart;

use super::{PluginState, Widget};

pub struct CapWidget {
    source_widgets: BTreeMap<String, Arc<dyn Widget>>,
}

impl CapWidget {
    pub fn new(source_widgets: BTreeMap<String, Arc<dyn Widget>>) -> Self {
        Self { source_widgets }
    }

    pub fn names(&self) -> Vec<String> {
        self.source_widgets
            .keys()
            .map(|name| format!("{name}_cap"))
            .collect()
    }

    fn source_name<'a>(&self, name: &'a str) -> Option<&'a str> {
        name.strip_suffix("_cap")
    }

    fn cfg<'a>(raw: &'a BTreeMap<String, String>, source: &str, key: &str) -> Option<&'a str> {
        raw.get(&format!("{source}_cap_{key}"))
            .or_else(|| raw.get(&format!("cap_{key}")))
            .map(String::as_str)
    }
}

impl Widget for CapWidget {
    fn process(&self, name: &str, state: &PluginState<'_>) -> String {
        let Some(source) = self.source_name(name) else {
            return String::new();
        };
        let Some(widget) = self.source_widgets.get(source) else {
            return String::new();
        };

        let fill = widget.fill_part(source, state);
        let cfg = &state.config.raw;
        let aliases = &state.config.color_aliases;

        let symbol = Self::cfg(cfg, source, "symbol").unwrap_or("");
        let bg = Self::cfg(cfg, source, "bg").and_then(|value| parse_color(value, aliases));
        let fg = Self::cfg(cfg, source, "fg")
            .and_then(|value| parse_color(value, aliases))
            .or_else(|| fill.as_ref().and_then(|part| part.bg))
            .or_else(|| fill.as_ref().and_then(|part| part.fg));

        if fg.is_none() && bg.is_none() {
            return String::new();
        }

        FormattedPart {
            fg,
            bg,
            effects: Effects::new(),
            fill: false,
            content: symbol.to_string(),
        }
        .render_content()
    }

    fn process_click(&self, _name: &str, _state: &PluginState<'_>, _col: usize) {}
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::PluginConfig;
    use crate::notify::tracker::NotificationTracker;
    use zellij_tile::prelude::{ModeInfo, PaneManifest};

    struct FillStub;

    impl Widget for FillStub {
        fn process(&self, _name: &str, _state: &PluginState<'_>) -> String {
            String::new()
        }

        fn process_click(&self, _name: &str, _state: &PluginState<'_>, _col: usize) {}

        fn fill_part(&self, _name: &str, _state: &PluginState<'_>) -> Option<FormattedPart> {
            Some(FormattedPart {
                fg: None,
                bg: parse_color("blue", &BTreeMap::new()),
                effects: Effects::new(),
                fill: true,
                content: String::new(),
            })
        }
    }

    fn state(
        raw: BTreeMap<String, String>,
    ) -> (
        PluginConfig,
        ModeInfo,
        PaneManifest,
        NotificationTracker,
        BTreeMap<String, crate::widgets::command::CommandResult>,
        BTreeMap<String, String>,
        Vec<zellij_tile::prelude::TabInfo>,
    ) {
        (
            PluginConfig::from_configuration(raw).unwrap(),
            ModeInfo::default(),
            PaneManifest::default(),
            NotificationTracker::default(),
            BTreeMap::new(),
            BTreeMap::new(),
            Vec::new(),
        )
    }

    #[test]
    fn cap_uses_source_fill_as_foreground() {
        let mut sources: BTreeMap<String, Arc<dyn Widget>> = BTreeMap::new();
        sources.insert("mode".to_string(), Arc::new(FillStub));
        let cap = CapWidget::new(sources);

        let (config, mode, panes, notifications, command_results, pipe_data, tabs) = state(
            BTreeMap::from([("cap_bg".to_string(), "black".to_string())]),
        );
        let state = PluginState {
            tabs: &tabs,
            panes: &panes,
            mode: &mode,
            config: &config,
            notifications: &notifications,
            command_results: &command_results,
            pipe_data: &pipe_data,
        };

        let rendered = cap.process("mode_cap", &state);
        assert!(rendered.contains(""));
        assert!(rendered.contains('\x1b'));
    }
}
