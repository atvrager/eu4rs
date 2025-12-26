//! Hydrate WorldState from save file data
//!
//! Converts parsed save data into a sim-compatible WorldState by:
//! 1. Loading static data from game files (adjacency, terrain, trade nodes)
//! 2. Overriding provinces/countries with save file values
//! 3. Stubbing out armies/fleets for passive simulation

use crate::ExtractedState;
use anyhow::Result;
use eu4data::adjacency::AdjacencyGraph;
use eu4sim_core::state::Date;
use eu4sim_core::{Fixed, WorldState};
use std::path::Path;

/// Hydrate a WorldState from save data, using game files for static data
///
/// This function:
/// 1. Loads base state from EU4 game files (via eu4sim::loader)
/// 2. Overrides province/country data with values from the save
/// 3. Clears armies/fleets for passive (no-AI) simulation
pub fn hydrate_from_save(
    game_path: &Path,
    save: &ExtractedState,
) -> Result<(WorldState, AdjacencyGraph)> {
    // Parse date from save meta
    let date = parse_date(&save.meta.date)?;

    // Load base state from game files
    log::info!("Loading game data from {:?}", game_path);
    let (mut world, adjacency) = eu4sim::loader::load_initial_state(game_path, date, 0)?;

    log::info!(
        "Base state loaded: {} provinces, {} countries",
        world.provinces.len(),
        world.countries.len()
    );

    // Override provinces with save data
    let mut provinces_updated = 0;
    for (&id, save_prov) in &save.provinces {
        if let Some(prov) = world.provinces.get_mut(&id) {
            // Update owner/controller
            if let Some(ref owner) = save_prov.owner {
                prov.owner = Some(owner.clone());
                prov.controller = Some(owner.clone());
            }

            // Update development values
            if let Some(tax) = save_prov.base_tax {
                prov.base_tax = Fixed::from_f32(tax as f32);
            }
            if let Some(prod) = save_prov.base_production {
                prov.base_production = Fixed::from_f32(prod as f32);
            }
            if let Some(mp) = save_prov.base_manpower {
                prov.base_manpower = Fixed::from_f32(mp as f32);
            }

            // Hydrate buildings from save
            // Note: This requires building_name_to_id to be populated
            // which happens when building definitions are loaded from game files.
            for building_name in &save_prov.buildings {
                if let Some(&building_id) = world.building_name_to_id.get(building_name) {
                    prov.buildings.insert(building_id);
                } else {
                    log::trace!(
                        "Building '{}' not found in definitions (province {})",
                        building_name,
                        id
                    );
                }
            }

            provinces_updated += 1;
        }
    }
    log::info!("Updated {} provinces from save", provinces_updated);

    // Override countries with save data
    // NOTE: Save file stores manpower in thousands (9.96 = 9,960 men)
    //       Sim stores manpower as raw men (9960 = 9,960 men)
    //       Treasury is in ducats in both
    let mut countries_updated = 0;
    for (tag, save_country) in &save.countries {
        if let Some(country) = world.countries.get_mut(tag) {
            // Update manpower (convert from thousands to raw men)
            if let Some(mp) = save_country.current_manpower {
                country.manpower = Fixed::from_f32((mp * 1000.0) as f32);
            }

            // Update treasury (ducats - no conversion needed)
            if let Some(treasury) = save_country.treasury {
                country.treasury = Fixed::from_f32(treasury as f32);
            }

            countries_updated += 1;
        } else {
            // Country exists in save but not in game data - create minimal entry
            log::debug!("Country {} in save but not in game data", tag);
        }
    }
    log::info!("Updated {} countries from save", countries_updated);

    // Clear armies/fleets for passive simulation
    // (Korea at game start has no active wars, so this is fine)
    let armies_cleared = world.armies.len();
    let fleets_cleared = world.fleets.len();
    world.armies.clear();
    world.fleets.clear();
    log::debug!(
        "Cleared {} armies, {} fleets for passive simulation",
        armies_cleared,
        fleets_cleared
    );

    Ok((world, adjacency))
}

/// Parse date string "YYYY.MM.DD" into Date
fn parse_date(date_str: &str) -> Result<Date> {
    let parts: Vec<&str> = date_str.split('.').collect();
    if parts.len() != 3 {
        anyhow::bail!("Invalid date format: {}", date_str);
    }

    let year: i32 = parts[0].parse()?;
    let month: u8 = parts[1].parse()?;
    let day: u8 = parts[2].parse()?;

    Ok(Date::new(year, month, day))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_date() {
        let date = parse_date("1444.11.11").unwrap();
        assert_eq!(date.year, 1444);
        assert_eq!(date.month, 11u8);
        assert_eq!(date.day, 11u8);
    }

    #[test]
    fn test_parse_date_invalid() {
        assert!(parse_date("1444-11-11").is_err());
        assert!(parse_date("1444.11").is_err());
    }
}
