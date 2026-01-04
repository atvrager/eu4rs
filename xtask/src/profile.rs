//! Tracy profiling automation
//!
//! Builds eu4game with Tracy enabled, runs it for a specified duration,
//! and generates a markdown report from the captured data.

use anyhow::{Context, Result};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::thread;
use std::time::Duration;

/// Run Tracy profiling session
pub fn run_profile(duration_secs: u64, output_dir: Option<PathBuf>) -> Result<()> {
    println!("🔬 Tracy Profiling Session\n");

    // Determine output directory
    let timestamp = chrono::Local::now().format("%Y%m%d_%H%M%S").to_string();
    let output = output_dir.unwrap_or_else(|| {
        PathBuf::from("profiling").join(&timestamp)
    });

    std::fs::create_dir_all(&output)
        .context("Failed to create output directory")?;

    println!("Output directory: {}", output.display());
    println!("Duration: {}s\n", duration_secs);

    // Step 1: Build with Tracy enabled
    println!("[1/3] Building eu4game with Tracy...");
    super::run_command("cargo", &[
        "build",
        "--release",
        "-p",
        "eu4game",
        "--features",
        "eu4game/tracy",
    ])?;

    // Step 2: Run the app
    println!("\n[2/3] Running eu4game ({}s)...", duration_secs);
    println!("Note: Tracy server should be running to capture data.\n");

    let binary = if cfg!(windows) {
        "target/release/eu4game.exe"
    } else {
        "target/release/eu4game"
    };

    // Run in background and kill after duration
    let mut child = Command::new(binary)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .context("Failed to start eu4game")?;

    // Wait for specified duration
    thread::sleep(Duration::from_secs(duration_secs));

    // Kill the process
    #[cfg(windows)]
    {
        let _ = Command::new("taskkill")
            .args(["/PID", &child.id().to_string(), "/F"])
            .status();
    }
    #[cfg(not(windows))]
    {
        let _ = child.kill();
    }

    let _ = child.wait();
    println!("Application stopped.");

    // Step 3: Find and process .tracy file
    println!("\n[3/3] Processing capture...");

    // Tracy saves captures as <appname>.tracy in current directory
    let tracy_file = find_latest_tracy_file(".")?;

    if let Some(tracy_path) = tracy_file {
        let dest = output.join("capture.tracy");
        std::fs::rename(&tracy_path, &dest)
            .context("Failed to move .tracy file")?;

        println!("✓ Capture saved: {}", dest.display());

        // Try to export CSV if tracy-csvexport is available
        if let Ok(_) = Command::new("tracy-csvexport").arg("--version").output() {
            println!("Exporting to CSV...");
            let csv_path = output.join("profile.csv");
            let status = Command::new("tracy-csvexport")
                .arg(&dest)
                .stdout(std::fs::File::create(&csv_path)?)
                .status()?;

            if status.success() {
                println!("✓ CSV exported: {}", csv_path.display());

                // Generate markdown report
                if let Err(e) = generate_report(&csv_path, &output) {
                    println!("⚠ Failed to generate report: {}", e);
                }
            } else {
                println!("⚠ tracy-csvexport failed");
            }
        } else {
            println!("⚠ tracy-csvexport not found. Install Tracy tools for CSV export.");
            println!("  Download: https://github.com/wolfpld/tracy/releases");
        }
    } else {
        println!("⚠ No .tracy file found. Make sure Tracy server was running.");
    }

    println!("\nProfiled data saved to: {}", output.display());

    // Create 'latest' symlink
    #[cfg(unix)]
    {
        let latest = Path::new("profiling/latest");
        let _ = std::fs::remove_file(latest);
        let _ = std::os::unix::fs::symlink(&output, latest);
    }

    Ok(())
}

/// Find the most recently modified .tracy file in a directory
fn find_latest_tracy_file(dir: &str) -> Result<Option<PathBuf>> {
    let mut tracy_files: Vec<_> = std::fs::read_dir(dir)?
        .filter_map(|e| e.ok())
        .filter(|e| {
            e.path().extension().and_then(|s| s.to_str()) == Some("tracy")
        })
        .collect();

    if tracy_files.is_empty() {
        return Ok(None);
    }

    // Sort by modification time, newest first
    tracy_files.sort_by_key(|e| {
        e.metadata()
            .and_then(|m| m.modified())
            .unwrap_or(std::time::SystemTime::UNIX_EPOCH)
    });
    tracy_files.reverse();

    Ok(Some(tracy_files[0].path()))
}

/// Generate markdown report from CSV
fn generate_report(csv_path: &Path, output_dir: &Path) -> Result<()> {
    // Call Python script to analyze CSV
    let script_path = Path::new("scripts/analyze_tracy.py");

    if !script_path.exists() {
        return Err(anyhow::anyhow!("scripts/analyze_tracy.py not found"));
    }

    let report_path = output_dir.join("report.md");

    let status = Command::new("python3")
        .arg(script_path)
        .arg(csv_path)
        .stdout(std::fs::File::create(&report_path)?)
        .status()?;

    if status.success() {
        println!("✓ Report generated: {}", report_path.display());
    } else {
        return Err(anyhow::anyhow!("Report generation failed"));
    }

    Ok(())
}
