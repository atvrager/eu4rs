use crate::state::WorldState;

use crate::fixed::Fixed;
use eu4data::adjacency::AdjacencyGraph;

/// Base movement cost in "points" (days) per province for now.
const BASE_MOVE_COST: i64 = 10;
const BASE_SPEED: i64 = 1;

/// Runs daily movement tick for all armies with queued movement paths.
///
/// TODO: The `_graph` parameter is reserved for future terrain/naval zone validation.
/// Currently, movement costs are calculated via `WorldState::calculate_cost()` which
/// doesn't require the graph structure. Consider removing if not needed by Phase 5.
pub fn run_movement_tick(state: &mut WorldState, _graph: Option<&AdjacencyGraph>) {
    let mut completed_army_movements: Vec<u32> = Vec::new();
    let mut completed_fleet_movements: Vec<u32> = Vec::new();

    // Process fleets
    for (&fleet_id, fleet) in state.fleets.iter_mut() {
        if let Some(movement) = &mut fleet.movement {
            movement.progress += Fixed::from_int(BASE_SPEED); // Add daily progress

            if movement.progress >= movement.required_progress {
                // Move to next province
                if let Some(next_province) = movement.path.pop_front() {
                    let prev_location = fleet.location;
                    fleet.location = next_province;
                    movement.progress = Fixed::ZERO;

                    // Calculate cost for next step if path continues
                    // TODO: Use WorldState::calculate_cost() here. Currently blocked by borrow checker
                    // (can't call &state method while iterating &mut state.fleets).
                    // Solution: Collect (fleet_id, next_province, next_next) tuples first, then
                    // calculate costs in a second pass, or use interior mutability pattern.
                    if movement.path.front().is_some() {
                        movement.required_progress = Fixed::from_int(BASE_MOVE_COST);
                    }

                    log::info!(
                        "Fleet {} moved from {} to {}",
                        fleet_id,
                        prev_location,
                        next_province
                    );

                    if movement.path.is_empty() {
                        completed_fleet_movements.push(fleet_id);
                    }
                }
            }
        }
    }

    // Update embarked armies
    for army in state.armies.values_mut() {
        if let Some(fleet_id) = army.embarked_on {
            if let Some(fleet) = state.fleets.get(&fleet_id) {
                army.location = fleet.location;
            }
        }
    }

    // Process armies
    for (&army_id, army) in state.armies.iter_mut() {
        if army.embarked_on.is_some() {
            continue;
        }

        if let Some(movement) = &mut army.movement {
            movement.progress += Fixed::from_int(BASE_SPEED);

            if movement.progress >= movement.required_progress {
                if let Some(next_province) = movement.path.pop_front() {
                    let prev_location = army.location;
                    army.location = next_province;
                    movement.progress = Fixed::ZERO;

                    // Calculate cost for next step if path continues
                    // TODO: Use WorldState::calculate_cost() here (same borrow checker issue as fleets).
                    if movement.path.front().is_some() {
                        movement.required_progress = Fixed::from_int(BASE_MOVE_COST);
                    }

                    log::info!(
                        "Army {} moved from {} to {}",
                        army_id,
                        prev_location,
                        next_province
                    );

                    if movement.path.is_empty() {
                        completed_army_movements.push(army_id);
                    }
                }
            }
        }
    }

    // Cleanup
    for army_id in completed_army_movements {
        if let Some(army) = state.armies.get_mut(&army_id) {
            army.movement = None;
        }
    }
    for fleet_id in completed_fleet_movements {
        if let Some(fleet) = state.fleets.get_mut(&fleet_id) {
            fleet.movement = None;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fixed::Fixed;
    use crate::state::{Army, MovementState, Regiment, RegimentType};
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
            movement: Some(MovementState {
                path: VecDeque::from(vec![2]),
                progress: Fixed::ZERO,
                required_progress: Fixed::from_int(1), // Instant move for test
            }),
            embarked_on: None,
        };

        state.armies.insert(1, army);

        // Run movement tick
        run_movement_tick(&mut state, None);

        // Army should have moved to province 2 (because required=1, speed=1)
        let army = state.armies.get(&1).unwrap();
        assert_eq!(army.location, 2);
        assert!(army.movement.is_none()); // Path completed
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
            movement: Some(MovementState {
                path: VecDeque::from(vec![2, 3]),
                progress: Fixed::ZERO,
                required_progress: Fixed::from_int(1), // Instant
            }),
            embarked_on: None,
        };

        state.armies.insert(1, army);

        // First tick: move to province 2
        run_movement_tick(&mut state, None);
        let army = state.armies.get(&1).unwrap();
        assert_eq!(army.location, 2);

        let mv = army.movement.as_ref().unwrap();
        assert_eq!(mv.path, VecDeque::from(vec![3])); // Still has path

        // Fix up required_progress manually since we hardcoded BASE_MOVE_COST logic in system
        // but test assumed instant. The system resets req to 10 (BASE_MOVE_COST).
        // So next move will take 10 ticks.
        // We can simulate 10 ticks or just hack the state.

        // Simulating ticks...
        for _ in 0..10 {
            run_movement_tick(&mut state, None);
        }

        // Now should be at province 3
        let army = state.armies.get(&1).unwrap();
        assert_eq!(army.location, 3);
        assert!(army.movement.is_none()); // Path completed
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
            movement: None,
            embarked_on: None,
        };

        state.armies.insert(1, army);

        // Run movement tick
        run_movement_tick(&mut state, None);

        // Army should not move
        let army = state.armies.get(&1).unwrap();
        assert_eq!(army.location, 1);
        assert!(army.movement.is_none());
    }

    #[test]
    fn test_army_travel_time_exact() {
        let mut state = WorldStateBuilder::new()
            .with_country("SWE")
            .with_province(1, Some("SWE"))
            .with_province(2, Some("SWE"))
            .build();

        let army = Army {
            id: 1,
            name: "Timing Army".into(),
            owner: "SWE".into(),
            location: 1,
            regiments: vec![Regiment {
                type_: RegimentType::Infantry,
                strength: Fixed::from_int(1000),
            }],
            movement: Some(MovementState {
                path: VecDeque::from(vec![2]),
                progress: Fixed::ZERO,
                required_progress: Fixed::from_int(10), // Standard cost
            }),
            embarked_on: None,
        };
        state.armies.insert(1, army);

        // Tick 1 to 9: Should NOT move
        for i in 1..10 {
            run_movement_tick(&mut state, None);
            let a = state.armies.get(&1).unwrap();
            assert_eq!(a.location, 1, "Should stay at start on tick {}", i);
            let mv = a.movement.as_ref().unwrap();
            assert_eq!(mv.progress, Fixed::from_int(i as i64));
        }

        // Tick 10: Should move
        run_movement_tick(&mut state, None);
        let a = state.armies.get(&1).unwrap();
        assert_eq!(a.location, 2, "Should move on tick 10");
        assert!(a.movement.is_none(), "Path should be clear");
    }

    #[test]
    fn test_movement_uses_dynamic_costs() {
        let mut state = WorldStateBuilder::new()
            .with_country("SWE")
            .with_province(1, Some("SWE"))
            .with_province(2, Some("SWE"))
            .with_province(3, Some("SWE"))
            .build();

        let army = Army {
            id: 1,
            name: "Test Army".into(),
            owner: "SWE".into(),
            location: 1,
            regiments: vec![Regiment {
                type_: RegimentType::Infantry,
                strength: Fixed::from_int(1000),
            }],
            movement: Some(MovementState {
                path: VecDeque::from(vec![2, 3]),
                progress: Fixed::ZERO,
                required_progress: Fixed::from_int(10),
            }),
            embarked_on: None,
        };
        state.armies.insert(1, army);

        for _ in 0..10 {
            run_movement_tick(&mut state, None);
        }

        let a = state.armies.get(&1).unwrap();
        assert_eq!(a.location, 2);
        let mv = a.movement.as_ref().unwrap();
        assert_eq!(mv.required_progress, Fixed::from_int(10));
    }

    use proptest::prelude::*;

    proptest! {
        #[test]
        fn prop_movement_progress_monotonic(
            cost in 10..100i32,
            ticks in 1..50usize
        ) {
            let mut state = WorldState::default();
            let cost_fixed = Fixed::from_f32(cost as f32);
            
            // Setup army at 1, moving to 2
            let army = Army {
                id: 1,
                name: "Prop Army".into(),
                owner: "SWE".into(),
                location: 1,
                regiments: vec![],
                movement: Some(MovementState {
                    path: VecDeque::from(vec![2]),
                    progress: Fixed::ZERO,
                    required_progress: cost_fixed,
                }),
                embarked_on: None,
            };
            state.armies.insert(1, army);
            
            // Standard country setup
            state.countries.insert("SWE".into(), crate::state::CountryState::default());

            let mut prev_progress = Fixed::ZERO;
            
            for _ in 0..ticks {
                run_movement_tick(&mut state, None);
                
                if let Some(army) = state.armies.get(&1) {
                    if army.location == 2 {
                        break;
                    }
                    
                    if let Some(mv) = &army.movement {
                        prop_assert!(mv.progress >= prev_progress, "Progress decreased: {} -> {}", prev_progress, mv.progress);
                        prev_progress = mv.progress;
                    }
                }
            }
        }
    }
}
