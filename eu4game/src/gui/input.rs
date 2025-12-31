//! Input handling system for GUI widgets.
//!
//! Provides hit testing and focus management for interactive UI elements.

use crate::gui::core::{UiEvent, WidgetId};
use crate::gui::types::Rect;

/// Manages keyboard focus for UI widgets.
///
/// Tracks which widget currently has focus and handles focus transitions,
/// ensuring proper FocusGained/FocusLost events are dispatched.
#[derive(Debug, Clone)]
#[allow(dead_code)] // Reserved for Phase 6+ panel integration
pub struct FocusManager {
    /// Currently focused widget, if any.
    focused: Option<WidgetId>,
}

impl FocusManager {
    /// Create a new focus manager with no focused widget.
    #[allow(dead_code)] // Reserved for Phase 6+ panel integration
    pub fn new() -> Self {
        Self { focused: None }
    }

    /// Get the currently focused widget.
    #[allow(dead_code)] // Reserved for Phase 6+ panel integration
    pub fn focused(&self) -> Option<WidgetId> {
        self.focused
    }

    /// Set focus to a specific widget.
    ///
    /// Returns a tuple of (previous_focus, new_focus) to help generate events.
    #[allow(dead_code)] // Reserved for Phase 6+ panel integration
    pub fn focus(&mut self, id: WidgetId) -> (Option<WidgetId>, WidgetId) {
        let previous = self.focused;
        self.focused = Some(id);
        (previous, id)
    }

    /// Clear focus from the current widget.
    ///
    /// Returns the previously focused widget, if any.
    #[allow(dead_code)] // Reserved for Phase 6+ panel integration
    pub fn clear_focus(&mut self) -> Option<WidgetId> {
        self.focused.take()
    }

    /// Check if a specific widget has focus.
    #[allow(dead_code)] // Reserved for Phase 6+ panel integration
    pub fn has_focus(&self, id: WidgetId) -> bool {
        self.focused == Some(id)
    }

    /// Handle widget removal - clear focus if the removed widget had it.
    ///
    /// Returns true if focus was cleared.
    #[allow(dead_code)] // Reserved for Phase 6+ panel integration
    pub fn handle_widget_removed(&mut self, id: WidgetId) -> bool {
        if self.has_focus(id) {
            self.clear_focus();
            true
        } else {
            false
        }
    }
}

impl Default for FocusManager {
    fn default() -> Self {
        Self::new()
    }
}

/// Widget entry for hit testing.
///
/// Associates a widget ID with its bounding box for efficient hit testing.
#[derive(Debug, Clone)]
#[allow(dead_code)] // Reserved for Phase 6+ panel integration
pub struct HitTestEntry {
    pub id: WidgetId,
    pub bounds: Rect,
}

/// Hit test a point against a list of widgets.
///
/// Returns the ID of the topmost widget containing the point,
/// or None if no widget was hit.
///
/// Widgets should be provided in reverse render order (topmost first)
/// for correct layering behavior.
#[allow(dead_code)] // Reserved for Phase 6+ panel integration
pub fn hit_test(x: f32, y: f32, widgets: &[HitTestEntry]) -> Option<WidgetId> {
    for entry in widgets {
        if entry.bounds.contains(x, y) {
            return Some(entry.id);
        }
    }
    None
}

/// Generate focus transition events.
///
/// Given previous and new focus states, returns the events that should
/// be dispatched to affected widgets.
#[allow(dead_code)] // Reserved for Phase 6+ panel integration
pub fn generate_focus_events(
    previous: Option<WidgetId>,
    new: Option<WidgetId>,
) -> Vec<(WidgetId, UiEvent)> {
    let mut events = Vec::new();

    // Send FocusLost to previous widget if different from new
    if let Some(prev_id) = previous
        && Some(prev_id) != new
    {
        events.push((prev_id, UiEvent::FocusLost));
    }

    // Send FocusGained to new widget if different from previous
    if let Some(new_id) = new
        && Some(new_id) != previous
    {
        events.push((new_id, UiEvent::FocusGained));
    }

    events
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_focus_manager_default() {
        let manager = FocusManager::new();
        assert!(manager.focused().is_none());
    }

    #[test]
    fn test_focus_manager_set_focus() {
        let mut manager = FocusManager::new();
        let widget_id = WidgetId::new(0, 1);

        let (prev, new) = manager.focus(widget_id);
        assert!(prev.is_none());
        assert_eq!(new, widget_id);
        assert_eq!(manager.focused(), Some(widget_id));
        assert!(manager.has_focus(widget_id));
    }

    #[test]
    fn test_focus_manager_clear_focus() {
        let mut manager = FocusManager::new();
        let widget_id = WidgetId::new(0, 1);

        manager.focus(widget_id);
        let prev = manager.clear_focus();
        assert_eq!(prev, Some(widget_id));
        assert!(manager.focused().is_none());
    }

    #[test]
    fn test_focus_manager_change_focus() {
        let mut manager = FocusManager::new();
        let widget1 = WidgetId::new(0, 1);
        let widget2 = WidgetId::new(1, 1);

        manager.focus(widget1);
        let (prev, new) = manager.focus(widget2);
        assert_eq!(prev, Some(widget1));
        assert_eq!(new, widget2);
        assert_eq!(manager.focused(), Some(widget2));
        assert!(!manager.has_focus(widget1));
        assert!(manager.has_focus(widget2));
    }

    #[test]
    fn test_focus_manager_handle_widget_removed() {
        let mut manager = FocusManager::new();
        let widget_id = WidgetId::new(0, 1);

        manager.focus(widget_id);
        assert!(manager.handle_widget_removed(widget_id));
        assert!(manager.focused().is_none());

        // Removing a non-focused widget should not clear focus
        manager.focus(widget_id);
        let other_widget = WidgetId::new(1, 1);
        assert!(!manager.handle_widget_removed(other_widget));
        assert_eq!(manager.focused(), Some(widget_id));
    }

    #[test]
    fn test_hit_test_no_widgets() {
        let widgets = vec![];
        assert!(hit_test(10.0, 10.0, &widgets).is_none());
    }

    #[test]
    fn test_hit_test_single_widget() {
        let widget_id = WidgetId::new(0, 1);
        let widgets = vec![HitTestEntry {
            id: widget_id,
            bounds: Rect {
                x: 0.0,
                y: 0.0,
                width: 100.0,
                height: 50.0,
            },
        }];

        // Hit inside
        assert_eq!(hit_test(50.0, 25.0, &widgets), Some(widget_id));

        // Miss outside
        assert!(hit_test(150.0, 25.0, &widgets).is_none());
    }

    #[test]
    fn test_hit_test_overlapping_widgets() {
        let widget1 = WidgetId::new(0, 1);
        let widget2 = WidgetId::new(1, 1);

        // Widget2 is topmost (first in list)
        let widgets = vec![
            HitTestEntry {
                id: widget2,
                bounds: Rect {
                    x: 25.0,
                    y: 25.0,
                    width: 50.0,
                    height: 50.0,
                },
            },
            HitTestEntry {
                id: widget1,
                bounds: Rect {
                    x: 0.0,
                    y: 0.0,
                    width: 100.0,
                    height: 100.0,
                },
            },
        ];

        // Point in overlap area should hit topmost widget
        assert_eq!(hit_test(50.0, 50.0, &widgets), Some(widget2));

        // Point only in widget1
        assert_eq!(hit_test(10.0, 10.0, &widgets), Some(widget1));

        // Point in neither
        assert!(hit_test(150.0, 150.0, &widgets).is_none());
    }

    #[test]
    fn test_generate_focus_events_from_none() {
        let widget_id = WidgetId::new(0, 1);
        let events = generate_focus_events(None, Some(widget_id));

        assert_eq!(events.len(), 1);
        assert_eq!(events[0].0, widget_id);
        assert!(matches!(events[0].1, UiEvent::FocusGained));
    }

    #[test]
    fn test_generate_focus_events_to_none() {
        let widget_id = WidgetId::new(0, 1);
        let events = generate_focus_events(Some(widget_id), None);

        assert_eq!(events.len(), 1);
        assert_eq!(events[0].0, widget_id);
        assert!(matches!(events[0].1, UiEvent::FocusLost));
    }

    #[test]
    fn test_generate_focus_events_change() {
        let widget1 = WidgetId::new(0, 1);
        let widget2 = WidgetId::new(1, 1);
        let events = generate_focus_events(Some(widget1), Some(widget2));

        assert_eq!(events.len(), 2);
        assert_eq!(events[0].0, widget1);
        assert!(matches!(events[0].1, UiEvent::FocusLost));
        assert_eq!(events[1].0, widget2);
        assert!(matches!(events[1].1, UiEvent::FocusGained));
    }

    #[test]
    fn test_generate_focus_events_no_change() {
        let widget_id = WidgetId::new(0, 1);
        let events = generate_focus_events(Some(widget_id), Some(widget_id));

        // No events when focus doesn't change
        assert_eq!(events.len(), 0);
    }
}
