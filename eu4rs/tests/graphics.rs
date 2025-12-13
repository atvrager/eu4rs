use std::process::Command;
use tempfile::tempdir;

#[test]
fn test_snapshot_generation() {
    let dir = tempdir().unwrap();
    let snapshot_path = dir.path().join("test_snapshot.png");
    let path_str = snapshot_path.to_str().unwrap();

    let output = Command::new("cargo")
        .args(["run", "--", "snapshot", "--output", path_str])
        .output()
        .expect("failed to execute process");

    // Check if the command succeeded
    assert!(output.status.success());

    // NOTE: In CI without Vulkan, this will exit 0 but with a warning.
    // If it did render, we check for file existence.
    // If it didn't, we check stderr for the waiver message?
    // Let's check: if file exists, it should be valid image. If not, check stderr.

    if snapshot_path.exists() {
        let metadata = std::fs::metadata(&snapshot_path).unwrap();
        assert!(metadata.len() > 0, "Snapshot file is empty");

        let img = image::open(&snapshot_path).expect("Failed to open generated snapshot");
        let (width, height) = (img.width(), img.height());
        assert!(width > 0);
        assert!(height > 0);

        // Load reference snapshot
        // The reference is stored in tests/reference_snapshot.png relative to cargo root
        let reference_path = std::path::Path::new("tests/reference_snapshot.png");
        if reference_path.exists() {
            let ref_img = image::open(reference_path).expect("Failed to open reference snapshot");

            assert_eq!(
                img.dimensions(),
                ref_img.dimensions(),
                "Dimensions mismatch"
            );

            // Pixel comparison (exact match for now)
            // We can iterate pixels and compare.
            use image::GenericImageView;
            let mut diff_count = 0;
            for (x, y, pixel) in img.pixels() {
                let ref_pixel = ref_img.get_pixel(x, y);
                if pixel != ref_pixel {
                    diff_count += 1;
                }
            }
            // Allow 0 diffs for now. If renderers vary slightly across GPUs (likely with float precision),
            // we might need a tolerance. But for exact same wgpu pipeline on same machine, it should be identical.
            // If checking in reference from one machine and testing on another, we might need tolerance.
            // Let's set a very strict tolerance or logging.
            if diff_count > 0 {
                panic!(
                    "Snapshot differs from reference! {} pixels different.",
                    diff_count
                );
            }
        } else {
            println!(
                "No reference snapshot found at {:?}. Skipping comparison.",
                reference_path
            );
            // Optional: fail if reference missing? User said "check that in as an artifact".
            // So we assume it should exist.
            panic!(
                "Reference snapshot missing! Please run 'cargo run -- snapshot --output tests/reference_snapshot.png' to generate it."
            );
        }
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr);
        // It's acceptable to not produce a file if we hit the CI waiver
        if !stderr.contains("No suitable graphics adapter found") {
            panic!(
                "Snapshot failed to generate file AND no CI waiver message found.\nStderr: {}",
                stderr
            );
        }
    }
}
