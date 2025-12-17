use crate::fixed::Fixed;
use crate::state::{RegimentType, WorldState};
use std::collections::HashMap;

/// Base combat power for each regiment type.
const INFANTRY_POWER: Fixed = Fixed::from_raw(10000); // 1.0
const CAVALRY_POWER: Fixed = Fixed::from_raw(15000); // 1.5
const ARTILLERY_POWER: Fixed = Fixed::from_raw(12000); // 1.2

/// Daily casualties rate during combat (percentage of strength).
const DAILY_CASUALTY_RATE: Fixed = Fixed::from_raw(100); // 0.01 = 1% per day

/// Runs daily combat resolution for all active wars.
///
/// Combat occurs when opposing armies are in the same province.
/// This is a simplified model: strength-based attrition with no terrain/tactics.
pub fn run_combat_tick(state: &mut WorldState) {
    // Find all province locations with armies
    let mut province_armies: HashMap<u32, Vec<u32>> = HashMap::new();

    for (army_id, army) in &state.armies {
        province_armies
            .entry(army.location)
            .or_default()
            .push(*army_id);
    }

    // For each province with multiple armies, check if they're at war
    for (_province_id, army_ids) in province_armies {
        if army_ids.len() < 2 {
            continue;
        }

        // Group armies by owner
        let mut owners: HashMap<String, Vec<u32>> = HashMap::new();
        for &army_id in &army_ids {
            if let Some(army) = state.armies.get(&army_id) {
                owners.entry(army.owner.clone()).or_default().push(army_id);
            }
        }

        // Check all pairs of owners for wars
        let owner_list: Vec<String> = owners.keys().cloned().collect();
        for i in 0..owner_list.len() {
            for j in (i + 1)..owner_list.len() {
                let owner1 = &owner_list[i];
                let owner2 = &owner_list[j];

                if state.diplomacy.are_at_war(owner1, owner2) {
                    // Combat!
                    resolve_combat(state, &owners[owner1], &owners[owner2], owner1, owner2);
                }
            }
        }
    }
}

/// Resolves combat between two groups of armies.
fn resolve_combat(
    state: &mut WorldState,
    side1_armies: &[u32],
    side2_armies: &[u32],
    owner1: &str,
    owner2: &str,
) {
    // Calculate total combat power for each side
    let power1 = calculate_total_power(state, side1_armies);
    let power2 = calculate_total_power(state, side2_armies);

    // Prevent division by zero
    if power1 == Fixed::ZERO && power2 == Fixed::ZERO {
        return;
    }

    // Calculate casualties based on power ratio
    // Side with more power deals proportionally more damage
    let total_power = power1 + power2;
    let side1_casualties_rate = if power2 > Fixed::ZERO {
        DAILY_CASUALTY_RATE.mul(power2.div(total_power))
    } else {
        Fixed::ZERO
    };
    let side2_casualties_rate = if power1 > Fixed::ZERO {
        DAILY_CASUALTY_RATE.mul(power1.div(total_power))
    } else {
        Fixed::ZERO
    };

    // Apply casualties to all regiments
    apply_casualties_to_armies(state, side1_armies, side1_casualties_rate);
    apply_casualties_to_armies(state, side2_armies, side2_casualties_rate);

    log::info!(
        "Combat: {} (power {}) vs {} (power {})",
        owner1,
        power1.to_f32(),
        owner2,
        power2.to_f32()
    );
}

/// Calculates total combat power for a group of armies.
fn calculate_total_power(state: &WorldState, army_ids: &[u32]) -> Fixed {
    let mut total = Fixed::ZERO;

    for &army_id in army_ids {
        if let Some(army) = state.armies.get(&army_id) {
            for regiment in &army.regiments {
                let base_power = match regiment.type_ {
                    RegimentType::Infantry => INFANTRY_POWER,
                    RegimentType::Cavalry => CAVALRY_POWER,
                    RegimentType::Artillery => ARTILLERY_POWER,
                };
                // Power scales with regiment strength (men count)
                total += base_power.mul(regiment.strength.div(Fixed::from_int(1000)));
            }
        }
    }

    total
}

/// Applies casualties to armies (reduces regiment strength).
fn apply_casualties_to_armies(state: &mut WorldState, army_ids: &[u32], casualty_rate: Fixed) {
    for &army_id in army_ids {
        if let Some(army) = state.armies.get_mut(&army_id) {
            for regiment in &mut army.regiments {
                let casualties = regiment.strength.mul(casualty_rate);
                regiment.strength -= casualties;

                // Ensure strength doesn't go negative
                if regiment.strength < Fixed::ZERO {
                    regiment.strength = Fixed::ZERO;
                }
            }

            // Remove destroyed regiments (strength <= 0)
            army.regiments.retain(|r| r.strength > Fixed::ZERO);
        }
    }

    // Remove armies with no regiments
    state.armies.retain(|_, army| !army.regiments.is_empty());
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::{Army, Date, Regiment};
    use crate::testing::WorldStateBuilder;

    #[test]
    fn test_combat_basic() {
        let mut state = WorldStateBuilder::new()
            .date(1444, 11, 11)
            .with_country("SWE")
            .with_country("DEN")
            .build();

        // Create armies in the same province
        let army1 = Army {
            id: 1,
            name: "Swedish Army".into(),
            owner: "SWE".into(),
            location: 1,
            regiments: vec![Regiment {
                type_: RegimentType::Infantry,
                strength: Fixed::from_int(1000),
            }],
        };

        let army2 = Army {
            id: 2,
            name: "Danish Army".into(),
            owner: "DEN".into(),
            location: 1,
            regiments: vec![Regiment {
                type_: RegimentType::Infantry,
                strength: Fixed::from_int(1000),
            }],
        };

        state.armies.insert(1, army1);
        state.armies.insert(2, army2);

        // Declare war
        let war = crate::state::War {
            id: 0,
            name: "SWE vs DEN".into(),
            attackers: vec!["SWE".into()],
            defenders: vec!["DEN".into()],
            start_date: Date::new(1444, 11, 11),
        };
        state.diplomacy.wars.insert(0, war);

        // Run combat
        run_combat_tick(&mut state);

        // Both armies should have casualties
        let swe_army = state.armies.get(&1).unwrap();
        let den_army = state.armies.get(&2).unwrap();

        assert!(swe_army.regiments[0].strength < Fixed::from_int(1000));
        assert!(den_army.regiments[0].strength < Fixed::from_int(1000));
    }

    #[test]
    fn test_combat_no_war_no_casualties() {
        let mut state = WorldStateBuilder::new()
            .date(1444, 11, 11)
            .with_country("SWE")
            .with_country("DEN")
            .build();

        // Create armies in the same province (but not at war)
        let army1 = Army {
            id: 1,
            name: "Swedish Army".into(),
            owner: "SWE".into(),
            location: 1,
            regiments: vec![Regiment {
                type_: RegimentType::Infantry,
                strength: Fixed::from_int(1000),
            }],
        };

        let army2 = Army {
            id: 2,
            name: "Danish Army".into(),
            owner: "DEN".into(),
            location: 1,
            regiments: vec![Regiment {
                type_: RegimentType::Infantry,
                strength: Fixed::from_int(1000),
            }],
        };

        state.armies.insert(1, army1);
        state.armies.insert(2, army2);

        // Run combat (should do nothing - no war)
        run_combat_tick(&mut state);

        // No casualties should occur
        let swe_army = state.armies.get(&1).unwrap();
        let den_army = state.armies.get(&2).unwrap();

        assert_eq!(swe_army.regiments[0].strength, Fixed::from_int(1000));
        assert_eq!(den_army.regiments[0].strength, Fixed::from_int(1000));
    }

    #[test]
    fn test_combat_asymmetric_casualties() {
        let mut state = WorldStateBuilder::new()
            .date(1444, 11, 11)
            .with_country("SWE")
            .with_country("DEN")
            .build();

        // Create one strong army and one weak army
        let army1 = Army {
            id: 1,
            name: "Swedish Army".into(),
            owner: "SWE".into(),
            location: 1,
            regiments: vec![Regiment {
                type_: RegimentType::Infantry,
                strength: Fixed::from_int(5000),
            }],
        };

        let army2 = Army {
            id: 2,
            name: "Danish Army".into(),
            owner: "DEN".into(),
            location: 1,
            regiments: vec![Regiment {
                type_: RegimentType::Infantry,
                strength: Fixed::from_int(1000),
            }],
        };

        state.armies.insert(1, army1);
        state.armies.insert(2, army2);

        // Declare war
        let war = crate::state::War {
            id: 0,
            name: "SWE vs DEN".into(),
            attackers: vec!["SWE".into()],
            defenders: vec!["DEN".into()],
            start_date: Date::new(1444, 11, 11),
        };
        state.diplomacy.wars.insert(0, war);

        // Run combat for a few days
        for _ in 0..10 {
            run_combat_tick(&mut state);
        }

        // Both armies should have casualties, but weak army should lose proportionally more
        let strong_army = state.armies.get(&1).unwrap();
        let weak_army = state.armies.get(&2).unwrap();

        // Strong army should have minor casualties (stronger army takes less damage)
        assert!(strong_army.regiments[0].strength < Fixed::from_int(5000));
        assert!(strong_army.regiments[0].strength > Fixed::from_int(4900));

        // Weak army should have significant casualties (weaker army takes more damage)
        assert!(weak_army.regiments[0].strength < Fixed::from_int(1000));
        assert!(weak_army.regiments[0].strength < Fixed::from_int(950));
    }
}
