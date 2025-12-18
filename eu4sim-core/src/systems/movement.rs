use crate::state::WorldState;

/// Runs daily movement tick for all armies with queued movement paths.
///
/// Movement rules:
/// - Advances 1 province per day
/// - Free movement (no movement cost for MVP)
/// - Triggers combat on arrival if enemies present
/// - Fleets move and carry embarked armies with them
pub fn run_movement_tick(state: &mut WorldState) {
    let mut completed_army_movements: Vec<u32> = Vec::new();
    let mut completed_fleet_movements: Vec<u32> = Vec::new();

    // Process all fleets with movement paths (fleets move first)
    for (&fleet_id, fleet) in state.fleets.iter_mut() {
        if let Some(path) = &mut fleet.movement_path {
            if let Some(next_province) = path.pop_front() {
                fleet.location = next_province;

                log::info!("Fleet {} moved to province {}", fleet_id, next_province);

                // If path is complete, clear it
                if path.is_empty() {
                    completed_fleet_movements.push(fleet_id);
                }
            }
        }
    }

    // Update embarked army locations to match their fleet
    for army in state.armies.values_mut() {
        if let Some(fleet_id) = army.embarked_on {
            if let Some(fleet) = state.fleets.get(&fleet_id) {
                army.location = fleet.location;
            }
        }
    }

    // Process all armies with movement paths (only non-embarked armies move independently)
    for (&army_id, army) in state.armies.iter_mut() {
        // Skip embarked armies (they move with their fleet)
        if army.embarked_on.is_some() {
            continue;
        }

        if let Some(path) = &mut army.movement_path {
            if let Some(next_province) = path.pop_front() {
                army.location = next_province;

                log::info!("Army {} moved to province {}", army_id, next_province);

                // If path is complete, clear it
                if path.is_empty() {
                    completed_army_movements.push(army_id);
                }
            }
        }
    }

    // Clear completed movement paths
    for army_id in completed_army_movements {
        if let Some(army) = state.armies.get_mut(&army_id) {
            army.movement_path = None;
        }
    }

    for fleet_id in completed_fleet_movements {
        if let Some(fleet) = state.fleets.get_mut(&fleet_id) {
            fleet.movement_path = None;
        }
    }

    // Combat is triggered by run_combat_tick in step.rs
    // which runs daily and checks for armies in the same province
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fixed::Fixed;
    use crate::state::{Army, Regiment, RegimentType};
    use crate::testing::WorldStateBuilder;
    use std::collections::VecDeque;

    #[test]
    fn test_movement_basic() {
        let mut state = WorldStateBuilder::new()
            .with_country("SWE")
            .with_province(1, Some("SWE"))
            .with_province(2, Some("SWE"))
            .build();

        // Create army with movement path
        let army = Army {
            id: 1,
            name: "Swedish Army".into(),
            owner: "SWE".into(),
            location: 1,
            regiments: vec![Regiment {
                type_: RegimentType::Infantry,
                strength: Fixed::from_int(1000),
            }],
            movement_path: Some(VecDeque::from(vec![2])),
            embarked_on: None,
        };

        state.armies.insert(1, army);

        // Run movement tick
        run_movement_tick(&mut state);

        // Army should have moved to province 2
        let army = state.armies.get(&1).unwrap();
        assert_eq!(army.location, 2);
        assert!(army.movement_path.is_none()); // Path completed
    }

    #[test]
    fn test_movement_multi_province_path() {
        let mut state = WorldStateBuilder::new()
            .with_country("SWE")
            .with_province(1, Some("SWE"))
            .with_province(2, Some("SWE"))
            .with_province(3, Some("SWE"))
            .build();

        // Create army with multi-province path
        let army = Army {
            id: 1,
            name: "Swedish Army".into(),
            owner: "SWE".into(),
            location: 1,
            regiments: vec![Regiment {
                type_: RegimentType::Infantry,
                strength: Fixed::from_int(1000),
            }],
            movement_path: Some(VecDeque::from(vec![2, 3])),
            embarked_on: None,
        };

        state.armies.insert(1, army);

        // First tick: move to province 2
        run_movement_tick(&mut state);
        let army = state.armies.get(&1).unwrap();
        assert_eq!(army.location, 2);
        assert_eq!(army.movement_path, Some(VecDeque::from(vec![3]))); // Still has path

        // Second tick: move to province 3
        run_movement_tick(&mut state);
        let army = state.armies.get(&1).unwrap();
        assert_eq!(army.location, 3);
        assert!(army.movement_path.is_none()); // Path completed
    }

    #[test]
    fn test_movement_no_path() {
        let mut state = WorldStateBuilder::new()
            .with_country("SWE")
            .with_province(1, Some("SWE"))
            .build();

        // Create army without movement path
        let army = Army {
            id: 1,
            name: "Swedish Army".into(),
            owner: "SWE".into(),
            location: 1,
            regiments: vec![Regiment {
                type_: RegimentType::Infantry,
                strength: Fixed::from_int(1000),
            }],
            movement_path: None,
            embarked_on: None,
        };

        state.armies.insert(1, army);

        // Run movement tick
        run_movement_tick(&mut state);

        // Army should not move
        let army = state.armies.get(&1).unwrap();
        assert_eq!(army.location, 1);
        assert!(army.movement_path.is_none());
    }
}
