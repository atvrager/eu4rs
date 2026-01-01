#![allow(dead_code)] // Phase 7.3 - will be used in Phase 7.4 (Rendering)
//! GuiListbox - scrollable list widget with data binding adapter pattern.
//!
//! Phase 7.3: Core primitive structure with scroll tracking and visible range calculation.
//! Phase 7.4: Rendering with scissor rect clipping.
//! Phase 7.5: Interaction (mouse wheel, scrollbar drag, keyboard navigation).

use crate::gui::binder::{Bindable, GuiNode};
use crate::gui::core::{EventResult, GuiRenderer, GuiWidget, UiContext, UiEvent};
use crate::gui::types::{GuiElement, Orientation, Rect};

/// Adapter pattern for binding data items to visual entry templates.
///
/// Implement this trait to customize how your data (e.g., Vec<SaveGame>)
/// maps to visual widgets in the listbox.
pub trait ListAdapter {
    /// Total number of items in the list.
    fn item_count(&self) -> usize;

    /// Bind data for the item at `index` to the provided entry template.
    ///
    /// The entry is a GuiElement::Window containing widgets (text, icon, button, etc.)
    /// that you should populate with data from your item.
    ///
    /// Returns the height of this entry in pixels (usually from the entry template's size).
    fn bind_entry(&self, index: usize, entry: &GuiElement) -> f32;
}

/// Handle to a scrollable listbox widget.
///
/// Generic over item data type `T` (though the adapter handles the actual binding).
#[derive(Debug)]
pub struct GuiListbox {
    /// The underlying listbox element data, if bound.
    element: Option<ListboxData>,
    /// Current scroll offset in pixels (0.0 = top of list).
    scroll_offset: f32,
    /// Item spacing in pixels (from listbox definition).
    spacing: f32,
    /// Total content height in pixels (calculated from all items).
    content_height: f32,
}

#[derive(Debug, Clone)]
struct ListboxData {
    name: String,
    position: (i32, i32),
    size: (u32, u32),
    orientation: Orientation,
    spacing: i32,
    scrollbar_type: Option<String>,
    background: Option<String>,
}

impl GuiListbox {
    /// Get the current scroll offset in pixels.
    pub fn scroll_offset(&self) -> f32 {
        self.scroll_offset
    }

    /// Get the maximum scroll offset (clamps to 0.0 if content fits within bounds).
    pub fn max_scroll(&self) -> f32 {
        if let Some(ref data) = self.element {
            let viewport_height = data.size.1 as f32;
            (self.content_height - viewport_height).max(0.0)
        } else {
            0.0
        }
    }

    /// Set scroll offset (clamped to [0.0, max_scroll]).
    pub fn set_scroll_offset(&mut self, offset: f32) {
        let max = self.max_scroll();
        self.scroll_offset = offset.clamp(0.0, max);
    }

    /// Scroll by a delta amount (positive = down, negative = up).
    pub fn scroll_by(&mut self, delta: f32) {
        self.set_scroll_offset(self.scroll_offset + delta);
    }

    /// Calculate the visible item range given an adapter.
    ///
    /// Returns (start_index, end_index) where end_index is exclusive.
    /// Only items in this range need to be rendered (Phase 7.4).
    pub fn visible_range(&self, adapter: &dyn ListAdapter) -> (usize, usize) {
        if self.element.is_none() {
            return (0, 0);
        }

        let viewport_height = self
            .element
            .as_ref()
            .map(|d| d.size.1 as f32)
            .unwrap_or(0.0);
        let item_count = adapter.item_count();

        if item_count == 0 {
            return (0, 0);
        }

        // Simple uniform row height calculation for Phase 7.3
        // Phase 7.4 will handle variable row heights via adapter
        let avg_row_height = if self.content_height > 0.0 {
            self.content_height / item_count as f32
        } else {
            40.0 // Default row height fallback
        };

        let start_index = (self.scroll_offset / avg_row_height).floor() as usize;
        let visible_rows = (viewport_height / avg_row_height).ceil() as usize + 1; // +1 for partial rows
        let end_index = (start_index + visible_rows).min(item_count);

        (start_index, end_index)
    }

    /// Set total content height (should be called after binding all items).
    pub fn set_content_height(&mut self, height: f32) {
        self.content_height = height;
    }

    /// Get the listbox size (width, height).
    pub fn size(&self) -> (u32, u32) {
        self.element.as_ref().map(|d| d.size).unwrap_or((0, 0))
    }

    /// Get the listbox position.
    pub fn position(&self) -> (i32, i32) {
        self.element.as_ref().map(|d| d.position).unwrap_or((0, 0))
    }

    /// Get item spacing.
    pub fn spacing(&self) -> f32 {
        self.spacing
    }

    /// Get the element name (for debugging).
    pub fn name(&self) -> &str {
        self.element
            .as_ref()
            .map(|d| d.name.as_str())
            .unwrap_or("<placeholder>")
    }
}

impl Bindable for GuiListbox {
    fn from_node(node: &GuiNode) -> Option<Self> {
        match node {
            GuiElement::Listbox {
                name,
                position,
                size,
                orientation,
                spacing,
                scrollbar_type,
                background,
            } => Some(Self {
                element: Some(ListboxData {
                    name: name.clone(),
                    position: *position,
                    size: *size,
                    orientation: *orientation,
                    spacing: *spacing,
                    scrollbar_type: scrollbar_type.clone(),
                    background: background.clone(),
                }),
                scroll_offset: 0.0,
                spacing: *spacing as f32,
                content_height: 0.0,
            }),
            _ => None,
        }
    }

    fn placeholder() -> Self {
        Self {
            element: None,
            scroll_offset: 0.0,
            spacing: 0.0,
            content_height: 0.0,
        }
    }
}

impl GuiWidget for GuiListbox {
    fn render(&self, _ctx: &UiContext, _renderer: &mut dyn GuiRenderer) {
        // Rendering will be implemented in Phase 7.4
        // Will use scissor rect clipping and render only visible items
    }

    fn handle_input(&mut self, _event: &UiEvent, _ctx: &UiContext) -> EventResult {
        // Input handling will be implemented in Phase 7.5
        // Will handle mouse wheel, scrollbar drag, keyboard navigation
        EventResult::Ignored
    }

    fn bounds(&self) -> Rect {
        if let Some(ref data) = self.element {
            Rect {
                x: data.position.0 as f32,
                y: data.position.1 as f32,
                width: data.size.0 as f32,
                height: data.size.1 as f32,
            }
        } else {
            Rect {
                x: 0.0,
                y: 0.0,
                width: 0.0,
                height: 0.0,
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct MockAdapter {
        items: Vec<String>,
    }

    impl ListAdapter for MockAdapter {
        fn item_count(&self) -> usize {
            self.items.len()
        }

        fn bind_entry(&self, _index: usize, _entry: &GuiElement) -> f32 {
            40.0 // Fixed row height for testing
        }
    }

    #[test]
    fn test_placeholder_listbox() {
        let listbox = GuiListbox::placeholder();
        assert_eq!(listbox.scroll_offset(), 0.0);
        assert_eq!(listbox.max_scroll(), 0.0);
        assert_eq!(listbox.name(), "<placeholder>");
    }

    #[test]
    fn test_bind_listbox() {
        let node = GuiElement::Listbox {
            name: "test_list".to_string(),
            position: (10, 20),
            size: (200, 300),
            orientation: Orientation::UpperLeft,
            spacing: 5,
            scrollbar_type: Some("standardlistbox_slider".to_string()),
            background: None,
        };

        let listbox = GuiListbox::from_node(&node).expect("Should bind to Listbox");
        assert_eq!(listbox.name(), "test_list");
        assert_eq!(listbox.position(), (10, 20));
        assert_eq!(listbox.size(), (200, 300));
        assert_eq!(listbox.spacing(), 5.0);
    }

    #[test]
    fn test_scroll_clamping() {
        let node = GuiElement::Listbox {
            name: "test_list".to_string(),
            position: (0, 0),
            size: (200, 300),
            orientation: Orientation::UpperLeft,
            spacing: 0,
            scrollbar_type: None,
            background: None,
        };

        let mut listbox = GuiListbox::from_node(&node).unwrap();
        listbox.set_content_height(500.0); // Content taller than viewport

        // Should clamp to 0.0 at minimum
        listbox.set_scroll_offset(-10.0);
        assert_eq!(listbox.scroll_offset(), 0.0);

        // Should clamp to max_scroll at maximum (500 - 300 = 200)
        listbox.set_scroll_offset(1000.0);
        assert_eq!(listbox.scroll_offset(), 200.0);

        // Should allow values in valid range
        listbox.set_scroll_offset(100.0);
        assert_eq!(listbox.scroll_offset(), 100.0);
    }

    #[test]
    fn test_scroll_by() {
        let node = GuiElement::Listbox {
            name: "test_list".to_string(),
            position: (0, 0),
            size: (200, 300),
            orientation: Orientation::UpperLeft,
            spacing: 0,
            scrollbar_type: None,
            background: None,
        };

        let mut listbox = GuiListbox::from_node(&node).unwrap();
        listbox.set_content_height(500.0);

        listbox.scroll_by(50.0);
        assert_eq!(listbox.scroll_offset(), 50.0);

        listbox.scroll_by(30.0);
        assert_eq!(listbox.scroll_offset(), 80.0);

        listbox.scroll_by(-20.0);
        assert_eq!(listbox.scroll_offset(), 60.0);
    }

    #[test]
    fn test_visible_range_calculation() {
        let node = GuiElement::Listbox {
            name: "test_list".to_string(),
            position: (0, 0),
            size: (200, 300), // Viewport height: 300px
            orientation: Orientation::UpperLeft,
            spacing: 0,
            scrollbar_type: None,
            background: None,
        };

        let mut listbox = GuiListbox::from_node(&node).unwrap();

        let adapter = MockAdapter {
            items: (0..20).map(|i| format!("Item {}", i)).collect(),
        };

        // Content height: 20 items * 40px = 800px
        listbox.set_content_height(800.0);

        // At scroll offset 0, should show first ~7.5 items (300px / 40px = 7.5)
        let (start, end) = listbox.visible_range(&adapter);
        assert_eq!(start, 0);
        assert!(end >= 7 && end <= 9); // Allow some margin for partial rows

        // Scroll down to middle
        listbox.set_scroll_offset(200.0); // Skip 5 items (200 / 40 = 5)
        let (start, end) = listbox.visible_range(&adapter);
        assert_eq!(start, 5);
        assert!(end >= 12 && end <= 14);

        // Scroll to bottom
        listbox.set_scroll_offset(500.0); // Max scroll (800 - 300 = 500)
        let (start, end) = listbox.visible_range(&adapter);
        assert!(start >= 12);
        assert_eq!(end, 20); // Should show up to the last item
    }

    #[test]
    fn test_visible_range_empty_list() {
        let node = GuiElement::Listbox {
            name: "test_list".to_string(),
            position: (0, 0),
            size: (200, 300),
            orientation: Orientation::UpperLeft,
            spacing: 0,
            scrollbar_type: None,
            background: None,
        };

        let listbox = GuiListbox::from_node(&node).unwrap();

        let adapter = MockAdapter { items: Vec::new() };

        let (start, end) = listbox.visible_range(&adapter);
        assert_eq!(start, 0);
        assert_eq!(end, 0);
    }

    #[test]
    fn test_max_scroll_content_fits() {
        let node = GuiElement::Listbox {
            name: "test_list".to_string(),
            position: (0, 0),
            size: (200, 300),
            orientation: Orientation::UpperLeft,
            spacing: 0,
            scrollbar_type: None,
            background: None,
        };

        let mut listbox = GuiListbox::from_node(&node).unwrap();
        listbox.set_content_height(200.0); // Content shorter than viewport

        // Should have no scroll when content fits
        assert_eq!(listbox.max_scroll(), 0.0);
    }
}
