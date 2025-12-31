//! EU4 GUI layout engine.
//!
//! Converts EU4 pixel coordinates with orientation anchors to screen/clip space.

use super::types::Orientation;

/// Get the anchor point for a window based on its orientation.
///
/// Windows are just anchor points with no size. The returned point
/// is the reference from which all child elements are positioned.
pub fn get_window_anchor(
    window_pos: (i32, i32),
    orientation: Orientation,
    screen_size: (u32, u32),
) -> (f32, f32) {
    let (sw, sh) = (screen_size.0 as f32, screen_size.1 as f32);
    let (wx, wy) = (window_pos.0 as f32, window_pos.1 as f32);

    match orientation {
        Orientation::UpperLeft => (wx, wy),
        Orientation::UpperRight => (sw + wx, wy),
        Orientation::LowerLeft => (wx, sh + wy),
        Orientation::LowerRight => (sw + wx, sh + wy),
        Orientation::Center => (sw / 2.0 + wx, sh / 2.0 + wy),
        Orientation::CenterUp => (sw / 2.0 + wx, wy),
        Orientation::CenterDown => (sw / 2.0 + wx, sh + wy),
    }
}

/// Position an element relative to a window anchor point.
///
/// The element's orientation determines how its position offset is interpreted.
/// Returns the top-left corner of the element in screen coordinates.
pub fn position_from_anchor(
    anchor: (f32, f32),
    element_pos: (i32, i32),
    element_orientation: Orientation,
    element_size: (u32, u32),
) -> (f32, f32) {
    let (px, py) = (element_pos.0 as f32, element_pos.1 as f32);
    let (ew, _eh) = (element_size.0 as f32, element_size.1 as f32);

    match element_orientation {
        Orientation::UpperLeft => {
            // Position is offset from anchor, element's top-left at that point
            (anchor.0 + px, anchor.1 + py)
        }
        Orientation::UpperRight => {
            // Position is offset from anchor, element extends left
            // px is typically negative, so element's right edge is at anchor.x + px
            // top-left is at (anchor.x + px - width, anchor.y + py)
            // But EU4 seems to treat px as position of element's left edge from anchor
            (anchor.0 + px, anchor.1 + py)
        }
        Orientation::LowerLeft => (anchor.0 + px, anchor.1 + py),
        Orientation::LowerRight => (anchor.0 + px, anchor.1 + py),
        Orientation::Center => (anchor.0 + px - ew / 2.0, anchor.1 + py),
        Orientation::CenterUp => (anchor.0 + px - ew / 2.0, anchor.1 + py),
        Orientation::CenterDown => (anchor.0 + px - ew / 2.0, anchor.1 + py),
    }
}

/// Resolve EU4 position to screen pixel coordinates.
///
/// EU4 positions are in pixels with optional negative values meaning
/// "from the opposite edge". The orientation determines the anchor point.
pub fn resolve_position(
    position: (i32, i32),
    orientation: Orientation,
    element_size: (u32, u32),
    screen_size: (u32, u32),
) -> (f32, f32) {
    let (px, py) = position;
    let (ew, eh) = (element_size.0 as f32, element_size.1 as f32);
    let (sw, sh) = (screen_size.0 as f32, screen_size.1 as f32);

    // Calculate base position based on orientation anchor
    let (anchor_x, anchor_y) = match orientation {
        Orientation::UpperLeft => (0.0, 0.0),
        Orientation::UpperRight => (sw, 0.0),
        Orientation::LowerLeft => (0.0, sh),
        Orientation::LowerRight => (sw, sh),
        Orientation::Center => (sw / 2.0, sh / 2.0),
        Orientation::CenterUp => (sw / 2.0, 0.0),
        Orientation::CenterDown => (sw / 2.0, sh),
    };

    // Offset adjustment based on orientation
    // For right/bottom anchors, positive X/Y moves left/up
    let (offset_x, offset_y) = match orientation {
        Orientation::UpperLeft => (px as f32, py as f32),
        Orientation::UpperRight => {
            // Anchored right: element extends left from anchor
            (px as f32 - ew, py as f32)
        }
        Orientation::LowerLeft => {
            // Anchored bottom-left: element extends up from anchor
            (px as f32, py as f32 - eh)
        }
        Orientation::LowerRight => {
            // Anchored bottom-right: element extends left and up
            (px as f32 - ew, py as f32 - eh)
        }
        Orientation::Center => {
            // Center anchor: offset from center, element centered on that point
            (px as f32 - ew / 2.0, py as f32 - eh / 2.0)
        }
        Orientation::CenterUp => (px as f32 - ew / 2.0, py as f32),
        Orientation::CenterDown => (px as f32 - ew / 2.0, py as f32 - eh),
    };

    (anchor_x + offset_x, anchor_y + offset_y)
}

/// Convert screen pixel position to clip space.
/// Top-left is (-1, 1), bottom-right is (1, -1).
pub fn to_clip_space(screen_pos: (f32, f32), screen_size: (u32, u32)) -> (f32, f32) {
    let (x, y) = screen_pos;
    let (w, h) = (screen_size.0 as f32, screen_size.1 as f32);

    let clip_x = (x / w) * 2.0 - 1.0;
    let clip_y = 1.0 - (y / h) * 2.0;

    (clip_x, clip_y)
}

/// Convert pixel size to clip space size.
pub fn size_to_clip_space(size: (u32, u32), screen_size: (u32, u32)) -> (f32, f32) {
    let (w, h) = (size.0 as f32, size.1 as f32);
    let (sw, sh) = (screen_size.0 as f32, screen_size.1 as f32);

    (w / sw * 2.0, h / sh * 2.0)
}

/// Resolve a child element's position within a parent.
///
/// Given the parent's screen position and size, plus the child's position
/// and orientation, returns the child's screen position (top-left corner).
pub fn resolve_child_position(
    parent_pos: (f32, f32),
    parent_size: (u32, u32),
    child_pos: (i32, i32),
    child_orientation: super::types::Orientation,
) -> (f32, f32) {
    use super::types::Orientation;

    let (px, py) = child_pos;
    let (pw, ph) = (parent_size.0 as f32, parent_size.1 as f32);

    match child_orientation {
        Orientation::UpperLeft => {
            // Position is offset from parent's top-left
            (parent_pos.0 + px as f32, parent_pos.1 + py as f32)
        }
        Orientation::UpperRight => {
            // Position is offset from parent's top-right
            // x is typically negative (left of right edge)
            (parent_pos.0 + pw + px as f32, parent_pos.1 + py as f32)
        }
        Orientation::LowerLeft => (parent_pos.0 + px as f32, parent_pos.1 + ph + py as f32),
        Orientation::LowerRight => (parent_pos.0 + pw + px as f32, parent_pos.1 + ph + py as f32),
        Orientation::Center => (
            parent_pos.0 + pw / 2.0 + px as f32,
            parent_pos.1 + ph / 2.0 + py as f32,
        ),
        Orientation::CenterUp => (
            parent_pos.0 + pw / 2.0 + px as f32,
            parent_pos.1 + py as f32,
        ),
        Orientation::CenterDown => (
            parent_pos.0 + pw / 2.0 + px as f32,
            parent_pos.1 + ph + py as f32,
        ),
    }
}

/// Convert a screen rectangle to clip space.
/// Returns (x, y, width, height) in clip space.
pub fn rect_to_clip_space(
    screen_pos: (f32, f32),
    size: (u32, u32),
    screen_size: (u32, u32),
) -> (f32, f32, f32, f32) {
    let (clip_x, clip_y) = to_clip_space(screen_pos, screen_size);
    let (clip_w, clip_h) = size_to_clip_space(size, screen_size);

    (clip_x, clip_y, clip_w, clip_h)
}

/// Compute the masked flag rectangle within an overlay frame.
///
/// The mask is typically smaller than the overlay frame and centered within it.
/// This function calculates the scaled and centered position for the flag texture
/// so it aligns with the mask area.
///
/// # Arguments
/// * `overlay_rect` - The clip-space rectangle (x, y, w, h) of the full overlay
/// * `mask_size` - The pixel dimensions (width, height) of the mask texture
/// * `overlay_size` - The pixel dimensions (width, height) of the overlay texture
///
/// # Returns
/// The clip-space rectangle (x, y, w, h) for the flag, scaled and centered within the overlay.
pub fn compute_masked_flag_rect(
    overlay_rect: (f32, f32, f32, f32),
    mask_size: (u32, u32),
    overlay_size: (u32, u32),
) -> (f32, f32, f32, f32) {
    let (clip_x, clip_y, clip_w, clip_h) = overlay_rect;
    let (mask_w, mask_h) = mask_size;
    let (overlay_w, overlay_h) = overlay_size;

    // Scale factors: how much smaller the mask is compared to overlay
    let scale_x = mask_w as f32 / overlay_w as f32;
    let scale_y = mask_h as f32 / overlay_h as f32;

    // Offset to center the flag within the overlay
    let offset_x = (1.0 - scale_x) / 2.0;
    let offset_y = (1.0 - scale_y) / 2.0;

    // Compute final flag dimensions and position
    let flag_w = clip_w * scale_x;
    let flag_h = clip_h * scale_y;
    let flag_x = clip_x + clip_w * offset_x;
    let flag_y = clip_y - clip_h * offset_y; // Y is inverted in clip space

    (flag_x, flag_y, flag_w, flag_h)
}

#[cfg(test)]
mod tests {
    use super::*;

    const SCREEN: (u32, u32) = (1920, 1080);

    // ========== get_window_anchor tests ==========

    #[test]
    fn test_window_anchor_upper_left() {
        let anchor = get_window_anchor((10, 20), Orientation::UpperLeft, SCREEN);
        assert!((anchor.0 - 10.0).abs() < 0.001);
        assert!((anchor.1 - 20.0).abs() < 0.001);
    }

    #[test]
    fn test_window_anchor_upper_right() {
        // EU4 speed_controls uses (0, 0) with UPPER_RIGHT
        let anchor = get_window_anchor((0, 0), Orientation::UpperRight, SCREEN);
        assert!((anchor.0 - 1920.0).abs() < 0.001);
        assert!((anchor.1 - 0.0).abs() < 0.001);
    }

    #[test]
    fn test_window_anchor_lower_right() {
        let anchor = get_window_anchor((-10, -20), Orientation::LowerRight, SCREEN);
        assert!((anchor.0 - 1910.0).abs() < 0.001);
        assert!((anchor.1 - 1060.0).abs() < 0.001);
    }

    #[test]
    fn test_window_anchor_center() {
        let anchor = get_window_anchor((0, 0), Orientation::Center, SCREEN);
        assert!((anchor.0 - 960.0).abs() < 0.001);
        assert!((anchor.1 - 540.0).abs() < 0.001);
    }

    // ========== position_from_anchor tests ==========

    #[test]
    fn test_position_from_anchor_upper_left() {
        let anchor = (1920.0, 0.0); // Upper right corner
        let pos = position_from_anchor(anchor, (-254, -1), Orientation::UpperLeft, (257, 170));
        // Simple offset from anchor
        assert!((pos.0 - 1666.0).abs() < 0.001); // 1920 - 254
        assert!((pos.1 - (-1.0)).abs() < 0.001);
    }

    #[test]
    fn test_position_from_anchor_upper_right() {
        // DateText: pos=(-227, 13), orientation=UPPER_RIGHT, size=(140, 32)
        let anchor = (1920.0, 0.0);
        let pos = position_from_anchor(anchor, (-227, 13), Orientation::UpperRight, (140, 32));
        assert!((pos.0 - 1693.0).abs() < 0.001); // 1920 - 227
        assert!((pos.1 - 13.0).abs() < 0.001);
    }

    #[test]
    fn test_position_from_anchor_center() {
        let anchor = (960.0, 540.0);
        let pos = position_from_anchor(anchor, (0, 0), Orientation::Center, (100, 50));
        // Element centered on anchor
        assert!((pos.0 - 910.0).abs() < 0.001); // 960 - 50
        assert!((pos.1 - 540.0).abs() < 0.001);
    }

    // ========== resolve_position tests ==========

    #[test]
    fn test_resolve_position_upper_left() {
        let pos = resolve_position((10, 20), Orientation::UpperLeft, (100, 50), SCREEN);
        assert!((pos.0 - 10.0).abs() < 0.001);
        assert!((pos.1 - 20.0).abs() < 0.001);
    }

    #[test]
    fn test_resolve_position_upper_right() {
        // Position relative to top-right corner
        // x=-10 means 10 pixels from right edge
        let pos = resolve_position((-10, 20), Orientation::UpperRight, (100, 50), SCREEN);
        // anchor_x = 1920, offset_x = -10 - 100 = -110
        // result = 1920 - 110 = 1810
        assert!((pos.0 - 1810.0).abs() < 0.001);
        assert!((pos.1 - 20.0).abs() < 0.001);
    }

    #[test]
    fn test_resolve_position_lower_left() {
        let pos = resolve_position((10, -20), Orientation::LowerLeft, (100, 50), SCREEN);
        // anchor_y = 1080, offset_y = -20 - 50 = -70
        assert!((pos.0 - 10.0).abs() < 0.001);
        assert!((pos.1 - 1010.0).abs() < 0.001); // 1080 - 70
    }

    #[test]
    fn test_resolve_position_lower_right() {
        let pos = resolve_position((-10, -20), Orientation::LowerRight, (100, 50), SCREEN);
        assert!((pos.0 - 1810.0).abs() < 0.001); // 1920 - 10 - 100
        assert!((pos.1 - 1010.0).abs() < 0.001); // 1080 - 20 - 50
    }

    // ========== resolve_child_position tests ==========

    #[test]
    fn test_child_position_upper_left() {
        let parent_pos = (100.0, 100.0);
        let parent_size = (200, 150);
        let pos = resolve_child_position(parent_pos, parent_size, (10, 20), Orientation::UpperLeft);
        assert!((pos.0 - 110.0).abs() < 0.001);
        assert!((pos.1 - 120.0).abs() < 0.001);
    }

    #[test]
    fn test_child_position_upper_right() {
        let parent_pos = (100.0, 100.0);
        let parent_size = (200, 150);
        // Child at (-10, 20) from parent's right edge
        let pos =
            resolve_child_position(parent_pos, parent_size, (-10, 20), Orientation::UpperRight);
        assert!((pos.0 - 290.0).abs() < 0.001); // 100 + 200 - 10
        assert!((pos.1 - 120.0).abs() < 0.001);
    }

    #[test]
    fn test_child_position_center() {
        let parent_pos = (100.0, 100.0);
        let parent_size = (200, 150);
        let pos = resolve_child_position(parent_pos, parent_size, (0, 0), Orientation::Center);
        assert!((pos.0 - 200.0).abs() < 0.001); // 100 + 100
        assert!((pos.1 - 175.0).abs() < 0.001); // 100 + 75
    }

    // ========== Clip space conversion tests ==========

    #[test]
    fn test_to_clip_space_corners() {
        // Top-left corner
        let (x, y) = to_clip_space((0.0, 0.0), SCREEN);
        assert!((x - (-1.0)).abs() < 0.001);
        assert!((y - 1.0).abs() < 0.001);

        // Center
        let (x, y) = to_clip_space((960.0, 540.0), SCREEN);
        assert!(x.abs() < 0.001);
        assert!(y.abs() < 0.001);

        // Bottom-right
        let (x, y) = to_clip_space((1920.0, 1080.0), SCREEN);
        assert!((x - 1.0).abs() < 0.001);
        assert!((y - (-1.0)).abs() < 0.001);
    }

    #[test]
    fn test_size_to_clip_space() {
        // 100x50 pixels on 1920x1080 screen
        let (w, h) = size_to_clip_space((100, 50), SCREEN);
        // 100/1920 * 2 = 0.1042
        // 50/1080 * 2 = 0.0926
        assert!((w - 0.10417).abs() < 0.001);
        assert!((h - 0.09259).abs() < 0.001);
    }

    #[test]
    fn test_rect_to_clip_space() {
        // A 100x50 rect at position (960, 540) (center of screen)
        let (x, y, w, h) = rect_to_clip_space((960.0, 540.0), (100, 50), SCREEN);
        assert!(x.abs() < 0.001); // Center X
        assert!(y.abs() < 0.001); // Center Y
        assert!((w - 0.10417).abs() < 0.001);
        assert!((h - 0.09259).abs() < 0.001);
    }

    // ========== EU4-specific scenario tests ==========

    #[test]
    fn test_eu4_speed_controls_layout() {
        // Simulate the actual EU4 speed_controls window layout
        // Window: pos=(0,0), orientation=UPPER_RIGHT
        let window_anchor = get_window_anchor((0, 0), Orientation::UpperRight, SCREEN);
        assert!((window_anchor.0 - 1920.0).abs() < 0.001);
        assert!((window_anchor.1 - 0.0).abs() < 0.001);

        // DateText: pos=(-227, 13), orientation=UPPER_RIGHT, size=(140, 32)
        let date_pos = position_from_anchor(
            window_anchor,
            (-227, 13),
            Orientation::UpperRight,
            (140, 32),
        );
        // Should be near right edge of screen
        assert!(date_pos.0 > 1600.0);
        assert!(date_pos.0 < 1920.0);
        assert!(date_pos.1 > 0.0);
        assert!(date_pos.1 < 50.0);
    }

    #[test]
    fn test_eu4_date_bg_layout() {
        // icon_date_bg: pos=(-254, -1), orientation=UPPER_RIGHT, size=(257, 170)
        let window_anchor = get_window_anchor((0, 0), Orientation::UpperRight, SCREEN);
        let bg_pos = position_from_anchor(
            window_anchor,
            (-254, -1),
            Orientation::UpperRight,
            (257, 170),
        );

        // Background should be positioned near top-right
        assert!((bg_pos.0 - 1666.0).abs() < 0.001); // 1920 - 254
        assert!((bg_pos.1 - (-1.0)).abs() < 0.001);
    }

    // ========== Masked flag rect tests ==========

    #[test]
    fn test_masked_flag_rect_same_size() {
        // When mask and overlay are the same size, flag should match overlay exactly
        let overlay_rect = (-0.5, 0.5, 0.2, 0.2);
        let (x, y, w, h) = compute_masked_flag_rect(overlay_rect, (100, 100), (100, 100));
        assert!((x - (-0.5)).abs() < 0.001);
        assert!((y - 0.5).abs() < 0.001);
        assert!((w - 0.2).abs() < 0.001);
        assert!((h - 0.2).abs() < 0.001);
    }

    #[test]
    fn test_masked_flag_rect_smaller_mask() {
        // EU4 shield: mask is 92x92, overlay is 152x152
        // Scale factor = 92/152 ≈ 0.605
        let overlay_rect = (0.0, 0.0, 1.0, 1.0);
        let (x, y, w, h) = compute_masked_flag_rect(overlay_rect, (92, 92), (152, 152));

        let expected_scale = 92.0 / 152.0;
        let expected_offset = (1.0 - expected_scale) / 2.0;

        assert!((w - expected_scale).abs() < 0.001);
        assert!((h - expected_scale).abs() < 0.001);
        assert!((x - expected_offset).abs() < 0.001);
        // Y offset is negative due to clip space inversion
        assert!((y - (-expected_offset)).abs() < 0.001);
    }

    #[test]
    fn test_masked_flag_rect_non_square() {
        // Test with non-square dimensions
        let overlay_rect = (0.0, 0.0, 0.4, 0.3);
        let (x, y, w, h) = compute_masked_flag_rect(overlay_rect, (80, 60), (100, 100));

        // scale_x = 80/100 = 0.8, scale_y = 60/100 = 0.6
        let scale_x = 0.8;
        let scale_y = 0.6;
        let offset_x = (1.0 - scale_x) / 2.0; // 0.1
        let offset_y = (1.0 - scale_y) / 2.0; // 0.2

        assert!((w - (0.4 * scale_x)).abs() < 0.001);
        assert!((h - (0.3 * scale_y)).abs() < 0.001);
        assert!((x - (0.0 + 0.4 * offset_x)).abs() < 0.001);
        assert!((y - (0.0 - 0.3 * offset_y)).abs() < 0.001);
    }

    #[test]
    fn test_masked_flag_rect_centered() {
        // Flag should be centered within overlay
        // With overlay at center of screen (0,0) in clip space
        let overlay_rect = (0.0, 0.0, 0.5, 0.5);
        let (x, y, w, h) = compute_masked_flag_rect(overlay_rect, (50, 50), (100, 100));

        // Scale = 0.5, offset = 0.25
        // Flag should be at (0.125, -0.125) with size (0.25, 0.25)
        assert!((x - 0.125).abs() < 0.001);
        assert!((y - (-0.125)).abs() < 0.001);
        assert!((w - 0.25).abs() < 0.001);
        assert!((h - 0.25).abs() < 0.001);
    }
}
