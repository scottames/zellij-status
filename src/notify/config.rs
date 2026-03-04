use std::collections::BTreeMap;

/// Configuration for notification appearance.
///
/// Parsed from the plugin's flat key-value config with keys:
/// - `notification_enabled` — "true" (default) or "false"
/// - `notification_waiting_icon` — icon string (default "⏳")
/// - `notification_in_progress_icon` — icon string (default "🔄")
/// - `notification_busy_icon` — alias of `notification_in_progress_icon`
/// - `notification_completed_icon` — icon string (default "✅")
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
}

impl Default for NotificationConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            waiting_icon: "\u{23f3}".to_string(),
            in_progress_icon: "\u{1f504}".to_string(),
            completed_icon: "\u{2705}".to_string(),
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

        if let Some(icon) = raw.get("notification_waiting_icon") {
            config.waiting_icon = icon.clone();
        }

        if let Some(icon) = raw.get("notification_busy_icon") {
            config.in_progress_icon = icon.clone();
        }

        if let Some(icon) = raw.get("notification_in_progress_icon") {
            config.in_progress_icon = icon.clone();
        }

        if let Some(icon) = raw.get("notification_completed_icon") {
            config.completed_icon = icon.clone();
        }

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
    }

    #[test]
    fn from_raw_empty_uses_defaults() {
        let raw = BTreeMap::new();
        let config = NotificationConfig::from_raw(&raw);
        assert!(config.enabled);
        assert_eq!(config.waiting_icon, "\u{23f3}");
        assert_eq!(config.in_progress_icon, "\u{1f504}");
        assert_eq!(config.completed_icon, "\u{2705}");
    }

    #[test]
    fn from_raw_custom_values() {
        let raw = BTreeMap::from([
            ("notification_enabled".to_string(), "true".to_string()),
            ("notification_waiting_icon".to_string(), "!".to_string()),
            ("notification_in_progress_icon".to_string(), "~".to_string()),
            ("notification_completed_icon".to_string(), "*".to_string()),
        ]);
        let config = NotificationConfig::from_raw(&raw);
        assert!(config.enabled);
        assert_eq!(config.waiting_icon, "!");
        assert_eq!(config.in_progress_icon, "~");
        assert_eq!(config.completed_icon, "*");
    }

    #[test]
    fn from_raw_busy_alias() {
        let raw = BTreeMap::from([("notification_busy_icon".to_string(), "...".to_string())]);
        let config = NotificationConfig::from_raw(&raw);
        assert_eq!(config.in_progress_icon, "...");
    }

    #[test]
    fn from_raw_in_progress_takes_precedence_over_busy_alias() {
        let raw = BTreeMap::from([
            ("notification_busy_icon".to_string(), "...".to_string()),
            ("notification_in_progress_icon".to_string(), "~".to_string()),
        ]);
        let config = NotificationConfig::from_raw(&raw);
        assert_eq!(config.in_progress_icon, "~");
    }

    #[test]
    fn from_raw_disabled() {
        let raw = BTreeMap::from([("notification_enabled".to_string(), "false".to_string())]);
        let config = NotificationConfig::from_raw(&raw);
        assert!(!config.enabled);
    }
}
