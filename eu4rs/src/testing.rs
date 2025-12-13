use image::RgbaImage;
use std::path::PathBuf;

pub fn assert_snapshot(actual: &RgbaImage, name: &str) {
    let manifest_dir = std::env::var("CARGO_MANIFEST_DIR").unwrap();
    let golden_dir = PathBuf::from(manifest_dir).join("tests/goldens");
    std::fs::create_dir_all(&golden_dir).unwrap();

    let golden_path = golden_dir.join(format!("{}.png", name));
    let update = std::env::var("UPDATE_SNAPSHOTS").is_ok();

    if update || !golden_path.exists() {
        actual
            .save(&golden_path)
            .expect("Failed to save golden image");
        println!("Saved golden: {:?}", golden_path);
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

    // Allow slight rendering differences (e.g. font anti-aliasing cross-platform)
    // But for now strict mapping is best if we bundle font.
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
