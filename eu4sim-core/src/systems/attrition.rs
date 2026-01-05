//! Attrition system - supply limit and monthly casualties.
//!
//! Armies that exceed a province's supply limit suffer monthly attrition.
//! Supply limit is based on total province development (1 regiment per 1 dev).
//! Additional attrition in winter and hostile territory.

use crate::fixed::Fixed;
use crate::state::{ArmyId, ProvinceId, WorldState};
use eu4data::defines::attrition as defines;
use std::collections::HashMap;
use tracing::instrument;

/// Run monthly attrition tick for all armies.
/// Called once per month in the simulation.
#[instrument(skip_all, name = "attrition")]
pub fn run_attrition_tick(state: &mut WorldState) {
    // Group armies by province (skip embarked and in-battle armies)
    let mut province_armies: HashMap<ProvinceId, Vec<ArmyId>> = HashMap::new();
    for (&army_id, army) in &state.armies {
        if army.embarked_on.is_none() && army.in_battle.is_none() {
            province_armies
                .entry(army.location)
                .or_default()
                .push(army_id);
        }
    }

    // Calculate attrition per province
    for (province_id, army_ids) in province_armies {
        apply_province_attrition(state, province_id, &army_ids);
    }
}

/// Apply attrition to armies in a specific province.
fn apply_province_attrition(state: &mut WorldState, province_id: ProvinceId, army_ids: &[ArmyId]) {
    let supply_limit = calculate_supply_limit(state, province_id);
    let total_regiments = count_regiments_in_province(state, army_ids);

    if total_regiments <= supply_limit {
        return; // No attrition - within supply limit
    }

    // Calculate base attrition rate
    let over_limit_ratio = (total_regiments as f32 - supply_limit as f32) / supply_limit as f32;
    let mut attrition_percent = defines::ATTRITION_BASE_PERCENT
        + over_limit_ratio * defines::ATTRITION_OVER_LIMIT_MULTIPLIER;

    // Add hostile territory bonus
    if is_hostile_territory(state, province_id, army_ids) {
        attrition_percent += defines::HOSTILE_ATTRITION;
    }

    // Add winter bonus (Dec, Jan, Feb)
    if matches!(state.date.month, 12 | 1 | 2) {
        attrition_percent += defines::WINTER_ATTRITION_BONUS;
    }

    // Apply to all armies in province
    let loss_factor = Fixed::from_f32(attrition_percent / 100.0);
    for &army_id in army_ids {
        if let Some(army) = state.armies.get_mut(&army_id) {
            for reg in &mut army.regiments {
                let loss = reg.strength.mul(loss_factor);
                reg.strength = (reg.strength - loss).max(Fixed::ZERO);
            }
        }
    }

    log::trace!(
        "Attrition in province {}: {} regiments, limit {}, loss {:.1}%",
        province_id,
        total_regiments,
        supply_limit,
        attrition_percent
    );
}

/// Calculate supply limit for a province based on development.
fn calculate_supply_limit(state: &WorldState, province_id: ProvinceId) -> u32 {
    let province = match state.provinces.get(&province_id) {
        Some(p) => p,
        None => return 0,
    };

    let dev = (province.base_tax + province.base_production + province.base_manpower).to_f32();
    (dev * defines::BASE_SUPPLY_LIMIT_PER_DEV) as u32
}

/// Count total regiments in a province (across all armies).
fn count_regiments_in_province(state: &WorldState, army_ids: &[ArmyId]) -> u32 {
    army_ids
        .iter()
        .filter_map(|&id| state.armies.get(&id))
        .map(|army| army.regiment_count())
        .sum()
}

/// Check if any army in the province is in hostile territory.
fn is_hostile_territory(state: &WorldState, province_id: ProvinceId, army_ids: &[ArmyId]) -> bool {
    let province = match state.provinces.get(&province_id) {
        Some(p) => p,
        None => return false,
    };

    // Get province owner/controller
    let controller = province.controller.as_ref().or(province.owner.as_ref());

    // Check if any army is at war with the province controller
    for &army_id in army_ids {
        if let Some(army) = state.armies.get(&army_id) {
            if let Some(ctrl) = controller {
                if state.diplomacy.are_at_war(&army.owner, ctrl) {
                    return true;
                }
            }
        }
    }

    false
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::{Army, ProvinceState, Regiment, RegimentType};
    use crate::testing::WorldStateBuilder;

    fn make_regiment(type_: RegimentType, strength: i64) -> Regiment {
        Regiment {
            type_,
            strength: Fixed::from_int(strength),
            morale: Fixed::from_int(1),
        }
    }

    #[test]
    fn test_no_attrition_within_supply_limit() {
        let mut state = WorldStateBuilder::new()
            .with_country("SWE")
            .with_province_state(
                1,
                ProvinceState {
                    owner: Some("SWE".into()),
                    base_tax: Fixed::from_int(5),
                    base_production: Fixed::from_int(5),
                    base_manpower: Fixed::from_int(5),
                    ..Default::default()
                },
            )
            .build();

        // Province has 15 dev = 15 supply limit
        // Army has 10 regiments - within limit
        let army = Army {
            id: 1,
            name: "Test Army".into(),
            owner: "SWE".into(),
            location: 1,
            previous_location: None,
            regiments: vec![make_regiment(RegimentType::Infantry, 1000); 10],
            movement: None,
            embarked_on: None,
            general: None,
            in_battle: None,
            infantry_count: 10,
            cavalry_count: 0,
            artillery_count: 0,
        };
        state.armies.insert(1, army);

        let initial_strength = state.armies.get(&1).unwrap().regiments[0].strength;

        // Run attrition
        run_attrition_tick(&mut state);

        // No attrition should have occurred
        assert_eq!(
            state.armies.get(&1).unwrap().regiments[0].strength,
            initial_strength
        );
    }

    #[test]
    fn test_attrition_over_supply_limit() {
        let mut state = WorldStateBuilder::new()
            .with_country("SWE")
            .with_province_state(
                1,
                ProvinceState {
                    owner: Some("SWE".into()),
                    base_tax: Fixed::from_int(5),
                    base_production: Fixed::from_int(5),
                    base_manpower: Fixed::from_int(5),
                    ..Default::default()
                },
            )
            .build();

        // Province has 15 dev = 15 supply limit
        // Army has 30 regiments - 100% over limit
        let army = Army {
            id: 1,
            name: "Test Army".into(),
            owner: "SWE".into(),
            location: 1,
            previous_location: None,
            regiments: vec![make_regiment(RegimentType::Infantry, 1000); 30],
            movement: None,
            embarked_on: None,
            general: None,
            in_battle: None,
            infantry_count: 30,
            cavalry_count: 0,
            artillery_count: 0,
        };
        state.armies.insert(1, army);

        let initial_strength = state.armies.get(&1).unwrap().regiments[0].strength;

        // Run attrition
        run_attrition_tick(&mut state);

        // Attrition should have occurred
        // 100% over limit = 1% base + 100% * 5% = 6% loss
        let expected_loss = initial_strength.mul(Fixed::from_f32(0.06));
        let expected_strength = initial_strength - expected_loss;

        assert_eq!(
            state.armies.get(&1).unwrap().regiments[0].strength,
            expected_strength
        );
    }

    #[test]
    fn test_winter_attrition() {
        let mut state = WorldStateBuilder::new()
            .date(1444, 12, 1) // December - winter
            .with_country("SWE")
            .with_province_state(
                1,
                ProvinceState {
                    owner: Some("SWE".into()),
                    base_tax: Fixed::from_int(5),
                    base_production: Fixed::from_int(5),
                    base_manpower: Fixed::from_int(5),
                    ..Default::default()
                },
            )
            .build();

        // Army 100% over limit
        let army = Army {
            id: 1,
            name: "Test Army".into(),
            owner: "SWE".into(),
            location: 1,
            previous_location: None,
            regiments: vec![make_regiment(RegimentType::Infantry, 1000); 30],
            movement: None,
            embarked_on: None,
            general: None,
            in_battle: None,
            infantry_count: 30,
            cavalry_count: 0,
            artillery_count: 0,
        };
        state.armies.insert(1, army);

        let initial_strength = state.armies.get(&1).unwrap().regiments[0].strength;

        // Run attrition
        run_attrition_tick(&mut state);

        // Winter attrition = 6% (base+over) + 2% (winter) = 8%
        let expected_loss = initial_strength.mul(Fixed::from_f32(0.08));
        let expected_strength = initial_strength - expected_loss;

        assert_eq!(
            state.armies.get(&1).unwrap().regiments[0].strength,
            expected_strength
        );
    }

    #[test]
    fn test_hostile_territory_attrition() {
        let mut state = WorldStateBuilder::new()
            .with_country("SWE")
            .with_country("DEN")
            .with_province_state(
                1,
                ProvinceState {
                    owner: Some("DEN".into()),
                    controller: Some("DEN".into()),
                    base_tax: Fixed::from_int(5),
                    base_production: Fixed::from_int(5),
                    base_manpower: Fixed::from_int(5),
                    ..Default::default()
                },
            )
            .build();

        // Declare war
        use crate::state::War;
        let war = War {
            id: 1,
            name: "Test War".into(),
            attackers: vec!["SWE".into()],
            defenders: vec!["DEN".into()],
            start_date: state.date,
            attacker_score: 0,
            attacker_battle_score: 0,
            defender_score: 0,
            defender_battle_score: 0,
            pending_peace: None,
        };
        state.diplomacy.wars.insert(1, war);

        // SWE army in DEN territory, 100% over limit
        let army = Army {
            id: 1,
            name: "Test Army".into(),
            owner: "SWE".into(),
            location: 1,
            previous_location: None,
            regiments: vec![make_regiment(RegimentType::Infantry, 1000); 30],
            movement: None,
            embarked_on: None,
            general: None,
            in_battle: None,
            infantry_count: 30,
            cavalry_count: 0,
            artillery_count: 0,
        };
        state.armies.insert(1, army);

        let initial_strength = state.armies.get(&1).unwrap().regiments[0].strength;

        // Run attrition
        run_attrition_tick(&mut state);

        // Hostile attrition = 6% (base+over) + 1% (hostile) = 7%
        let expected_loss = initial_strength.mul(Fixed::from_f32(0.07));
        let expected_strength = initial_strength - expected_loss;

        assert_eq!(
            state.armies.get(&1).unwrap().regiments[0].strength,
            expected_strength
        );
    }

    #[test]
    fn test_supply_limit_calculation() {
        let state = WorldStateBuilder::new()
            .with_province_state(
                1,
                ProvinceState {
                    base_tax: Fixed::from_int(3),
                    base_production: Fixed::from_int(4),
                    base_manpower: Fixed::from_int(5),
                    ..Default::default()
                },
            )
            .build();

        let limit = calculate_supply_limit(&state, 1);
        assert_eq!(limit, 12); // 3 + 4 + 5 = 12
    }

    #[test]
    fn test_embarked_armies_skip_attrition() {
        let mut state = WorldStateBuilder::new()
            .with_country("SWE")
            .with_province_state(
                1,
                ProvinceState {
                    owner: Some("SWE".into()),
                    base_tax: Fixed::ONE,
                    base_production: Fixed::ONE,
                    base_manpower: Fixed::ONE,
                    ..Default::default()
                },
            )
            .build();

        // Embarked army - should not suffer attrition
        let army = Army {
            id: 1,
            name: "Embarked Army".into(),
            owner: "SWE".into(),
            location: 1,
            previous_location: None,
            regiments: vec![make_regiment(RegimentType::Infantry, 1000); 30],
            movement: None,
            embarked_on: Some(1), // Embarked on fleet 1
            general: None,
            in_battle: None,
            infantry_count: 0,
            cavalry_count: 0,
            artillery_count: 0,
        };
        state.armies.insert(1, army);

        let initial_strength = state.armies.get(&1).unwrap().regiments[0].strength;

        // Run attrition
        run_attrition_tick(&mut state);

        // No attrition for embarked armies
        assert_eq!(
            state.armies.get(&1).unwrap().regiments[0].strength,
            initial_strength
        );
    }
}
