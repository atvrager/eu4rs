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
// HeadlessRenderer - Shared base for headless rendering
// ============================================================================

/// Shared headless rendering infrastructure for test harnesses.
///
/// Provides GPU context and offscreen rendering with pixel readback,
/// eliminating duplication between `HeadlessApp` and `MapTestHarness`.
///
/// # Example
///
/// ```rust,ignore
/// let mut renderer = HeadlessRenderer::new((1920, 1080))?;
/// let view = renderer.view();
/// // ... render to view ...
/// let image = renderer.capture_frame();
/// ```
#[allow(dead_code)]
pub struct HeadlessRenderer {
    gpu: HeadlessGpu,
    target: OffscreenTarget,
}

#[allow(dead_code)]
impl HeadlessRenderer {
    /// Create a new headless renderer at the specified resolution.
    ///
    /// Returns `None` if no GPU adapter is available (CI waiver).
    pub fn new(size: (u32, u32)) -> Option<Self> {
        let gpu = pollster::block_on(HeadlessGpu::new())?;
        let target = OffscreenTarget::new(&gpu.device, size.0, size.1);
        Some(Self { gpu, target })
    }

    /// Resize the offscreen render target.
    pub fn resize(&mut self, width: u32, height: u32) {
        self.target = OffscreenTarget::new(&self.gpu.device, width, height);
    }

    /// Get the current render target view and dimensions.
    ///
    /// Returns a fresh view for each call (views are cheap to create).
    pub fn view(&self) -> (wgpu::TextureView, u32, u32) {
        let view = self
            .target
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());
        (view, self.target.size.0, self.target.size.1)
    }

    /// Get the target size.
    pub fn size(&self) -> (u32, u32) {
        self.target.size
    }

    /// Capture the current rendered frame as an image.
    ///
    /// Performs GPU→CPU readback with proper alignment and padding removal.
    pub fn capture_frame(&self) -> RgbaImage {
        self.target.read_pixels(&self.gpu.device, &self.gpu.queue)
    }

    /// Access the GPU device.
    pub fn device(&self) -> &wgpu::Device {
        &self.gpu.device
    }

    /// Access the GPU queue.
    pub fn queue(&self) -> &wgpu::Queue {
        &self.gpu.queue
    }

    /// Get the surface format.
    pub fn format(&self) -> wgpu::TextureFormat {
        self.gpu.format
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
    /// Shared headless renderer.
    renderer: HeadlessRenderer,
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
        let renderer = HeadlessRenderer::new(size)?;
        let screen_manager = ScreenManager::new();

        Some(Self {
            renderer,
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
    pub fn capture_frame(&self) -> RgbaImage {
        self.renderer.capture_frame()
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

// ============================================================================
// Map Rendering Test Harness (Phase 15.1)
// ============================================================================

#[allow(dead_code)]
pub struct MapTestHarness {
    /// Shared headless renderer.
    headless: HeadlessRenderer,
    /// Map renderer with pipelines and textures.
    map_renderer: crate::render::Renderer,
    /// Camera for view/projection control.
    camera: crate::camera::Camera,

    /// Province map image (for dimensions).
    province_map: image::RgbaImage,
    /// Province lookup table.
    province_lookup: eu4data::map::ProvinceLookup,
    /// Heightmap for terrain shading.
    heightmap: Option<image::GrayImage>,

    /// Game world state.
    world_state: eu4sim_core::WorldState,
    /// Country colors for political mode.
    country_colors: std::collections::HashMap<String, [u8; 3]>,
    /// Trade network for trade mode.
    trade_network: Option<eu4data::tradenodes::TradeNetwork>,
    /// Religion definitions for religion mode.
    religions: std::collections::HashMap<String, eu4data::religions::Religion>,
    /// Culture definitions for culture mode.
    cultures: std::collections::HashMap<String, eu4data::cultures::Culture>,
    /// Province-to-region mapping for region mode.
    region_mapping: Option<eu4data::regions::ProvinceRegionMapping>,
    /// Sea provinces for economy mode.
    sea_provinces: std::collections::HashSet<u32>,

    /// Current map mode.
    current_map_mode: crate::gui::MapMode,
    /// Whether lookup texture needs updating.
    lookup_dirty: bool,
}

#[allow(dead_code)]
impl MapTestHarness {
    /// Create a new map test harness.
    ///
    /// Returns `None` if:
    /// - No GPU adapter available (CI waiver)
    /// - EU4 game data not found (CI waiver)
    pub fn new() -> Option<Self> {
        use std::collections::{HashMap, HashSet};

        // Create headless renderer (1920x1080 default for map tests)
        let headless = HeadlessRenderer::new((1920, 1080))?;

        // Load province map and data
        let game_path = eu4data::path::detect_game_path()?;
        log::info!("MapTestHarness: Loading from {}", game_path.display());

        let provinces_path = game_path.join("map/provinces.bmp");
        let definitions_path = game_path.join("map/definition.csv");
        let heightmap_path = game_path.join("map/heightmap.bmp");

        let province_map = image::open(&provinces_path).ok()?.to_rgba8();
        let province_lookup = eu4data::map::ProvinceLookup::load(&definitions_path).ok()?;
        let heightmap = image::open(&heightmap_path).ok().map(|h| h.to_luma8());

        // Load world state (minimal - just for testing)
        let start_date = eu4sim_core::state::Date::new(1444, 11, 11);
        let (world_state, _adjacency) =
            eu4sim::loader::load_initial_state(&game_path, start_date, 42).ok()?;

        // Load country colors
        let tags = eu4data::countries::load_tags(&game_path).ok()?;
        let country_map = eu4data::countries::load_country_map(&game_path, &tags);
        let country_colors: HashMap<String, [u8; 3]> = country_map
            .into_iter()
            .filter_map(|(tag, country)| {
                if country.color.len() >= 3 {
                    Some((tag, [country.color[0], country.color[1], country.color[2]]))
                } else {
                    None
                }
            })
            .collect();

        // Load trade network
        let trade_network = eu4data::tradenodes::load_trade_network(&game_path).ok();

        // Load religions
        let religions = eu4data::religions::load_religions(&game_path).ok()?;

        // Load cultures
        let cultures = eu4data::cultures::load_cultures(&game_path).ok()?;

        // Load region mapping
        let region_mapping = eu4data::regions::load_region_mapping(&game_path).ok();

        // Build sea provinces set
        let sea_provinces: HashSet<u32> = world_state
            .provinces
            .iter()
            .filter(|(_, p)| p.is_sea)
            .map(|(id, _)| *id)
            .collect();

        // Create lookup table for renderer
        let lookup: HashMap<(u8, u8, u8), u32> = province_lookup
            .by_color
            .iter()
            .map(|(color, id)| (*color, *id))
            .collect();

        // Create map renderer
        let map_renderer = crate::render::Renderer::new(
            headless.device(),
            headless.queue(),
            headless.format(),
            &province_map,
            &lookup,
            heightmap.as_ref(),
        );

        // Create camera centered on map
        let map_width = province_map.width() as f32;
        let map_height = province_map.height() as f32;
        let content_aspect = map_width as f64 / map_height as f64;
        let camera = crate::camera::Camera::new(content_aspect);

        log::info!("MapTestHarness: Initialized successfully");

        Some(Self {
            headless,
            map_renderer,
            camera,
            province_map,
            province_lookup,
            heightmap,
            world_state,
            country_colors,
            trade_network,
            religions,
            cultures,
            region_mapping,
            sea_provinces,
            current_map_mode: crate::gui::MapMode::Political,
            lookup_dirty: true,
        })
    }

    /// Set camera position (in texture coordinates, 0.0-1.0).
    pub fn set_camera_position(&mut self, x: f64, y: f64) {
        self.camera.position = (x, y);
    }

    /// Set camera zoom level.
    pub fn set_camera_zoom(&mut self, zoom: f64) {
        self.camera.zoom = zoom.clamp(0.1, 10.0);
    }

    /// Set camera position and zoom at once.
    pub fn set_camera(&mut self, position: (f64, f64), zoom: f64) {
        self.set_camera_position(position.0, position.1);
        self.set_camera_zoom(zoom);
    }

    /// Get current camera (for inspection).
    pub fn camera(&self) -> &crate::camera::Camera {
        &self.camera
    }

    /// Set the current map mode.
    ///
    /// This will mark the lookup texture as dirty, causing it to be
    /// regenerated on the next render.
    pub fn set_map_mode(&mut self, mode: crate::gui::MapMode) {
        if self.current_map_mode != mode {
            self.current_map_mode = mode;
            self.lookup_dirty = true;
        }
    }

    /// Get current map mode.
    pub fn map_mode(&self) -> crate::gui::MapMode {
        self.current_map_mode
    }

    /// Render the current map state to an image.
    ///
    /// This will:
    /// 1. Update lookup texture if map mode changed
    /// 2. Update camera uniforms
    /// 3. Render map to offscreen texture
    /// 4. Read back pixels from GPU
    pub fn render_to_image(&mut self, size: (u32, u32)) -> RgbaImage {
        let (width, height) = size;

        // Update lookup texture if needed
        if self.lookup_dirty {
            self.update_lookup();
            self.lookup_dirty = false;
        }

        // Update camera uniforms
        let camera_uniform = self.camera.to_uniform(width as f32, height as f32);
        self.map_renderer
            .update_camera(self.headless.queue(), camera_uniform);

        // Update map mode uniform
        let map_mode_value = match self.current_map_mode {
            crate::gui::MapMode::Political => 0.0,
            crate::gui::MapMode::Terrain => 1.0,
            crate::gui::MapMode::Trade => 2.0,
            crate::gui::MapMode::Religion => 3.0,
            crate::gui::MapMode::Culture => 4.0,
            crate::gui::MapMode::Economy => 5.0,
            crate::gui::MapMode::Empire => 6.0,
            crate::gui::MapMode::Region => 7.0,
            crate::gui::MapMode::Diplomacy => 8.0,
            crate::gui::MapMode::Players => 9.0,
        };
        self.map_renderer
            .update_map_mode(self.headless.queue(), map_mode_value, (width, height));

        // Create offscreen target
        let target = OffscreenTarget::new(self.headless.device(), width, height);

        // Render to offscreen texture
        let mut encoder =
            self.headless
                .device()
                .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                    label: Some("Map Test Render Encoder"),
                });

        {
            let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("Map Test Render Pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &target.view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
            });

            // Draw map (fullscreen triangle)
            render_pass.set_pipeline(&self.map_renderer.pipeline);
            render_pass.set_bind_group(0, &self.map_renderer.bind_group, &[]);
            render_pass.draw(0..3, 0..1);
        }

        self.headless
            .queue()
            .submit(std::iter::once(encoder.finish()));

        // Read back pixels
        target.read_pixels(self.headless.device(), self.headless.queue())
    }

    /// Update the lookup texture based on current map mode.
    ///
    /// This generates the province color data and uploads it to the GPU.
    fn update_lookup(&mut self) {
        use crate::gui::MapMode;

        match self.current_map_mode {
            MapMode::Political => self.update_political_lookup(),
            MapMode::Terrain => {
                // Terrain mode uses heightmap shading, no lookup needed
            }
            MapMode::Trade => self.update_trade_lookup(),
            MapMode::Religion => self.update_religion_lookup(),
            MapMode::Culture => self.update_culture_lookup(),
            MapMode::Economy => self.update_economy_lookup(),
            MapMode::Empire => self.update_empire_lookup(),
            MapMode::Region => self.update_region_lookup(),
            MapMode::Diplomacy => {
                // TODO: Implement diplomacy mode
            }
            MapMode::Players => {
                // TODO: Implement players mode
            }
        }
    }

    fn update_political_lookup(&mut self) {
        use crate::render::LOOKUP_SIZE;

        let mut data = vec![[0u8; 4]; LOOKUP_SIZE as usize];

        for (&province_id, province) in &self.world_state.provinces {
            let id = province_id as usize;
            if id >= LOOKUP_SIZE as usize {
                continue;
            }

            if let Some(ref owner) = province.owner
                && let Some(color) = self.country_colors.get(owner)
            {
                data[id] = [color[0], color[1], color[2], 255];
            }
        }

        self.write_lookup_data(&data);
    }

    fn update_trade_lookup(&mut self) {
        use crate::render::LOOKUP_SIZE;

        let mut data = vec![[0u8; 4]; LOOKUP_SIZE as usize];

        if let Some(ref network) = self.trade_network {
            for (&province_id, _province) in &self.world_state.provinces {
                let id = province_id as usize;
                if id >= LOOKUP_SIZE as usize {
                    continue;
                }

                if let Some(&trade_node_id) = self.world_state.province_trade_node.get(&province_id)
                {
                    // Find the trade node definition by ID (compare using .0 to access inner u16)
                    if let Some(node_def) = network.nodes.iter().find(|n| n.id.0 == trade_node_id.0)
                    {
                        let color = &node_def.color;
                        data[id] = [color[0], color[1], color[2], 255];
                    }
                }
            }
        }

        self.write_lookup_data(&data);
    }

    fn update_religion_lookup(&mut self) {
        use crate::render::LOOKUP_SIZE;

        let mut data = vec![[0u8; 4]; LOOKUP_SIZE as usize];

        for (&province_id, province) in &self.world_state.provinces {
            let id = province_id as usize;
            if id >= LOOKUP_SIZE as usize {
                continue;
            }

            if let Some(ref religion_name) = province.religion
                && let Some(religion) = self.religions.get(religion_name)
            {
                let color = &religion.color;
                data[id] = [color[0], color[1], color[2], 255];
            }
        }

        self.write_lookup_data(&data);
    }

    fn update_culture_lookup(&mut self) {
        use crate::render::LOOKUP_SIZE;

        let mut data = vec![[0u8; 4]; LOOKUP_SIZE as usize];

        for (&province_id, province) in &self.world_state.provinces {
            let id = province_id as usize;
            if id >= LOOKUP_SIZE as usize {
                continue;
            }

            if let Some(ref culture_name) = province.culture
                && let Some(culture) = self.cultures.get(culture_name)
            {
                let color = &culture.color;
                data[id] = [color[0], color[1], color[2], 255];
            }
        }

        self.write_lookup_data(&data);
    }

    fn update_economy_lookup(&mut self) {
        use crate::render::LOOKUP_SIZE;

        let mut data = vec![[0u8; 4]; LOOKUP_SIZE as usize];

        for (&province_id, province) in &self.world_state.provinces {
            let id = province_id as usize;
            if id >= LOOKUP_SIZE as usize {
                continue;
            }

            if self.sea_provinces.contains(&province_id) {
                data[id] = [40, 80, 120, 255];
                continue;
            }

            let total_dev = province.base_tax.to_f32()
                + province.base_production.to_f32()
                + province.base_manpower.to_f32();

            let intensity = ((total_dev / 30.0).min(1.0) * 200.0) as u8;
            data[id] = [intensity, intensity / 2, 0, 255];
        }

        self.write_lookup_data(&data);
    }

    fn update_empire_lookup(&mut self) {
        use crate::render::LOOKUP_SIZE;

        let mut data = vec![[0u8; 4]; LOOKUP_SIZE as usize];

        let emperor = self.world_state.global.hre.emperor.as_ref();

        for (&province_id, province) in &self.world_state.provinces {
            let id = province_id as usize;
            if id >= LOOKUP_SIZE as usize {
                continue;
            }

            if let Some(ref owner) = province.owner {
                let is_emperor = emperor == Some(owner);
                let is_hre_member = province.is_in_hre;

                let color = if is_emperor {
                    [255, 215, 0, 255] // Gold for emperor
                } else if is_hre_member {
                    [180, 180, 255, 255] // Light blue for HRE members
                } else if let Some(country_color) = self.country_colors.get(owner) {
                    [country_color[0], country_color[1], country_color[2], 255]
                } else {
                    [128, 128, 128, 255] // Gray for unknown
                };
                data[id] = color;
            }
        }

        self.write_lookup_data(&data);
    }

    fn update_region_lookup(&mut self) {
        use crate::render::LOOKUP_SIZE;

        let mut data = vec![[0u8; 4]; LOOKUP_SIZE as usize];

        if let Some(ref mapping) = self.region_mapping {
            for (&province_id, region_name) in &mapping.province_to_region {
                let id = province_id as usize;
                if id >= LOOKUP_SIZE as usize {
                    continue;
                }

                let hash = region_name
                    .bytes()
                    .fold(0u32, |acc, b| acc.wrapping_mul(31).wrapping_add(b as u32));
                let r = ((hash >> 16) & 0xFF) as u8;
                let g = ((hash >> 8) & 0xFF) as u8;
                let b = (hash & 0xFF) as u8;
                data[id] = [r, g, b, 255];
            }
        }

        self.write_lookup_data(&data);
    }

    fn write_lookup_data(&mut self, data: &[[u8; 4]]) {
        use crate::render::LOOKUP_SIZE;

        let bytes: Vec<u8> = data.iter().flat_map(|c| *c).collect();

        self.headless.queue().write_texture(
            wgpu::ImageCopyTexture {
                texture: &self.map_renderer.lookup_texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            &bytes,
            wgpu::ImageDataLayout {
                offset: 0,
                bytes_per_row: Some(LOOKUP_SIZE * 4),
                rows_per_image: Some(1),
            },
            wgpu::Extent3d {
                width: LOOKUP_SIZE,
                height: 1,
                depth_or_array_layers: 1,
            },
        );
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

    // ========================================================================
    // Map Rendering Tests (Phase 15)
    // ========================================================================

    #[test]
    fn test_map_harness_political_mode() {
        let Some(mut harness) = MapTestHarness::new() else {
            return;
        };

        // Set camera to center of map (0.5, 0.5 in texture coords) at 1:1 zoom
        harness.set_camera((0.5, 0.5), 1.0);

        // Set political mode
        harness.set_map_mode(crate::gui::MapMode::Political);

        // Render at 1920x1080 (consistent with other tests)
        let image = harness.render_to_image((1920, 1080));

        // Verify dimensions
        assert_eq!(image.width(), 1920);
        assert_eq!(image.height(), 1080);

        // Save as golden snapshot
        assert_snapshot(&image, "map_political_center");
    }

    #[test]
    fn test_map_harness_mode_switching() {
        let Some(mut harness) = MapTestHarness::new() else {
            return;
        };

        // Start in political mode
        assert_eq!(harness.map_mode(), crate::gui::MapMode::Political);

        // Switch to terrain mode
        harness.set_map_mode(crate::gui::MapMode::Terrain);
        assert_eq!(harness.map_mode(), crate::gui::MapMode::Terrain);

        // Switch to trade mode
        harness.set_map_mode(crate::gui::MapMode::Trade);
        assert_eq!(harness.map_mode(), crate::gui::MapMode::Trade);

        // Render should work in trade mode at 1920x1080
        let image = harness.render_to_image((1920, 1080));
        assert_eq!(image.width(), 1920);
        assert_eq!(image.height(), 1080);
    }

    #[test]
    fn test_map_harness_camera_control() {
        let Some(mut harness) = MapTestHarness::new() else {
            return;
        };

        // Set specific camera position (texture coords)
        harness.set_camera_position(0.3, 0.7);
        assert_eq!(harness.camera().position.0, 0.3);
        assert_eq!(harness.camera().position.1, 0.7);

        // Set zoom
        harness.set_camera_zoom(2.0);
        assert_eq!(harness.camera().zoom, 2.0);

        // Set both at once
        harness.set_camera((0.8, 0.2), 0.5);
        assert_eq!(harness.camera().position.0, 0.8);
        assert_eq!(harness.camera().position.1, 0.2);
        assert_eq!(harness.camera().zoom, 0.5);
    }

    // ========================================================================
    // Terrain Mode Tests (Phase 15.3)
    // ========================================================================

    #[test]
    fn test_map_terrain_center() {
        let Some(mut harness) = MapTestHarness::new() else {
            return;
        };

        harness.set_camera((0.5, 0.5), 1.0);
        harness.set_map_mode(crate::gui::MapMode::Terrain);

        let image = harness.render_to_image((1920, 1080));
        assert_snapshot(&image, "map_terrain_center");
    }

    #[test]
    fn test_map_terrain_europe() {
        let Some(mut harness) = MapTestHarness::new() else {
            return;
        };

        // Focus on Western Europe (Alps region)
        harness.set_camera((0.52, 0.42), 2.0);
        harness.set_map_mode(crate::gui::MapMode::Terrain);

        let image = harness.render_to_image((1920, 1080));
        assert_snapshot(&image, "map_terrain_europe");
    }

    #[test]
    fn test_map_terrain_asia() {
        let Some(mut harness) = MapTestHarness::new() else {
            return;
        };

        // Focus on Himalayas region
        harness.set_camera((0.70, 0.42), 2.0);
        harness.set_map_mode(crate::gui::MapMode::Terrain);

        let image = harness.render_to_image((1920, 1080));
        assert_snapshot(&image, "map_terrain_asia");
    }

    // ========================================================================
    // Trade Mode Tests (Phase 15.2)
    // ========================================================================

    #[test]
    fn test_map_trade_center() {
        let Some(mut harness) = MapTestHarness::new() else {
            return;
        };

        harness.set_camera((0.5, 0.5), 1.0);
        harness.set_map_mode(crate::gui::MapMode::Trade);

        let image = harness.render_to_image((1920, 1080));
        assert_snapshot(&image, "map_trade_center");
    }

    #[test]
    fn test_map_trade_europe() {
        let Some(mut harness) = MapTestHarness::new() else {
            return;
        };

        // Focus on European trade nodes (English Channel, Venice, Baltic)
        harness.set_camera((0.52, 0.40), 2.0);
        harness.set_map_mode(crate::gui::MapMode::Trade);

        let image = harness.render_to_image((1920, 1080));
        assert_snapshot(&image, "map_trade_europe");
    }

    #[test]
    fn test_map_trade_asia() {
        let Some(mut harness) = MapTestHarness::new() else {
            return;
        };

        // Focus on Asian trade nodes (Beijing, Hangzhou, Canton, Malacca)
        harness.set_camera((0.75, 0.45), 2.0);
        harness.set_map_mode(crate::gui::MapMode::Trade);

        let image = harness.render_to_image((1920, 1080));
        assert_snapshot(&image, "map_trade_asia");
    }

    // ========================================================================
    // Religion Mode Tests
    // ========================================================================

    #[test]
    fn test_map_religion_center() {
        let Some(mut harness) = MapTestHarness::new() else {
            return;
        };

        harness.set_camera((0.5, 0.5), 1.0);
        harness.set_map_mode(crate::gui::MapMode::Religion);

        let image = harness.render_to_image((1920, 1080));
        assert_snapshot(&image, "map_religion_center");
    }

    #[test]
    fn test_map_religion_europe() {
        let Some(mut harness) = MapTestHarness::new() else {
            return;
        };

        // Focus on Europe (Catholic vs Protestant divide)
        harness.set_camera((0.52, 0.42), 2.0);
        harness.set_map_mode(crate::gui::MapMode::Religion);

        let image = harness.render_to_image((1920, 1080));
        assert_snapshot(&image, "map_religion_europe");
    }

    #[test]
    fn test_map_religion_middle_east() {
        let Some(mut harness) = MapTestHarness::new() else {
            return;
        };

        // Focus on Middle East (Islamic regions)
        harness.set_camera((0.58, 0.46), 2.0);
        harness.set_map_mode(crate::gui::MapMode::Religion);

        let image = harness.render_to_image((1920, 1080));
        assert_snapshot(&image, "map_religion_middle_east");
    }

    // ========================================================================
    // Culture Mode Tests
    // ========================================================================

    #[test]
    fn test_map_culture_center() {
        let Some(mut harness) = MapTestHarness::new() else {
            return;
        };

        harness.set_camera((0.5, 0.5), 1.0);
        harness.set_map_mode(crate::gui::MapMode::Culture);

        let image = harness.render_to_image((1920, 1080));
        assert_snapshot(&image, "map_culture_center");
    }

    #[test]
    fn test_map_culture_europe() {
        let Some(mut harness) = MapTestHarness::new() else {
            return;
        };

        // Focus on Europe (diverse culture groups)
        harness.set_camera((0.52, 0.42), 2.0);
        harness.set_map_mode(crate::gui::MapMode::Culture);

        let image = harness.render_to_image((1920, 1080));
        assert_snapshot(&image, "map_culture_europe");
    }

    // ========================================================================
    // Economy Mode Tests
    // ========================================================================

    #[test]
    fn test_map_economy_center() {
        let Some(mut harness) = MapTestHarness::new() else {
            return;
        };

        harness.set_camera((0.5, 0.5), 1.0);
        harness.set_map_mode(crate::gui::MapMode::Economy);

        let image = harness.render_to_image((1920, 1080));
        assert_snapshot(&image, "map_economy_center");
    }

    // ========================================================================
    // Empire Mode Tests (HRE)
    // ========================================================================

    #[test]
    fn test_map_empire_center() {
        let Some(mut harness) = MapTestHarness::new() else {
            return;
        };

        harness.set_camera((0.5, 0.5), 1.0);
        harness.set_map_mode(crate::gui::MapMode::Empire);

        let image = harness.render_to_image((1920, 1080));
        assert_snapshot(&image, "map_empire_center");
    }

    #[test]
    fn test_map_empire_hre() {
        let Some(mut harness) = MapTestHarness::new() else {
            return;
        };

        // Focus on HRE region in Europe
        harness.set_camera((0.52, 0.42), 2.0);
        harness.set_map_mode(crate::gui::MapMode::Empire);

        let image = harness.render_to_image((1920, 1080));
        assert_snapshot(&image, "map_empire_hre");
    }

    // ========================================================================
    // Region Mode Tests
    // ========================================================================

    #[test]
    fn test_map_region_center() {
        let Some(mut harness) = MapTestHarness::new() else {
            return;
        };

        harness.set_camera((0.5, 0.5), 1.0);
        harness.set_map_mode(crate::gui::MapMode::Region);

        let image = harness.render_to_image((1920, 1080));
        assert_snapshot(&image, "map_region_center");
    }
}
