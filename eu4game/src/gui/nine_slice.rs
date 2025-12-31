//! 9-slice (cornered) sprite geometry generation.

/// A single axis-aligned quad with UVs.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct Quad {
    pub pos: [f32; 2],
    pub size: [f32; 2],
    pub uv_pos: [f32; 2],
    pub uv_size: [f32; 2],
}

/// The result of a 9-slice geometry generation.
#[derive(Debug, Clone, PartialEq)]
pub enum NineSliceResult {
    /// Normal 9-slice with all 9 quads.
    Full(Box<[Quad; 9]>),
    /// Fallback for degenerate cases (size < 2*border).
    Fallback(Quad),
}

/// Generates 9 quads for a cornered tile sprite.
///
/// Quads are returned in row-major order:
/// [TL, Top, TR,
///  L,  C,   R,
///  BL, Bot, BR]
pub fn generate_9_slice_quads(
    pos: (f32, f32),
    size: (f32, f32),
    border: (f32, f32),
    texture_size: (u32, u32),
) -> NineSliceResult {
    let texture_size = (texture_size.0 as f32, texture_size.1 as f32);

    // Handle degenerate case: target too small for borders
    if size.0 < 2.0 * border.0 || size.1 < 2.0 * border.1 {
        return NineSliceResult::Fallback(Quad {
            pos: [pos.0, pos.1],
            size: [size.0, size.1],
            uv_pos: [0.0, 0.0],
            uv_size: [1.0, 1.0],
        });
    }

    // Calculate vertex x-coordinates
    let x = [
        pos.0,                     // left edge
        pos.0 + border.0,          // left border end
        pos.0 + size.0 - border.0, // right border start
        pos.0 + size.0,            // right edge
    ];

    // Calculate vertex y-coordinates
    let y = [
        pos.1,                     // top edge
        pos.1 + border.1,          // top border end
        pos.1 + size.1 - border.1, // bottom border start
        pos.1 + size.1,            // bottom edge
    ];

    // Calculate UV coordinates (normalized)
    let border_u = border.0 / texture_size.0;
    let border_v = border.1 / texture_size.1;
    let u = [0.0, border_u, 1.0 - border_u, 1.0];
    let v = [0.0, border_v, 1.0 - border_v, 1.0];

    // Generate 9 quads: row-major order (TL, T, TR, L, C, R, BL, B, BR)
    let mut quads = [Quad::default(); 9];
    for row in 0..3 {
        for col in 0..3 {
            let idx = row * 3 + col;
            quads[idx] = Quad {
                pos: [x[col], y[row]],
                size: [x[col + 1] - x[col], y[row + 1] - y[row]],
                uv_pos: [u[col], v[row]],
                uv_size: [u[col + 1] - u[col], v[row + 1] - v[row]],
            };
        }
    }

    NineSliceResult::Full(Box::new(quads))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_degenerate_falls_back() {
        let result = generate_9_slice_quads(
            (0.0, 0.0),
            (10.0, 10.0), // size
            (20.0, 20.0), // border bigger than size!
            (64, 64),
        );
        assert!(matches!(result, NineSliceResult::Fallback(_)));
    }

    #[test]
    fn test_zero_border_single_quad() {
        let result = generate_9_slice_quads(
            (0.0, 0.0),
            (100.0, 100.0),
            (0.0, 0.0), // zero border
            (64, 64),
        );
        // With zero border, all quads end up being correct but some have 0 size
        if let NineSliceResult::Full(quads) = result {
            // Center quad (index 4) should be full size
            assert_eq!(quads[4].size, [100.0, 100.0]);
            assert_eq!(quads[4].uv_size, [1.0, 1.0]);

            // Corner quads should have zero size
            assert_eq!(quads[0].size, [0.0, 0.0]);
        }
    }

    #[test]
    fn test_normal_9_slice() {
        let result = generate_9_slice_quads(
            (10.0, 10.0),   // pos
            (100.0, 100.0), // size
            (10.0, 10.0),   // border
            (100, 100),     // texture size
        );

        if let NineSliceResult::Full(ref quads) = result {
            let assert_approx = |a: [f32; 2], b: [f32; 2]| {
                assert!((a[0] - b[0]).abs() < 1e-6, "left: {:?}, right: {:?}", a, b);
                assert!((a[1] - b[1]).abs() < 1e-6, "left: {:?}, right: {:?}", a, b);
            };

            // TL
            assert_approx(quads[0].pos, [10.0, 10.0]);
            assert_approx(quads[0].size, [10.0, 10.0]);
            assert_approx(quads[0].uv_pos, [0.0, 0.0]);
            assert_approx(quads[0].uv_size, [0.1, 0.1]);

            // Center
            assert_approx(quads[4].pos, [20.0, 20.0]);
            assert_approx(quads[4].size, [80.0, 80.0]);
            assert_approx(quads[4].uv_pos, [0.1, 0.1]);
            assert_approx(quads[4].uv_size, [0.8, 0.8]);

            // BR
            assert_approx(quads[8].pos, [100.0, 100.0]);
            assert_approx(quads[8].size, [10.0, 10.0]);
            assert_approx(quads[8].uv_pos, [0.9, 0.9]);
            assert_approx(quads[8].uv_size, [0.1, 0.1]);
        } else {
            panic!("Expected Full 9-slice");
        }
    }
}
