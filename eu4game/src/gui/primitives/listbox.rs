#![allow(dead_code)] // Phase 7.3-7.4 - will be used in Phase 8 (Frontend Panels)
//! GuiListbox - scrollable list widget with data binding adapter pattern.
//!
//! Phase 7.3: Core primitive structure with scroll tracking and visible range calculation.
//! Phase 7.4: Rendering with scissor rect clipping.
//! Phase 7.5: Interaction (mouse wheel, scrollbar drag, keyboard navigation).
//!
//! # Rendering Integration (Phase 7.4)
//!
//! Listbox rendering requires:
//! 1. **Scissor rect clipping** - prevent items from drawing outside listbox bounds
//! 2. **Visible range rendering** - only render items in view (performance optimization)
//! 3. **Entry positioning** - translate entry templates based on scroll offset
//! 4. **Scrollbar rendering** - draw scrollbar when content overflows
//!
//! ## Example Integration
//!
//! ```ignore
//! // In a panel's render method:
//! let listbox_bounds = listbox.bounds();
//! let (start_idx, end_idx) = listbox.visible_range(&adapter);
//!
//! // Set scissor rect to clip rendering to listbox bounds
//! render_pass.set_scissor_rect(
//!     listbox_bounds.x as u32,
//!     listbox_bounds.y as u32,
//!     listbox_bounds.width as u32,
//!     listbox_bounds.height as u32,
//! );
//!
//! // Render only visible items
//! for idx in start_idx..end_idx {
//!     let entry_y_offset = calculate_entry_offset(idx, listbox.scroll_offset(), &adapter);
//!     render_entry_at(idx, entry_y_offset, &adapter, render_pass, sprite_renderer);
//! }
//!
//! // Reset scissor rect to full viewport
//! render_pass.set_scissor_rect(0, 0, screen_width, screen_height);
//!
//! // Render scrollbar if needed
//! if listbox.max_scroll() > 0.0 {
//!     render_scrollbar(&listbox, render_pass, sprite_renderer);
//! }
//! ```

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
        // Phase 7.4: Rendering support implemented via helper functions below.
        // Full integration requires panel-specific rendering code that:
        // 1. Sets scissor rect using set_scissor_rect_for_listbox()
        // 2. Iterates visible range and renders entries with calculate_entry_y_offset()
        // 3. Resets scissor rect
        // 4. Renders scrollbar if needed using get_scrollbar_bounds()
        //
        // See module documentation for example integration code.
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

/// Rendering helper functions (Phase 7.4).
impl GuiListbox {
    /// Get scissor rect bounds for clipping listbox rendering.
    ///
    /// Returns `(x, y, width, height)` in screen pixels, suitable for passing
    /// to `wgpu::RenderPass::set_scissor_rect()`.
    ///
    /// # Example
    /// ```ignore
    /// let (x, y, w, h) = listbox.get_scissor_rect(screen_anchor);
    /// render_pass.set_scissor_rect(x, y, w, h);
    /// ```
    pub fn get_scissor_rect(&self, anchor: (f32, f32)) -> (u32, u32, u32, u32) {
        let bounds = self.bounds();
        let x = (anchor.0 + bounds.x).max(0.0) as u32;
        let y = (anchor.1 + bounds.y).max(0.0) as u32;
        let w = bounds.width as u32;
        let h = bounds.height as u32;
        (x, y, w, h)
    }

    /// Calculate the Y offset for rendering an entry at the given index.
    ///
    /// Takes into account:
    /// - Cumulative height of all previous entries
    /// - Item spacing
    /// - Current scroll offset
    ///
    /// Returns the Y position relative to the listbox's origin.
    ///
    /// # Example
    /// ```ignore
    /// for idx in start..end {
    ///     let entry_y = listbox.calculate_entry_y_offset(idx, &adapter);
    ///     render_entry_at(listbox_y + entry_y, idx, &adapter);
    /// }
    /// ```
    pub fn calculate_entry_y_offset(&self, index: usize, adapter: &dyn ListAdapter) -> f32 {
        // Calculate cumulative height of all entries before this one
        let mut y_offset = 0.0;
        for i in 0..index {
            if i < adapter.item_count() {
                // For now, use a dummy entry to get height. In real implementation,
                // the adapter should provide a way to get entry height without binding.
                // This is simplified for Phase 7.4.
                let dummy_entry = GuiElement::Window {
                    name: String::new(),
                    position: (0, 0),
                    size: (0, 0),
                    orientation: Orientation::UpperLeft,
                    children: Vec::new(),
                };
                let entry_height = adapter.bind_entry(i, &dummy_entry);
                y_offset += entry_height + self.spacing;
            }
        }

        // Subtract scroll offset to shift content up/down
        y_offset - self.scroll_offset
    }

    /// Get the bounds for rendering a scrollbar.
    ///
    /// Returns `Some((x, y, width, height))` if a scrollbar should be rendered,
    /// or `None` if the content fits within the viewport.
    ///
    /// The scrollbar is positioned on the right edge of the listbox, with
    /// a thumb position and size proportional to the scroll state.
    ///
    /// # Example
    /// ```ignore
    /// if let Some((x, y, w, h)) = listbox.get_scrollbar_bounds(anchor) {
    ///     let thumb_bounds = listbox.get_scrollbar_thumb_bounds(anchor);
    ///     render_scrollbar_track(x, y, w, h);
    ///     render_scrollbar_thumb(thumb_bounds);
    /// }
    /// ```
    pub fn get_scrollbar_bounds(&self, anchor: (f32, f32)) -> Option<(f32, f32, f32, f32)> {
        if self.max_scroll() <= 0.0 {
            return None; // No scrollbar needed if content fits
        }

        let bounds = self.bounds();
        const SCROLLBAR_WIDTH: f32 = 12.0; // Standard scrollbar width

        Some((
            anchor.0 + bounds.x + bounds.width - SCROLLBAR_WIDTH,
            anchor.1 + bounds.y,
            SCROLLBAR_WIDTH,
            bounds.height,
        ))
    }

    /// Get the bounds for rendering the scrollbar thumb (slider).
    ///
    /// Returns `(x, y, width, height)` for the thumb based on current scroll position
    /// and content height.
    ///
    /// # Example
    /// ```ignore
    /// let (x, y, w, h) = listbox.get_scrollbar_thumb_bounds(anchor);
    /// render_scrollbar_thumb(x, y, w, h);
    /// ```
    pub fn get_scrollbar_thumb_bounds(&self, anchor: (f32, f32)) -> (f32, f32, f32, f32) {
        const SCROLLBAR_WIDTH: f32 = 12.0;
        const MIN_THUMB_HEIGHT: f32 = 20.0;

        let bounds = self.bounds();
        let viewport_height = bounds.height;
        let content_height = self.content_height;

        // Calculate thumb height as proportion of viewport to content
        let thumb_ratio = viewport_height / content_height.max(1.0);
        let thumb_height = (viewport_height * thumb_ratio).max(MIN_THUMB_HEIGHT);

        // Calculate thumb position as proportion of scroll
        let scroll_ratio = if self.max_scroll() > 0.0 {
            self.scroll_offset / self.max_scroll()
        } else {
            0.0
        };
        let max_thumb_travel = viewport_height - thumb_height;
        let thumb_y = scroll_ratio * max_thumb_travel;

        (
            anchor.0 + bounds.x + bounds.width - SCROLLBAR_WIDTH,
            anchor.1 + bounds.y + thumb_y,
            SCROLLBAR_WIDTH,
            thumb_height,
        )
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

    // Phase 7.4: Rendering helper tests

    #[test]
    fn test_get_scissor_rect() {
        let node = GuiElement::Listbox {
            name: "test_list".to_string(),
            position: (10, 20),
            size: (200, 300),
            orientation: Orientation::UpperLeft,
            spacing: 0,
            scrollbar_type: None,
            background: None,
        };

        let listbox = GuiListbox::from_node(&node).unwrap();
        let anchor = (100.0, 50.0);

        let (x, y, w, h) = listbox.get_scissor_rect(anchor);
        assert_eq!(x, 110); // anchor.x + position.x = 100 + 10
        assert_eq!(y, 70); // anchor.y + position.y = 50 + 20
        assert_eq!(w, 200); // width
        assert_eq!(h, 300); // height
    }

    #[test]
    fn test_calculate_entry_y_offset() {
        let node = GuiElement::Listbox {
            name: "test_list".to_string(),
            position: (0, 0),
            size: (200, 300),
            orientation: Orientation::UpperLeft,
            spacing: 5,
            scrollbar_type: None,
            background: None,
        };

        let mut listbox = GuiListbox::from_node(&node).unwrap();
        listbox.set_content_height(800.0);

        let adapter = MockAdapter {
            items: (0..10).map(|i| format!("Item {}", i)).collect(),
        };

        // Entry 0 should be at offset 0 when scroll is 0
        let y0 = listbox.calculate_entry_y_offset(0, &adapter);
        assert_eq!(y0, 0.0);

        // Entry 1 should be at 40px (entry height) + 5px (spacing) = 45px
        let y1 = listbox.calculate_entry_y_offset(1, &adapter);
        assert_eq!(y1, 45.0);

        // Entry 2 should be at 90px (2 * 45)
        let y2 = listbox.calculate_entry_y_offset(2, &adapter);
        assert_eq!(y2, 90.0);

        // Scroll down by 50px
        listbox.set_scroll_offset(50.0);

        // Entry 0 should now be at -50px (scrolled up out of view)
        let y0_scrolled = listbox.calculate_entry_y_offset(0, &adapter);
        assert_eq!(y0_scrolled, -50.0);

        // Entry 1 should be at -5px (just barely visible at top)
        let y1_scrolled = listbox.calculate_entry_y_offset(1, &adapter);
        assert_eq!(y1_scrolled, -5.0);
    }

    #[test]
    fn test_get_scrollbar_bounds_no_scroll_needed() {
        let node = GuiElement::Listbox {
            name: "test_list".to_string(),
            position: (10, 20),
            size: (200, 300),
            orientation: Orientation::UpperLeft,
            spacing: 0,
            scrollbar_type: None,
            background: None,
        };

        let mut listbox = GuiListbox::from_node(&node).unwrap();
        listbox.set_content_height(200.0); // Fits within viewport

        let anchor = (0.0, 0.0);
        let scrollbar = listbox.get_scrollbar_bounds(anchor);
        assert!(scrollbar.is_none()); // No scrollbar when content fits
    }

    #[test]
    fn test_get_scrollbar_bounds_scroll_needed() {
        let node = GuiElement::Listbox {
            name: "test_list".to_string(),
            position: (10, 20),
            size: (200, 300),
            orientation: Orientation::UpperLeft,
            spacing: 0,
            scrollbar_type: None,
            background: None,
        };

        let mut listbox = GuiListbox::from_node(&node).unwrap();
        listbox.set_content_height(500.0); // Needs scrolling

        let anchor = (100.0, 50.0);
        let scrollbar = listbox.get_scrollbar_bounds(anchor);
        assert!(scrollbar.is_some());

        let (x, y, w, h) = scrollbar.unwrap();
        // Should be on right edge: anchor.x + pos.x + width - scrollbar_width
        // = 100 + 10 + 200 - 12 = 298
        assert_eq!(x, 298.0);
        // Should start at listbox top: anchor.y + pos.y = 50 + 20 = 70
        assert_eq!(y, 70.0);
        // Scrollbar width is 12px
        assert_eq!(w, 12.0);
        // Scrollbar height matches listbox height
        assert_eq!(h, 300.0);
    }

    #[test]
    fn test_get_scrollbar_thumb_bounds() {
        let node = GuiElement::Listbox {
            name: "test_list".to_string(),
            position: (10, 20),
            size: (200, 300),
            orientation: Orientation::UpperLeft,
            spacing: 0,
            scrollbar_type: None,
            background: None,
        };

        let mut listbox = GuiListbox::from_node(&node).unwrap();
        listbox.set_content_height(600.0); // 2x viewport height

        let anchor = (100.0, 50.0);

        // At scroll position 0, thumb should be at top
        listbox.set_scroll_offset(0.0);
        let (x, y, w, h) = listbox.get_scrollbar_thumb_bounds(anchor);
        assert_eq!(x, 298.0); // Same as scrollbar track
        assert_eq!(y, 70.0); // At top
        assert_eq!(w, 12.0);
        // Thumb height should be viewport/content ratio = 300/600 = 0.5
        // So thumb height = 300 * 0.5 = 150px
        assert_eq!(h, 150.0);

        // Scroll to middle (150px = half of max_scroll which is 300)
        listbox.set_scroll_offset(150.0);
        let (_, y_mid, _, h_mid) = listbox.get_scrollbar_thumb_bounds(anchor);
        // Thumb should be halfway down the available travel space
        // Max travel = viewport - thumb_height = 300 - 150 = 150px
        // At 50% scroll, thumb moves 75px down
        assert_eq!(y_mid, 70.0 + 75.0); // base + half of max_travel
        assert_eq!(h_mid, 150.0);

        // Scroll to bottom (300px = max_scroll)
        listbox.set_scroll_offset(300.0);
        let (_, y_bottom, _, _) = listbox.get_scrollbar_thumb_bounds(anchor);
        // Thumb should be at bottom: base + max_travel = 70 + 150 = 220
        assert_eq!(y_bottom, 220.0);
    }
}
