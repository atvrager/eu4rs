//! Element matching and position resolution.
//!
//! Dynamically extracts element positions from EU4 GUI files using eu4game's
//! parser. Falls back to baseline coordinates if parsing fails.

use super::gui_parser::{find_element, parse_gui_files, GuiDatabases};
use super::mapping::REGION_MAPPINGS;
use super::position_calculator::calculate_screen_position;
use super::types::{GuiFile, RegionMapping, ResolvedRegion};
use anyhow::Result;

/// Resolve all regions by parsing GUI files and extracting element positions.
///
/// Tries to parse GUI files dynamically to extract real element positions.
/// Falls back to baseline coordinates if parsing fails or elements aren't found.
pub fn resolve_all_regions(game_path: &str) -> Result<Vec<ResolvedRegion>> {
    println!("Resolving {} regions...", REGION_MAPPINGS.len());

    // Try to parse GUI files
    let gui_dbs = match parse_gui_files(game_path) {
        Ok(dbs) => {
            println!("  Successfully parsed GUI files");
            Some(dbs)
        }
        Err(e) => {
            println!("  Warning: GUI parsing failed: {}", e);
            println!("  Falling back to baseline coordinates");
            None
        }
    };

    let mut resolved = Vec::new();
    let mut parse_success_count = 0;
    let mut fallback_count = 0;

    for mapping in REGION_MAPPINGS {
        let region = if let Some(ref dbs) = gui_dbs {
            // Try dynamic resolution from GUI files
            match resolve_from_gui(mapping, dbs) {
                Some(r) => {
                    parse_success_count += 1;
                    r
                }
                None => {
                    println!(
                        "  ✗ {}: element not found, using baseline",
                        mapping.display_name
                    );
                    fallback_count += 1;
                    resolve_from_baseline(mapping)
                }
            }
        } else {
            // GUI parsing failed entirely, use all baselines
            fallback_count += 1;
            resolve_from_baseline(mapping)
        };

        resolved.push(region);
    }

    println!();
    println!(
        "Resolved {}/{} regions:",
        resolved.len(),
        REGION_MAPPINGS.len()
    );
    println!("  {} from GUI files", parse_success_count);
    println!("  {} from baseline fallback", fallback_count);

    Ok(resolved)
}

/// Resolve a region using parsed GUI data.
///
/// Finds the element in the appropriate GUI file, calculates its screen position,
/// and returns a ResolvedRegion. Returns None if the element can't be found.
fn resolve_from_gui(mapping: &RegionMapping, dbs: &GuiDatabases) -> Option<ResolvedRegion> {
    // Select database and window name based on GUI file
    let (db, window_name) = match mapping.gui_file {
        GuiFile::TopBar => (&dbs.topbar, "topbar"),
        GuiFile::SpeedControls => (&dbs.speed_controls, "speed_controls"),
        GuiFile::ProvinceView => (&dbs.provinceview, "provinceview"),
    };

    // Find element using fuzzy matching
    let element = find_element(db, window_name, mapping.element_patterns, &dbs.interner)?;

    // Get parent window for layout calculation
    let window_symbol = dbs.interner.intern(window_name);
    let window = db.get(&window_symbol)?;

    // Calculate screen position (1920x1080)
    let (x, y, width, height) = calculate_screen_position(element, window, (1920, 1080));

    // Log success with matched element name
    println!(
        "  ✓ {}: matched '{}' at ({}, {})",
        mapping.display_name,
        element.name(),
        x,
        y
    );

    Some(ResolvedRegion {
        const_name: mapping.const_name.to_string(),
        display_name: mapping.display_name.to_string(),
        x,
        y,
        width,
        height,
        color: mapping.color,
        group: mapping.group,
        matched_element: Some(element.name().to_string()),
    })
}

/// Resolve a region using baseline coordinates.
///
/// These coordinates were manually calibrated for 1920x1080 vanilla EU4.
/// Used as fallback when GUI parsing fails or element isn't found.
fn resolve_from_baseline(mapping: &RegionMapping) -> ResolvedRegion {
    let (x, y, width, height) = get_baseline_coords(mapping.const_name);

    ResolvedRegion {
        const_name: mapping.const_name.to_string(),
        display_name: mapping.display_name.to_string(),
        x,
        y,
        width,
        height,
        color: mapping.color,
        group: mapping.group,
        matched_element: None, // Not matched yet - future enhancement
    }
}

/// Get baseline coordinates from original regions.rs.
///
/// These coordinates were manually calibrated for 1920x1080 vanilla EU4.
/// Used as fallback when GUI parsing is unavailable or fails.
fn get_baseline_coords(const_name: &str) -> (u32, u32, u32, u32) {
    // (x, y, width, height) for 1920x1080 resolution
    match const_name {
        // Top bar - Resources (row 1)
        "TREASURY" => (169, 13, 48, 21),
        "MANPOWER" => (255, 12, 50, 24),
        "SAILORS" => (336, 11, 50, 24),

        // Top bar - Monarch points (row 2)
        "ADM_MANA" => (520, 55, 34, 20),
        "DIP_MANA" => (577, 55, 33, 19),
        "MIL_MANA" => (639, 56, 34, 21),

        // Top bar - Country stats
        "STABILITY" => (419, 16, 30, 20),
        "CORRUPTION" => (485, 14, 50, 24),
        "PRESTIGE" => (545, 17, 37, 19),
        "GOVT_STRENGTH" => (615, 15, 37, 22),
        "POWER_PROJ" => (700, 14, 40, 20),

        // Top bar - Envoys
        "MERCHANTS" => (734, 32, 40, 20),
        "COLONISTS" => (774, 39, 40, 13),
        "DIPLOMATS" => (816, 35, 37, 17),
        "MISSIONARIES" => (859, 35, 34, 18),

        // Top bar - Info displays
        "COUNTRY" => (146, 49, 344, 30),
        "AGE" => (740, 54, 160, 21),
        "DATE" => (1697, 16, 132, 21),

        // Province panel - Header
        "PROV_NAME" => (106, 418, 157, 24),
        "PROV_STATE" => (269, 421, 171, 24),

        // Province panel - Development values
        "PROV_TAX" => (83, 552, 25, 18),
        "PROV_PROD" => (160, 554, 25, 18),
        "PROV_MANP" => (238, 553, 25, 18),

        // Province panel - Development buttons
        "PROV_TAX_BTN" => (48, 553, 22, 22),
        "PROV_PROD_BTN" => (125, 557, 22, 22),
        "PROV_MANP_BTN" => (204, 555, 22, 22),

        // Unknown region - should never happen
        _ => (0, 0, 32, 32),
    }
}
