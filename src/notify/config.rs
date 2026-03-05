use std::collections::BTreeMap;

/// Configuration for notification appearance.
///
/// Parsed from the plugin's flat key-value config with keys:
/// - `notification_enabled` — "true" (default) or "false"
/// - `notification_icon_waiting` — icon string (default "⏳")
/// - `notification_icon_in_progress` — icon string (default "🔄")
/// - `notification_icon_completed` — icon string (default "✅")
/// - `notification_format_tab` — per-tab format fallback with `{icon}` (default `{icon}`)
/// - `notification_format_waiting` — per-tab waiting format (fallback: `notification_format_tab`)
/// - `notification_format_in_progress` — per-tab in-progress format (fallback: `notification_format_tab`)
/// - `notification_format_completed` — per-tab completed format (fallback: `notification_format_tab`)
#[derive(Debug, Clone)]
pub struct NotificationConfig {
    /// Whether the notification system is active.
    pub enabled: bool,
    /// Icon displayed when a pane is waiting for user input or approval.
    pub waiting_icon: String,
    /// Icon displayed when a pane has an operation actively running.
    pub in_progress_icon: String,
    /// Icon displayed when a pane's operation has completed.
    pub completed_icon: String,
    /// Per-tab notification format fallback with `{icon}` placeholder.
    pub tab_format: String,
    /// Per-tab waiting notification format with `{icon}` placeholder.
    pub waiting_format: String,
    /// Per-tab in-progress notification format with `{icon}` placeholder.
    pub in_progress_format: String,
    /// Per-tab completed notification format with `{icon}` placeholder.
    pub completed_format: String,
}

impl Default for NotificationConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            waiting_icon: "\u{23f3}".to_string(),
            in_progress_icon: "\u{1f504}".to_string(),
            completed_icon: "\u{2705}".to_string(),
            tab_format: "{icon}".to_string(),
            waiting_format: "{icon}".to_string(),
            in_progress_format: "{icon}".to_string(),
            completed_format: "{icon}".to_string(),
        }
    }
}

impl NotificationConfig {
    /// Parse notification config from the raw plugin configuration map.
    pub fn from_raw(raw: &BTreeMap<String, String>) -> Self {
        let mut config = Self::default();

        if let Some(enabled) = raw.get("notification_enabled") {
            config.enabled = enabled == "true";
        }

        if let Some(icon) = raw.get("notification_icon_waiting") {
            config.waiting_icon = icon.clone();
        }

        if let Some(icon) = raw.get("notification_icon_in_progress") {
            config.in_progress_icon = icon.clone();
        }

        if let Some(icon) = raw.get("notification_icon_completed") {
            config.completed_icon = icon.clone();
        }

        if let Some(format) = raw.get("notification_format_tab") {
            config.tab_format = format.clone();
        }

        config.waiting_format = raw
            .get("notification_format_waiting")
            .cloned()
            .unwrap_or_else(|| config.tab_format.clone());
        config.in_progress_format = raw
            .get("notification_format_in_progress")
            .cloned()
            .unwrap_or_else(|| config.tab_format.clone());
        config.completed_format = raw
            .get("notification_format_completed")
            .cloned()
            .unwrap_or_else(|| config.tab_format.clone());

        config
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults() {
        let config = NotificationConfig::default();
        assert!(config.enabled);
        assert_eq!(config.waiting_icon, "\u{23f3}");
        assert_eq!(config.in_progress_icon, "\u{1f504}");
        assert_eq!(config.completed_icon, "\u{2705}");
        assert_eq!(config.tab_format, "{icon}");
        assert_eq!(config.waiting_format, "{icon}");
        assert_eq!(config.in_progress_format, "{icon}");
        assert_eq!(config.completed_format, "{icon}");
    }

    #[test]
    fn from_raw_empty_uses_defaults() {
        let raw = BTreeMap::new();
        let config = NotificationConfig::from_raw(&raw);
        assert!(config.enabled);
        assert_eq!(config.waiting_icon, "\u{23f3}");
        assert_eq!(config.in_progress_icon, "\u{1f504}");
        assert_eq!(config.completed_icon, "\u{2705}");
        assert_eq!(config.tab_format, "{icon}");
        assert_eq!(config.waiting_format, "{icon}");
        assert_eq!(config.in_progress_format, "{icon}");
        assert_eq!(config.completed_format, "{icon}");
    }

    #[test]
    fn from_raw_custom_values() {
        let raw = BTreeMap::from([
            ("notification_enabled".to_string(), "true".to_string()),
            ("notification_icon_waiting".to_string(), "!".to_string()),
            ("notification_icon_in_progress".to_string(), "~".to_string()),
            ("notification_icon_completed".to_string(), "*".to_string()),
            (
                "notification_format_tab".to_string(),
                "#[fg=yellow]{icon}".to_string(),
            ),
            (
                "notification_format_waiting".to_string(),
                "#[fg=orange]{icon}".to_string(),
            ),
            (
                "notification_format_in_progress".to_string(),
                "#[fg=default]{icon}".to_string(),
            ),
            (
                "notification_format_completed".to_string(),
                "#[fg=green]{icon}".to_string(),
            ),
        ]);
        let config = NotificationConfig::from_raw(&raw);
        assert!(config.enabled);
        assert_eq!(config.waiting_icon, "!");
        assert_eq!(config.in_progress_icon, "~");
        assert_eq!(config.completed_icon, "*");
        assert_eq!(config.tab_format, "#[fg=yellow]{icon}");
        assert_eq!(config.waiting_format, "#[fg=orange]{icon}");
        assert_eq!(config.in_progress_format, "#[fg=default]{icon}");
        assert_eq!(config.completed_format, "#[fg=green]{icon}");
    }

    #[test]
    fn from_raw_ignores_legacy_icon_keys() {
        let raw = BTreeMap::from([
            ("notification_waiting_icon".to_string(), "W".to_string()),
            ("notification_busy_icon".to_string(), "B".to_string()),
            ("notification_in_progress_icon".to_string(), "P".to_string()),
            ("notification_completed_icon".to_string(), "C".to_string()),
        ]);
        let config = NotificationConfig::from_raw(&raw);
        assert_eq!(config.waiting_icon, "\u{23f3}");
        assert_eq!(config.in_progress_icon, "\u{1f504}");
        assert_eq!(config.completed_icon, "\u{2705}");
    }

    #[test]
    fn from_raw_disabled() {
        let raw = BTreeMap::from([("notification_enabled".to_string(), "false".to_string())]);
        let config = NotificationConfig::from_raw(&raw);
        assert!(!config.enabled);
    }

    #[test]
    fn format_fallbacks_use_tab_format_when_state_formats_missing() {
        let raw = BTreeMap::from([(
            "notification_format_tab".to_string(),
            "#[fg=peach]{icon}".to_string(),
        )]);
        let config = NotificationConfig::from_raw(&raw);
        assert_eq!(config.tab_format, "#[fg=peach]{icon}");
        assert_eq!(config.waiting_format, "#[fg=peach]{icon}");
        assert_eq!(config.in_progress_format, "#[fg=peach]{icon}");
        assert_eq!(config.completed_format, "#[fg=peach]{icon}");
    }

    #[test]
    fn state_format_overrides_tab_fallback() {
        let raw = BTreeMap::from([
            (
                "notification_format_tab".to_string(),
                "#[fg=yellow]{icon}".to_string(),
            ),
            (
                "notification_format_completed".to_string(),
                "#[fg=green,bold]{icon}".to_string(),
            ),
        ]);
        let config = NotificationConfig::from_raw(&raw);
        assert_eq!(config.waiting_format, "#[fg=yellow]{icon}");
        assert_eq!(config.in_progress_format, "#[fg=yellow]{icon}");
        assert_eq!(config.completed_format, "#[fg=green,bold]{icon}");
    }

    #[test]
    fn state_format_overrides_are_independent() {
        let raw = BTreeMap::from([
            (
                "notification_format_tab".to_string(),
                "#[fg=yellow]{icon}".to_string(),
            ),
            (
                "notification_format_waiting".to_string(),
                "#[fg=orange]{icon}".to_string(),
            ),
            (
                "notification_format_in_progress".to_string(),
                "#[fg=blue]{icon}".to_string(),
            ),
        ]);
        let config = NotificationConfig::from_raw(&raw);
        assert_eq!(config.waiting_format, "#[fg=orange]{icon}");
        assert_eq!(config.in_progress_format, "#[fg=blue]{icon}");
        assert_eq!(config.completed_format, "#[fg=yellow]{icon}");
    }
}
