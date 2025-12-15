use image::RgbaImage;
use std::path::{Path, PathBuf};

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
        // Force success
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

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn test_assert_snapshot_match() {
        let dir = tempdir().unwrap();
        let golden_dir = dir.path();

        // 1. Create an image
        let mut img = RgbaImage::new(2, 2);
        for (_, _, pixel) in img.enumerate_pixels_mut() {
            *pixel = image::Rgba([255, 0, 0, 255]);
        }

        // 2. First run: should create the snapshot (no panic)
        assert_snapshot_at(&img, "test_snap", golden_dir, false);
        let snap_path = golden_dir.join("test_snap.png");
        assert!(snap_path.exists());

        // 3. Second run: identical image should pass
        assert_snapshot_at(&img, "test_snap", golden_dir, false);

        // 4. Mismatch: modify image and expect panic
        let mut img_bad = img.clone();
        img_bad.put_pixel(0, 0, image::Rgba([0, 255, 0, 255]));

        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            assert_snapshot_at(&img_bad, "test_snap", golden_dir, false);
        }));
        assert!(result.is_err(), "Should have panicked on mismatch");

        // Check if _actual was saved
        let actual_path = golden_dir.join("test_snap_actual.png");
        assert!(
            actual_path.exists(),
            "Should have saved actual image on failure"
        );
    }
}
