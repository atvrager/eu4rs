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
    /// Currently selected item index (Phase 7.5).
    selected_index: Option<usize>,
    /// Whether the scrollbar is currently being dragged (Phase 7.5).
    scrollbar_dragging: bool,
    /// Mouse Y position when scrollbar drag started (Phase 7.5).
    scrollbar_drag_start_y: Option<f32>,
    /// Scroll offset when drag started (Phase 7.5).
    scrollbar_drag_start_offset: f32,
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

    /// Get the currently selected item index (Phase 7.5).
    pub fn selected_index(&self) -> Option<usize> {
        self.selected_index
    }

    /// Set the selected item index (Phase 7.5).
    ///
    /// Pass `None` to clear selection. Index is not validated against item count.
    pub fn set_selected_index(&mut self, index: Option<usize>) {
        self.selected_index = index;
    }

    /// Select the next item (down arrow / down movement).
    ///
    /// Wraps around to the first item if at the end.
    pub fn select_next(&mut self, item_count: usize) {
        if item_count == 0 {
            return;
        }
        self.selected_index = Some(match self.selected_index {
            Some(idx) if idx + 1 < item_count => idx + 1,
            _ => 0, // Wrap to start
        });
    }

    /// Select the previous item (up arrow / up movement).
    ///
    /// Wraps around to the last item if at the beginning.
    pub fn select_previous(&mut self, item_count: usize) {
        if item_count == 0 {
            return;
        }
        self.selected_index = Some(match self.selected_index {
            Some(0) | None => item_count - 1, // Wrap to end
            Some(idx) => idx - 1,
        });
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
                selected_index: None,
                scrollbar_dragging: false,
                scrollbar_drag_start_y: None,
                scrollbar_drag_start_offset: 0.0,
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
            selected_index: None,
            scrollbar_dragging: false,
            scrollbar_drag_start_y: None,
            scrollbar_drag_start_offset: 0.0,
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

    fn handle_input(&mut self, event: &UiEvent, _ctx: &UiContext) -> EventResult {
        use crate::gui::core::{ButtonState as InputButtonState, KeyCode, MouseButton};

        if self.element.is_none() {
            return EventResult::Ignored;
        }

        match event {
            // Mouse wheel scrolling (Phase 7.5)
            UiEvent::MouseWheel { delta_y, x, y } => {
                let bounds = self.bounds();
                // Check if mouse is over the listbox
                if bounds.contains(*x, *y) {
                    // Scroll by delta
                    // Positive delta_y = scroll down (increase offset)
                    // Negative delta_y = scroll up (decrease offset)
                    const SCROLL_SPEED: f32 = 40.0; // pixels per wheel notch
                    self.scroll_by(delta_y * SCROLL_SPEED);
                    return EventResult::Consumed;
                }
                EventResult::Ignored
            }

            // Mouse button events - handle clicks and scrollbar dragging (Phase 7.5)
            UiEvent::MouseButton {
                button: MouseButton::Left,
                state: InputButtonState::Pressed,
                x,
                y,
            } => {
                let bounds = self.bounds();

                // Check if clicking on scrollbar thumb
                if self.max_scroll() > 0.0 {
                    let (thumb_x, thumb_y, thumb_w, thumb_h) =
                        self.get_scrollbar_thumb_bounds((0.0, 0.0));

                    if *x >= thumb_x
                        && *x < thumb_x + thumb_w
                        && *y >= thumb_y
                        && *y < thumb_y + thumb_h
                    {
                        // Start scrollbar drag
                        self.scrollbar_dragging = true;
                        self.scrollbar_drag_start_y = Some(*y);
                        self.scrollbar_drag_start_offset = self.scroll_offset;
                        return EventResult::Consumed;
                    }
                }

                // Check if clicking on listbox content (item selection)
                if bounds.contains(*x, *y) {
                    // Calculate which item was clicked
                    // This is simplified - real implementation would use adapter
                    // to get accurate heights
                    let _relative_y = *y - bounds.y + self.scroll_offset;

                    // For now, assume uniform row heights
                    // In real use, the panel should handle item selection
                    // by iterating entries and checking bounds

                    return EventResult::Consumed;
                }

                EventResult::Ignored
            }

            UiEvent::MouseButton {
                button: MouseButton::Left,
                state: InputButtonState::Released,
                ..
            } => {
                if self.scrollbar_dragging {
                    self.scrollbar_dragging = false;
                    self.scrollbar_drag_start_y = None;
                    return EventResult::Consumed;
                }
                EventResult::Ignored
            }

            // Mouse move - handle scrollbar dragging (Phase 7.5)
            UiEvent::MouseMove { y, .. } => {
                if self.scrollbar_dragging
                    && let Some(start_y) = self.scrollbar_drag_start_y
                {
                    let delta_y = *y - start_y;

                    // Convert mouse movement to scroll offset
                    // Thumb travel distance is (viewport_height - thumb_height)
                    // Content scroll range is max_scroll
                    let bounds = self.bounds();
                    let viewport_height = bounds.height;
                    let (_, _, _, thumb_height) = self.get_scrollbar_thumb_bounds((0.0, 0.0));
                    let max_thumb_travel = viewport_height - thumb_height;

                    if max_thumb_travel > 0.0 {
                        let scroll_ratio = delta_y / max_thumb_travel;
                        let scroll_delta = scroll_ratio * self.max_scroll();
                        self.set_scroll_offset(self.scrollbar_drag_start_offset + scroll_delta);
                    }

                    return EventResult::Consumed;
                }
                EventResult::Ignored
            }

            // Keyboard navigation (Phase 7.5)
            UiEvent::KeyPress { key, .. } => {
                // Note: In real implementation, listbox should only handle
                // keyboard events when it has focus
                match key {
                    KeyCode::Up => {
                        // select_previous requires item count
                        // In real use, panel provides this via adapter
                        EventResult::Ignored
                    }
                    KeyCode::Down => {
                        // select_next requires item count
                        // In real use, panel provides this via adapter
                        EventResult::Ignored
                    }
                    KeyCode::PageUp => {
                        // Scroll up one page (viewport height)
                        let bounds = self.bounds();
                        let page_size = bounds.height;
                        self.scroll_by(-page_size);
                        EventResult::Consumed
                    }
                    KeyCode::PageDown => {
                        // Scroll down one page
                        let bounds = self.bounds();
                        let page_size = bounds.height;
                        self.scroll_by(page_size);
                        EventResult::Consumed
                    }
                    KeyCode::Home => {
                        // Scroll to top
                        self.set_scroll_offset(0.0);
                        EventResult::Consumed
                    }
                    KeyCode::End => {
                        // Scroll to bottom
                        self.set_scroll_offset(self.max_scroll());
                        EventResult::Consumed
                    }
                    _ => EventResult::Ignored,
                }
            }

            _ => EventResult::Ignored,
        }
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
        assert!((7..=9).contains(&end)); // Allow some margin for partial rows

        // Scroll down to middle
        listbox.set_scroll_offset(200.0); // Skip 5 items (200 / 40 = 5)
        let (start, end) = listbox.visible_range(&adapter);
        assert_eq!(start, 5);
        assert!((12..=14).contains(&end));

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

    // Phase 7.5: Interaction tests

    #[test]
    fn test_selection_management() {
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

        // Initially no selection
        assert_eq!(listbox.selected_index(), None);

        // Set selection
        listbox.set_selected_index(Some(5));
        assert_eq!(listbox.selected_index(), Some(5));

        // Clear selection
        listbox.set_selected_index(None);
        assert_eq!(listbox.selected_index(), None);
    }

    #[test]
    fn test_select_next_previous() {
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
        let item_count = 10;

        // Select first item
        listbox.set_selected_index(Some(0));

        // Select next
        listbox.select_next(item_count);
        assert_eq!(listbox.selected_index(), Some(1));

        // Select previous
        listbox.select_previous(item_count);
        assert_eq!(listbox.selected_index(), Some(0));

        // Wrap around at start (previous)
        listbox.select_previous(item_count);
        assert_eq!(listbox.selected_index(), Some(9));

        // Wrap around at end (next)
        listbox.select_next(item_count);
        assert_eq!(listbox.selected_index(), Some(0));
    }

    #[test]
    fn test_mouse_wheel_scrolling() {
        use crate::gui::core::UiContext;

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
        listbox.set_content_height(600.0);

        let ctx = UiContext {
            mouse_pos: (100.0, 100.0),
            time: 0.0,
            delta_time: 0.016,
            localizer: &crate::gui::core::NoOpLocalizer,
            focused_widget: None,
        };

        // Scroll down (positive delta_y)
        let event = UiEvent::MouseWheel {
            delta_y: 1.0,
            x: 100.0,
            y: 100.0,
        };
        let result = listbox.handle_input(&event, &ctx);
        assert_eq!(result, EventResult::Consumed);
        assert_eq!(listbox.scroll_offset(), 40.0); // SCROLL_SPEED = 40

        // Scroll up (negative delta_y)
        let event = UiEvent::MouseWheel {
            delta_y: -0.5,
            x: 100.0,
            y: 100.0,
        };
        let result = listbox.handle_input(&event, &ctx);
        assert_eq!(result, EventResult::Consumed);
        assert_eq!(listbox.scroll_offset(), 20.0); // 40 - 20

        // Mouse wheel outside listbox bounds should be ignored
        let event = UiEvent::MouseWheel {
            delta_y: 1.0,
            x: 300.0,
            y: 400.0,
        };
        let result = listbox.handle_input(&event, &ctx);
        assert_eq!(result, EventResult::Ignored);
        assert_eq!(listbox.scroll_offset(), 20.0); // Unchanged
    }

    #[test]
    fn test_keyboard_navigation() {
        use crate::gui::core::{KeyCode, Modifiers, UiContext};

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
        listbox.set_content_height(600.0);

        let ctx = UiContext {
            mouse_pos: (0.0, 0.0),
            time: 0.0,
            delta_time: 0.016,
            localizer: &crate::gui::core::NoOpLocalizer,
            focused_widget: None,
        };

        // PageDown
        let event = UiEvent::KeyPress {
            key: KeyCode::PageDown,
            modifiers: Modifiers::default(),
        };
        let result = listbox.handle_input(&event, &ctx);
        assert_eq!(result, EventResult::Consumed);
        assert_eq!(listbox.scroll_offset(), 300.0); // Page size = viewport height

        // PageUp
        let event = UiEvent::KeyPress {
            key: KeyCode::PageUp,
            modifiers: Modifiers::default(),
        };
        let result = listbox.handle_input(&event, &ctx);
        assert_eq!(result, EventResult::Consumed);
        assert_eq!(listbox.scroll_offset(), 0.0);

        // Home (already at top)
        let event = UiEvent::KeyPress {
            key: KeyCode::Home,
            modifiers: Modifiers::default(),
        };
        let result = listbox.handle_input(&event, &ctx);
        assert_eq!(result, EventResult::Consumed);
        assert_eq!(listbox.scroll_offset(), 0.0);

        // End
        let event = UiEvent::KeyPress {
            key: KeyCode::End,
            modifiers: Modifiers::default(),
        };
        let result = listbox.handle_input(&event, &ctx);
        assert_eq!(result, EventResult::Consumed);
        assert_eq!(listbox.scroll_offset(), 300.0); // max_scroll = 600 - 300
    }

    #[test]
    fn test_scrollbar_drag() {
        use crate::gui::core::{ButtonState, MouseButton, UiContext};

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
        listbox.set_content_height(600.0); // 2x viewport

        let ctx = UiContext {
            mouse_pos: (0.0, 0.0),
            time: 0.0,
            delta_time: 0.016,
            localizer: &crate::gui::core::NoOpLocalizer,
            focused_widget: None,
        };

        // Get scrollbar thumb position at scroll offset 0
        let (thumb_x, thumb_y, _, _) = listbox.get_scrollbar_thumb_bounds((0.0, 0.0));

        // Click on scrollbar thumb to start drag
        let event = UiEvent::MouseButton {
            button: MouseButton::Left,
            state: ButtonState::Pressed,
            x: thumb_x + 5.0,
            y: thumb_y + 10.0,
        };
        let result = listbox.handle_input(&event, &ctx);
        assert_eq!(result, EventResult::Consumed);
        assert!(listbox.scrollbar_dragging);

        // Drag thumb down by 75 pixels (should scroll to middle)
        let event = UiEvent::MouseMove {
            x: thumb_x + 5.0,
            y: thumb_y + 85.0, // 75px down from start
        };
        let result = listbox.handle_input(&event, &ctx);
        assert_eq!(result, EventResult::Consumed);
        // Max scroll is 300, max thumb travel is 150
        // 75px thumb movement = 50% of travel = 150px scroll
        assert_eq!(listbox.scroll_offset(), 150.0);

        // Release mouse button
        let event = UiEvent::MouseButton {
            button: MouseButton::Left,
            state: ButtonState::Released,
            x: thumb_x + 5.0,
            y: thumb_y + 85.0,
        };
        let result = listbox.handle_input(&event, &ctx);
        assert_eq!(result, EventResult::Consumed);
        assert!(!listbox.scrollbar_dragging);
    }
}
