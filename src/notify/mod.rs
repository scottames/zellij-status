pub mod config;
pub mod protocol;
pub mod tracker;

/// Types of notifications a pane can have.
///
/// Priority ordering: `Waiting` > `Completed`. When aggregating across
/// panes in a tab, if any pane has `Waiting`, the tab shows the waiting
/// icon regardless of other panes' states.
#[derive(Debug, Clone, Copy, Hash, Eq, PartialEq)]
pub enum NotificationType {
    /// An operation is still in progress.
    Waiting,
    /// An operation has finished.
    Completed,
}
