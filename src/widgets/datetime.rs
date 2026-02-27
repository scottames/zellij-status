use std::collections::BTreeMap;

use super::{PluginState, Widget};

/// Displays the current date/time with a configurable format and timezone.
///
/// Config keys:
/// - `datetime_format` — strftime format string (default: `"%Y-%m-%d %H:%M"`)
/// - `datetime_timezone` — IANA timezone name (default: UTC, since WASI lacks
///   local timezone info)
pub struct DateTimeWidget {
    /// strftime format string.
    format: String,
    /// Parsed timezone (None = UTC).
    timezone: Option<chrono_tz::Tz>,
}

impl DateTimeWidget {
    pub fn new(config: &BTreeMap<String, String>) -> Self {
        let format = config
            .get("datetime_format")
            .cloned()
            .unwrap_or_else(|| "%Y-%m-%d %H:%M".to_string());

        let timezone = config
            .get("datetime_timezone")
            .and_then(|tz_str| tz_str.parse::<chrono_tz::Tz>().ok());

        Self { format, timezone }
    }
}

impl Widget for DateTimeWidget {
    fn process(&self, _name: &str, _state: &PluginState<'_>) -> String {
        let now = chrono::Utc::now();
        match self.timezone {
            Some(tz) => {
                let local = now.with_timezone(&tz);
                local.format(&self.format).to_string()
            }
            None => now.format(&self.format).to_string(),
        }
    }

    fn process_click(&self, _name: &str, _state: &PluginState<'_>, _col: usize) {
        // No click action for datetime widget.
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_format() {
        let w = DateTimeWidget::new(&BTreeMap::new());
        let output = w.format;
        assert_eq!(output, "%Y-%m-%d %H:%M");
    }

    #[test]
    fn custom_format() {
        let config = BTreeMap::from([("datetime_format".to_string(), "%H:%M:%S".to_string())]);
        let w = DateTimeWidget::new(&config);
        assert_eq!(w.format, "%H:%M:%S");
    }

    #[test]
    fn valid_timezone_parses() {
        let config = BTreeMap::from([(
            "datetime_timezone".to_string(),
            "America/Los_Angeles".to_string(),
        )]);
        let w = DateTimeWidget::new(&config);
        assert!(w.timezone.is_some());
    }

    #[test]
    fn invalid_timezone_falls_back_to_none() {
        let config = BTreeMap::from([(
            "datetime_timezone".to_string(),
            "Not/A/Timezone".to_string(),
        )]);
        let w = DateTimeWidget::new(&config);
        assert!(w.timezone.is_none());
    }

    #[test]
    fn produces_nonempty_output() {
        let config = BTreeMap::from([("datetime_format".to_string(), "%Y".to_string())]);
        let w = DateTimeWidget::new(&config);
        // We can't assert the exact value (it depends on the current year),
        // but it should be a 4-digit string.
        let output = {
            // Build a minimal PluginState — DateTimeWidget doesn't use it
            let tabs = vec![];
            let mode = zellij_tile::prelude::ModeInfo::default();
            let panes = zellij_tile::prelude::PaneManifest::default();
            let config_parsed =
                crate::config::PluginConfig::from_configuration(std::collections::BTreeMap::new())
                    .unwrap();
            let notifications = crate::notify::tracker::NotificationTracker::default();
            let state = crate::widgets::PluginState {
                tabs: &tabs,
                panes: &panes,
                mode: &mode,
                config: &config_parsed,
                notifications: &notifications,
            };
            w.process("datetime", &state)
        };
        assert_eq!(output.len(), 4);
        assert!(output.chars().all(|c| c.is_ascii_digit()));
    }
}
