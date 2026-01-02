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
}
