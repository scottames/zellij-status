pub mod config;
pub mod protocol;
pub mod tracker;

/// Types of notifications a pane can have.
///
/// Priority ordering: `Waiting` > `InProgress` > `Completed`.
/// When aggregating across panes in a tab, higher-priority states win.
#[derive(Debug, Clone, Copy, Hash, Eq, PartialEq)]
pub enum NotificationType {
    /// An operation is blocked and waiting for user input or approval.
    Waiting,
    /// An operation is actively running.
    InProgress,
    /// An operation has finished.
    Completed,
}
