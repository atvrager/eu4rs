#[derive(Debug, Clone)]
pub struct Camera {
    /// Position of the center of the camera in World Space (Texture Coordinates 0.0 - 1.0)
    pub position: (f64, f64),
    /// Zoom level. 1.0 = 1:1 view (fitting width). > 1.0 = Zoomed In.
    pub zoom: f64,
    /// Aspect ratio of the world content (texture width / height)
    pub content_aspect: f64,
}

impl Camera {
    pub fn new(content_aspect: f64) -> Self {
        Self {
            position: (0.5, 0.5), // Center of the map
            zoom: 1.0,
            content_aspect,
        }
    }

    /// Pans the camera by a delta in screen pixels.
    pub fn pan(&mut self, dx: f64, dy: f64, screen_width: f64, screen_height: f64) {
        // We need to convert pixel delta to texture space delta.
        // The width of the screen corresponds to (1.0 / zoom) in texture space.

        if screen_width == 0.0 || screen_height == 0.0 {
            return;
        }

        // We want the zoom to be consistent horizontally.
        // Horizontal view size in Texture Space = 1.0 / zoom.
        let view_width_tex = 1.0 / self.zoom;

        let screen_aspect = screen_width / screen_height;
        let view_height_tex = view_width_tex * self.content_aspect / screen_aspect;

        let dx_tex = dx / screen_width * view_width_tex;
        let dy_tex = dy / screen_height * view_height_tex;

        // Panning direction: Dragging mouse RIGHT (positive dx) should move camera LEFT (negative x)
        // so we subtract dx.
        self.position.0 -= dx_tex;
        self.position.1 -= dy_tex;

        // Wrap X
        self.position.0 = self.position.0.rem_euclid(1.0);

        // Clamp Y (Dynamic based on View Height)
        // We want the Visible Top Edge >= -0.05
        // And Visible Bottom Edge <= 1.05
        // Visible Top = CenterY - ViewHeight/2
        // Visible Bottom = CenterY + ViewHeight/2

        let half_view_h = view_height_tex / 2.0;
        let min_center_y = -0.05 + half_view_h;
        let max_center_y = 1.05 - half_view_h;

        // If view is larger than allowed area (zoomed way out), we clamp to center?
        // Or we enforce max zoom out elsewhere.
        // For now, simple clamp. If min > max, average them.
        if min_center_y > max_center_y {
            self.position.1 = 0.5;
        } else {
            self.position.1 = self.position.1.clamp(min_center_y, max_center_y);
        }
    }

    /// Zooms the camera towards a specific screen pixel.
    pub fn zoom(
        &mut self,
        factor: f64,
        pivot_x: f64,
        pivot_y: f64,
        screen_width: f64,
        screen_height: f64,
    ) {
        // Zoom logic:
        // 1. Calculate World Pos of pivot before zoom.
        // 2. Apply zoom.
        // 3. Calculate World Pos of pivot after zoom (if we didn't move camera).
        // 4. Move camera to align new pivot world pos with old pivot world pos.

        if screen_width == 0.0 || screen_height == 0.0 {
            return;
        }

        // Enforce Min/Max Zoom
        // Max Zoom Out: View fits map + 5% border?
        // ViewHeight <= 1.10 (0.05 top + 1.0 + 0.05 bot)
        // ViewHeight = (1.0/zoom) * content_aspect / screen_aspect
        // 1.0/zoom <= 1.10 * screen_aspect / content_aspect
        // zoom >= (1.0 / 1.10) * (content_aspect / screen_aspect)
        // This makes min zoom dependent on window aspect.

        let screen_aspect = screen_width / screen_height;
        let min_zoom = (1.0 / 1.10) * (self.content_aspect / screen_aspect);
        let max_zoom = 50.0; // Arbitrary high zoom in

        let old_world_pos = self.screen_to_world(pivot_x, pivot_y, screen_width, screen_height);

        let new_zoom = (self.zoom * factor).clamp(min_zoom, max_zoom);

        // If we hit a limit, we update self.zoom and re-calculate position to maintain pivot if possible?
        // Actually, if we clamp zoom, the pivot math still holds, we just use the clamped value.
        self.zoom = new_zoom;

        // Re-calcs for positional correction
        let view_width_tex = 1.0 / self.zoom;
        let view_height_tex = view_width_tex * self.content_aspect / screen_aspect;

        // The offset from center in UV space
        // Center is 0.5 screen relative.
        let center_offset_u_screen = (pivot_x / screen_width) - 0.5;
        let center_offset_v_screen = (pivot_y / screen_height) - 0.5;

        // Convert screen offset to world offset
        let offset_x = center_offset_u_screen * view_width_tex;
        let offset_y = center_offset_v_screen * view_height_tex;

        self.position.0 = old_world_pos.0 - offset_x;
        self.position.1 = old_world_pos.1 - offset_y;

        self.position.0 = self.position.0.rem_euclid(1.0);

        // Clamp Y in zoom too, in case zoom out pushed us over
        let half_view_h = view_height_tex / 2.0;
        let min_center_y = -0.05 + half_view_h;
        let max_center_y = 1.05 - half_view_h;

        if min_center_y > max_center_y {
            self.position.1 = 0.5;
        } else {
            self.position.1 = self.position.1.clamp(min_center_y, max_center_y);
        }
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

        // Camera position is the center (0.5, 0.5 of screen).
        // World = CamPos + (ScreenUV - 0.5) * ViewSize

        let world_x = self.position.0 + (u_screen - 0.5) * view_width_tex;
        let world_y = self.position.1 + (v_screen - 0.5) * view_height_tex;

        (world_x.rem_euclid(1.0), world_y)
    }

    /// Returns the raw data for a Uniform Buffer.
    /// Format: [pos_x, pos_y, inv_zoom_x, inv_zoom_y]
    /// We pass 1.0/zoom corrected for aspect ratio to simplify shader math.
    pub fn to_uniform_data(&self, screen_width: f32, screen_height: f32) -> [f32; 4] {
        if screen_width == 0.0 || screen_height == 0.0 {
            return [0.0; 4];
        }
        let screen_aspect = screen_width / screen_height;
        // inv_zoom_x implies the width of the view in texture coords.
        let view_width_tex = 1.0 / self.zoom as f32;
        let view_height_tex = view_width_tex * self.content_aspect as f32 / screen_aspect;

        [
            self.position.0 as f32,
            self.position.1 as f32,
            view_width_tex,
            view_height_tex,
        ]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_screen_to_world_center() {
        let cam = Camera::new(1.0); // Square content
        let (wx, wy) = cam.screen_to_world(500.0, 500.0, 1000.0, 1000.0);
        assert!((wx - 0.5).abs() < 1e-6);
        assert!((wy - 0.5).abs() < 1e-6);
    }

    #[test]
    fn test_aspect_ratio() {
        // Content is 2:1 (e.g. 2000x1000)
        // Screen is 1:1 (e.g. 1000x1000)
        // Formula: ViewHeightTex = ViewWidthTex * content_aspect / screen_aspect
        // 1.0 * 2.0 / 1.0 = 2.0. Correct.

        let cam = Camera::new(2.0);
        let (_, wy_top) = cam.screen_to_world(500.0, 0.0, 1000.0, 1000.0);
        let (_, wy_bot) = cam.screen_to_world(500.0, 1000.0, 1000.0, 1000.0);

        let height_tex = wy_bot - wy_top;
        assert!((height_tex - 2.0).abs() < 1e-6);
    }

    #[test]
    fn test_zoom_in() {
        let mut cam = Camera::new(1.0);
        // Zoom in to 2x
        cam.zoom(2.0, 500.0, 500.0, 1000.0, 1000.0);

        // Center should still be 0.5, 0.5
        let (wx, _wy) = cam.screen_to_world(500.0, 500.0, 1000.0, 1000.0);
        assert!((wx - 0.5).abs() < 1e-6);
    }

    #[test]
    fn test_zoom_pivot() {
        let mut cam = Camera::new(1.0);
        // Point at (250, 500) which is 0.25 on X screen => 0.5 + (0.25 - 0.5) * 1.0 = 0.25 world X
        let px = 250.0;
        let py = 500.0;
        let (wx1, _) = cam.screen_to_world(px, py, 1000.0, 1000.0);
        assert!((wx1 - 0.25).abs() < 1e-6);

        // Zoom in x2 at that pivot
        cam.zoom(2.0, px, py, 1000.0, 1000.0);

        // The world coordinate under the cursor should STILL be 0.25
        let (wx2, _) = cam.screen_to_world(px, py, 1000.0, 1000.0);
        assert!(
            (wx2 - 0.25).abs() < 1e-6,
            "World X expected 0.25, got {}",
            wx2
        );

        // Since we zoomed in, the center of the camera must have moved left relative to world to keep 0.25 at the left quarter of screen.
        // New view width is 0.5.
        // 0.25 screen is -0.125 from center in view space.
        // Center = 0.25 - (-0.125) = 0.375.
        // Let's check cam pos
        assert!(
            (cam.position.0 - 0.375).abs() < 1e-6,
            "Cam Pos expected 0.375, got {}",
            cam.position.0
        );
    }

    #[test]
    fn test_pan_wrapping() {
        let mut cam = Camera::new(1.0);
        // Drag 1000 pixels right (full screen width).
        // Should move camera left by 1.0 world unit.
        // 0.5 - 1.0 = -0.5 => Wrap => 0.5.
        cam.pan(1000.0, 0.0, 1000.0, 1000.0);
        assert!((cam.position.0 - 0.5).abs() < 1e-6);

        // Drag 500 pixels right.
        // 0.5 - 0.5 = 0.0.
        cam.pan(500.0, 0.0, 1000.0, 1000.0);
        // Might be 0.0 or 1.0 (both valid for wrapping conceptually, but rem_euclid is [0, div)).
        assert!((cam.position.0 - 0.0).abs() < 1e-6);
    }
}
