//! Snapshot testing utilities for GUI visual verification.
//!
//! Provides golden image comparison for GUI components, enabling
//! regression testing of visual output.

use image::RgbaImage;
use std::path::{Path, PathBuf};

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
