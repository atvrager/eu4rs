use crate::fixed::Fixed;
use crate::state::{ProvinceId, WorldState};
use eu4data::adjacency::AdjacencyGraph;
use rayon::prelude::*;
use tracing::instrument;

const BASE_SPEED: i64 = 1;

/// Result of processing a single unit's movement
struct MovementResult {
    unit_id: u32,
    new_location: Option<ProvinceId>,
    new_previous_location: Option<ProvinceId>,
    new_progress: Fixed,
    path_consumed: bool, // Did we pop from the path?
    completed: bool,
    cost_update: Option<(ProvinceId, ProvinceId)>, // (from, to) for next leg
}

/// Process a single fleet's movement (pure function, no mutation)
#[instrument(skip_all, name = "fleet_move")]
fn process_fleet_movement(
    fleet_id: u32,
    location: ProvinceId,
    movement_progress: Fixed,
    movement_required: Fixed,
    path_front: Option<ProvinceId>,
    path_next: Option<ProvinceId>,
    path_len: usize,
) -> MovementResult {
    let new_progress = movement_progress + Fixed::from_int(BASE_SPEED);

    if new_progress >= movement_required {
        if let Some(next_province) = path_front {
            let cost_update = path_next.map(|next_next| (next_province, next_next));
            let completed = path_len == 1; // Only one item in path, will be empty after pop

            log::trace!(
                "Fleet {} moved from {} to {}",
                fleet_id,
                location,
                next_province
            );

            return MovementResult {
                unit_id: fleet_id,
                new_location: Some(next_province),
                new_previous_location: None,
                new_progress: Fixed::ZERO,
                path_consumed: true,
                completed,
                cost_update,
            };
        }
    }

    MovementResult {
        unit_id: fleet_id,
        new_location: None,
        new_previous_location: None,
        new_progress,
        path_consumed: false,
        completed: false,
        cost_update: None,
    }
}

/// Process a single army's movement (pure function, no mutation)
#[instrument(skip_all, name = "army_move")]
fn process_army_movement(
    army_id: u32,
    location: ProvinceId,
    movement_progress: Fixed,
    movement_required: Fixed,
    path_front: Option<ProvinceId>,
    path_next: Option<ProvinceId>,
    path_len: usize,
) -> MovementResult {
    let new_progress = movement_progress + Fixed::from_int(BASE_SPEED);

    if new_progress >= movement_required {
        if let Some(next_province) = path_front {
            let cost_update = path_next.map(|next_next| (next_province, next_next));
            let completed = path_len == 1;

            log::trace!(
                "Army {} moved from {} to {}",
                army_id,
                location,
                next_province
            );

            return MovementResult {
                unit_id: army_id,
                new_location: Some(next_province),
                new_previous_location: Some(location),
                new_progress: Fixed::ZERO,
                path_consumed: true,
                completed,
                cost_update,
            };
        }
    }

    MovementResult {
        unit_id: army_id,
        new_location: None,
        new_previous_location: None,
        new_progress,
        path_consumed: false,
        completed: false,
        cost_update: None,
    }
}

/// Runs daily movement tick for all armies with queued movement paths.
#[instrument(skip_all, name = "movement")]
pub fn run_movement_tick(state: &mut WorldState, _graph: Option<&AdjacencyGraph>) {
    // === PHASE 1: Extract fleet data for parallel processing ===
    let fleet_inputs: Vec<_> = state
        .fleets
        .iter()
        .filter_map(|(&fleet_id, fleet)| {
            fleet.movement.as_ref().map(|m| {
                (
                    fleet_id,
                    fleet.location,
                    m.progress,
                    m.required_progress,
                    m.path.front().copied(),
                    m.path.get(1).copied(),
                    m.path.len(),
                )
            })
        })
        .collect();

    // Process fleets in parallel
    let fleet_results: Vec<MovementResult> = {
        let _span = tracing::info_span!("fleets_parallel", count = fleet_inputs.len()).entered();
        fleet_inputs
            .into_par_iter()
            .map(|(id, loc, prog, req, front, next, len)| {
                process_fleet_movement(id, loc, prog, req, front, next, len)
            })
            .collect()
    };

    // Apply fleet results
    let mut fleet_cost_updates: Vec<(u32, ProvinceId, ProvinceId)> = Vec::new();
    let mut completed_fleet_movements: Vec<u32> = Vec::new();

    for result in fleet_results {
        if let Some(fleet) = state.fleets.get_mut(&result.unit_id) {
            if let Some(movement) = &mut fleet.movement {
                movement.progress = result.new_progress;

                if result.path_consumed {
                    movement.path.pop_front();
                }

                if let Some(new_loc) = result.new_location {
                    fleet.location = new_loc;
                }

                if result.completed {
                    completed_fleet_movements.push(result.unit_id);
                }

                if let Some((from, to)) = result.cost_update {
                    fleet_cost_updates.push((result.unit_id, from, to));
                }
            }
        }
    }

    // Update embarked armies (must happen after fleets move)
    let embarked_updates: Vec<_> = state
        .armies
        .iter()
        .filter_map(|(&army_id, army)| {
            army.embarked_on.and_then(|fleet_id| {
                state
                    .fleets
                    .get(&fleet_id)
                    .map(|fleet| (army_id, fleet.location))
            })
        })
        .collect();

    for (army_id, new_location) in embarked_updates {
        if let Some(army) = state.armies.get_mut(&army_id) {
            army.location = new_location;
        }
    }

    // === PHASE 2: Extract army data for parallel processing ===
    let army_inputs: Vec<_> = state
        .armies
        .iter()
        .filter_map(|(&army_id, army)| {
            // Skip embarked armies
            if army.embarked_on.is_some() {
                return None;
            }
            army.movement.as_ref().map(|m| {
                (
                    army_id,
                    army.location,
                    m.progress,
                    m.required_progress,
                    m.path.front().copied(),
                    m.path.get(1).copied(),
                    m.path.len(),
                )
            })
        })
        .collect();

    // Process armies in parallel
    let army_results: Vec<MovementResult> = {
        let _span = tracing::info_span!("armies_parallel", count = army_inputs.len()).entered();
        army_inputs
            .into_par_iter()
            .map(|(id, loc, prog, req, front, next, len)| {
                process_army_movement(id, loc, prog, req, front, next, len)
            })
            .collect()
    };

    // Apply army results
    let mut army_cost_updates: Vec<(u32, ProvinceId, ProvinceId)> = Vec::new();
    let mut completed_army_movements: Vec<u32> = Vec::new();

    for result in army_results {
        if let Some(army) = state.armies.get_mut(&result.unit_id) {
            if let Some(movement) = &mut army.movement {
                movement.progress = result.new_progress;

                if result.path_consumed {
                    movement.path.pop_front();
                }

                if let Some(new_loc) = result.new_location {
                    army.previous_location = result.new_previous_location;
                    army.location = new_loc;
                }

                if result.completed {
                    completed_army_movements.push(result.unit_id);
                }

                if let Some((from, to)) = result.cost_update {
                    army_cost_updates.push((result.unit_id, from, to));
                }
            }
        }
    }

    // === PHASE 3: Apply dynamic costs ===
    use eu4data::adjacency::CostCalculator;

    for (fleet_id, from, to) in fleet_cost_updates {
        let cost = state.calculate_cost(from, to);
        if let Some(fleet) = state.fleets.get_mut(&fleet_id) {
            if let Some(movement) = &mut fleet.movement {
                movement.required_progress = Fixed::from_int(cost as i64);
            }
        }
    }

    for (army_id, from, to) in army_cost_updates {
        let cost = state.calculate_cost(from, to);
        if let Some(army) = state.armies.get_mut(&army_id) {
            if let Some(movement) = &mut army.movement {
                movement.required_progress = Fixed::from_int(cost as i64);
            }
        }
    }

    // === PHASE 4: Cleanup completed movements ===
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
    use crate::state::{Army, MovementState, Regiment, RegimentType, Terrain};
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
            previous_location: None,
            regiments: vec![Regiment {
                type_: RegimentType::Infantry,
                strength: Fixed::from_int(1000),
                morale: Fixed::from_f32(eu4data::defines::combat::BASE_MORALE),
            }],
            movement: Some(MovementState {
                path: VecDeque::from(vec![2]),
                progress: Fixed::ZERO,
                required_progress: Fixed::from_int(1), // Instant move for test
            }),
            embarked_on: None,
            general: None,
            in_battle: None,
            infantry_count: 0,
            cavalry_count: 0,
            artillery_count: 0,
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
            previous_location: None,
            regiments: vec![Regiment {
                type_: RegimentType::Infantry,
                strength: Fixed::from_int(1000),
                morale: Fixed::from_f32(eu4data::defines::combat::BASE_MORALE),
            }],
            movement: Some(MovementState {
                path: VecDeque::from(vec![2, 3]),
                progress: Fixed::ZERO,
                required_progress: Fixed::from_int(1), // Instant
            }),
            embarked_on: None,
            general: None,
            in_battle: None,
            infantry_count: 0,
            cavalry_count: 0,
            artillery_count: 0,
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
            previous_location: None,
            regiments: vec![Regiment {
                type_: RegimentType::Infantry,
                strength: Fixed::from_int(1000),
                morale: Fixed::from_f32(eu4data::defines::combat::BASE_MORALE),
            }],
            movement: Some(MovementState {
                path: VecDeque::from(vec![2]),
                progress: Fixed::ZERO,
                required_progress: Fixed::from_int(10), // Standard cost
            }),
            embarked_on: None,
            general: None,
            in_battle: None,
            infantry_count: 0,
            cavalry_count: 0,
            artillery_count: 0,
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
            .with_terrain(2, Terrain::Plains)
            .with_terrain(3, Terrain::Mountains)
            .build();

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
            movement: Some(MovementState {
                path: VecDeque::from(vec![2, 3]),
                progress: Fixed::ZERO,
                required_progress: Fixed::from_int(5), // Short leg 1
            }),
            embarked_on: None,
            general: None,
            in_battle: None,
            infantry_count: 0,
            cavalry_count: 0,
            artillery_count: 0,
        };
        state.armies.insert(1, army);

        // Move to province 2 (takes 5 ticks)
        for _ in 0..5 {
            run_movement_tick(&mut state, None);
        }
        let a = state.armies.get(&1).unwrap();
        assert_eq!(a.location, 2);

        // Verify next leg cost is mountain-based (20 days)
        let mv = a.movement.as_ref().unwrap();
        assert_eq!(mv.required_progress, Fixed::from_int(20));

        // Finish movement through mountains
        for _ in 0..20 {
            run_movement_tick(&mut state, None);
        }
        let a = state.armies.get(&1).unwrap();
        assert_eq!(a.location, 3);
        assert!(a.movement.is_none());
    }

    #[test]
    fn test_sea_movement_cost() {
        let state = WorldStateBuilder::new()
            .with_country("SWE")
            .with_province(1, Some("SWE"))
            .with_province(2, Some("SWE"))
            .with_terrain(2, Terrain::Sea)
            .build();

        use eu4data::adjacency::CostCalculator;
        let cost = state.calculate_cost(1, 2);
        assert_eq!(cost, 5); // Sea is 0.5x
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
                previous_location: None,
                regiments: vec![],
                movement: Some(MovementState {
                    path: VecDeque::from(vec![2]),
                    progress: Fixed::ZERO,
                    required_progress: cost_fixed,
                }),
                embarked_on: None,
            general: None,
            in_battle: None,
            infantry_count: 0,
            cavalry_count: 0,
            artillery_count: 0,
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
