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

// ============================================================================
// 3D Camera for terrain rendering
// ============================================================================

use glam::{Mat4, Vec3};

/// 3D camera for terrain mesh rendering.
///
/// Uses a look-at model where the camera has a position and looks at a target
/// point on the terrain. Zooming moves the camera closer to/further from the
/// target rather than changing UV scale.
#[derive(Debug, Clone)]
pub struct Camera3D {
    /// Camera position in world space.
    pub position: Vec3,
    /// Point the camera is looking at.
    pub target: Vec3,
    /// Up vector (usually Y-up).
    pub up: Vec3,
    /// Vertical field of view in radians.
    pub fov_y: f32,
    /// Near clipping plane distance.
    pub near: f32,
    /// Far clipping plane distance.
    pub far: f32,
    /// Map width in world units (for bounds clamping).
    pub map_width: f32,
    /// Map height in world units (for bounds clamping).
    pub map_height: f32,
}

impl Camera3D {
    /// Default camera height above terrain.
    pub const DEFAULT_HEIGHT: f32 = 500.0;
    /// Minimum camera distance to target.
    pub const MIN_DISTANCE: f32 = 50.0;
    /// Maximum camera distance to target.
    pub const MAX_DISTANCE: f32 = 2000.0;

    /// Creates a new 3D camera looking down at the map center.
    ///
    /// The camera is positioned above the center of the map (0.5, 0.5 in UV
    /// coords, which maps to world coordinates based on map dimensions).
    pub fn new(map_width: f32, map_height: f32) -> Self {
        let center_x = map_width / 2.0;
        let center_z = map_height / 2.0;

        Self {
            position: Vec3::new(center_x, Self::DEFAULT_HEIGHT, center_z + 200.0),
            target: Vec3::new(center_x, 0.0, center_z),
            up: Vec3::Y,
            fov_y: std::f32::consts::FRAC_PI_4, // 45 degrees
            near: 1.0,
            far: 5000.0,
            map_width,
            map_height,
        }
    }

    /// Computes the view matrix (world to camera space).
    pub fn view_matrix(&self) -> Mat4 {
        Mat4::look_at_rh(self.position, self.target, self.up)
    }

    /// Computes the perspective projection matrix.
    pub fn projection_matrix(&self, aspect_ratio: f32) -> Mat4 {
        Mat4::perspective_rh(self.fov_y, aspect_ratio, self.near, self.far)
    }

    /// Computes the combined view-projection matrix.
    pub fn view_projection_matrix(&self, aspect_ratio: f32) -> Mat4 {
        self.projection_matrix(aspect_ratio) * self.view_matrix()
    }

    /// Returns the distance from camera to target.
    pub fn distance(&self) -> f32 {
        (self.position - self.target).length()
    }

    /// Zooms by moving the camera closer to or further from the target.
    ///
    /// `factor` > 1.0 zooms in (closer), < 1.0 zooms out (further).
    pub fn zoom(&mut self, factor: f32) {
        let direction = (self.position - self.target).normalize();
        let current_distance = self.distance();
        let new_distance =
            (current_distance / factor).clamp(Self::MIN_DISTANCE, Self::MAX_DISTANCE);

        self.position = self.target + direction * new_distance;
    }

    /// Pans the camera by moving both position and target.
    ///
    /// Movement is in world XZ plane (horizontal movement over terrain).
    /// X wraps around for seamless horizontal scrolling (Phase 7).
    /// Z is clamped to map bounds.
    pub fn pan(&mut self, dx: f32, dz: f32) {
        let offset = Vec3::new(dx, 0.0, dz);
        self.position += offset;
        self.target += offset;

        // Clamp Z to map bounds (with padding for camera angle)
        let z_padding = 100.0; // Allow some overshoot at edges
        let min_z = -z_padding;
        let max_z = self.map_height + z_padding;

        if self.target.z < min_z {
            let correction = min_z - self.target.z;
            self.target.z = min_z;
            self.position.z += correction;
        } else if self.target.z > max_z {
            let correction = max_z - self.target.z;
            self.target.z = max_z;
            self.position.z += correction;
        }

        // Wrap X for seamless horizontal scrolling (Phase 7)
        // Keep target.x in [0, map_width) range, adjust position to match
        if self.target.x < 0.0 {
            let wrap = self.map_width;
            self.target.x += wrap;
            self.position.x += wrap;
        } else if self.target.x >= self.map_width {
            let wrap = self.map_width;
            self.target.x -= wrap;
            self.position.x -= wrap;
        }
    }

    /// Generates uniform data for the terrain shader.
    pub fn to_uniform(&self, aspect_ratio: f32) -> CameraUniform3D {
        CameraUniform3D {
            view_proj: self.view_projection_matrix(aspect_ratio).to_cols_array_2d(),
        }
    }

    /// Returns the view frustum for culling (Phase 8).
    pub fn frustum(&self, aspect_ratio: f32) -> Frustum {
        Frustum::from_view_projection(self.view_projection_matrix(aspect_ratio))
    }

    /// Converts screen coordinates to terrain UV coordinates.
    ///
    /// Casts a ray from the camera through the screen point and intersects
    /// it with the terrain plane (y=0). Returns UV coordinates normalized
    /// to 0.0-1.0 range, or None if the ray doesn't hit the terrain.
    ///
    /// # Arguments
    /// * `screen_x` - Screen X coordinate (0 = left)
    /// * `screen_y` - Screen Y coordinate (0 = top)
    /// * `screen_width` - Width of the screen in pixels
    /// * `screen_height` - Height of the screen in pixels
    pub fn screen_to_terrain_uv(
        &self,
        screen_x: f64,
        screen_y: f64,
        screen_width: f64,
        screen_height: f64,
    ) -> Option<(f64, f64)> {
        if screen_width == 0.0 || screen_height == 0.0 {
            return None;
        }

        // Convert screen position to normalized device coordinates (-1 to 1)
        // Note: Y is flipped because screen Y goes down, NDC Y goes up
        let ndc_x = (2.0 * screen_x / screen_width - 1.0) as f32;
        let ndc_y = (1.0 - 2.0 * screen_y / screen_height) as f32;

        // Get inverse view-projection matrix
        let aspect_ratio = (screen_width / screen_height) as f32;
        let view_proj = self.view_projection_matrix(aspect_ratio);
        let inv_view_proj = view_proj.inverse();

        // Unproject near and far points to get ray in world space
        let near_point = inv_view_proj.project_point3(Vec3::new(ndc_x, ndc_y, -1.0));
        let far_point = inv_view_proj.project_point3(Vec3::new(ndc_x, ndc_y, 1.0));

        let ray_origin = near_point;
        let ray_direction = (far_point - near_point).normalize();

        // Intersect ray with y=0 plane
        // Ray equation: P = origin + t * direction
        // Plane equation: P.y = 0
        // Solve for t: origin.y + t * direction.y = 0
        //              t = -origin.y / direction.y

        // Check if ray is parallel to plane (direction.y ≈ 0)
        if ray_direction.y.abs() < 1e-6 {
            return None;
        }

        let t = -ray_origin.y / ray_direction.y;

        // Check if intersection is behind the camera
        if t < 0.0 {
            return None;
        }

        // Calculate intersection point
        let hit_point = ray_origin + ray_direction * t;

        // Convert to UV coordinates (0 to 1 range)
        let u = hit_point.x as f64 / self.map_width as f64;
        let v = hit_point.z as f64 / self.map_height as f64;

        // Return wrapped X (for horizontal wrapping) and clamped V
        let u_wrapped = u.rem_euclid(1.0);

        // Only return valid UV if within reasonable bounds
        if !(-0.1..=1.1).contains(&v) {
            return None;
        }

        Some((u_wrapped, v.clamp(0.0, 1.0)))
    }
}

/// Uniform data for the 3D terrain shader.
#[repr(C)]
#[derive(Debug, Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
pub struct CameraUniform3D {
    /// Combined view-projection matrix.
    pub view_proj: [[f32; 4]; 4],
}

impl Default for CameraUniform3D {
    fn default() -> Self {
        Self {
            view_proj: Mat4::IDENTITY.to_cols_array_2d(),
        }
    }
}

// ============================================================================
// Frustum Culling (Phase 8)
// ============================================================================

use glam::Vec4;

/// A frustum plane represented as (a, b, c, d) where ax + by + cz + d = 0.
/// The normal (a, b, c) points inward (toward the frustum interior).
#[derive(Debug, Clone, Copy)]
pub struct Plane {
    /// Normal and distance: (nx, ny, nz, d)
    pub normal_d: Vec4,
}

impl Plane {
    /// Creates a plane from coefficients, normalizing the normal vector.
    pub fn new(a: f32, b: f32, c: f32, d: f32) -> Self {
        let len = (a * a + b * b + c * c).sqrt();
        if len > 1e-6 {
            Self {
                normal_d: Vec4::new(a / len, b / len, c / len, d / len),
            }
        } else {
            Self {
                normal_d: Vec4::new(0.0, 1.0, 0.0, 0.0),
            }
        }
    }

    /// Returns the signed distance from a point to the plane.
    /// Positive = inside (toward normal), negative = outside.
    pub fn distance_to_point(&self, point: Vec3) -> f32 {
        self.normal_d.x * point.x
            + self.normal_d.y * point.y
            + self.normal_d.z * point.z
            + self.normal_d.w
    }
}

/// View frustum for culling, with 6 planes extracted from view-projection matrix.
#[derive(Debug, Clone, Copy)]
pub struct Frustum {
    /// Left, right, bottom, top, near, far planes.
    pub planes: [Plane; 6],
}

impl Frustum {
    /// Extracts frustum planes from a view-projection matrix.
    ///
    /// Uses the Gribb/Hartmann method for extracting planes from clip space.
    pub fn from_view_projection(vp: Mat4) -> Self {
        let cols = vp.to_cols_array_2d();
        let row0 = Vec4::new(cols[0][0], cols[1][0], cols[2][0], cols[3][0]);
        let row1 = Vec4::new(cols[0][1], cols[1][1], cols[2][1], cols[3][1]);
        let row2 = Vec4::new(cols[0][2], cols[1][2], cols[2][2], cols[3][2]);
        let row3 = Vec4::new(cols[0][3], cols[1][3], cols[2][3], cols[3][3]);

        // Extract planes (pointing inward)
        let left = row3 + row0;
        let right = row3 - row0;
        let bottom = row3 + row1;
        let top = row3 - row1;
        let near = row3 + row2;
        let far = row3 - row2;

        Self {
            planes: [
                Plane::new(left.x, left.y, left.z, left.w),
                Plane::new(right.x, right.y, right.z, right.w),
                Plane::new(bottom.x, bottom.y, bottom.z, bottom.w),
                Plane::new(top.x, top.y, top.z, top.w),
                Plane::new(near.x, near.y, near.z, near.w),
                Plane::new(far.x, far.y, far.z, far.w),
            ],
        }
    }

    /// Tests if an AABB is at least partially inside the frustum.
    ///
    /// Returns true if the AABB intersects or is fully inside the frustum.
    /// Uses the "p-vertex" optimization for fast rejection.
    pub fn intersects_aabb(&self, min: Vec3, max: Vec3) -> bool {
        for plane in &self.planes {
            // Find the "positive vertex" - the corner furthest in the direction of the plane normal
            let p = Vec3::new(
                if plane.normal_d.x >= 0.0 {
                    max.x
                } else {
                    min.x
                },
                if plane.normal_d.y >= 0.0 {
                    max.y
                } else {
                    min.y
                },
                if plane.normal_d.z >= 0.0 {
                    max.z
                } else {
                    min.z
                },
            );

            // If the positive vertex is outside (negative distance), the AABB is fully outside
            if plane.distance_to_point(p) < 0.0 {
                return false;
            }
        }
        true
    }
}

/// Uniform data for terrain rendering settings.
#[repr(C)]
#[derive(Debug, Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
pub struct TerrainSettings {
    /// Height scale for heightmap displacement (50-100 typical).
    pub height_scale: f32,
    /// X-axis offset for horizontal wrapping (Phase 7).
    /// Used to render terrain copies at -map_width, 0, +map_width.
    pub x_offset: f32,
    /// Padding for GPU alignment (vec2 = 8 bytes).
    pub _padding: [f32; 2],
}

impl TerrainSettings {
    /// Default height scale that produces visible terrain relief.
    pub const DEFAULT_HEIGHT_SCALE: f32 = 80.0;

    /// Creates terrain settings with the default height scale.
    pub fn new() -> Self {
        Self {
            height_scale: Self::DEFAULT_HEIGHT_SCALE,
            x_offset: 0.0,
            _padding: [0.0; 2],
        }
    }

    /// Creates terrain settings with a custom height scale (for tuning).
    #[allow(dead_code)]
    pub fn with_height_scale(height_scale: f32) -> Self {
        Self {
            height_scale,
            x_offset: 0.0,
            _padding: [0.0; 2],
        }
    }

    /// Creates terrain settings with x_offset for horizontal wrapping.
    pub fn with_x_offset(height_scale: f32, x_offset: f32) -> Self {
        Self {
            height_scale,
            x_offset,
            _padding: [0.0; 2],
        }
    }
}

impl Default for TerrainSettings {
    fn default() -> Self {
        Self::new()
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

    // =========================================================================
    // Camera3D tests
    // =========================================================================

    #[test]
    fn test_view_matrix_look_at_origin() {
        // Camera at (0, 10, 10) looking at origin should produce a valid view matrix
        let cam = Camera3D {
            position: Vec3::new(0.0, 10.0, 10.0),
            target: Vec3::ZERO,
            up: Vec3::Y,
            fov_y: std::f32::consts::FRAC_PI_4,
            near: 1.0,
            far: 1000.0,
            map_width: 5632.0,
            map_height: 2048.0,
        };

        let view = cam.view_matrix();

        // Origin should be transformed to somewhere in front of camera (negative Z in view space)
        let origin_view = view.transform_point3(Vec3::ZERO);
        assert!(
            origin_view.z < 0.0,
            "Origin should be in front of camera (negative Z), got z={}",
            origin_view.z
        );

        // Camera position should transform to origin in view space
        let cam_view = view.transform_point3(cam.position);
        assert!(
            cam_view.length() < 0.001,
            "Camera position should be at view origin, got {:?}",
            cam_view
        );
    }

    #[test]
    fn test_projection_matrix_aspect_ratio() {
        let cam = Camera3D::new(5632.0, 2048.0);

        let proj_wide = cam.projection_matrix(16.0 / 9.0);
        let proj_square = cam.projection_matrix(1.0);

        // Different aspect ratios should produce different matrices
        assert_ne!(
            proj_wide.to_cols_array(),
            proj_square.to_cols_array(),
            "Different aspect ratios should produce different projection matrices"
        );

        // The first column (X scaling) should differ based on aspect ratio
        // Wider aspect means less X scaling (larger view horizontally)
        assert!(
            proj_wide.col(0).x < proj_square.col(0).x,
            "Wide aspect should have smaller X scale"
        );
    }

    #[test]
    fn test_camera3d_pan_updates_position() {
        let mut cam = Camera3D::new(5632.0, 2048.0);
        let initial_pos = cam.position;
        let initial_target = cam.target;

        cam.pan(100.0, 50.0);

        // Both position and target should move by the same amount
        assert!(
            (cam.position.x - initial_pos.x - 100.0).abs() < 0.001,
            "Position X should move by dx"
        );
        assert!(
            (cam.position.z - initial_pos.z - 50.0).abs() < 0.001,
            "Position Z should move by dz"
        );
        assert!(
            (cam.target.x - initial_target.x - 100.0).abs() < 0.001,
            "Target X should move by dx"
        );
        assert!(
            (cam.target.z - initial_target.z - 50.0).abs() < 0.001,
            "Target Z should move by dz"
        );

        // Y should remain unchanged (pan is horizontal only)
        assert_eq!(
            cam.position.y, initial_pos.y,
            "Position Y should not change"
        );
        assert_eq!(cam.target.y, initial_target.y, "Target Y should not change");
    }

    #[test]
    fn test_camera3d_zoom_changes_distance() {
        let mut cam = Camera3D::new(5632.0, 2048.0);
        let initial_distance = cam.distance();

        // Zoom in (factor > 1)
        cam.zoom(2.0);
        let zoomed_in_distance = cam.distance();
        assert!(
            zoomed_in_distance < initial_distance,
            "Zoom in should decrease distance: {} < {}",
            zoomed_in_distance,
            initial_distance
        );

        // Zoom out (factor < 1)
        cam.zoom(0.5);
        let zoomed_out_distance = cam.distance();
        assert!(
            zoomed_out_distance > zoomed_in_distance,
            "Zoom out should increase distance: {} > {}",
            zoomed_out_distance,
            zoomed_in_distance
        );

        // Target should remain unchanged during zoom
        let cam2 = Camera3D::new(5632.0, 2048.0);
        let original_target = cam2.target;
        let mut cam2 = cam2;
        cam2.zoom(2.0);
        assert_eq!(
            cam2.target, original_target,
            "Target should not move during zoom"
        );
    }

    #[test]
    fn test_camera3d_zoom_limits() {
        let mut cam = Camera3D::new(5632.0, 2048.0);

        // Zoom in extremely
        for _ in 0..100 {
            cam.zoom(2.0);
        }
        assert!(
            cam.distance() >= Camera3D::MIN_DISTANCE,
            "Distance should not go below minimum: {} >= {}",
            cam.distance(),
            Camera3D::MIN_DISTANCE
        );

        // Zoom out extremely
        for _ in 0..100 {
            cam.zoom(0.5);
        }
        assert!(
            cam.distance() <= Camera3D::MAX_DISTANCE,
            "Distance should not exceed maximum: {} <= {}",
            cam.distance(),
            Camera3D::MAX_DISTANCE
        );
    }

    #[test]
    fn test_camera3d_uniform_size() {
        // CameraUniform3D should be exactly 64 bytes (4x4 f32 matrix)
        assert_eq!(
            std::mem::size_of::<CameraUniform3D>(),
            64,
            "CameraUniform3D should be 64 bytes for GPU uniform alignment"
        );
    }

    #[test]
    fn test_camera3d_view_projection_combines_correctly() {
        let cam = Camera3D::new(5632.0, 2048.0);
        let aspect = 16.0 / 9.0;

        let view = cam.view_matrix();
        let proj = cam.projection_matrix(aspect);
        let combined = cam.view_projection_matrix(aspect);

        // Combined should equal proj * view
        let expected = proj * view;
        assert_eq!(
            combined.to_cols_array(),
            expected.to_cols_array(),
            "view_projection should equal projection * view"
        );
    }

    #[test]
    fn test_camera3d_pan_clamps_z() {
        let mut cam = Camera3D::new(5632.0, 2048.0);

        // Pan way past the bottom edge (Z = map_height)
        for _ in 0..100 {
            cam.pan(0.0, 100.0);
        }

        // Z should be clamped near map_height + padding
        let max_z = 2048.0 + 100.0; // map_height + z_padding
        assert!(
            cam.target.z <= max_z,
            "Target Z {} should be clamped to max {}",
            cam.target.z,
            max_z
        );

        // Pan way past the top edge (Z = 0)
        for _ in 0..200 {
            cam.pan(0.0, -100.0);
        }

        // Z should be clamped near 0 - padding
        let min_z = -100.0; // -z_padding
        assert!(
            cam.target.z >= min_z,
            "Target Z {} should be clamped to min {}",
            cam.target.z,
            min_z
        );
    }

    #[test]
    fn test_camera3d_pan_wraps_x() {
        let mut cam = Camera3D::new(5632.0, 2048.0);
        let map_width = 5632.0;

        // Pan past the right edge (X >= map_width should wrap to X - map_width)
        cam.target.x = map_width - 10.0; // Start near right edge
        cam.position.x = cam.target.x;
        cam.pan(20.0, 0.0); // Pan past the edge

        // Should have wrapped to near 0
        assert!(
            cam.target.x >= 0.0 && cam.target.x < 50.0,
            "Target X {} should wrap to near 0 after crossing map_width",
            cam.target.x
        );

        // Pan past the left edge (X < 0 should wrap to X + map_width)
        cam.target.x = 10.0; // Start near left edge
        cam.position.x = cam.target.x;
        cam.pan(-20.0, 0.0); // Pan past the edge

        // Should have wrapped to near map_width
        assert!(
            cam.target.x > map_width - 50.0 && cam.target.x < map_width,
            "Target X {} should wrap to near map_width after crossing 0",
            cam.target.x
        );
    }

    #[test]
    fn test_screen_to_terrain_uv_center() {
        // Use default camera which looks at terrain at an angle (not straight down)
        let cam = Camera3D::new(5632.0, 2048.0);

        // Screen center should hit the terrain near where the camera is looking
        let result = cam.screen_to_terrain_uv(960.0, 540.0, 1920.0, 1080.0);
        assert!(result.is_some(), "Screen center should hit terrain");

        let (u, v) = result.unwrap();
        // Default camera target is at map center
        assert!((u - 0.5).abs() < 0.05, "Center U should be ~0.5, got {}", u);
        assert!((v - 0.5).abs() < 0.05, "Center V should be ~0.5, got {}", v);
    }

    #[test]
    fn test_screen_to_terrain_uv_off_map() {
        // Camera at edge of map looking away from terrain (towards z > map_height)
        let cam = Camera3D {
            position: Vec3::new(2816.0, 500.0, 2048.0 + 500.0), // Behind map edge
            target: Vec3::new(2816.0, 500.0, 2048.0 + 1000.0),  // Looking further away
            up: Vec3::Y,
            fov_y: std::f32::consts::FRAC_PI_4,
            near: 1.0,
            far: 5000.0,
            map_width: 5632.0,
            map_height: 2048.0,
        };

        // Clicking at center of screen shoots a ray that misses terrain (behind us)
        let result = cam.screen_to_terrain_uv(960.0, 540.0, 1920.0, 1080.0);
        // The ray direction is roughly horizontal (looking away from map)
        // Intersection with y=0 would be behind the camera
        assert!(
            result.is_none(),
            "Ray looking away from map should miss terrain"
        );
    }

    #[test]
    fn test_screen_to_terrain_uv_zero_screen() {
        let cam = Camera3D::new(5632.0, 2048.0);
        let result = cam.screen_to_terrain_uv(100.0, 100.0, 0.0, 0.0);
        assert!(result.is_none(), "Zero screen size should return None");
    }

    // =========================================================================
    // TerrainSettings tests
    // =========================================================================

    #[test]
    fn test_terrain_settings_size() {
        // TerrainSettings must be 16 bytes (f32 + vec3<f32> padding = 16 bytes)
        // This matches GPU uniform buffer alignment requirements
        assert_eq!(
            std::mem::size_of::<TerrainSettings>(),
            16,
            "TerrainSettings should be 16 bytes for GPU uniform alignment"
        );
    }

    #[test]
    fn test_terrain_settings_default() {
        let settings = TerrainSettings::default();
        assert_eq!(
            settings.height_scale,
            TerrainSettings::DEFAULT_HEIGHT_SCALE,
            "Default height scale should be {}",
            TerrainSettings::DEFAULT_HEIGHT_SCALE
        );
    }

    #[test]
    fn test_terrain_settings_custom() {
        let settings = TerrainSettings::with_height_scale(100.0);
        assert_eq!(
            settings.height_scale, 100.0,
            "Custom height scale should be set"
        );
    }

    // =========================================================================
    // Frustum Culling tests (Phase 8)
    // =========================================================================

    #[test]
    fn test_frustum_aabb_inside() {
        // Camera looking at map center
        let cam = Camera3D::new(5632.0, 2048.0);
        let frustum = cam.frustum(16.0 / 9.0);

        // An AABB right in front of the camera should be visible
        let min = Vec3::new(2500.0, 0.0, 900.0);
        let max = Vec3::new(3100.0, 100.0, 1100.0);

        assert!(
            frustum.intersects_aabb(min, max),
            "AABB in front of camera should be visible"
        );
    }

    #[test]
    fn test_frustum_aabb_behind_camera() {
        // Camera looking at map center
        let cam = Camera3D::new(5632.0, 2048.0);
        let frustum = cam.frustum(16.0 / 9.0);

        // An AABB far behind the camera should be culled
        // Camera is at center_z + 200, looking at center_z
        // So behind would be z > camera.position.z + some margin
        let min = Vec3::new(2500.0, 0.0, 2000.0);
        let max = Vec3::new(3100.0, 100.0, 2500.0);

        assert!(
            !frustum.intersects_aabb(min, max),
            "AABB behind camera should be culled"
        );
    }

    #[test]
    fn test_frustum_aabb_far_left() {
        // Camera looking at map center
        let cam = Camera3D::new(5632.0, 2048.0);
        let frustum = cam.frustum(16.0 / 9.0);

        // An AABB far to the left of the view should be culled
        let min = Vec3::new(-1000.0, 0.0, 900.0);
        let max = Vec3::new(-500.0, 100.0, 1100.0);

        assert!(
            !frustum.intersects_aabb(min, max),
            "AABB far left of view should be culled"
        );
    }

    #[test]
    fn test_frustum_aabb_far_right() {
        // Camera looking at map center
        let cam = Camera3D::new(5632.0, 2048.0);
        let frustum = cam.frustum(16.0 / 9.0);

        // An AABB far to the right of the view should be culled
        let min = Vec3::new(6000.0, 0.0, 900.0);
        let max = Vec3::new(6500.0, 100.0, 1100.0);

        assert!(
            !frustum.intersects_aabb(min, max),
            "AABB far right of view should be culled"
        );
    }
}
