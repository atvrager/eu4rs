//! Element matching and position resolution.
//!
//! NOTE: This module currently uses hardcoded coordinates from the original
//! regions.rs as a baseline. Future enhancement would integrate with eu4game's
//! GUI parser to dynamically extract positions from .gui files.

use super::mapping::REGION_MAPPINGS;
use super::types::{RegionMapping, ResolvedRegion};
use anyhow::Result;

/// Resolve all regions by mapping them to known coordinates.
///
/// Currently uses hardcoded coordinates. Future enhancement: Parse GUI files
/// from game_path/interface/*.gui and calculate positions dynamically.
pub fn resolve_all_regions(_game_path: &str) -> Result<Vec<ResolvedRegion>> {
    println!("Resolving {} regions...", REGION_MAPPINGS.len());
    println!("  Note: Currently using baseline coordinates from original regions.rs");
    println!("  Future: Will parse GUI files dynamically");
    println!();

    let mut resolved = Vec::new();

    for mapping in REGION_MAPPINGS {
        let region = resolve_from_baseline(mapping);
        resolved.push(region);
    }

    println!("Resolved {}/{} regions", resolved.len(), REGION_MAPPINGS.len());

    Ok(resolved)
}

/// Resolve a region using baseline coordinates.
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
/// Future versions will calculate these dynamically from GUI files.
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
