use crate::args::MapMode;
use crate::logger::ConsoleLog;
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
    /// Whether the debug console overlay is open.
    pub console_open: bool,
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
    /// Current tick in the timeline (if replay mode).
    pub timeline_tick: Option<u64>,
    /// Bounds (min_tick, max_tick) of the timeline.
    pub timeline_bounds: Option<(u64, u64)>,
    /// Readable date at the current timeline tick.
    pub timeline_date: Option<String>,
    /// Whether the user is currently dragging the timeline slider.
    pub is_dragging_slider: bool,
}

impl UIState {
    /// Creates a new `UIState` with default values.
    ///
    /// Starts with sidebar closed, Province map mode, and `dirty = true` to force an initial draw.
    pub fn new() -> Self {
        Self {
            sidebar_open: false,
            console_open: false,
            selected_province: None,
            hovered_tooltip: None,
            cursor_pos: None,
            map_mode: MapMode::Province,
            dirty: true, // Initial dirty to draw first frame
            timeline_tick: None,
            timeline_bounds: None,
            timeline_date: None,
            is_dragging_slider: false,
        }
    }

    /// Mark the UI as dirty, forcing a redraw on the next frame.
    #[allow(dead_code)]
    pub fn set_dirty(&mut self) {
        self.dirty = true;
    }

    pub fn toggle_console(&mut self) {
        self.console_open = !self.console_open;
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
    #[allow(dead_code)]
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

    pub fn render(
        &self,
        text_renderer: &TextRenderer,
        width: u32,
        height: u32,
        console_log: &ConsoleLog,
    ) -> RgbaImage {
        let mut image = RgbaImage::new(width, height);

        // 1. Draw Sidebar if open
        if self.sidebar_open {
            let sidebar_w = 300;
            let sidebar_x = width.saturating_sub(sidebar_w);

            // Background
            for y in 0..height {
                for x in sidebar_x..width {
                    image.put_pixel(x, y, Rgba([30, 30, 30, 240]));
                }
            }

            // Text
            if let Some((id, text)) = &self.selected_province {
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
        if let Some((cx, _)) = self.cursor_pos {
            let show_tooltip = if self.sidebar_open {
                cx < (width as f64 - 300.0)
            } else {
                true
            };

            #[allow(clippy::collapsible_if)]
            if show_tooltip {
                if let Some(text) = &self.hovered_tooltip {
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
            let mode_text = format!("Map Mode: {:?}", self.map_mode);
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

        // 5. Draw Time Slider if in Replay Mode
        if let (Some(tick), Some((min, max))) = (self.timeline_tick, self.timeline_bounds) {
            let slider_h = 40;
            let slider_w = width.saturating_sub(600); // Center it, 300px margin
            let slider_x = 300;
            let slider_y = height.saturating_sub(slider_h + 20);

            // Background Track
            for y in slider_y..(slider_y + slider_h) {
                for x in slider_x..(slider_x + slider_w) {
                    if x < width && y < height {
                        image.put_pixel(x, y, Rgba([30, 30, 30, 180]));
                    }
                }
            }

            // Fill Bar
            let progress = if max > min {
                (tick - min) as f64 / (max - min) as f64
            } else {
                0.0
            };
            let fill_w = (progress * slider_w as f64) as u32;
            for y in (slider_y + 15)..(slider_y + 25) {
                for x in slider_x..(slider_x + fill_w) {
                    if x < width && y < height {
                        image.put_pixel(x, y, Rgba([200, 160, 40, 255]));
                    }
                }
            }

            // Thumb (Circle-ish)
            let thumb_x = slider_x + fill_w;
            let thumb_r: i32 = 10;
            for dy in -thumb_r..=thumb_r {
                for dx in -thumb_r..=thumb_r {
                    if dx * dx + dy * dy <= thumb_r * thumb_r {
                        let tx = (thumb_x as i32 + dx) as u32;
                        let ty = (slider_y as i32 + 20 + dy) as u32;
                        if tx < width && ty < height {
                            image.put_pixel(tx, ty, Rgba([255, 255, 255, 255]));
                        }
                    }
                }
            }

            // Date Label (with background for visibility)
            if let Some(date_str) = &self.timeline_date {
                let box_w = 150;
                let box_h = 30;
                let box_x = slider_x + (slider_w / 2) - (box_w / 2);
                let box_y = slider_y.saturating_sub(box_h + 5);

                // Background
                for y in box_y..(box_y + box_h) {
                    for x in box_x..(box_x + box_w) {
                        if x < width && y < height {
                            image.put_pixel(x, y, Rgba([20, 20, 20, 200]));
                        }
                    }
                }

                // Text (centered in box)
                let text_img = text_renderer.render(date_str, box_w, box_h);
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

        // 6. Draw Console if Open
        if self.console_open {
            let logs = console_log.get_lines();
            let console_img = draw_console_overlay(&logs, text_renderer, width, height / 2); // Half height console?

            // Blit console at top (overlays map mode)
            for (tx, ty, px) in console_img.enumerate_pixels() {
                if px[3] > 0 || px[0] != 0 {
                    // Simple check for non-empty
                    if tx < width && ty < height {
                        image.put_pixel(tx, ty, *px);
                    }
                }
            }
        }

        image
    }

    pub fn render_loading_screen(
        &self,
        text_renderer: &TextRenderer,
        width: u32,
        height: u32,
        console_log: &ConsoleLog,
    ) -> RgbaImage {
        // Just reuse the console logic but full screen and different title?
        // Or specific loading screen logic.
        // For now, let's use the draw_console_overlay logic passed for full screen.
        let logs = console_log.get_lines();
        draw_console_overlay(&logs, text_renderer, width, height)
    }
}

/// Helper to render console lines to an image
fn draw_console_overlay(
    logs: &[(log::Level, String)],
    text_renderer: &TextRenderer,
    width: u32,
    height: u32,
) -> RgbaImage {
    let mut image = RgbaImage::new(width, height);

    // Semi-transparent background
    for p in image.pixels_mut() {
        *p = Rgba([10, 10, 15, 230]);
    }

    let line_height = 30; // Compact
    let start_x = 10;
    let mut current_y = height as i32 - 40;

    for (level, msg) in logs.iter().rev() {
        if current_y < 0 {
            break;
        }

        let color_marker = match level {
            log::Level::Error => "[ERROR] ",
            log::Level::Warn => "[WARN]  ",
            log::Level::Info => "[INFO]  ",
            log::Level::Debug => "[DEBUG] ",
            log::Level::Trace => "[TRACE] ",
        };

        let full_line = format!("{}{}", color_marker, msg);
        let text_img = text_renderer.render(&full_line, width - 20, line_height as u32);

        for (tx, ty, px) in text_img.enumerate_pixels() {
            if px[3] > 0 {
                let target_x = start_x + tx;
                let target_y = current_y as u32 + ty;
                if target_x < width && target_y < height {
                    let color = match level {
                        log::Level::Error => Rgba([255, 100, 100, px[3]]),
                        log::Level::Warn => Rgba([255, 255, 100, px[3]]),
                        _ => *px,
                    };
                    image.put_pixel(target_x, target_y, color);
                }
            }
        }
        current_y -= line_height;
    }
    image
}

#[path = "ui/tests.rs"]
#[cfg(test)]
mod tests;
