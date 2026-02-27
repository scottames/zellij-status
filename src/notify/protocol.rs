use super::NotificationType;

/// Result of parsing a pipe message.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PipeNotification {
    /// The type of notification (waiting or completed).
    pub notification_type: NotificationType,
    /// The pane ID that the notification applies to.
    pub pane_id: u32,
}

/// Parse a pipe message name into a notification.
///
/// Expected format: `zellij-status::EVENT::PANE_ID`
/// where EVENT is `waiting` or `completed` and PANE_ID is a u32.
///
/// Also checks the payload for the same format (broadcast pipes may
/// put the message in payload instead of name).
///
/// Returns `None` if the message doesn't match our protocol.
pub fn parse_pipe_message(name: &str, payload: Option<&str>) -> Option<PipeNotification> {
    // Try name first, then payload
    parse_notification_str(name).or_else(|| payload.and_then(parse_notification_str))
}

/// Parse a single string in `zellij-status::EVENT::PANE_ID` format.
fn parse_notification_str(s: &str) -> Option<PipeNotification> {
    let parts: Vec<&str> = s.split("::").collect();
    if parts.len() < 3 || parts[0] != "zellij-status" {
        return None;
    }

    let notification_type = match parts[1].to_lowercase().as_str() {
        "waiting" => NotificationType::Waiting,
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
}
