//! Province coring system.
//!
//! Coring establishes permanent ownership claims, reducing overextension
//! and removing the 75% autonomy floor for uncored provinces.
//!
//! ## EU4 Mechanics (Simplified)
//! - **Cost**: 10 ADM per development point
//! - **Duration**: 36 months base
//! - **Overextension**: 1% per development in uncored provinces
//! - **Autonomy Floor**: 75% for uncored, 0% for cored

use crate::fixed::Fixed;
use crate::state::{CoringProgress, Date, ProvinceId, ProvinceState, Tag, WorldState};

/// Cost to core a province: 10 ADM per development point.
pub const CORING_COST_PER_DEV: i64 = 10;

/// Base time to core a province: 36 months.
pub const BASE_CORING_TIME: u8 = 36;

/// Autonomy floor for uncored provinces (75%).
pub const UNCORED_AUTONOMY_FLOOR: Fixed = Fixed::from_raw(7500);

/// Calculate the ADM cost to core a province.
pub fn calculate_coring_cost(province: &ProvinceState) -> Fixed {
    let dev = province.base_tax + province.base_production + province.base_manpower;
    dev * Fixed::from_int(CORING_COST_PER_DEV)
}

/// Calculate total development of a province.
pub fn province_development(province: &ProvinceState) -> Fixed {
    province.base_tax + province.base_production + province.base_manpower
}

/// Calculate the effective autonomy for income/manpower calculations.
/// Uncored provinces have a 75% floor; cored provinces use base autonomy (0 for now).
pub fn effective_autonomy(province: &ProvinceState, owner: &Tag) -> Fixed {
    // We don't track base autonomy yet, assume 0
    let base = Fixed::ZERO;

    // Uncored provinces have +75% autonomy floor
    let has_core = province.cores.contains(owner);
    let floor = if has_core {
        Fixed::ZERO
    } else {
        UNCORED_AUTONOMY_FLOOR
    };

    base.max(floor)
}

/// Start coring a province.
///
/// Returns an error message if coring cannot begin.
pub fn start_coring(
    state: &mut WorldState,
    country: Tag,
    province_id: ProvinceId,
    current_date: Date,
) -> Result<(), String> {
    let province = state
        .provinces
        .get(&province_id)
        .ok_or("Province not found")?;

    // Must own the province
    if province.owner.as_ref() != Some(&country) {
        return Err("Not owner".into());
    }

    // Must not already have core
    if province.cores.contains(&country) {
        return Err("Already has core".into());
    }

    // Must not be actively coring
    if province.coring_progress.is_some() {
        return Err("Already coring".into());
    }

    // Check and deduct ADM cost
    let base_cost = calculate_coring_cost(province);

    // Apply core_creation modifier to cost
    let core_creation_mod = state
        .modifiers
        .country_core_creation
        .get(&country)
        .copied()
        .unwrap_or(Fixed::ZERO);
    let cost_factor = Fixed::ONE + core_creation_mod;
    let cost = base_cost.mul(cost_factor).max(Fixed::ONE); // Minimum cost of 1

    let country_state = state
        .countries
        .get_mut(&country)
        .ok_or("Country not found")?;

    if country_state.adm_mana < cost {
        return Err(format!(
            "Insufficient ADM: need {}, have {}",
            cost, country_state.adm_mana
        ));
    }

    country_state.adm_mana -= cost;

    // Apply core_creation modifier to time
    let base_time = BASE_CORING_TIME as f32;
    let modified_time = (base_time * (1.0 + core_creation_mod.to_f32())).max(1.0) as u8;

    // Start coring progress
    if let Some(province) = state.provinces.get_mut(&province_id) {
        province.coring_progress = Some(CoringProgress {
            coring_country: country.clone(),
            start_date: current_date,
            progress: 0,
            required: modified_time,
        });
    }

    log::info!(
        "{} started coring province {} (cost: {} ADM)",
        country,
        province_id,
        cost
    );

    Ok(())
}

/// Advance coring progress for all provinces (called monthly in step_world).
pub fn tick_coring(state: &mut WorldState) {
    let mut completions = Vec::new();
    let mut cancellations = Vec::new();
    let mut progress_updates = Vec::new();

    // First pass: identify completions, cancellations, and progress updates
    for (&prov_id, province) in state.provinces.iter() {
        if let Some(progress) = &province.coring_progress {
            // Check owner still matches coring country
            if province.owner.as_ref() == Some(&progress.coring_country) {
                if progress.progress + 1 >= progress.required {
                    completions.push((prov_id, progress.coring_country.clone()));
                } else {
                    // Needs progress increment
                    progress_updates.push(prov_id);
                }
            } else {
                // Lost province while coring - cancel
                cancellations.push(prov_id);
            }
        }
    }

    // Apply progress increments
    for prov_id in progress_updates {
        if let Some(province) = state.provinces.get_mut(&prov_id) {
            if let Some(ref mut progress) = province.coring_progress {
                progress.progress += 1;
            }
        }
    }

    // Cancel lost cores
    for prov_id in cancellations {
        if let Some(province) = state.provinces.get_mut(&prov_id) {
            log::info!("Coring cancelled for province {} (owner changed)", prov_id);
            province.coring_progress = None;
        }
    }

    // Complete cores
    for (prov_id, country) in completions {
        if let Some(province) = state.provinces.get_mut(&prov_id) {
            province.cores.insert(country.clone());
            province.coring_progress = None;
            log::info!("{} completed coring province {}", country, prov_id);
        }
    }
}

/// Recalculate overextension for all countries.
/// Called after peace deals or at the start of each month.
pub fn recalculate_overextension(state: &mut WorldState) {
    // Collect uncored development per country
    let mut uncored_dev: std::collections::HashMap<Tag, Fixed> = std::collections::HashMap::new();

    for province in state.provinces.values() {
        if let Some(owner) = &province.owner {
            if !province.cores.contains(owner) {
                let dev = province_development(province);
                *uncored_dev.entry(owner.clone()).or_insert(Fixed::ZERO) += dev;
            }
        }
    }

    // Update country overextension (1 dev = 1% OE)
    for (tag, country) in state.countries.iter_mut() {
        let oe = uncored_dev.get(tag).copied().unwrap_or(Fixed::ZERO);
        country.overextension = oe;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testing::WorldStateBuilder;

    #[test]
    fn test_coring_cost_calculation() {
        let province = ProvinceState {
            base_tax: Fixed::from_int(5),
            base_production: Fixed::from_int(5),
            base_manpower: Fixed::from_int(5),
            ..Default::default()
        };

        let cost = calculate_coring_cost(&province);
        assert_eq!(cost, Fixed::from_int(150)); // 15 dev * 10 ADM
    }

    #[test]
    fn test_effective_autonomy_cored() {
        let mut province = ProvinceState {
            owner: Some("FRA".into()),
            ..Default::default()
        };
        province.cores.insert("FRA".into());

        let autonomy = effective_autonomy(&province, &"FRA".into());
        assert_eq!(autonomy, Fixed::ZERO);
    }

    #[test]
    fn test_effective_autonomy_uncored() {
        let province = ProvinceState {
            owner: Some("FRA".into()),
            ..Default::default()
        };
        // No core

        let autonomy = effective_autonomy(&province, &"FRA".into());
        assert_eq!(autonomy, UNCORED_AUTONOMY_FLOOR); // 75%
    }

    #[test]
    fn test_start_coring_success() {
        let mut state = WorldStateBuilder::new()
            .with_country("FRA")
            .with_province(1, Some("FRA"))
            .build();

        // Remove core to simulate conquest
        state.provinces.get_mut(&1).unwrap().cores.clear();
        state.countries.get_mut("FRA").unwrap().adm_mana = Fixed::from_int(1000);

        let date = state.date;
        let result = start_coring(&mut state, "FRA".into(), 1, date);
        assert!(result.is_ok());
        assert!(state.provinces.get(&1).unwrap().coring_progress.is_some());
    }

    #[test]
    fn test_start_coring_insufficient_adm() {
        let mut state = WorldStateBuilder::new()
            .with_country("FRA")
            .with_province(1, Some("FRA"))
            .build();

        state.provinces.get_mut(&1).unwrap().cores.clear();
        state.countries.get_mut("FRA").unwrap().adm_mana = Fixed::from_int(1); // Not enough

        let date = state.date;
        let result = start_coring(&mut state, "FRA".into(), 1, date);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Insufficient ADM"));
    }

    #[test]
    fn test_coring_completion() {
        let mut state = WorldStateBuilder::new()
            .with_country("FRA")
            .with_province(1, Some("FRA"))
            .build();

        state.provinces.get_mut(&1).unwrap().cores.clear();
        state.provinces.get_mut(&1).unwrap().coring_progress = Some(CoringProgress {
            coring_country: "FRA".into(),
            start_date: state.date,
            progress: 35, // One tick away from completion
            required: 36,
        });

        tick_coring(&mut state);

        assert!(state.provinces.get(&1).unwrap().cores.contains("FRA"));
        assert!(state.provinces.get(&1).unwrap().coring_progress.is_none());
    }

    #[test]
    fn test_coring_cancelled_on_loss() {
        let mut state = WorldStateBuilder::new()
            .with_country("FRA")
            .with_country("ENG")
            .with_province(1, Some("FRA"))
            .build();

        state.provinces.get_mut(&1).unwrap().cores.clear();
        state.provinces.get_mut(&1).unwrap().coring_progress = Some(CoringProgress {
            coring_country: "FRA".into(),
            start_date: state.date,
            progress: 10,
            required: 36,
        });

        // Simulate conquest
        state.provinces.get_mut(&1).unwrap().owner = Some("ENG".into());

        tick_coring(&mut state);

        // Coring cancelled
        assert!(state.provinces.get(&1).unwrap().coring_progress.is_none());
        assert!(!state.provinces.get(&1).unwrap().cores.contains("FRA"));
    }

    #[test]
    fn test_overextension_calculation() {
        let mut state = WorldStateBuilder::new()
            .with_country("FRA")
            .with_province(1, Some("FRA"))
            .with_province(2, Some("FRA"))
            .build();

        // Province 1: cored, Province 2: uncored
        state
            .provinces
            .get_mut(&1)
            .unwrap()
            .cores
            .insert("FRA".into());
        state.provinces.get_mut(&2).unwrap().cores.clear();

        // Set development
        let p2 = state.provinces.get_mut(&2).unwrap();
        p2.base_tax = Fixed::from_int(5);
        p2.base_production = Fixed::from_int(5);
        p2.base_manpower = Fixed::from_int(5);

        recalculate_overextension(&mut state);

        // 15 dev uncored = 15% OE
        assert_eq!(
            state.countries.get("FRA").unwrap().overextension,
            Fixed::from_int(15)
        );
    }
}
