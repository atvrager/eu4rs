//! Auto-generate eu4-bridge OCR regions from EU4 GUI files.
//!
//! This module parses EU4's `.gui` files using eu4game's parser, matches
//! elements to OCR regions, calculates screen positions, and generates
//! Rust code for `eu4-bridge/src/regions.rs`.

pub mod codegen;
pub mod gui_parser;
pub mod mapping;
pub mod position_calculator;
pub mod resolver;
pub mod types;

use mapping::REGION_MAPPINGS;

/// Entry point for region generation.
///
/// # Arguments
/// * `game_path` - Path to EU4 installation directory
/// * `output_path` - Where to write generated regions.rs
/// * `dry_run` - If true, print to stdout instead of writing file
///
/// # Returns
/// Generated Rust code as a string.
pub fn generate_regions(
    game_path: &str,
    output_path: Option<&str>,
    dry_run: bool,
) -> anyhow::Result<String> {
    println!("Generating OCR regions from EU4 GUI files...");
    println!("  Mappings: {}", REGION_MAPPINGS.len());
    println!();

    // Phase 2: Parse GUI files and resolve elements
    let resolved_regions = resolver::resolve_all_regions(game_path)?;

    // Phase 3: Generate Rust code from resolved regions
    println!("\nGenerating Rust code...");
    let output = codegen::generate_regions_file(&resolved_regions);

    if dry_run {
        println!("\nGenerated code ({} lines):", output.lines().count());
        println!("{}", "=".repeat(80));
        println!("{}", output);
        println!("{}", "=".repeat(80));
    } else if let Some(path) = output_path {
        // Phase 4: Write to file
        write_output_file(path, &output)?;
    }

    Ok(output)
}

/// Write the generated code to the output file.
fn write_output_file(path: &str, content: &str) -> anyhow::Result<()> {
    use std::fs;
    use std::io::Write;
    use std::path::Path;

    let output_path = Path::new(path);

    // Create parent directory if it doesn't exist
    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent)?;
    }

    // Create backup of existing file
    if output_path.exists() {
        let backup_path = output_path.with_extension("rs.bak");
        fs::copy(output_path, &backup_path)?;
        println!("  Backed up existing file to: {}", backup_path.display());
    }

    // Write new content
    let mut file = fs::File::create(output_path)?;
    file.write_all(content.as_bytes())?;

    println!(
        "  Wrote {} bytes to: {}",
        content.len(),
        output_path.display()
    );

    Ok(())
}
