//! EU4 Bridge - Connect trained AI to real Europa Universalis IV.
//!
//! Phase A: Screen capture and OCR proof of concept.

mod capture;
mod regions;

use anyhow::Result;
use clap::{Parser, Subcommand};
use image::{Rgb, RgbImage};
use imageproc::drawing::draw_hollow_rect_mut;
use imageproc::rect::Rect;

#[derive(Parser)]
#[command(name = "eu4-bridge")]
#[command(about = "Bridge between EU4 game and AI via screen capture")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// List all visible windows
    ListWindows,

    /// Capture a window screenshot
    Capture {
        /// Window title to search for (substring match)
        #[arg(short, long, default_value = "Europa Universalis")]
        window: String,

        /// Output file path
        #[arg(short, long, default_value = "capture.png")]
        output: String,
    },

    /// Test capture loop (capture every N seconds)
    Watch {
        /// Window title to search for
        #[arg(short, long, default_value = "Europa Universalis")]
        window: String,

        /// Interval in seconds
        #[arg(short, long, default_value = "5")]
        interval: u64,
    },

    /// Capture screenshot with OCR region overlays for calibration
    Calibrate {
        /// Window title to search for (substring match)
        #[arg(short, long, default_value = "Europa Universalis")]
        window: String,

        /// Output file path for annotated screenshot
        #[arg(short, long, default_value = "calibrate.png")]
        output: String,
    },

    /// Adjust region boxes on existing screenshot (no game required)
    Adjust {
        /// Input screenshot file
        #[arg(short, long, default_value = "calibrate.png")]
        input: String,

        /// Output file path
        #[arg(short, long, default_value = "adjusted.png")]
        output: String,

        /// Global X offset for all regions (positive = right)
        #[arg(long, default_value = "0", allow_hyphen_values = true)]
        dx: i32,

        /// Global Y offset for all regions (positive = down)
        #[arg(long, default_value = "0", allow_hyphen_values = true)]
        dy: i32,

        /// Scale factor for box sizes (1.0 = no change)
        #[arg(long, default_value = "1.0")]
        scale: f32,

        /// Per-region X offset: "name:offset" (e.g., "Date:-50")
        #[arg(long = "rx", value_name = "NAME:DX")]
        region_dx: Vec<String>,

        /// Per-region Y offset: "name:offset" (e.g., "Treasury:10")
        #[arg(long = "ry", value_name = "NAME:DY")]
        region_dy: Vec<String>,
    },
}

fn main() -> Result<()> {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();

    let cli = Cli::parse();

    match cli.command {
        Commands::ListWindows => {
            let windows = capture::list_windows()?;
            println!("Found {} windows:", windows.len());
            for w in &windows {
                println!(
                    "  \"{}\" - {}x{} at ({}, {})",
                    w.title, w.width, w.height, w.x, w.y
                );
            }
        }

        Commands::Capture { window, output } => {
            let win = capture::find_window(&window)?;
            capture::capture_and_save(&win, &output)?;
            println!("Captured to {}", output);
        }

        Commands::Watch { window, interval } => {
            let win = capture::find_window(&window)?;
            println!(
                "Watching \"{}\" every {}s (Ctrl+C to stop)",
                win.title(),
                interval
            );

            let mut frame = 0;
            loop {
                let filename = format!("frame_{:04}.png", frame);
                if let Err(e) = capture::capture_and_save(&win, &filename) {
                    log::error!("Capture failed: {}", e);
                }
                frame += 1;
                std::thread::sleep(std::time::Duration::from_secs(interval));
            }
        }

        Commands::Calibrate { window, output } => {
            let win = capture::find_window(&window)?;
            let screenshot = capture::capture_window(&win)?;

            // Convert RGBA to RGB for imageproc
            let (width, height) = (screenshot.width(), screenshot.height());
            let mut rgb_image: RgbImage = RgbImage::new(width, height);
            for (x, y, pixel) in screenshot.enumerate_pixels() {
                rgb_image.put_pixel(x, y, Rgb([pixel[0], pixel[1], pixel[2]]));
            }

            // Draw region overlays (2px thick hollow rectangles)
            for region in regions::ALL_REGIONS {
                let color = Rgb(region.color);
                let rect =
                    Rect::at(region.x as i32, region.y as i32).of_size(region.width, region.height);

                // Draw multiple rectangles for thickness
                draw_hollow_rect_mut(&mut rgb_image, rect, color);
                if region.width > 2 && region.height > 2 {
                    let inner = Rect::at(region.x as i32 + 1, region.y as i32 + 1)
                        .of_size(region.width - 2, region.height - 2);
                    draw_hollow_rect_mut(&mut rgb_image, inner, color);
                }
            }

            // Save annotated image
            rgb_image.save(&output)?;
            println!("Saved calibration image to: {}", output);
            println!();

            // Print legend
            regions::print_legend();

            println!();
            println!("Adjust region coordinates in src/regions.rs if boxes are misaligned.");
        }

        Commands::Adjust {
            input,
            output,
            dx,
            dy,
            scale,
            region_dx,
            region_dy,
        } => {
            // Load existing screenshot (strips any existing overlays by loading original)
            // For now, just load the image and redraw boxes with offsets
            let img = image::open(&input)?;
            let mut rgb_image = img.to_rgb8();

            // Parse per-region offsets
            let parse_offsets = |args: &[String]| -> std::collections::HashMap<String, i32> {
                args.iter()
                    .filter_map(|s| {
                        let parts: Vec<&str> = s.split(':').collect();
                        if parts.len() == 2 {
                            parts[1].parse().ok().map(|v| (parts[0].to_lowercase(), v))
                        } else {
                            None
                        }
                    })
                    .collect()
            };

            let rx_map = parse_offsets(&region_dx);
            let ry_map = parse_offsets(&region_dy);

            println!("Applying adjustments: dx={}, dy={}, scale={:.2}", dx, dy, scale);
            println!();

            // Draw adjusted regions
            for region in regions::ALL_REGIONS {
                let name_lower = region.name.to_lowercase().replace(' ', "");

                // Apply global + per-region offsets
                let extra_dx = rx_map.get(&name_lower).copied().unwrap_or(0);
                let extra_dy = ry_map.get(&name_lower).copied().unwrap_or(0);

                let new_x = (region.x as i32 + dx + extra_dx).max(0) as u32;
                let new_y = (region.y as i32 + dy + extra_dy).max(0) as u32;
                let new_w = ((region.width as f32) * scale) as u32;
                let new_h = ((region.height as f32) * scale) as u32;

                let color = Rgb(region.color);
                let rect = Rect::at(new_x as i32, new_y as i32).of_size(new_w, new_h);

                // Draw thick border
                draw_hollow_rect_mut(&mut rgb_image, rect, color);
                if new_w > 2 && new_h > 2 {
                    let inner =
                        Rect::at(new_x as i32 + 1, new_y as i32 + 1).of_size(new_w - 2, new_h - 2);
                    draw_hollow_rect_mut(&mut rgb_image, inner, color);
                }

                println!(
                    "  {:12} x={:>4}, y={:>3}, w={:>3}, h={:>2}",
                    region.name, new_x, new_y, new_w, new_h
                );
            }

            rgb_image.save(&output)?;
            println!();
            println!("Saved to: {}", output);
            println!();
            println!("Usage examples:");
            println!("  Move all boxes right 50px:  --dx 50");
            println!("  Move all boxes down 10px:   --dy 10");
            println!("  Move just Date left 30px:   --rx date:-30");
            println!("  Scale all boxes 1.5x:       --scale 1.5");
        }
    }

    Ok(())
}
