//! Unit tests for text.rs rendering functions.

use super::*;

// -------------------------------------------------------------------------
// Coordinate conversion tests
// -------------------------------------------------------------------------

#[test]
fn test_pixel_x_to_clip_left_edge() {
    // At x=0, clip space should be -1
    assert_eq!(pixel_x_to_clip(0.0, 800.0), -1.0);
}

#[test]
fn test_pixel_x_to_clip_right_edge() {
    // At x=screen_width, clip space should be 1
    assert_eq!(pixel_x_to_clip(800.0, 800.0), 1.0);
}

#[test]
fn test_pixel_x_to_clip_center() {
    // At x=screen_width/2, clip space should be 0
    assert_eq!(pixel_x_to_clip(400.0, 800.0), 0.0);
}

#[test]
fn test_pixel_y_to_clip_top_edge() {
    // At y=0 (top), clip space should be 1 (flipped)
    assert_eq!(pixel_y_to_clip(0.0, 600.0), 1.0);
}

#[test]
fn test_pixel_y_to_clip_bottom_edge() {
    // At y=screen_height (bottom), clip space should be -1
    assert_eq!(pixel_y_to_clip(600.0, 600.0), -1.0);
}

#[test]
fn test_pixel_y_to_clip_center() {
    // At y=screen_height/2, clip space should be 0
    assert_eq!(pixel_y_to_clip(300.0, 600.0), 0.0);
}

#[test]
fn test_width_to_clip_full_width() {
    // Full screen width = 2.0 in clip space
    assert_eq!(width_to_clip(800.0, 800.0), 2.0);
}

#[test]
fn test_width_to_clip_half_width() {
    // Half screen width = 1.0 in clip space
    assert_eq!(width_to_clip(400.0, 800.0), 1.0);
}

#[test]
fn test_height_to_clip_full_height() {
    // Full screen height = 2.0 in clip space
    assert_eq!(height_to_clip(600.0, 600.0), 2.0);
}

#[test]
fn test_height_to_clip_quarter_height() {
    // Quarter screen height = 0.5 in clip space
    assert_eq!(height_to_clip(150.0, 600.0), 0.5);
}

// -------------------------------------------------------------------------
// Text measurement tests
// -------------------------------------------------------------------------

#[test]
fn test_measure_text_width_empty() {
    let glyphs = HashMap::new();
    assert_eq!(measure_text_width("", &glyphs), 0.0);
}

#[test]
fn test_measure_text_width_single_char() {
    let mut glyphs = HashMap::new();
    glyphs.insert(
        'A',
        GlyphInfo {
            uv: [0.0, 0.0, 0.1, 0.1],
            size: [10.0, 20.0],
            advance: 12.0,
            bearing_y: 0.0,
            bearing_x: 0.0,
        },
    );
    assert_eq!(measure_text_width("A", &glyphs), 12.0);
}

#[test]
fn test_measure_text_width_multiple_chars() {
    let mut glyphs = HashMap::new();
    glyphs.insert(
        'H',
        GlyphInfo {
            uv: [0.0, 0.0, 0.1, 0.1],
            size: [10.0, 20.0],
            advance: 12.0,
            bearing_y: 0.0,
            bearing_x: 0.0,
        },
    );
    glyphs.insert(
        'i',
        GlyphInfo {
            uv: [0.0, 0.0, 0.1, 0.1],
            size: [4.0, 20.0],
            advance: 5.0,
            bearing_y: 0.0,
            bearing_x: 0.0,
        },
    );
    // "Hi" = 12 + 5 = 17
    assert_eq!(measure_text_width("Hi", &glyphs), 17.0);
}

#[test]
fn test_measure_text_width_missing_chars_skipped() {
    let mut glyphs = HashMap::new();
    glyphs.insert(
        'A',
        GlyphInfo {
            uv: [0.0, 0.0, 0.1, 0.1],
            size: [10.0, 20.0],
            advance: 10.0,
            bearing_y: 0.0,
            bearing_x: 0.0,
        },
    );
    // "ABC" but only 'A' is in glyphs - missing chars are skipped
    assert_eq!(measure_text_width("ABC", &glyphs), 10.0);
}

// -------------------------------------------------------------------------
// GlyphInfo tests
// -------------------------------------------------------------------------

#[test]
fn test_glyph_info_debug() {
    let glyph = GlyphInfo {
        uv: [0.0, 0.0, 0.1, 0.1],
        size: [10.0, 20.0],
        advance: 12.0,
        bearing_y: 2.0,
        bearing_x: 1.0,
    };
    let debug_str = format!("{:?}", glyph);
    assert!(debug_str.contains("GlyphInfo"));
    assert!(debug_str.contains("advance"));
}

#[test]
fn test_glyph_info_clone() {
    let glyph = GlyphInfo {
        uv: [0.0, 0.0, 0.1, 0.1],
        size: [10.0, 20.0],
        advance: 12.0,
        bearing_y: 2.0,
        bearing_x: 1.0,
    };
    let cloned = glyph;
    assert_eq!(cloned.advance, 12.0);
    assert_eq!(cloned.bearing_x, 1.0);
}

// -------------------------------------------------------------------------
// TextQuad tests
// -------------------------------------------------------------------------

#[test]
fn test_text_quad_size() {
    // TextQuad should be 48 bytes (2+2+2+2+4 floats = 12 floats * 4 bytes)
    assert_eq!(std::mem::size_of::<TextQuad>(), 48);
}

#[test]
fn test_text_quad_alignment() {
    // TextQuad should be properly aligned for GPU
    assert!(std::mem::align_of::<TextQuad>() >= 4);
}

#[test]
fn test_text_quad_creation() {
    let quad = TextQuad {
        pos: [-0.5, 0.5],
        size: [0.1, 0.2],
        uv_min: [0.0, 0.0],
        uv_max: [0.1, 0.1],
        color: [1.0, 1.0, 1.0, 1.0],
    };
    assert_eq!(quad.pos[0], -0.5);
    assert_eq!(quad.size[1], 0.2);
    assert_eq!(quad.color[3], 1.0);
}

// -------------------------------------------------------------------------
// Constants tests
// -------------------------------------------------------------------------

#[test]
fn test_atlas_size_is_power_of_two() {
    assert!(ATLAS_SIZE.is_power_of_two());
}

#[test]
fn test_default_font_size_reasonable() {
    // Font size should be in a sensible range for UI text
    let font_size = DEFAULT_FONT_SIZE;
    assert!(font_size >= 8.0, "Font size {} too small", font_size);
    assert!(font_size <= 72.0, "Font size {} too large", font_size);
}
