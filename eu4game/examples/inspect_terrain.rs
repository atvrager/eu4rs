//! Diagnostic tool to inspect terrain.bmp loading and color distribution
//!
//! Usage: cargo run --example inspect_terrain

use eu4game::world_loader;

fn main() {
    env_logger::init();

    println!("=== Terrain Texture Inspector ===\n");

    match world_loader::load_terrain_texture() {
        Some(img) => {
            println!("✓ Loaded terrain texture: {}x{}", img.width(), img.height());
            println!();

            // Sample some pixels to see color distribution
            let samples = [
                (0, 0, "Top-left"),
                (img.width() / 2, img.height() / 2, "Center"),
                (img.width() - 1, 0, "Top-right"),
                (0, img.height() - 1, "Bottom-left"),
                (img.width() - 1, img.height() - 1, "Bottom-right"),
            ];

            println!("Sample pixels:");
            for (x, y, label) in samples {
                let pixel = img.get_pixel(x, y);
                println!(
                    "  {} ({}, {}): R={:3} G={:3} B={:3} A={:3}",
                    label, x, y, pixel[0], pixel[1], pixel[2], pixel[3]
                );
            }
            println!();

            // Analyze color distribution
            let mut color_histogram: std::collections::HashMap<[u8; 3], u32> =
                std::collections::HashMap::new();

            for pixel in img.pixels() {
                let rgb = [pixel[0], pixel[1], pixel[2]];
                *color_histogram.entry(rgb).or_insert(0) += 1;
            }

            println!("Color statistics:");
            println!("  Unique colors: {}", color_histogram.len());

            // Find most common colors
            let mut colors: Vec<_> = color_histogram.iter().collect();
            colors.sort_by(|a, b| b.1.cmp(a.1));

            println!("  Top 10 most common colors:");
            for (i, (color, count)) in colors.iter().take(10).enumerate() {
                let percentage = (*count as f64 / (img.width() * img.height()) as f64) * 100.0;
                println!(
                    "    {}. RGB({:3}, {:3}, {:3}): {:7} pixels ({:.2}%)",
                    i + 1,
                    color[0],
                    color[1],
                    color[2],
                    count,
                    percentage
                );
            }
            println!();

            // Check if it's monochromatic
            let has_color = colors.iter().any(|(rgb, _)| {
                let r = rgb[0];
                let g = rgb[1];
                let b = rgb[2];
                // Check if colors differ significantly
                let max_diff = r.max(g).max(b) - r.min(g).min(b);
                max_diff > 10
            });

            if !has_color {
                println!("⚠ WARNING: Texture appears to be monochromatic/grayscale!");
                println!("  This suggests terrain.bmp is being loaded incorrectly.");
                println!("  EU4 uses an indexed palette that needs proper color mapping.");
            } else {
                println!("✓ Texture has color variation");
            }
        }
        None => {
            println!("✗ Failed to load terrain texture");
            println!("  Make sure EU4 is installed and the path is detected correctly");
        }
    }
}
