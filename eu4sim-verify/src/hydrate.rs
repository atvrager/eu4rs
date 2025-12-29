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

    // Recompute province modifiers from buildings
    // This must happen after loading buildings from the save file
    log::info!("Recomputing province modifiers from buildings");
    for (&province_id, province) in world.provinces.iter() {
        let province_clone = province.clone();
        eu4sim_core::systems::buildings::recompute_province_modifiers(
            province_id,
            &province_clone,
            &world.building_defs,
            &mut world.modifiers,
        );
    }

    // Override countries with save data
    // NOTE: Save file stores manpower in thousands (9.96 = 9,960 men)
    //       Sim stores manpower as raw men (9960 = 9,960 men)
    //       Treasury is in ducats in both
    let mut countries_updated = 0;
    for (tag, save_country) in &save.countries {
        if let Some(country) = world.countries.get_mut(tag) {
            // Debug: Log BEFORE override
            if tag == "KOR" {
                log::debug!(
                    "{} treasury BEFORE override: {} ducats",
                    tag,
                    country.treasury.to_f32()
                );
            }
            // Update manpower (convert from thousands to raw men)
            if let Some(mp) = save_country.current_manpower {
                country.manpower = Fixed::from_f32((mp * 1000.0) as f32);
            }

            // Update treasury (ducats - no conversion needed)
            if let Some(treasury) = save_country.treasury {
                country.treasury = Fixed::from_f32(treasury as f32);
                log::debug!("{} treasury from save: {} ducats", tag, treasury);
            }

            // Update tribute type (for tributary states)
            if let Some(tribute_type_val) = save_country.tribute_type {
                country.tribute_type = eu4sim_core::state::TributeType::from_i32(tribute_type_val);
                if let Some(ref tt) = country.tribute_type {
                    log::debug!("{} tribute_type from save: {:?}", tag, tt);
                }
            }

            // Update monarch power from save
            if let Some(adm) = save_country.adm_power {
                country.adm_mana = Fixed::from_f32(adm as f32);
            }
            if let Some(dip) = save_country.dip_power {
                country.dip_mana = Fixed::from_f32(dip as f32);
            }
            if let Some(mil) = save_country.mil_power {
                country.mil_mana = Fixed::from_f32(mil as f32);
            }

            // Update ruler stats from save (determines monthly power generation)
            if let Some(adm) = save_country.ruler_adm {
                country.ruler_adm = adm as u8;
            }
            if let Some(dip) = save_country.ruler_dip {
                country.ruler_dip = dip as u8;
            }
            if let Some(mil) = save_country.ruler_mil {
                country.ruler_mil = mil as u8;
            }
            if save_country.ruler_adm.is_some() {
                log::debug!(
                    "{} ruler stats from save: ADM={}, DIP={}, MIL={}",
                    tag,
                    country.ruler_adm,
                    country.ruler_dip,
                    country.ruler_mil
                );
            }

            // Update ruler dynasty from save (needed for HRE elections)
            if let Some(ref dynasty) = save_country.ruler_dynasty {
                country.ruler_dynasty = Some(dynasty.clone());
                log::trace!("{} ruler dynasty from save: {}", tag, dynasty);
            }

            // Extract advisors from save
            country.advisors = save_country
                .advisors
                .iter()
                .filter(|adv| adv.is_hired) // Only hired advisors cost money
                .map(|adv| {
                    // Map advisor type string to enum category
                    // EU4 has many advisor types, categorize them into ADM/DIP/MIL
                    let advisor_type = categorize_advisor_type(&adv.advisor_type);

                    // Calculate monthly cost based on skill level
                    // EU4 formula is complex, but approximation: base_cost × skill²
                    // Base cost varies by nation size and modifiers, but ~2-5 ducats is typical
                    // For skill 1: ~5 ducats/month, skill 2: ~20, skill 3: ~45, etc.
                    let base_cost = 5.0;
                    let skill_multiplier = (adv.skill as f32).powi(2);
                    let monthly_cost = Fixed::from_f32(base_cost * skill_multiplier);

                    eu4sim_core::state::Advisor {
                        name: format!("{} (skill {})", adv.advisor_type, adv.skill),
                        skill: adv.skill,
                        advisor_type,
                        monthly_cost,
                    }
                })
                .collect();

            countries_updated += 1;
        } else {
            // Country exists in save but not in game data - create minimal entry
            log::debug!("Country {} in save but not in game data", tag);
        }
    }
    log::info!("Updated {} countries from save", countries_updated);

    // Apply country modifiers from save
    log::info!("Applying country modifiers from save");
    let mut modifiers_applied = 0;
    for (tag, save_country) in &save.countries {
        for modifier_name in &save_country.active_modifiers {
            if let Some(modifier_def) = world.event_modifiers.get(modifier_name) {
                // Apply tax modifier
                if let Some(tax_mod) = modifier_def.global_tax_modifier {
                    let current = world
                        .modifiers
                        .country_tax_modifier
                        .get(tag)
                        .copied()
                        .unwrap_or(Fixed::ZERO);
                    world
                        .modifiers
                        .country_tax_modifier
                        .insert(tag.clone(), current + Fixed::from_f32(tax_mod));
                    log::debug!(
                        "Applied tax modifier {} to {}: +{:.3}",
                        modifier_name,
                        tag,
                        tax_mod
                    );
                }

                // Apply production efficiency modifier
                if let Some(prod_eff) = modifier_def.production_efficiency {
                    // TODO: Add country_production_efficiency to GameModifiers
                    // For now, log and skip
                    log::debug!(
                        "Skipping production_efficiency modifier {} for {}: +{:.3} (not implemented)",
                        modifier_name,
                        tag,
                        prod_eff
                    );
                }

                // Apply trade efficiency modifier
                if let Some(trade_eff) = modifier_def.trade_efficiency {
                    let current = world
                        .modifiers
                        .country_trade_efficiency
                        .get(tag)
                        .copied()
                        .unwrap_or(Fixed::ZERO);
                    world
                        .modifiers
                        .country_trade_efficiency
                        .insert(tag.clone(), current + Fixed::from_f32(trade_eff));
                    log::debug!(
                        "Applied trade_efficiency modifier {} to {}: +{:.3}",
                        modifier_name,
                        tag,
                        trade_eff
                    );
                }

                // Apply goods produced modifier
                if let Some(goods_mod) = modifier_def.global_trade_goods_size_modifier {
                    let current = world
                        .modifiers
                        .country_goods_produced
                        .get(tag)
                        .copied()
                        .unwrap_or(Fixed::ZERO);
                    world
                        .modifiers
                        .country_goods_produced
                        .insert(tag.clone(), current + Fixed::from_f32(goods_mod));
                    log::debug!(
                        "Applied goods_produced modifier {} to {}: +{:.3}",
                        modifier_name,
                        tag,
                        goods_mod
                    );
                }

                modifiers_applied += 1;
            } else {
                log::debug!(
                    "Event modifier definition not found for '{}' (country: {})",
                    modifier_name,
                    tag
                );
            }
        }
    }
    log::info!("Applied {} country modifiers from save", modifiers_applied);

    // Override subjects with save data
    // The base state already has subjects from history/diplomacy, but save file
    // may have different relationships (new vassals, released subjects, etc.)
    let mut subjects_updated = 0;
    world.diplomacy.subjects.clear(); // Start fresh with save data
    for (subject_tag, save_subject) in &save.subjects {
        // Look up subject type ID by name
        if let Some(type_id) = world.subject_types.id_by_name(&save_subject.subject_type) {
            let relationship = eu4sim_core::state::SubjectRelationship {
                overlord: save_subject.overlord.clone(),
                subject: save_subject.subject.clone(),
                subject_type: type_id,
                start_date: save_subject
                    .start_date
                    .as_ref()
                    .and_then(|s| parse_date(s).ok())
                    .unwrap_or(date),
                liberty_desire: 0,
                integration_progress: 0,
                integrating: false,
            };
            world
                .diplomacy
                .subjects
                .insert(subject_tag.clone(), relationship);
            subjects_updated += 1;
        } else {
            log::warn!(
                "Unknown subject type '{}' for {} -> {}",
                save_subject.subject_type,
                save_subject.overlord,
                save_subject.subject
            );
        }
    }
    log::info!(
        "Updated {} subject relationships from save",
        subjects_updated
    );

    // Use total expenses MINUS fort maintenance as fixed expenses
    // Fort maintenance is calculated from provinces, but everything else
    // (army, navy, state maintenance, etc.) comes from the save ledger
    for (tag, save_country) in &save.countries {
        if let Some(country) = world.countries.get_mut(tag) {
            let total_expenses = save_country.total_monthly_expenses.unwrap_or(0.0);
            let fort_maint = save_country.fort_maintenance.unwrap_or(0.0);

            // Fixed expenses = total - fort_maintenance
            // Fort maintenance is calculated from province.fort_level in expense system
            // Everything else (army, navy, state maintenance, advisors, corruption, etc.)
            // is baked into the ledger total and applied as fixed expense
            let fixed_expenses = total_expenses - fort_maint;

            country.fixed_expenses = Fixed::from_f32(fixed_expenses as f32);

            if tag == "KOR" {
                let state_maint = save_country.state_maintenance.unwrap_or(0.0);
                let army_maint = save_country.army_maintenance.unwrap_or(0.0);
                let navy_maint = save_country.navy_maintenance.unwrap_or(0.0);
                let corruption = save_country.root_out_corruption.unwrap_or(0.0);

                log::info!(
                    "{} expenses from save ledger: total={:.2}, fort={:.2}, fixed={:.2} ducats/month",
                    tag,
                    total_expenses,
                    fort_maint,
                    fixed_expenses
                );
                log::info!(
                    "  Fixed expenses breakdown: state={:.2}, army={:.2}, navy={:.2}, corruption={:.2}",
                    state_maint,
                    army_maint,
                    navy_maint,
                    corruption
                );
            } else {
                log::debug!(
                    "{} expenses from save ledger: {:.2} ducats/month",
                    tag,
                    fixed_expenses
                );
            }
        }
    }

    // Clear armies and fleets for passive simulation to avoid combat/movement AI
    let armies_cleared = world.armies.len();
    let fleets_cleared = world.fleets.len();

    // Log fleet details for debugging
    let korea_fleets: Vec<_> = world.fleets.values().filter(|f| f.owner == "KOR").collect();
    log::info!(
        "Korea has {} fleets with {} total ships",
        korea_fleets.len(),
        korea_fleets.iter().map(|f| f.ships.len()).sum::<usize>()
    );

    world.armies.clear();
    world.fleets.clear();
    log::info!(
        "Cleared {} armies and {} fleets for passive simulation (maintenance costs preserved)",
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

/// Categorize EU4 advisor type string into ADM/DIP/MIL category.
///
/// EU4 has many specific advisor types (philosopher, treasurer, etc.).
/// This maps them to the three main categories for the simulation.
fn categorize_advisor_type(advisor_type_str: &str) -> eu4sim_core::state::AdvisorType {
    use eu4sim_core::state::AdvisorType;

    match advisor_type_str {
        // Administrative advisors
        "philosopher" | "natural_scientist" | "artist" | "treasurer" | "theologian"
        | "master_of_mint" | "inquisitor" => AdvisorType::Administrative,

        // Diplomatic advisors
        "statesman" | "naval_reformer" | "trader" | "spymaster" | "colonial_governor"
        | "diplomat" => AdvisorType::Diplomatic,

        // Military advisors
        "army_reformer"
        | "army_organiser"
        | "commandant"
        | "quartermaster"
        | "recruitmaster"
        | "fortification_expert"
        | "grand_captain" => AdvisorType::Military,

        // Default to administrative if unknown
        _ => {
            log::warn!(
                "Unknown advisor type '{}', defaulting to Administrative",
                advisor_type_str
            );
            AdvisorType::Administrative
        }
    }
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
