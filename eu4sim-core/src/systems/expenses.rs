use crate::fixed::Fixed;
use crate::state::WorldState;
use eu4data::defines::economy as defines;

/// Runs monthly expense calculations.
///
/// Deducts costs from treasury.
pub fn run_expenses_tick(state: &mut WorldState) {
    // 1. Army Maintenance
    // Iterate armies, sum cost per country
    let mut army_costs = std::collections::HashMap::new();

    for army in state.armies.values() {
        let mut cost = Fixed::ZERO;
        for reg in &army.regiments {
            // Base cost per regiment
            let base_cost = Fixed::from_f32(defines::BASE_ARMY_COST);

            // Apply unit-type specific cost modifier
            let type_mod = match reg.type_ {
                crate::state::RegimentType::Infantry => state
                    .modifiers
                    .country_infantry_cost
                    .get(&army.owner)
                    .copied()
                    .unwrap_or(Fixed::ZERO),
                crate::state::RegimentType::Cavalry => state
                    .modifiers
                    .country_cavalry_cost
                    .get(&army.owner)
                    .copied()
                    .unwrap_or(Fixed::ZERO),
                crate::state::RegimentType::Artillery => Fixed::ZERO, // No artillery_cost modifier yet
            };

            let modified_cost = base_cost.mul(Fixed::ONE + type_mod);
            cost += modified_cost;
        }
        *army_costs.entry(army.owner.clone()).or_insert(Fixed::ZERO) += cost;
    }

    // Apply Army Costs (with modifiers) and record for display
    let country_tags: Vec<String> = state.countries.keys().cloned().collect();
    for tag in country_tags.clone() {
        if let Some(country) = state.countries.get_mut(&tag) {
            if let Some(&base_cost) = army_costs.get(&tag) {
                let modifier = state
                    .modifiers
                    .land_maintenance_modifier
                    .get(&tag)
                    .copied()
                    .unwrap_or(Fixed::ZERO);

                let factor = Fixed::ONE + modifier;
                let final_cost = base_cost.mul(factor);

                country.treasury -= final_cost;
                country.income.expenses += final_cost;
            }
        }
    }

    // 2. Fleet Maintenance
    let mut fleet_costs = std::collections::HashMap::new();

    for fleet in state.fleets.values() {
        let mut cost = Fixed::ZERO;
        for ship in &fleet.ships {
            // Base cost per ship
            let base_cost = Fixed::from_f32(defines::BASE_NAVY_COST);

            // Apply ship-type specific cost modifier
            // TODO: Different ship types have different costs in EU4
            // For now, use the same cost for all ship types
            let type_mod = match ship.type_ {
                crate::state::ShipType::HeavyShip => Fixed::ZERO, // Could be 4x base
                crate::state::ShipType::LightShip => Fixed::ZERO,  // 1x base
                crate::state::ShipType::Galley => Fixed::ZERO,     // 0.75x base
                crate::state::ShipType::Transport => Fixed::ZERO,  // 0.75x base
            };

            let modified_cost = base_cost.mul(Fixed::ONE + type_mod);
            cost += modified_cost;
        }
        *fleet_costs.entry(fleet.owner.clone()).or_insert(Fixed::ZERO) += cost;
    }

    // Apply Fleet Costs (with naval maintenance modifier)
    for tag in country_tags.clone() {
        if let Some(country) = state.countries.get_mut(&tag) {
            if let Some(&base_cost) = fleet_costs.get(&tag) {
                let modifier = state
                    .modifiers
                    .country_naval_maintenance
                    .get(&tag)
                    .copied()
                    .unwrap_or(Fixed::ZERO);

                let factor = Fixed::ONE + modifier;
                let final_cost = base_cost.mul(factor);

                country.treasury -= final_cost;
                country.income.expenses += final_cost;
            }
        }
    }

    // 3. Fort Maintenance
    let mut fort_costs = std::collections::HashMap::new();

    for province in state.provinces.values() {
        if province.fort_level > 0 && !province.is_mothballed {
            if let Some(owner) = &province.owner {
                // Fort cost scales with fort level
                let cost = Fixed::from_f32(defines::BASE_FORT_COST)
                    .mul(Fixed::from_int(province.fort_level as i64));
                *fort_costs.entry(owner.clone()).or_insert(Fixed::ZERO) += cost;
            }
        }
    }

    // Apply Fort Costs and record for display
    for tag in country_tags {
        if let Some(country) = state.countries.get_mut(&tag) {
            if let Some(&base_cost) = fort_costs.get(&tag) {
                let modifier = state
                    .modifiers
                    .fort_maintenance_modifier
                    .get(&tag)
                    .copied()
                    .unwrap_or(Fixed::ZERO);

                let factor = Fixed::ONE + modifier;
                let final_cost = base_cost.mul(factor);

                country.treasury -= final_cost;
                country.income.expenses += final_cost;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::{Army, ProvinceState, Regiment, RegimentType};
    use crate::testing::WorldStateBuilder;

    #[test]
    fn test_army_maintenance() {
        let mut state = WorldStateBuilder::new().with_country("SWE").build();

        // Add an army manually (since builder doesn't support army yet)
        let army = Army {
            id: 1,
            name: "Test Army".into(),
            owner: "SWE".into(),
            location: 1,
            previous_location: None,
            regiments: vec![
                Regiment {
                    type_: RegimentType::Infantry,
                    strength: Fixed::from_int(1000),
                    morale: Fixed::from_f32(eu4data::defines::combat::BASE_MORALE),
                },
                Regiment {
                    type_: RegimentType::Infantry,
                    strength: Fixed::from_int(1000),
                    morale: Fixed::from_f32(eu4data::defines::combat::BASE_MORALE),
                },
            ],
            movement: None,
            embarked_on: None,
            general: None,
            in_battle: None,
            infantry_count: 0,
            cavalry_count: 0,
            artillery_count: 0,
        };
        state.armies.insert(1, army);

        // Run
        run_expenses_tick(&mut state);

        // Expected cost: 2 regiments * 0.2 = 0.4
        // Initial Treasury (default) = 100.0 (from builder)
        // 100.0 - 0.4 = 99.6
        let swe = state.countries.get("SWE").unwrap();
        assert_eq!(swe.treasury, Fixed::from_f32(99.6));
    }

    #[test]
    fn test_fort_maintenance() {
        let mut state = WorldStateBuilder::new()
            .with_country("SWE")
            .with_province_state(
                1,
                ProvinceState {
                    owner: Some("SWE".into()),
                    fort_level: 1,
                    is_mothballed: false,
                    base_tax: Fixed::ONE,
                    base_production: Fixed::ONE,
                    base_manpower: Fixed::ONE,
                    ..Default::default()
                },
            )
            .build();

        // Run
        run_expenses_tick(&mut state);

        // Expected: 1 fort * 1.0 = 1.0 cost
        // Initial Treasury = 100.0
        // Result = 99.0
        let swe = state.countries.get("SWE").unwrap();
        assert_eq!(swe.treasury, Fixed::from_f32(99.0));
    }

    #[test]
    fn test_fleet_maintenance() {
        let mut state = WorldStateBuilder::new().with_country("SWE").build();

        // Add a fleet manually
        let fleet = crate::state::Fleet {
            id: 1,
            name: "Test Fleet".into(),
            owner: "SWE".into(),
            location: 1, // Sea province
            ships: vec![
                crate::state::Ship {
                    type_: crate::state::ShipType::LightShip,
                    hull: Fixed::from_int(100),
                    durability: Fixed::from_int(100),
                },
                crate::state::Ship {
                    type_: crate::state::ShipType::Transport,
                    hull: Fixed::from_int(100),
                    durability: Fixed::from_int(100),
                },
            ],
            embarked_armies: vec![],
            movement: None,
            admiral: None,
            in_battle: None,
        };
        state.fleets.insert(1, fleet);

        // Run
        run_expenses_tick(&mut state);

        // Expected cost: 2 ships * 0.2 = 0.4
        // Initial Treasury = 100.0
        // 100.0 - 0.4 = 99.6
        let swe = state.countries.get("SWE").unwrap();
        assert_eq!(swe.treasury, Fixed::from_f32(99.6));
    }

    #[test]
    fn test_combined_maintenance() {
        let mut state = WorldStateBuilder::new()
            .with_country("SWE")
            .with_province_state(
                1,
                ProvinceState {
                    owner: Some("SWE".into()),
                    fort_level: 1,
                    is_mothballed: false,
                    base_tax: Fixed::ONE,
                    base_production: Fixed::ONE,
                    base_manpower: Fixed::ONE,
                    ..Default::default()
                },
            )
            .build();

        // Add army
        let army = Army {
            id: 1,
            name: "Test Army".into(),
            owner: "SWE".into(),
            location: 1,
            previous_location: None,
            regiments: vec![Regiment {
                type_: RegimentType::Infantry,
                strength: Fixed::from_int(1000),
                morale: Fixed::from_f32(eu4data::defines::combat::BASE_MORALE),
            }],
            movement: None,
            embarked_on: None,
            general: None,
            in_battle: None,
            infantry_count: 0,
            cavalry_count: 0,
            artillery_count: 0,
        };
        state.armies.insert(1, army);

        // Add fleet
        let fleet = crate::state::Fleet {
            id: 1,
            name: "Test Fleet".into(),
            owner: "SWE".into(),
            location: 2,
            ships: vec![crate::state::Ship {
                type_: crate::state::ShipType::HeavyShip,
                hull: Fixed::from_int(100),
                durability: Fixed::from_int(100),
            }],
            embarked_armies: vec![],
            movement: None,
            admiral: None,
            in_battle: None,
        };
        state.fleets.insert(1, fleet);

        // Run
        run_expenses_tick(&mut state);

        // Expected: 1 army (0.2) + 1 fleet (0.2) + 1 fort (1.0) = 1.4
        // Initial Treasury = 100.0
        // Result = 98.6
        let swe = state.countries.get("SWE").unwrap();
        assert_eq!(swe.treasury, Fixed::from_f32(98.6));
    }

    // TODO(review): Add determinism test (run twice, compare results)
}
