//! GUI code generator for build-time rendering method generation.
//!
//! This module implements build-time code generation for GUI panel rendering,
//! following the same pattern as `eu4data/build.rs` and `xtask/region_gen`.
//!
//! # Architecture
//!
//! Parse .gui files → Intermediate representation → Generate Rust code
//!
//! # Example
//!
//! ```bash
//! cargo xtask generate-gui-renderer --panel left
//! ```
//!
//! Generates `eu4game/src/generated/gui/left_panel.rs` with rendering methods.

pub mod codegen;
pub mod parser;
pub mod types;

use std::path::Path;

/// Entry point for GUI renderer code generation.
///
/// # Arguments
/// * `game_path` - Path to EU4 installation directory
/// * `panel_name` - Name of panel to generate ("left", "topbar", "speed_controls", or "all")
/// * `output_dir` - Directory to write generated files
/// * `dry_run` - If true, print to stdout instead of writing files
///
/// # Returns
/// Generated Rust code as a string.
pub fn generate_gui_renderer(
    game_path: &str,
    panel_name: &str,
    output_dir: Option<&str>,
    dry_run: bool,
) -> anyhow::Result<String> {
    println!("Generating GUI renderer code for panel: {}", panel_name);
    println!("Game path: {}", game_path);

    // Phase 1: Parse GUI files
    println!("Parsing GUI files...");
    let gui_trees = parser::parse_all_gui_files(Path::new(game_path))?;
    println!("Parsed {} GUI trees", gui_trees.len());

    // Phase 2: Generate code
    println!("Generating Rust code...");

    if panel_name == "all" {
        // Generate separate file for each panel
        let mut generated_files = Vec::new();

        for name in gui_trees.keys() {
            println!("Generating code for panel: {}", name);
            let output = codegen::generate_panel(name, &gui_trees)?;

            if dry_run {
                println!(
                    "\nGenerated code for {} ({} lines):",
                    name,
                    output.lines().count()
                );
                println!("{}", "=".repeat(80));
                println!("{}", output);
                println!("{}", "=".repeat(80));
            } else if let Some(dir) = output_dir {
                write_output_file(dir, name, &output)?;
                generated_files.push(name.clone());
            }
        }

        if !dry_run && output_dir.is_some() {
            println!("\nGenerated {} panel files:", generated_files.len());
            for name in &generated_files {
                println!("  - {}_panel.rs", name);
            }
        }

        Ok(String::new()) // Return empty string for "all" mode
    } else {
        // Generate single panel
        let output = codegen::generate_panel(panel_name, &gui_trees)?;

        if dry_run {
            println!("\nGenerated code ({} lines):", output.lines().count());
            println!("{}", "=".repeat(80));
            println!("{}", output);
            println!("{}", "=".repeat(80));
        } else if let Some(dir) = output_dir {
            // Phase 3: Write to file
            write_output_file(dir, panel_name, &output)?;
        }

        Ok(output)
    }
}

/// Write the generated code to the output file.
fn write_output_file(dir: &str, panel_name: &str, content: &str) -> anyhow::Result<()> {
    use std::fs;
    use std::io::Write;

    let dir_path = Path::new(dir);
    fs::create_dir_all(dir_path)?;

    let filename = format!("{}_panel.rs", panel_name);
    let output_path = dir_path.join(&filename);

    // Create backup if file exists
    if output_path.exists() {
        let backup_path = output_path.with_extension("rs.bak");
        fs::copy(&output_path, &backup_path)?;
        println!("Backed up existing file to: {}", backup_path.display());
    }

    // Write new content
    let mut file = fs::File::create(&output_path)?;
    file.write_all(content.as_bytes())?;

    println!(
        "Wrote {} bytes to: {}",
        content.len(),
        output_path.display()
    );

    Ok(())
}
