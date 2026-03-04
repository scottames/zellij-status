use super::NotificationType;

/// Result of parsing a pipe message as a notification.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PipeNotification {
    /// The type of notification (waiting, in_progress/busy, or completed).
    pub notification_type: NotificationType,
    /// The pane ID that the notification applies to.
    pub pane_id: u32,
}

/// Result of parsing a pipe message as widget data.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PipeData {
    /// Widget key, including the `pipe_` prefix (e.g., `"pipe_my_key"`).
    pub key: String,
    /// The data value to display.
    pub value: String,
}

/// Parse a pipe message name into a notification.
///
/// Expected format: `zellij-status::EVENT::PANE_ID`
/// where EVENT is `waiting`, `in_progress`/`busy`, or `completed` and PANE_ID is a u32.
///
/// Also checks the payload for the same format (broadcast pipes may
/// put the message in payload instead of name).
///
/// Returns `None` if the message doesn't match our protocol.
pub fn parse_pipe_message(name: &str, payload: Option<&str>) -> Option<PipeNotification> {
    // Try name first, then payload
    parse_notification_str(name).or_else(|| payload.and_then(parse_notification_str))
}

/// Parse a pipe message for widget data.
///
/// Expected formats:
/// - `zellij-status::pipe::KEY::VALUE` — key and value in name
/// - `zellij-status::pipe::KEY` with payload as value
///
/// Also checks the payload for the same format (broadcast pipes may
/// put the message in payload instead of name).
///
/// The returned key includes the `pipe_` prefix to match widget names
/// (e.g., sending `zellij-status::pipe::status::ok` yields key `"pipe_status"`).
pub fn parse_pipe_data(name: &str, payload: Option<&str>) -> Option<PipeData> {
    parse_pipe_data_str(name, payload)
        .or_else(|| payload.and_then(|p| parse_pipe_data_str(p, None)))
}

/// Parse a single string in `zellij-status::pipe::KEY[::VALUE]` format.
fn parse_pipe_data_str(s: &str, payload: Option<&str>) -> Option<PipeData> {
    // Use splitn(4, ...) so the value can contain "::"
    let parts: Vec<&str> = s.splitn(4, "::").collect();

    if parts.len() < 3 || parts[0] != "zellij-status" || parts[1] != "pipe" {
        return None;
    }

    let key = format!("pipe_{}", parts[2]);

    let value = if parts.len() >= 4 {
        parts[3].to_string()
    } else {
        // Fall back to payload if value is not in the name
        payload.unwrap_or("").to_string()
    };

    Some(PipeData { key, value })
}

/// Parse a single string in `zellij-status::EVENT::PANE_ID` format.
fn parse_notification_str(s: &str) -> Option<PipeNotification> {
    let parts: Vec<&str> = s.split("::").collect();
    if parts.len() < 3 || parts[0] != "zellij-status" {
        return None;
    }

    let notification_type = match parts[1].to_lowercase().as_str() {
        "waiting" => NotificationType::Waiting,
        "in_progress" | "busy" => NotificationType::InProgress,
        "completed" => NotificationType::Completed,
        _ => return None,
    };

    let pane_id: u32 = parts[2].parse().ok()?;

    Some(PipeNotification {
        notification_type,
        pane_id,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_waiting_from_name() {
        let result = parse_pipe_message("zellij-status::waiting::42", None);
        assert_eq!(
            result,
            Some(PipeNotification {
                notification_type: NotificationType::Waiting,
                pane_id: 42,
            })
        );
    }

    #[test]
    fn parse_completed_from_name() {
        let result = parse_pipe_message("zellij-status::completed::7", None);
        assert_eq!(
            result,
            Some(PipeNotification {
                notification_type: NotificationType::Completed,
                pane_id: 7,
            })
        );
    }

    #[test]
    fn parse_in_progress_from_name() {
        let result = parse_pipe_message("zellij-status::in_progress::8", None);
        assert_eq!(
            result,
            Some(PipeNotification {
                notification_type: NotificationType::InProgress,
                pane_id: 8,
            })
        );
    }

    #[test]
    fn parse_busy_alias_from_name() {
        let result = parse_pipe_message("zellij-status::busy::9", None);
        assert_eq!(
            result,
            Some(PipeNotification {
                notification_type: NotificationType::InProgress,
                pane_id: 9,
            })
        );
    }

    #[test]
    fn parse_from_payload_fallback() {
        let result = parse_pipe_message("some-other-name", Some("zellij-status::waiting::99"));
        assert_eq!(
            result,
            Some(PipeNotification {
                notification_type: NotificationType::Waiting,
                pane_id: 99,
            })
        );
    }

    #[test]
    fn parse_in_progress_from_payload_fallback() {
        let result = parse_pipe_message("some-other-name", Some("zellij-status::busy::99"));
        assert_eq!(
            result,
            Some(PipeNotification {
                notification_type: NotificationType::InProgress,
                pane_id: 99,
            })
        );
    }

    #[test]
    fn name_takes_priority_over_payload() {
        let result = parse_pipe_message(
            "zellij-status::waiting::1",
            Some("zellij-status::completed::2"),
        );
        assert_eq!(
            result,
            Some(PipeNotification {
                notification_type: NotificationType::Waiting,
                pane_id: 1,
            })
        );
    }

    #[test]
    fn ignores_wrong_prefix() {
        assert_eq!(
            parse_pipe_message("zellij-attention::waiting::42", None),
            None
        );
    }

    #[test]
    fn ignores_unknown_event() {
        assert_eq!(parse_pipe_message("zellij-status::error::42", None), None);
    }

    #[test]
    fn ignores_invalid_pane_id() {
        assert_eq!(
            parse_pipe_message("zellij-status::waiting::abc", None),
            None
        );
    }

    #[test]
    fn ignores_too_few_parts() {
        assert_eq!(parse_pipe_message("zellij-status::waiting", None), None);
    }

    #[test]
    fn ignores_empty_string() {
        assert_eq!(parse_pipe_message("", None), None);
    }

    #[test]
    fn case_insensitive_event_type() {
        let result = parse_pipe_message("zellij-status::WAITING::42", None);
        assert_eq!(
            result,
            Some(PipeNotification {
                notification_type: NotificationType::Waiting,
                pane_id: 42,
            })
        );
    }

    #[test]
    fn case_insensitive_busy_alias() {
        let result = parse_pipe_message("zellij-status::BUSY::42", None);
        assert_eq!(
            result,
            Some(PipeNotification {
                notification_type: NotificationType::InProgress,
                pane_id: 42,
            })
        );
    }

    // -- parse_pipe_data tests --

    #[test]
    fn parse_pipe_data_from_name() {
        let result = parse_pipe_data("zellij-status::pipe::status::running", None);
        assert_eq!(
            result,
            Some(PipeData {
                key: "pipe_status".to_string(),
                value: "running".to_string(),
            })
        );
    }

    #[test]
    fn parse_pipe_data_value_with_colons() {
        let result = parse_pipe_data("zellij-status::pipe::msg::hello::world", None);
        assert_eq!(
            result,
            Some(PipeData {
                key: "pipe_msg".to_string(),
                value: "hello::world".to_string(),
            })
        );
    }

    #[test]
    fn parse_pipe_data_from_payload() {
        let result = parse_pipe_data("zellij-status::pipe::key", Some("my value"));
        assert_eq!(
            result,
            Some(PipeData {
                key: "pipe_key".to_string(),
                value: "my value".to_string(),
            })
        );
    }

    #[test]
    fn parse_pipe_data_name_value_over_payload() {
        let result = parse_pipe_data("zellij-status::pipe::key::from_name", Some("from_payload"));
        assert_eq!(
            result,
            Some(PipeData {
                key: "pipe_key".to_string(),
                value: "from_name".to_string(),
            })
        );
    }

    #[test]
    fn parse_pipe_data_payload_fallback_format() {
        let result = parse_pipe_data("some-other-name", Some("zellij-status::pipe::key::value"));
        assert_eq!(
            result,
            Some(PipeData {
                key: "pipe_key".to_string(),
                value: "value".to_string(),
            })
        );
    }

    #[test]
    fn parse_pipe_data_ignores_wrong_prefix() {
        assert_eq!(
            parse_pipe_data("other-plugin::pipe::key::value", None),
            None
        );
    }

    #[test]
    fn parse_pipe_data_ignores_notification_format() {
        // Notifications use "waiting"/"in_progress"/"busy"/"completed", not "pipe"
        assert_eq!(parse_pipe_data("zellij-status::waiting::42", None), None);
    }

    #[test]
    fn parse_pipe_data_empty_value_from_payload() {
        let result = parse_pipe_data("zellij-status::pipe::key", None);
        assert_eq!(
            result,
            Some(PipeData {
                key: "pipe_key".to_string(),
                value: String::new(),
            })
        );
    }
}
