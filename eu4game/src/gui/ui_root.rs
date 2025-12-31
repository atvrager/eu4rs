//! UI root - central input dispatch and panel management.
//!
//! The UiRoot manages all UI panels, routes input events, and handles keyboard focus.

use crate::gui::core::{UiAction, UiEvent, WidgetId};
use crate::gui::input::{FocusManager, HitTestEntry, generate_focus_events, hit_test};
use crate::gui::types::Rect;

/// Central manager for UI panels and input routing.
///
/// Coordinates input dispatch, focus management, and panel visibility.
#[derive(Debug)]
pub struct UiRoot {
    /// Keyboard focus manager.
    focus: FocusManager,
    /// Cached widget bounds for hit testing.
    /// Rebuilt when panel layouts change.
    hit_test_cache: Vec<HitTestEntry>,
    /// Whether the hit test cache needs rebuilding.
    #[allow(dead_code)] // Will be used when panels are integrated
    cache_dirty: bool,
}

impl UiRoot {
    /// Create a new UI root.
    pub fn new() -> Self {
        Self {
            focus: FocusManager::new(),
            hit_test_cache: Vec::new(),
            cache_dirty: true,
        }
    }

    /// Mark the hit test cache as dirty (needs rebuilding).
    ///
    /// Call this when panel layouts change (windows move, resize, or visibility changes).
    #[allow(dead_code)] // Will be used when panels are integrated
    pub fn invalidate_cache(&mut self) {
        self.cache_dirty = true;
    }

    /// Rebuild the hit test cache from current panel state.
    ///
    /// In a full implementation, this would traverse all panels and build a flat list
    /// of widget bounds in reverse render order (topmost first).
    /// For now, this is a placeholder.
    #[allow(dead_code)] // Will be used when panels are integrated
    fn rebuild_cache(&mut self) {
        if !self.cache_dirty {
            return;
        }

        // In production:
        // 1. Clear cache
        // 2. Traverse all visible panels in reverse Z-order
        // 3. For each panel, add widget bounds to cache
        // 4. Mark cache as clean

        self.hit_test_cache.clear();
        self.cache_dirty = false;
    }

    /// Add a widget to the hit test cache.
    ///
    /// Helper for panel implementations to register interactive widgets.
    #[allow(dead_code)] // Will be used when panels are integrated
    pub fn register_widget(&mut self, id: WidgetId, bounds: Rect) {
        self.hit_test_cache.push(HitTestEntry { id, bounds });
    }

    /// Dispatch an input event to the appropriate widget.
    ///
    /// Returns an action if a widget (like a button) requests a screen transition.
    #[allow(dead_code)] // Will be used when panels are integrated
    pub fn dispatch_event(&mut self, event: &UiEvent) -> Option<UiAction> {
        self.rebuild_cache();

        match event {
            UiEvent::MouseButton {
                button: _,
                state: _,
                x,
                y,
            } => {
                // Hit test to find widget under cursor
                if let Some(widget_id) = hit_test(*x, *y, &self.hit_test_cache) {
                    // Dispatch to widget
                    // In production, this would call the widget's handle_input()
                    // and potentially return a UiAction

                    // For now, return None (no action)
                    let _ = widget_id; // Suppress unused warning
                }
                None
            }
            UiEvent::MouseMove { x: _, y: _ } => {
                // Mouse move events don't typically generate actions
                None
            }
            UiEvent::KeyPress {
                key: _,
                modifiers: _,
            }
            | UiEvent::TextInput { character: _ } => {
                // Dispatch to focused widget (if any)
                if let Some(_focused_id) = self.focus.focused() {
                    // In production, dispatch to focused widget
                    // and potentially return UiAction
                }
                None
            }
            UiEvent::FocusGained | UiEvent::FocusLost => {
                // Focus events are typically generated internally,
                // not dispatched from external input
                None
            }
        }
    }

    /// Set keyboard focus to a specific widget.
    ///
    /// Generates focus transition events for the affected widgets.
    #[allow(dead_code)] // Will be used when panels support focus
    pub fn set_focus(&mut self, id: WidgetId) -> Vec<(WidgetId, UiEvent)> {
        let (previous, new) = self.focus.focus(id);
        generate_focus_events(previous, Some(new))
    }

    /// Clear keyboard focus.
    ///
    /// Generates a FocusLost event for the previously focused widget (if any).
    #[allow(dead_code)] // Will be used when panels support focus
    pub fn clear_focus(&mut self) -> Vec<(WidgetId, UiEvent)> {
        let previous = self.focus.clear_focus();
        generate_focus_events(previous, None)
    }

    /// Get the currently focused widget.
    #[allow(dead_code)] // Will be used when panels query focus state
    pub fn focused_widget(&self) -> Option<WidgetId> {
        self.focus.focused()
    }

    /// Handle widget removal - clears focus if the removed widget had it.
    #[allow(dead_code)] // Will be used when panels remove widgets dynamically
    pub fn handle_widget_removed(&mut self, id: WidgetId) -> Vec<(WidgetId, UiEvent)> {
        if self.focus.handle_widget_removed(id) {
            // Focus was cleared, generate FocusLost event
            generate_focus_events(Some(id), None)
        } else {
            Vec::new()
        }
    }
}

impl Default for UiRoot {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::gui::core::{ButtonState, KeyCode, MouseButton};

    #[test]
    fn test_ui_root_creation() {
        let root = UiRoot::new();
        assert!(root.focused_widget().is_none());
        assert!(root.cache_dirty);
    }

    #[test]
    fn test_cache_invalidation() {
        let mut root = UiRoot::new();
        root.cache_dirty = false; // Simulate clean cache

        root.invalidate_cache();
        assert!(root.cache_dirty);
    }

    #[test]
    fn test_cache_rebuild() {
        let mut root = UiRoot::new();
        root.cache_dirty = true;

        root.rebuild_cache();
        assert!(!root.cache_dirty);
        assert!(root.hit_test_cache.is_empty()); // No widgets registered yet
    }

    #[test]
    fn test_register_widget() {
        let mut root = UiRoot::new();
        let widget_id = WidgetId::new(0, 1);
        let bounds = Rect {
            x: 10.0,
            y: 20.0,
            width: 100.0,
            height: 50.0,
        };

        root.register_widget(widget_id, bounds);
        assert_eq!(root.hit_test_cache.len(), 1);
        assert_eq!(root.hit_test_cache[0].id, widget_id);
    }

    #[test]
    fn test_dispatch_mouse_event_no_widgets() {
        let mut root = UiRoot::new();

        let action = root.dispatch_event(&UiEvent::MouseButton {
            button: MouseButton::Left,
            state: ButtonState::Released,
            x: 50.0,
            y: 50.0,
        });

        assert!(action.is_none());
    }

    #[test]
    fn test_dispatch_keyboard_event_no_focus() {
        use crate::gui::core::Modifiers;

        let mut root = UiRoot::new();

        let action = root.dispatch_event(&UiEvent::KeyPress {
            key: KeyCode::Escape,
            modifiers: Modifiers::default(),
        });

        assert!(action.is_none());
    }

    #[test]
    fn test_set_focus_generates_events() {
        let mut root = UiRoot::new();
        let widget1 = WidgetId::new(0, 1);
        let widget2 = WidgetId::new(1, 1);

        // Set initial focus
        let events = root.set_focus(widget1);
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].0, widget1);
        assert!(matches!(events[0].1, UiEvent::FocusGained));

        // Change focus
        let events = root.set_focus(widget2);
        assert_eq!(events.len(), 2);
        assert_eq!(events[0].0, widget1);
        assert!(matches!(events[0].1, UiEvent::FocusLost));
        assert_eq!(events[1].0, widget2);
        assert!(matches!(events[1].1, UiEvent::FocusGained));
    }

    #[test]
    fn test_clear_focus_generates_event() {
        let mut root = UiRoot::new();
        let widget_id = WidgetId::new(0, 1);

        root.set_focus(widget_id);

        let events = root.clear_focus();
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].0, widget_id);
        assert!(matches!(events[0].1, UiEvent::FocusLost));
        assert!(root.focused_widget().is_none());
    }

    #[test]
    fn test_handle_widget_removed_with_focus() {
        let mut root = UiRoot::new();
        let widget_id = WidgetId::new(0, 1);

        root.set_focus(widget_id);

        let events = root.handle_widget_removed(widget_id);
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].0, widget_id);
        assert!(matches!(events[0].1, UiEvent::FocusLost));
        assert!(root.focused_widget().is_none());
    }

    #[test]
    fn test_handle_widget_removed_without_focus() {
        let mut root = UiRoot::new();
        let widget1 = WidgetId::new(0, 1);
        let widget2 = WidgetId::new(1, 1);

        root.set_focus(widget1);

        let events = root.handle_widget_removed(widget2);
        assert!(events.is_empty());
        assert_eq!(root.focused_widget(), Some(widget1));
    }
}
