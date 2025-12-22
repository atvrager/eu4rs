//! Window capture for EU4 screen reading.
//!
//! Uses `xcap` for cross-platform window capture.
//! Supports X11, Wayland, Windows, and macOS.

use anyhow::{Context, Result};
use image::RgbaImage;
use xcap::Window;

/// Find a window by title substring.
///
/// Returns the first window whose title contains the given substring (case-insensitive).
pub fn find_window(title_substring: &str) -> Result<Window> {
    let windows = Window::all().context("Failed to enumerate windows")?;

    let needle = title_substring.to_lowercase();

    for window in windows {
        let title = window.title().to_lowercase();
        if title.contains(&needle) {
            log::info!(
                "Found window: \"{}\" ({}x{})",
                window.title(),
                window.width(),
                window.height()
            );
            return Ok(window);
        }
    }

    anyhow::bail!("No window found containing \"{}\"", title_substring)
}

/// List all visible windows (for debugging).
pub fn list_windows() -> Result<Vec<WindowInfo>> {
    let windows = Window::all().context("Failed to enumerate windows")?;

    let mut infos = Vec::new();
    for window in windows {
        let title = window.title();
        if title.is_empty() {
            continue; // Skip untitled windows
        }

        infos.push(WindowInfo {
            title: title.to_string(),
            width: window.width(),
            height: window.height(),
            x: window.x(),
            y: window.y(),
        });
    }

    Ok(infos)
}

/// Basic window information.
#[derive(Debug, Clone)]
pub struct WindowInfo {
    pub title: String,
    pub width: u32,
    pub height: u32,
    pub x: i32,
    pub y: i32,
}

/// Capture a screenshot of the given window.
///
/// Returns an RGBA image of the window contents.
pub fn capture_window(window: &Window) -> Result<RgbaImage> {
    let capture = window.capture_image().context("Failed to capture window")?;

    log::debug!(
        "Captured {}x{} screenshot",
        capture.width(),
        capture.height()
    );

    Ok(capture)
}

/// Capture a screenshot and save it to a file (for debugging).
pub fn capture_and_save(window: &Window, path: &str) -> Result<()> {
    let image = capture_window(window)?;
    image.save(path).context("Failed to save screenshot")?;
    log::info!("Saved screenshot to {}", path);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_list_windows() {
        // This will fail in headless CI, but useful for local testing
        let windows = list_windows();
        if let Ok(windows) = windows {
            println!("Found {} windows", windows.len());
            for w in windows.iter().take(10) {
                println!("  {} ({}x{})", w.title, w.width, w.height);
            }
        }
    }
}
