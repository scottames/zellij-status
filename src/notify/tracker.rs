use std::collections::{HashMap, HashSet};

use zellij_tile::prelude::PaneManifest;

use super::NotificationType;

/// Tracks per-pane notification state and provides tab-level aggregation.
///
/// Notifications are stored as `pane_id → {notification_types}`. When
/// querying a tab, we aggregate across all non-plugin panes in that tab
/// with priority ordering: `Waiting` > `InProgress` > `Completed`.
#[derive(Debug, Default)]
pub struct NotificationTracker {
    /// Per-pane notification state: pane_id → set of active notification types.
    notifications: HashMap<u32, HashSet<NotificationType>>,
}

impl NotificationTracker {
    /// Add a notification for a pane.
    ///
    /// Replaces any existing notifications for this pane (latest event wins,
    /// matching zellij-attention's behavior).
    pub fn add(&mut self, pane_id: u32, notification_type: NotificationType) {
        let mut types = HashSet::new();
        types.insert(notification_type);
        self.notifications.insert(pane_id, types);
    }

    /// Clear all notifications for a specific pane.
    pub fn clear_pane(&mut self, pane_id: u32) {
        self.notifications.remove(&pane_id);
    }

    /// Returns true if there are no tracked notifications.
    pub fn is_empty(&self) -> bool {
        self.notifications.is_empty()
    }

    /// Returns the total number of panes with active notifications.
    pub fn total_count(&self) -> usize {
        self.notifications.len()
    }

    /// Get the aggregate notification state for a tab.
    ///
    /// Iterates all non-plugin panes in the tab and returns the highest-priority
    /// notification type found:
    /// - `Waiting` (highest priority — any pane waiting means tab shows waiting)
    /// - `InProgress`
    /// - `Completed`
    /// - `None` (no notifications)
    pub fn get_tab_notification(
        &self,
        tab_position: usize,
        panes: &PaneManifest,
    ) -> Option<NotificationType> {
        let tab_panes = panes.panes.get(&tab_position)?;
        let mut has_in_progress = false;
        let mut has_completed = false;

        for pane in tab_panes {
            if pane.is_plugin {
                continue;
            }
            if let Some(types) = self.notifications.get(&pane.id) {
                if types.contains(&NotificationType::Waiting) {
                    return Some(NotificationType::Waiting);
                }
                if types.contains(&NotificationType::InProgress) {
                    has_in_progress = true;
                }
                if types.contains(&NotificationType::Completed) {
                    has_completed = true;
                }
            }
        }

        if has_in_progress {
            Some(NotificationType::InProgress)
        } else if has_completed {
            Some(NotificationType::Completed)
        } else {
            None
        }
    }

    /// Find the focused pane in the active tab and clear its notifications.
    ///
    /// Returns `true` if any notification was cleared.
    pub fn clear_focused(
        &mut self,
        tabs: &[zellij_tile::prelude::TabInfo],
        panes: &PaneManifest,
    ) -> bool {
        let Some(active_tab) = tabs.iter().find(|t| t.active) else {
            return false;
        };
        let Some(tab_panes) = panes.panes.get(&active_tab.position) else {
            return false;
        };

        let focused = tab_panes.iter().find(|p| {
            !p.is_plugin && p.is_focused && (p.is_floating == active_tab.are_floating_panes_visible)
        });

        if let Some(pane) = focused
            && self.notifications.remove(&pane.id).is_some()
        {
            return true;
        }

        false
    }

    /// Remove notification entries for pane IDs that no longer exist.
    ///
    /// Returns `true` if any stale entries were removed.
    pub fn clean_stale(&mut self, panes: &PaneManifest) -> bool {
        if self.notifications.is_empty() || panes.panes.is_empty() {
            return false;
        }

        let current_pane_ids: HashSet<u32> = panes
            .panes
            .values()
            .flat_map(|tab_panes| tab_panes.iter().filter(|p| !p.is_plugin).map(|p| p.id))
            .collect();

        let stale_ids: Vec<u32> = self
            .notifications
            .keys()
            .filter(|id| !current_pane_ids.contains(id))
            .copied()
            .collect();

        if stale_ids.is_empty() {
            return false;
        }

        for id in &stale_ids {
            self.notifications.remove(id);
        }

        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use zellij_tile::prelude::{PaneInfo, PaneManifest};

    fn make_pane(id: u32, is_plugin: bool, is_focused: bool) -> PaneInfo {
        PaneInfo {
            id,
            is_plugin,
            is_focused,
            ..Default::default()
        }
    }

    fn make_manifest(entries: Vec<(usize, Vec<PaneInfo>)>) -> PaneManifest {
        PaneManifest {
            panes: entries.into_iter().collect(),
        }
    }

    // ---- add / clear ----

    #[test]
    fn add_and_check_not_empty() {
        let mut tracker = NotificationTracker::default();
        assert!(tracker.is_empty());
        tracker.add(1, NotificationType::Waiting);
        assert!(!tracker.is_empty());
    }

    #[test]
    fn clear_pane_removes_notification() {
        let mut tracker = NotificationTracker::default();
        tracker.add(1, NotificationType::Waiting);
        tracker.clear_pane(1);
        assert!(tracker.is_empty());
    }

    #[test]
    fn add_replaces_existing() {
        let mut tracker = NotificationTracker::default();
        tracker.add(1, NotificationType::Waiting);
        tracker.add(1, NotificationType::Completed);

        let panes = make_manifest(vec![(0, vec![make_pane(1, false, false)])]);
        // Should be Completed since add replaces
        assert_eq!(
            tracker.get_tab_notification(0, &panes),
            Some(NotificationType::Completed)
        );
    }

    // ---- tab aggregation ----

    #[test]
    fn tab_notification_none_when_empty() {
        let tracker = NotificationTracker::default();
        let panes = make_manifest(vec![(0, vec![make_pane(1, false, false)])]);
        assert_eq!(tracker.get_tab_notification(0, &panes), None);
    }

    #[test]
    fn tab_notification_waiting_priority() {
        let mut tracker = NotificationTracker::default();
        tracker.add(1, NotificationType::InProgress);
        tracker.add(2, NotificationType::Waiting);

        let panes = make_manifest(vec![(
            0,
            vec![make_pane(1, false, false), make_pane(2, false, false)],
        )]);

        // Waiting takes priority over InProgress
        assert_eq!(
            tracker.get_tab_notification(0, &panes),
            Some(NotificationType::Waiting)
        );
    }

    #[test]
    fn tab_notification_in_progress_over_completed() {
        let mut tracker = NotificationTracker::default();
        tracker.add(1, NotificationType::Completed);
        tracker.add(2, NotificationType::InProgress);

        let panes = make_manifest(vec![(
            0,
            vec![make_pane(1, false, false), make_pane(2, false, false)],
        )]);

        assert_eq!(
            tracker.get_tab_notification(0, &panes),
            Some(NotificationType::InProgress)
        );
    }

    #[test]
    fn tab_notification_skips_plugin_panes() {
        let mut tracker = NotificationTracker::default();
        tracker.add(1, NotificationType::Waiting);

        // Pane 1 is a plugin pane — should be skipped
        let panes = make_manifest(vec![(0, vec![make_pane(1, true, false)])]);
        assert_eq!(tracker.get_tab_notification(0, &panes), None);
    }

    #[test]
    fn tab_notification_nonexistent_tab() {
        let tracker = NotificationTracker::default();
        let panes = make_manifest(vec![]);
        assert_eq!(tracker.get_tab_notification(5, &panes), None);
    }

    // ---- focus clear ----

    #[test]
    fn clear_focused_removes_notification() {
        let mut tracker = NotificationTracker::default();
        tracker.add(10, NotificationType::Waiting);

        let tabs = vec![zellij_tile::prelude::TabInfo {
            position: 0,
            active: true,
            ..Default::default()
        }];
        let panes = make_manifest(vec![(0, vec![make_pane(10, false, true)])]);

        assert!(tracker.clear_focused(&tabs, &panes));
        assert!(tracker.is_empty());
    }

    #[test]
    fn clear_focused_ignores_plugin_panes() {
        let mut tracker = NotificationTracker::default();
        tracker.add(10, NotificationType::Waiting);

        let tabs = vec![zellij_tile::prelude::TabInfo {
            position: 0,
            active: true,
            ..Default::default()
        }];
        // Pane 10 is a plugin — should not be cleared
        let panes = make_manifest(vec![(0, vec![make_pane(10, true, true)])]);

        assert!(!tracker.clear_focused(&tabs, &panes));
        assert!(!tracker.is_empty());
    }

    #[test]
    fn clear_focused_no_active_tab() {
        let mut tracker = NotificationTracker::default();
        tracker.add(10, NotificationType::Waiting);

        let tabs = vec![zellij_tile::prelude::TabInfo {
            position: 0,
            active: false,
            ..Default::default()
        }];
        let panes = make_manifest(vec![(0, vec![make_pane(10, false, true)])]);

        assert!(!tracker.clear_focused(&tabs, &panes));
    }

    // ---- stale cleanup ----

    #[test]
    fn clean_stale_removes_gone_panes() {
        let mut tracker = NotificationTracker::default();
        tracker.add(1, NotificationType::Waiting);
        tracker.add(2, NotificationType::Completed);

        // Only pane 1 exists now
        let panes = make_manifest(vec![(0, vec![make_pane(1, false, false)])]);
        assert!(tracker.clean_stale(&panes));

        // Pane 2 should be gone
        assert!(!tracker.is_empty()); // pane 1 still exists
        let panes_with_2 = make_manifest(vec![(0, vec![make_pane(2, false, false)])]);
        assert_eq!(tracker.get_tab_notification(0, &panes_with_2), None);
    }

    #[test]
    fn clean_stale_noop_when_empty() {
        let mut tracker = NotificationTracker::default();
        let panes = make_manifest(vec![(0, vec![make_pane(1, false, false)])]);
        assert!(!tracker.clean_stale(&panes));
    }

    #[test]
    fn clean_stale_noop_when_all_exist() {
        let mut tracker = NotificationTracker::default();
        tracker.add(1, NotificationType::Waiting);

        let panes = make_manifest(vec![(0, vec![make_pane(1, false, false)])]);
        assert!(!tracker.clean_stale(&panes));
    }
}
