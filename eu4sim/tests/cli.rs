// The cargo_bin! macro requires build script setup that's overkill for simple tests.
// Suppress deprecation warning on the function until we need custom build-dir support.
#![allow(deprecated)]

use assert_cmd::cargo::cargo_bin;
use std::process::Command;

#[test]
fn test_game_path_respected() {
    // Test that --game-path is actually used, not auto-detected
    let mut cmd = Command::new(cargo_bin("eu4sim"));

    // Point to a non-existent directory - should fail to load, not fall back to Steam
    let output = cmd
        .arg("--game-path")
        .arg("/nonexistent/path")
        .arg("-t")
        .arg("1")
        .output()
        .expect("failed to execute process");

    // Should fail because the path doesn't exist, not succeed by falling back to Steam
    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);

    // Should contain an error about the path, not successfully load from Steam
    assert!(
        stderr.contains("nonexistent")
            || stderr.contains("No such file")
            || stderr.contains("cannot find"),
        "Should fail with path error, not silently use Steam path. Stderr: {}",
        stderr
    );
}

#[test]
fn test_help_flag() {
    let mut cmd = Command::new(cargo_bin("eu4sim"));
    let output = cmd.arg("--help").output().expect("failed to execute");

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("Usage:"));
    assert!(stdout.contains("--game-path"));
}
