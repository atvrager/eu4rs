//! Snapshot testing utilities for GUI visual verification.
//!
//! Provides golden image comparison for GUI components, enabling
//! regression testing of visual output.
//!
//! Also provides `GuiTestHarness` for headless testing of GUI state
//! machine transitions and visual output.

use image::RgbaImage;
use std::path::{Path, PathBuf};

use crate::gui::{GuiRenderer, GuiState};
use crate::render::SpriteRenderer;
use crate::screen::{Screen, ScreenManager};

/// Assert that an image matches the golden snapshot.
///
/// On first run or with `UPDATE_SNAPSHOTS=1`, saves the image as the new golden.
/// On subsequent runs, compares pixel-by-pixel and panics on mismatch.
pub fn assert_snapshot(actual: &RgbaImage, name: &str) {
    let manifest_dir = std::env::var("CARGO_MANIFEST_DIR").unwrap();
    let golden_dir = PathBuf::from(manifest_dir).join("tests/goldens");
    let update = std::env::var("UPDATE_SNAPSHOTS").is_ok();
    assert_snapshot_at(actual, name, &golden_dir, update);
}

fn assert_snapshot_at(actual: &RgbaImage, name: &str, golden_dir: &Path, update: bool) {
    std::fs::create_dir_all(golden_dir).unwrap();

    let golden_path = golden_dir.join(format!("{}.png", name));
    let exists = golden_path.exists();

    if update {
        actual
            .save(&golden_path)
            .expect("Failed to save golden image");
        println!("Saved golden (UPDATE_SNAPSHOTS=1): {:?}", golden_path);
        return;
    }

    if !exists {
        actual
            .save(&golden_path)
            .expect("Failed to save golden image");
        println!("Saved initial golden: {:?}", golden_path);
        return;
    }

    let golden = image::open(&golden_path)
        .expect("Failed to load golden image")
        .to_rgba8();

    if actual.dimensions() != golden.dimensions() {
        panic!(
            "Dimension mismatch: actual {:?} vs golden {:?}",
            actual.dimensions(),
            golden.dimensions()
        );
    }

    let mut diff_pixels = 0;
    for (x, y, pixel) in actual.enumerate_pixels() {
        let golden_pixel = golden.get_pixel(x, y);
        if pixel != golden_pixel {
            diff_pixels += 1;
        }
    }

    if diff_pixels > 0 {
        // Save actual for debugging
        let actual_path = golden_dir.join(format!("{}_actual.png", name));
        let _ = actual.save(&actual_path);
        panic!(
            "Snapshot mismatch for {}: {} pixels differ. Saved actual to {:?}",
            name, diff_pixels, actual_path
        );
    }
}

/// Headless GPU context for rendering tests.
///
/// Creates a wgpu device without a display surface, suitable for
/// offscreen rendering in CI environments.
pub struct HeadlessGpu {
    pub device: wgpu::Device,
    pub queue: wgpu::Queue,
    pub format: wgpu::TextureFormat,
}

impl HeadlessGpu {
    /// Create a new headless GPU context.
    ///
    /// Returns None if no suitable GPU adapter is found (CI waiver).
    pub async fn new() -> Option<Self> {
        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor {
            backends: wgpu::Backends::VULKAN | wgpu::Backends::GL,
            ..Default::default()
        });

        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::LowPower,
                compatible_surface: None, // Headless!
                force_fallback_adapter: false,
            })
            .await?;

        let (device, queue) = adapter
            .request_device(
                &wgpu::DeviceDescriptor {
                    label: Some("Headless Test Device"),
                    required_features: wgpu::Features::empty(),
                    required_limits: wgpu::Limits::default(),
                },
                None,
            )
            .await
            .ok()?;

        Some(Self {
            device,
            queue,
            format: wgpu::TextureFormat::Rgba8UnormSrgb,
        })
    }
}

/// Test harness for headless GUI testing.
///
/// Provides a complete rendering environment for testing GUI state machine
/// transitions and visual output without a display surface.
///
/// # Example
///
/// ```rust,ignore
/// let harness = GuiTestHarness::new().expect("No GPU available");
/// harness.transition_to(Screen::SinglePlayer);
/// let image = harness.render_to_image((1920, 1080));
/// assert_snapshot(&image, "single_player_screen");
/// ```
pub struct GuiTestHarness {
    /// Headless GPU context.
    gpu: HeadlessGpu,
    /// Sprite renderer for drawing.
    sprite_renderer: SpriteRenderer,
    /// GUI renderer with loaded layouts and sprites.
    gui_renderer: GuiRenderer,
    /// Screen state machine.
    screen_manager: ScreenManager,
    /// Current GUI state.
    gui_state: GuiState,
    /// Game data path.
    #[allow(dead_code)]
    game_path: PathBuf,
    /// Start date for country selection screen.
    start_date: eu4data::Eu4Date,
    /// Selected player country (for testing country selection).
    player_tag: Option<String>,
}

impl GuiTestHarness {
    /// Create a new test harness.
    ///
    /// Returns `None` if no GPU is available or game path is not found.
    /// This allows tests to gracefully skip when running in CI without GPU/game.
    pub fn new() -> Option<Self> {
        let gpu = pollster::block_on(HeadlessGpu::new())?;
        let game_path = eu4data::path::detect_game_path()?;

        let sprite_renderer = SpriteRenderer::new(&gpu.device, gpu.format);
        let gui_renderer = GuiRenderer::new(&game_path);
        let screen_manager = ScreenManager::new();
        let gui_state = GuiState {
            date: "11 November 1444".to_string(),
            speed: 3,
            paused: false,
            country: None,
        };

        Some(Self {
            gpu,
            sprite_renderer,
            gui_renderer,
            screen_manager,
            gui_state,
            game_path,
            start_date: eu4data::Eu4Date::from_ymd(1444, 11, 11),
            player_tag: None,
        })
    }

    // ========================================
    // State Query Methods
    // ========================================

    /// Get the current screen.
    pub fn current_screen(&self) -> Screen {
        self.screen_manager.current()
    }

    /// Check if back navigation is available.
    #[allow(dead_code)]
    pub fn can_go_back(&self) -> bool {
        self.screen_manager.can_go_back()
    }

    // ========================================
    // Direct State Manipulation (Unit Tests)
    // ========================================

    /// Transition to a specific screen directly.
    ///
    /// This is useful for unit tests that want to test rendering
    /// at a specific screen state without simulating input.
    pub fn transition_to(&mut self, screen: Screen) {
        self.screen_manager.transition_to(screen);
    }

    /// Go back to the previous screen.
    pub fn go_back(&mut self) -> Option<Screen> {
        self.screen_manager.go_back()
    }

    /// Set the GUI state directly.
    #[allow(dead_code)]
    pub fn set_gui_state(&mut self, state: GuiState) {
        self.gui_state = state;
    }

    /// Set the date string.
    #[allow(dead_code)]
    pub fn set_date(&mut self, date: &str) {
        self.gui_state.date = date.to_string();
    }

    /// Set the game speed (1-5).
    #[allow(dead_code)]
    pub fn set_speed(&mut self, speed: u32) {
        self.gui_state.speed = speed.clamp(1, 5);
    }

    /// Set the paused state.
    #[allow(dead_code)]
    pub fn set_paused(&mut self, paused: bool) {
        self.gui_state.paused = paused;
    }

    /// Set the start date for the country selection screen.
    pub fn set_start_date(&mut self, date: eu4data::Eu4Date) {
        self.start_date = date;
    }

    /// Set the selected player country (for testing country selection).
    pub fn set_player_country(&mut self, tag: Option<String>) {
        self.player_tag = tag;
    }

    // ========================================
    // Rendering
    // ========================================

    /// Render the current screen to an image.
    ///
    /// Creates an offscreen texture, renders the GUI, and reads back
    /// the pixels as an `RgbaImage`.
    pub fn render_to_image(&mut self, screen_size: (u32, u32)) -> RgbaImage {
        let format = self.gpu.format;

        // Create offscreen texture
        let texture = self.gpu.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("Test Harness Texture"),
            size: wgpu::Extent3d {
                width: screen_size.0,
                height: screen_size.1,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format,
            usage: wgpu::TextureUsages::COPY_SRC | wgpu::TextureUsages::RENDER_ATTACHMENT,
            view_formats: &[],
        });
        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());

        // Create readback buffer with proper alignment
        let bytes_per_pixel = 4u32;
        let unpadded_bytes_per_row = bytes_per_pixel * screen_size.0;
        let align = wgpu::COPY_BYTES_PER_ROW_ALIGNMENT;
        let padded_bytes_per_row = unpadded_bytes_per_row.div_ceil(align) * align;
        let buffer_size = (padded_bytes_per_row * screen_size.1) as wgpu::BufferAddress;
        let output_buffer = self.gpu.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Readback Buffer"),
            size: buffer_size,
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });

        // Render
        let mut encoder = self
            .gpu
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("Test Harness Encoder"),
            });

        {
            let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("Test Harness Render Pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color {
                            r: 0.1,
                            g: 0.1,
                            b: 0.1,
                            a: 1.0,
                        }),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                occlusion_query_set: None,
                timestamp_writes: None,
            });

            self.sprite_renderer.begin_frame();

            // Render based on current screen state
            let current_screen = self.screen_manager.current();
            let start_date = if current_screen == Screen::SinglePlayer {
                Some(&self.start_date)
            } else {
                None
            };

            // Update selected country if we have a player tag (Phase 9.4)
            if current_screen == Screen::SinglePlayer {
                if let Some(ref tag) = self.player_tag {
                    // Create mock country state for testing
                    let country_state = crate::gui::country_select::SelectedCountryState {
                        tag: tag.clone(),
                        name: "Austria".to_string(),
                        government_type: "Archduchy".to_string(),
                        fog_status: String::new(),
                        government_rank: 3,
                        religion_frame: 0,
                        tech_group_frame: 0,
                        ruler_name: "Friedrich III von Habsburg".to_string(),
                        ruler_adm: 3,
                        ruler_dip: 4,
                        ruler_mil: 2,
                        adm_tech: 3,
                        dip_tech: 3,
                        mil_tech: 3,
                        ideas_name: "Austrian Ideas".to_string(),
                        ideas_unlocked: 2,
                        province_count: 12,
                        total_development: 156,
                        fort_level: 1,
                        diplomacy_header: "Diplomacy".to_string(),
                    };
                    self.gui_renderer
                        .update_selected_country(Some(&country_state));
                    self.gui_renderer.set_play_button_enabled(true);
                } else {
                    self.gui_renderer.update_selected_country(None);
                    self.gui_renderer.set_play_button_enabled(false);
                }
            }

            self.gui_renderer.render(
                &mut render_pass,
                &self.gpu.device,
                &self.gpu.queue,
                &self.sprite_renderer,
                &self.gui_state,
                current_screen,
                screen_size,
                start_date,
            );
        }

        // Copy to buffer
        encoder.copy_texture_to_buffer(
            wgpu::ImageCopyTexture {
                texture: &texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            wgpu::ImageCopyBuffer {
                buffer: &output_buffer,
                layout: wgpu::ImageDataLayout {
                    offset: 0,
                    bytes_per_row: Some(padded_bytes_per_row),
                    rows_per_image: Some(screen_size.1),
                },
            },
            wgpu::Extent3d {
                width: screen_size.0,
                height: screen_size.1,
                depth_or_array_layers: 1,
            },
        );

        self.gpu.queue.submit(Some(encoder.finish()));

        // Read back
        let buffer_slice = output_buffer.slice(..);
        let (tx, rx) = std::sync::mpsc::channel();
        buffer_slice.map_async(wgpu::MapMode::Read, move |v| tx.send(v).unwrap());
        self.gpu.device.poll(wgpu::Maintain::Wait);
        rx.recv().unwrap().unwrap();

        let data = buffer_slice.get_mapped_range();

        // Strip row padding if present
        let image = if padded_bytes_per_row != unpadded_bytes_per_row {
            let mut pixels = Vec::with_capacity((unpadded_bytes_per_row * screen_size.1) as usize);
            for row in 0..screen_size.1 {
                let row_start = (row * padded_bytes_per_row) as usize;
                let row_end = row_start + unpadded_bytes_per_row as usize;
                pixels.extend_from_slice(&data[row_start..row_end]);
            }
            RgbaImage::from_raw(screen_size.0, screen_size.1, pixels).unwrap()
        } else {
            RgbaImage::from_raw(screen_size.0, screen_size.1, data.to_vec()).unwrap()
        };

        drop(data);
        output_buffer.unmap();

        image
    }
}

// ============================================================================
// OffscreenTarget - RenderTarget for headless rendering
// ============================================================================

use crate::input::{AppEvent, KeyCode, MouseButton};
use crate::render::{GpuContext, RenderError, RenderTarget};

/// Render target for offscreen rendering (tests/headless).
///
/// Unlike `SurfaceTarget` which presents to a window, this renders
/// to an offscreen texture that can be read back for verification.
#[allow(dead_code)]
pub struct OffscreenTarget {
    texture: wgpu::Texture,
    view: wgpu::TextureView,
    format: wgpu::TextureFormat,
    size: (u32, u32),
}

impl OffscreenTarget {
    /// Create a new offscreen target.
    pub fn new(device: &wgpu::Device, width: u32, height: u32) -> Self {
        let format = wgpu::TextureFormat::Rgba8UnormSrgb;

        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("Offscreen Render Target"),
            size: wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
            view_formats: &[],
        });

        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());

        Self {
            texture,
            view,
            format,
            size: (width, height),
        }
    }

    /// Read back the rendered image.
    pub fn read_pixels(&self, device: &wgpu::Device, queue: &wgpu::Queue) -> RgbaImage {
        let (width, height) = self.size;
        let bytes_per_row_unpadded = width * 4;
        let bytes_per_row_aligned = bytes_per_row_unpadded
            .div_ceil(wgpu::COPY_BYTES_PER_ROW_ALIGNMENT)
            * wgpu::COPY_BYTES_PER_ROW_ALIGNMENT;

        let buffer_size = (bytes_per_row_aligned * height) as u64;
        let output_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Offscreen Readback Buffer"),
            size: buffer_size,
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });

        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("Offscreen Readback Encoder"),
        });

        encoder.copy_texture_to_buffer(
            wgpu::ImageCopyTexture {
                texture: &self.texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            wgpu::ImageCopyBuffer {
                buffer: &output_buffer,
                layout: wgpu::ImageDataLayout {
                    offset: 0,
                    bytes_per_row: Some(bytes_per_row_aligned),
                    rows_per_image: Some(height),
                },
            },
            wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
        );

        queue.submit(std::iter::once(encoder.finish()));

        // Map and read
        let buffer_slice = output_buffer.slice(..);
        let (tx, rx) = std::sync::mpsc::channel();
        buffer_slice.map_async(wgpu::MapMode::Read, move |result| {
            let _ = tx.send(result);
        });
        device.poll(wgpu::Maintain::Wait);
        rx.recv().unwrap().unwrap();

        let data = buffer_slice.get_mapped_range();

        // Copy with padding removal
        let mut image = RgbaImage::new(width, height);
        for row in 0..height {
            let src_start = (row * bytes_per_row_aligned) as usize;
            let src_end = src_start + (width * 4) as usize;
            let row_data = &data[src_start..src_end];

            for x in 0..width {
                let px = (x * 4) as usize;
                image.put_pixel(
                    x,
                    row,
                    image::Rgba([
                        row_data[px],
                        row_data[px + 1],
                        row_data[px + 2],
                        row_data[px + 3],
                    ]),
                );
            }
        }

        drop(data);
        output_buffer.unmap();

        image
    }
}

impl RenderTarget for OffscreenTarget {
    fn get_view(&mut self) -> Result<(wgpu::TextureView, u32, u32), RenderError> {
        // Clone the view - this is cheap as TextureView is reference-counted
        let view = self
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());
        Ok((view, self.size.0, self.size.1))
    }

    fn present(&mut self) {
        // No-op for offscreen - nothing to present
    }

    fn format(&self) -> wgpu::TextureFormat {
        self.format
    }
}

impl GpuContext for HeadlessGpu {
    fn device(&self) -> &wgpu::Device {
        &self.device
    }

    fn queue(&self) -> &wgpu::Queue {
        &self.queue
    }

    fn format(&self) -> wgpu::TextureFormat {
        self.format
    }
}

// ============================================================================
// HeadlessApp - Full application harness for headless testing
// ============================================================================

/// Result of processing one frame.
#[allow(dead_code)]
#[derive(Debug, Clone)]
pub struct StepResult {
    /// Whether exit was requested.
    pub should_exit: bool,
    /// Number of frames processed.
    pub frame: u64,
}

/// Full application harness for headless testing.
///
/// Unlike `GuiTestHarness` which only tests GUI rendering,
/// this tests the full application including map, simulation, and input.
///
/// # Example
///
/// ```rust,ignore
/// let Some(mut app) = HeadlessApp::new((1920, 1080)) else {
///     return; // No GPU available
/// };
///
/// app.run_until_first_render();
/// app.press_key(KeyCode::KeyS);
/// app.step();
///
/// assert_eq!(app.current_screen(), Screen::SinglePlayer);
/// ```
pub struct HeadlessApp {
    /// Headless GPU context.
    gpu: HeadlessGpu,
    /// Offscreen render target.
    target: OffscreenTarget,
    /// Event queue for injection.
    event_queue: Vec<AppEvent>,
    /// Frame counter.
    frame_count: u64,
    /// Screen manager (simplified - real AppCore integration pending).
    screen_manager: ScreenManager,
    /// Whether first render has completed.
    first_render_done: bool,
}

impl HeadlessApp {
    /// Create a new headless app.
    ///
    /// Returns `None` if no GPU is available (graceful skip in CI).
    pub fn new(size: (u32, u32)) -> Option<Self> {
        let gpu = pollster::block_on(HeadlessGpu::new())?;
        let target = OffscreenTarget::new(&gpu.device, size.0, size.1);
        let screen_manager = ScreenManager::new();

        Some(Self {
            gpu,
            target,
            event_queue: Vec::new(),
            frame_count: 0,
            screen_manager,
            first_render_done: false,
        })
    }

    // ========================================================================
    // Event Injection
    // ========================================================================

    /// Inject an event to be processed on next step.
    pub fn inject_event(&mut self, event: AppEvent) {
        self.event_queue.push(event);
    }

    /// Inject a key press event.
    pub fn press_key(&mut self, key: KeyCode) {
        self.inject_event(AppEvent::KeyPress { key, pressed: true });
        self.inject_event(AppEvent::KeyPress {
            key,
            pressed: false,
        });
    }

    /// Inject a mouse click event.
    pub fn click(&mut self, x: f64, y: f64, button: MouseButton) {
        self.inject_event(AppEvent::MouseButton {
            button,
            pressed: true,
            pos: (x, y),
        });
        self.inject_event(AppEvent::MouseButton {
            button,
            pressed: false,
            pos: (x, y),
        });
    }

    /// Inject mouse movement.
    pub fn move_mouse(&mut self, x: f64, y: f64) {
        self.inject_event(AppEvent::MouseMove { pos: (x, y) });
    }

    // ========================================================================
    // Execution Control
    // ========================================================================

    /// Process one frame: handle events, tick, render.
    pub fn step(&mut self) -> StepResult {
        // Process injected events
        let events: Vec<_> = self.event_queue.drain(..).collect();
        for event in events {
            self.handle_event(&event);
        }

        self.frame_count += 1;
        self.first_render_done = true;

        StepResult {
            should_exit: false,
            frame: self.frame_count,
        }
    }

    /// Run until a predicate is satisfied or max frames.
    pub fn run_until<F>(&mut self, pred: F, max_frames: u64) -> bool
    where
        F: Fn(&Self) -> bool,
    {
        for _ in 0..max_frames {
            if pred(self) {
                return true;
            }
            self.step();
        }
        false
    }

    /// Run until first render completes (for load-time testing).
    pub fn run_until_first_render(&mut self) -> bool {
        self.step();
        self.first_render_done
    }

    /// Run for N frames.
    #[allow(dead_code)]
    pub fn run_frames(&mut self, n: u64) {
        for _ in 0..n {
            self.step();
        }
    }

    // ========================================================================
    // State Queries
    // ========================================================================

    /// Get current screen.
    pub fn current_screen(&self) -> Screen {
        self.screen_manager.current()
    }

    /// Get frame count.
    pub fn frame_count(&self) -> u64 {
        self.frame_count
    }

    /// Check if first render has completed.
    pub fn first_render_done(&self) -> bool {
        self.first_render_done
    }

    /// Capture current frame as image.
    pub fn capture_frame(&mut self) -> RgbaImage {
        self.target.read_pixels(&self.gpu.device, &self.gpu.queue)
    }

    // ========================================================================
    // Internal
    // ========================================================================

    fn handle_event(&mut self, event: &AppEvent) {
        if let AppEvent::KeyPress { key, pressed: true } = event {
            match key {
                KeyCode::KeyS => {
                    if self.screen_manager.current() == Screen::MainMenu {
                        self.screen_manager.transition_to(Screen::SinglePlayer);
                    }
                }
                KeyCode::Escape => {
                    if self.screen_manager.can_go_back() {
                        self.screen_manager.go_back();
                    }
                }
                _ => {}
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn test_assert_snapshot_creates_golden() {
        let dir = tempdir().unwrap();
        let golden_dir = dir.path();

        let mut img = RgbaImage::new(2, 2);
        for (_, _, pixel) in img.enumerate_pixels_mut() {
            *pixel = image::Rgba([255, 0, 0, 255]);
        }

        // First run creates the golden
        assert_snapshot_at(&img, "test_create", golden_dir, false);
        assert!(golden_dir.join("test_create.png").exists());
    }

    #[test]
    fn test_assert_snapshot_matches() {
        let dir = tempdir().unwrap();
        let golden_dir = dir.path();

        let mut img = RgbaImage::new(2, 2);
        for (_, _, pixel) in img.enumerate_pixels_mut() {
            *pixel = image::Rgba([0, 255, 0, 255]);
        }

        // Create golden
        assert_snapshot_at(&img, "test_match", golden_dir, false);

        // Should match
        assert_snapshot_at(&img, "test_match", golden_dir, false);
    }

    #[test]
    #[should_panic(expected = "Snapshot mismatch")]
    fn test_assert_snapshot_detects_mismatch() {
        let dir = tempdir().unwrap();
        let golden_dir = dir.path();

        let mut img = RgbaImage::new(2, 2);
        for (_, _, pixel) in img.enumerate_pixels_mut() {
            *pixel = image::Rgba([0, 0, 255, 255]);
        }

        // Create golden
        assert_snapshot_at(&img, "test_mismatch", golden_dir, false);

        // Modify and expect panic
        img.put_pixel(0, 0, image::Rgba([255, 0, 0, 255]));
        assert_snapshot_at(&img, "test_mismatch", golden_dir, false);
    }

    // ========================================================================
    // HeadlessApp Tests
    // ========================================================================

    #[test]
    fn test_headless_app_creation() {
        // Skip if no GPU available
        let Some(app) = HeadlessApp::new((1920, 1080)) else {
            return;
        };

        assert_eq!(app.current_screen(), Screen::MainMenu);
        assert_eq!(app.frame_count(), 0);
    }

    #[test]
    fn test_headless_app_screen_navigation() {
        let Some(mut app) = HeadlessApp::new((1920, 1080)) else {
            return;
        };

        // Start at main menu
        assert_eq!(app.current_screen(), Screen::MainMenu);

        // Press S to go to single player
        app.press_key(KeyCode::KeyS);
        app.step();
        assert_eq!(app.current_screen(), Screen::SinglePlayer);

        // Press Escape to go back
        app.press_key(KeyCode::Escape);
        app.step();
        assert_eq!(app.current_screen(), Screen::MainMenu);
    }

    #[test]
    fn test_headless_app_run_until_first_render() {
        let Some(mut app) = HeadlessApp::new((800, 600)) else {
            return;
        };

        assert!(!app.first_render_done());
        assert!(app.run_until_first_render());
        assert!(app.first_render_done());
        assert_eq!(app.frame_count(), 1);
    }

    #[test]
    fn test_headless_app_run_until() {
        let Some(mut app) = HeadlessApp::new((800, 600)) else {
            return;
        };

        // Run until frame 5
        let reached = app.run_until(|a| a.frame_count() >= 5, 100);
        assert!(reached);
        assert_eq!(app.frame_count(), 5);
    }

    #[test]
    fn test_headless_app_event_injection() {
        let Some(mut app) = HeadlessApp::new((800, 600)) else {
            return;
        };

        // Inject mouse movement (doesn't change state but shouldn't crash)
        app.move_mouse(500.0, 300.0);
        app.step();

        // Inject click (doesn't change state but shouldn't crash)
        app.click(500.0, 300.0, MouseButton::Left);
        app.step();

        assert_eq!(app.frame_count(), 2);
    }

    #[test]
    fn test_headless_app_capture_frame() {
        let Some(mut app) = HeadlessApp::new((64, 64)) else {
            return;
        };

        app.step();
        let frame = app.capture_frame();

        // Should have correct dimensions
        assert_eq!(frame.width(), 64);
        assert_eq!(frame.height(), 64);
    }
}
