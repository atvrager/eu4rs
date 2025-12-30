//! Camera system for pan/zoom over the world map.
//!
//! Operates in texture space (0.0-1.0) with X-axis wrapping
//! for seamless world navigation.

/// Camera state for viewing the world map.
#[derive(Debug, Clone)]
pub struct Camera {
    /// Position of the camera center in texture coordinates (0.0-1.0).
    pub position: (f64, f64),
    /// Zoom level. 1.0 = fit width, >1.0 = zoomed in.
    pub zoom: f64,
    /// Aspect ratio of the content (width / height).
    pub content_aspect: f64,
}

impl Camera {
    /// Creates a new camera centered on the map.
    pub fn new(content_aspect: f64) -> Self {
        Self {
            position: (0.5, 0.5),
            zoom: 1.0,
            content_aspect,
        }
    }

    /// Pans the camera by screen pixel deltas.
    pub fn pan(&mut self, dx: f64, dy: f64, screen_width: f64, screen_height: f64) {
        if screen_width == 0.0 || screen_height == 0.0 {
            return;
        }

        let view_width_tex = 1.0 / self.zoom;
        let screen_aspect = screen_width / screen_height;
        let view_height_tex = view_width_tex * self.content_aspect / screen_aspect;

        let dx_tex = dx / screen_width * view_width_tex;
        let dy_tex = dy / screen_height * view_height_tex;

        // Dragging right moves camera left (subtracts)
        self.position.0 -= dx_tex;
        self.position.1 -= dy_tex;

        // Wrap X axis
        self.position.0 = self.position.0.rem_euclid(1.0);

        // Clamp Y to keep map visible
        self.clamp_y(view_height_tex);
    }

    /// Zooms the camera towards a screen pixel pivot point.
    pub fn zoom(
        &mut self,
        factor: f64,
        pivot_x: f64,
        pivot_y: f64,
        screen_width: f64,
        screen_height: f64,
    ) {
        if screen_width == 0.0 || screen_height == 0.0 {
            return;
        }

        let screen_aspect = screen_width / screen_height;
        let min_zoom = (1.0 / 1.10) * (self.content_aspect / screen_aspect);
        let max_zoom = 50.0;

        // Get world position under cursor before zoom
        let old_world_pos = self.screen_to_world(pivot_x, pivot_y, screen_width, screen_height);

        // Apply zoom with clamping
        self.zoom = (self.zoom * factor).clamp(min_zoom, max_zoom);

        // Recalculate view dimensions
        let view_width_tex = 1.0 / self.zoom;
        let view_height_tex = view_width_tex * self.content_aspect / screen_aspect;

        // Calculate offset from center in screen space
        let center_offset_u = (pivot_x / screen_width) - 0.5;
        let center_offset_v = (pivot_y / screen_height) - 0.5;

        // Convert to world space offset
        let offset_x = center_offset_u * view_width_tex;
        let offset_y = center_offset_v * view_height_tex;

        // Position camera so the pivot world pos stays under cursor
        self.position.0 = old_world_pos.0 - offset_x;
        self.position.1 = old_world_pos.1 - offset_y;

        // Wrap and clamp
        self.position.0 = self.position.0.rem_euclid(1.0);
        self.clamp_y(view_height_tex);
    }

    /// Converts screen coordinates to world (texture) coordinates.
    pub fn screen_to_world(
        &self,
        x: f64,
        y: f64,
        screen_width: f64,
        screen_height: f64,
    ) -> (f64, f64) {
        if screen_width == 0.0 || screen_height == 0.0 {
            return (0.0, 0.0);
        }

        let view_width_tex = 1.0 / self.zoom;
        let screen_aspect = screen_width / screen_height;
        let view_height_tex = view_width_tex * self.content_aspect / screen_aspect;

        let u_screen = x / screen_width;
        let v_screen = y / screen_height;

        let world_x = self.position.0 + (u_screen - 0.5) * view_width_tex;
        let world_y = self.position.1 + (v_screen - 0.5) * view_height_tex;

        (world_x.rem_euclid(1.0), world_y)
    }

    /// Generates uniform data for the shader.
    pub fn to_uniform(&self, screen_width: f32, screen_height: f32) -> CameraUniform {
        if screen_width == 0.0 || screen_height == 0.0 {
            return CameraUniform::default();
        }

        let screen_aspect = screen_width / screen_height;
        let view_width_tex = 1.0 / self.zoom as f32;
        let view_height_tex = view_width_tex * self.content_aspect as f32 / screen_aspect;

        CameraUniform {
            pos: [self.position.0 as f32, self.position.1 as f32],
            zoom: [view_width_tex, view_height_tex],
        }
    }

    /// Clamps Y position to keep the map visible.
    fn clamp_y(&mut self, view_height_tex: f64) {
        let half_view_h = view_height_tex / 2.0;
        let min_center_y = -0.05 + half_view_h;
        let max_center_y = 1.05 - half_view_h;

        if min_center_y > max_center_y {
            self.position.1 = 0.5;
        } else {
            self.position.1 = self.position.1.clamp(min_center_y, max_center_y);
        }
    }
}

/// Uniform data for the camera shader.
#[repr(C)]
#[derive(Debug, Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
pub struct CameraUniform {
    pub pos: [f32; 2],
    pub zoom: [f32; 2],
}

impl Default for CameraUniform {
    fn default() -> Self {
        Self {
            pos: [0.5, 0.5],
            zoom: [1.0, 1.0],
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SCREEN_W: f64 = 1920.0;
    const SCREEN_H: f64 = 1080.0;
    // EU4 map aspect ratio (5632 / 2048)
    const CONTENT_ASPECT: f64 = 2.75;

    #[test]
    fn test_new_camera_centered() {
        let cam = Camera::new(CONTENT_ASPECT);
        assert_eq!(cam.position, (0.5, 0.5));
        assert_eq!(cam.zoom, 1.0);
    }

    #[test]
    fn test_screen_to_world_center() {
        let cam = Camera::new(CONTENT_ASPECT);
        let (wx, wy) = cam.screen_to_world(SCREEN_W / 2.0, SCREEN_H / 2.0, SCREEN_W, SCREEN_H);
        assert!(
            (wx - 0.5).abs() < 1e-6,
            "Center X should be 0.5, got {}",
            wx
        );
        assert!(
            (wy - 0.5).abs() < 1e-6,
            "Center Y should be 0.5, got {}",
            wy
        );
    }

    #[test]
    fn test_screen_to_world_corners() {
        let cam = Camera::new(CONTENT_ASPECT);
        // Top-left corner should be left of center (lower X, accounting for wrap)
        let (wx_left, _) = cam.screen_to_world(0.0, SCREEN_H / 2.0, SCREEN_W, SCREEN_H);
        // Right edge should be right of center (higher X, but may wrap to 0)
        let (wx_right, _) = cam.screen_to_world(SCREEN_W, SCREEN_H / 2.0, SCREEN_W, SCREEN_H);

        // The difference should span the view width
        // With wrap, right might be < left if it wrapped
        let view_width = 1.0 / cam.zoom;
        let span = if wx_right > wx_left {
            wx_right - wx_left
        } else {
            // Wrapped: (1.0 - wx_left) + wx_right
            (1.0 - wx_left) + wx_right
        };
        assert!(
            (span - view_width).abs() < 1e-6,
            "View should span {}, got {} (left={}, right={})",
            view_width,
            span,
            wx_left,
            wx_right
        );
    }

    #[test]
    fn test_zoom_preserves_center_pivot() {
        let mut cam = Camera::new(CONTENT_ASPECT);
        let pivot_x = SCREEN_W / 2.0;
        let pivot_y = SCREEN_H / 2.0;

        let before = cam.screen_to_world(pivot_x, pivot_y, SCREEN_W, SCREEN_H);
        cam.zoom(2.0, pivot_x, pivot_y, SCREEN_W, SCREEN_H);
        let after = cam.screen_to_world(pivot_x, pivot_y, SCREEN_W, SCREEN_H);

        assert!(
            (before.0 - after.0).abs() < 1e-6,
            "Pivot X should stay same"
        );
        assert!(
            (before.1 - after.1).abs() < 1e-6,
            "Pivot Y should stay same"
        );
    }

    #[test]
    fn test_zoom_preserves_offset_pivot() {
        let mut cam = Camera::new(CONTENT_ASPECT);
        // Use an offset from center that won't hit Y clamp after zoom
        // Offset by 1/4 screen in X direction only (Y stays at center to avoid clamp)
        let pivot_x = SCREEN_W * 0.75;
        let pivot_y = SCREEN_H * 0.5;

        let before = cam.screen_to_world(pivot_x, pivot_y, SCREEN_W, SCREEN_H);
        cam.zoom(2.0, pivot_x, pivot_y, SCREEN_W, SCREEN_H);
        let after = cam.screen_to_world(pivot_x, pivot_y, SCREEN_W, SCREEN_H);

        // X may wrap but should be equivalent
        let x_diff = (before.0 - after.0).abs();
        let x_diff_wrapped = (1.0 - x_diff).abs();
        assert!(
            x_diff < 1e-6 || x_diff_wrapped < 1e-6,
            "Pivot X should stay same (before={}, after={})",
            before.0,
            after.0
        );
        assert!(
            (before.1 - after.1).abs() < 1e-6,
            "Pivot Y should stay same (before={}, after={})",
            before.1,
            after.1
        );
    }

    #[test]
    fn test_zoom_increases_zoom_level() {
        let mut cam = Camera::new(CONTENT_ASPECT);
        let initial_zoom = cam.zoom;
        cam.zoom(2.0, SCREEN_W / 2.0, SCREEN_H / 2.0, SCREEN_W, SCREEN_H);
        assert!(cam.zoom > initial_zoom, "Zoom should increase");
        assert!((cam.zoom - 2.0).abs() < 1e-6, "Zoom should be 2.0");
    }

    #[test]
    fn test_zoom_has_limits() {
        let mut cam = Camera::new(CONTENT_ASPECT);
        // Try to zoom out extremely
        for _ in 0..100 {
            cam.zoom(0.5, SCREEN_W / 2.0, SCREEN_H / 2.0, SCREEN_W, SCREEN_H);
        }
        assert!(cam.zoom > 0.1, "Zoom should have minimum limit");

        // Try to zoom in extremely
        for _ in 0..100 {
            cam.zoom(2.0, SCREEN_W / 2.0, SCREEN_H / 2.0, SCREEN_W, SCREEN_H);
        }
        assert!(cam.zoom <= 50.0, "Zoom should have maximum limit");
    }

    #[test]
    fn test_pan_moves_position() {
        let mut cam = Camera::new(CONTENT_ASPECT);
        let initial_pos = cam.position;
        // Drag right (positive dx) should move camera left (decrease x)
        cam.pan(100.0, 0.0, SCREEN_W, SCREEN_H);
        assert!(
            cam.position.0 < initial_pos.0 || cam.position.0 > 0.9,
            "Pan right should move view left (with wrap)"
        );
    }

    #[test]
    fn test_pan_wraps_x() {
        let mut cam = Camera::new(CONTENT_ASPECT);
        // Pan a full screen width multiple times
        for _ in 0..10 {
            cam.pan(SCREEN_W, 0.0, SCREEN_W, SCREEN_H);
        }
        // X should wrap to stay in 0..1
        assert!(
            cam.position.0 >= 0.0 && cam.position.0 < 1.0,
            "X should wrap to 0..1, got {}",
            cam.position.0
        );
    }

    #[test]
    fn test_pan_clamps_y() {
        let mut cam = Camera::new(CONTENT_ASPECT);
        // Pan up a lot
        for _ in 0..100 {
            cam.pan(0.0, -SCREEN_H, SCREEN_W, SCREEN_H);
        }
        // Y should be clamped, not wrap
        assert!(
            cam.position.1 >= -0.1 && cam.position.1 <= 1.1,
            "Y should be clamped near 0..1, got {}",
            cam.position.1
        );
    }

    #[test]
    fn test_uniform_generation() {
        let cam = Camera::new(CONTENT_ASPECT);
        let uniform = cam.to_uniform(SCREEN_W as f32, SCREEN_H as f32);

        assert_eq!(uniform.pos, [0.5, 0.5]);
        assert!(uniform.zoom[0] > 0.0, "Zoom X should be positive");
        assert!(uniform.zoom[1] > 0.0, "Zoom Y should be positive");
    }

    #[test]
    fn test_zero_screen_size_safety() {
        let cam = Camera::new(CONTENT_ASPECT);
        // Should not panic with zero dimensions
        let _ = cam.screen_to_world(100.0, 100.0, 0.0, 0.0);
        let uniform = cam.to_uniform(0.0, 0.0);
        assert_eq!(uniform.pos, [0.5, 0.5]); // Default
    }
}
