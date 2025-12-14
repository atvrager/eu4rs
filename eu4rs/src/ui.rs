use crate::args::MapMode;
use crate::text::TextRenderer;
use image::{Rgba, RgbaImage};

/// Manages the state of the User Interface overlay.
///
/// This struct holds the current transient state of UI elements such as the sidebar,
/// hovered tooltips, cursor position, and the active map mode. It also tracks a `dirty` flag
/// to indicate when the UI texture needs to be regenerated.
#[derive(Debug, Clone)]
pub struct UIState {
    /// Whether the province details sidebar (right side) is currently open.
    pub sidebar_open: bool,
    /// The currently selected province ID and its detailed text, if any.
    pub selected_province: Option<(u32, String)>,
    /// The text to display in the floating tooltip (bottom-left), if any.
    pub hovered_tooltip: Option<String>,
    /// The current cursor position in screen coordinates (pixels).
    pub cursor_pos: Option<(f64, f64)>,
    /// The currently active map mode (e.g., Province, Political).
    pub map_mode: MapMode,
    /// Flag indicating if the UI state has changed and the texture needs regeneration.
    /// This optimization prevents unnecessary CPU drawing and GPU uploads.
    pub dirty: bool,
}

impl UIState {
    /// Creates a new `UIState` with default values.
    ///
    /// Starts with sidebar closed, Province map mode, and `dirty = true` to force an initial draw.
    pub fn new() -> Self {
        Self {
            sidebar_open: false,
            selected_province: None,
            hovered_tooltip: None,
            cursor_pos: None,
            map_mode: MapMode::Province,
            dirty: true, // Initial dirty to draw first frame
        }
    }

    /// Mark the UI as dirty, forcing a redraw on the next frame.
    pub fn set_dirty(&mut self) {
        self.dirty = true;
    }

    /// Sets the sidebar visibility state.
    ///
    /// If the state changes, the `dirty` flag is set to true.
    pub fn set_sidebar_open(&mut self, open: bool) {
        if self.sidebar_open != open {
            self.sidebar_open = open;
            self.dirty = true;
        }
    }

    /// Sets the selected province.
    ///
    /// If the selection changes, the `dirty` flag is set to true.
    pub fn set_selected_province(&mut self, sel: Option<(u32, String)>) {
        if self.selected_province != sel {
            self.selected_province = sel;
            self.dirty = true;
        }
    }

    /// Sets the content of the hovered tooltip.
    ///
    /// If the content changes, the `dirty` flag is set to true.
    pub fn set_hovered_tooltip(&mut self, tooltip: Option<String>) {
        if self.hovered_tooltip != tooltip {
            self.hovered_tooltip = tooltip;
            self.dirty = true;
        }
    }

    /// Updates the cursor position.
    ///
    /// If the position changes, the `dirty` flag is set to true.
    /// Note: This can cause frequent redraws if the mouse is moving constantly.
    pub fn set_cursor_pos(&mut self, pos: Option<(f64, f64)>) {
        // Cursor pos changes every frame mouse moves, so dirtiness might be frequent.
        // But UI rendering depends on it for tooltip visibility logic.
        if self.cursor_pos != pos {
            self.cursor_pos = pos;
            self.dirty = true;
        }
    }

    /// Handles click events to determine if they interact with UI elements.
    ///
    /// Returns `true` if the click (at `x` coordinate) overlaps a UI element (like the sidebar),
    /// indicating that the event should be consumed and not propagate to the map.
    pub fn on_click(&mut self, x: f64, width: f64) -> bool {
        if self.sidebar_open {
            // Check if click is in sidebar (Right 300px)
            let sidebar_x = width - 300.0;
            if x >= sidebar_x {
                return true; // Consumed by sidebar
            }
        }
        false
    }
}

/// Renders the UI state into an RGBA image.
///
/// This function generates the complete visual representation of the UI overlay, including:
/// - The sidebar (if open) with selected province details.
/// - The bottom-left tooltip path hovering over the map.
/// - The top-left map mode indicator box.
///
/// It uses the provided `TextRenderer` to draw text elements.
///
/// # Arguments
///
/// * `state` - The current `UIState` snapshot to render.
/// * `text_renderer` - Renderer for text glyphs.
/// * `width` - Width of the output image (should match window width).
/// * `height` - Height of the output image (should match window height).
pub fn draw_ui(
    state: &UIState,
    text_renderer: &TextRenderer,
    width: u32,
    height: u32,
) -> RgbaImage {
    let mut image = RgbaImage::new(width, height);

    // Clear to transparent (0,0,0,0) logic is default for new image?
    // Actually image::new creates zeroed pixels, which is transparent for RGBA.
    // But let's be explicit if we need to. default is 0,0,0,0.

    // 1. Draw Sidebar if open
    if state.sidebar_open {
        let sidebar_w = 300;
        let sidebar_x = width.saturating_sub(sidebar_w);

        // Background
        for y in 0..height {
            for x in sidebar_x..width {
                image.put_pixel(x, y, Rgba([30, 30, 30, 240]));
            }
        }

        // Text
        if let Some((id, text)) = &state.selected_province {
            // Render text into a temp buffer then blit?
            // Or just use renderer on this image?
            // TextRenderer currently creates a new image.
            // Let's modify TextRenderer usage or just blit.
            let content = format!("Province {}\n\n{}", id, text);
            let text_img = text_renderer.render(&content, sidebar_w, height);

            // Blit text_img onto image at sidebar_x, 0
            for (tx, ty, px) in text_img.enumerate_pixels() {
                if px[3] > 0 {
                    let target_x = sidebar_x + tx;
                    if target_x < width {
                        image.put_pixel(target_x, ty, *px);
                    }
                }
            }
        }
    }

    // 2. Draw Bottom-Left Tooltip if cursor is over map
    if let Some((cx, _)) = state.cursor_pos {
        let show_tooltip = if state.sidebar_open {
            cx < (width as f64 - 300.0)
        } else {
            true
        };

        #[allow(clippy::collapsible_if)]
        if show_tooltip {
            if let Some(text) = &state.hovered_tooltip {
                // Determine size
                // Simple hack: count lines. TextRenderer uses 24px font + 6px padding = 30px line height?
                // Let's assume a fixed box for now or use TextRenderer to measure (not implemented yet).
                // We'll just draw a fixed box at bottom left.
                let box_h = 40;
                let box_w = 400;
                let box_x = 10;
                let box_y = height.saturating_sub(box_h + 10);

                // Background
                for y in box_y..(box_y + box_h) {
                    for x in box_x..(box_x + box_w) {
                        if x < width && y < height {
                            image.put_pixel(x, y, Rgba([20, 20, 20, 200]));
                        }
                    }
                }

                // Text
                let text_img = text_renderer.render(text, box_w, box_h);
                // Blit
                for (tx, ty, px) in text_img.enumerate_pixels() {
                    if px[3] > 0 {
                        let target_x = box_x + tx;
                        let target_y = box_y + ty;
                        if target_x < width && target_y < height {
                            image.put_pixel(target_x, target_y, *px);
                        }
                    }
                }
            }
        }
    }

    // 3. Draw Top-Left Map Mode Indicator
    {
        let mode_text = format!("Map Mode: {:?}", state.map_mode);
        let box_h = 40;
        let box_w = 300;
        let box_x = 10;
        let box_y = 10;

        // Background
        for y in box_y..(box_y + box_h) {
            for x in box_x..(box_x + box_w) {
                if x < width && y < height {
                    image.put_pixel(x, y, Rgba([20, 20, 20, 200]));
                }
            }
        }

        // Text
        let text_img = text_renderer.render(&mode_text, box_w, box_h);
        for (tx, ty, px) in text_img.enumerate_pixels() {
            if px[3] > 0 {
                let target_x = box_x + tx;
                let target_y = box_y + ty;
                if target_x < width && target_y < height {
                    image.put_pixel(target_x, target_y, *px);
                }
            }
        }
    }

    image
}
