use crate::fixed::Fixed;
use crate::input::{Command, DevType, PlayerInputs};
use crate::state::{MovementState, WorldState};
use thiserror::Error;

#[derive(Error, Debug)]
pub enum ActionError {
    #[error("Insufficient funds: required {required}, available {available}")]
    InsufficientFunds { required: f32, available: f32 },
    #[error("Country not found: {tag}")]
    CountryNotFound { tag: String },
    #[error("Already at war with {target}")]
    AlreadyAtWar { target: String },
    #[error("Cannot declare war on self")]
    CannotDeclareWarOnSelf,
    #[error("Army not found: {army_id}")]
    ArmyNotFound { army_id: u32 },
    #[error("Army {army_id} is not owned by {tag}")]
    ArmyNotOwned { army_id: u32, tag: String },
    #[error("Fleet not found: {fleet_id}")]
    FleetNotFound { fleet_id: u32 },
    #[error("Fleet {fleet_id} is not owned by {tag}")]
    FleetNotOwned { fleet_id: u32, tag: String },
    #[error("Province {destination} is not adjacent to {current}")]
    NotAdjacent { current: u32, destination: u32 },
    #[error("No path exists from {start} to {destination}")]
    NoPathExists { start: u32, destination: u32 },
    #[error("No military access to {province} (owned by {owner})")]
    NoMilitaryAccess { province: u32, owner: String },
    #[error("Army and fleet are not in the same location")]
    NotSameLocation,
    #[error("Fleet has insufficient capacity")]
    InsufficientCapacity,
    #[error("Army {army_id} is not embarked")]
    ArmyNotEmbarked { army_id: u32 },
    #[error("Destination {destination} is not adjacent to fleet location {fleet_location}")]
    DestinationNotAdjacent {
        destination: u32,
        fleet_location: u32,
    },
    #[error("Insufficient mana for this action")]
    InsufficientMana,
    #[error("Invalid country tag")]
    InvalidTag,
    #[error("Invalid province ID")]
    InvalidProvinceId,
    #[error("Province is not owned by this country")]
    NotOwned,
}

/// Advance the world by one tick.
pub fn step_world(
    state: &WorldState,
    inputs: &[PlayerInputs],
    adjacency: Option<&eu4data::adjacency::AdjacencyGraph>,
    config: &crate::config::SimConfig,
) -> WorldState {
    let mut new_state = state.clone();

    // 1. Advance Date
    new_state.date = state.date.add_days(1);

    // 2. Process Inputs
    for player_input in inputs {
        for cmd in &player_input.commands {
            if let Err(e) = execute_command(&mut new_state, &player_input.country, cmd, adjacency) {
                log::warn!(
                    "Failed to execute command for {}: {}",
                    player_input.country,
                    e
                );
            }
        }
    }

    // 3. Run Systems
    // Movement runs daily (advances armies along their paths)
    crate::systems::run_movement_tick(&mut new_state, adjacency);

    // Combat runs daily (whenever armies are engaged)
    crate::systems::run_combat_tick(&mut new_state);

    // Economic systems run monthly (on 1st of each month)
    if new_state.date.day == 1 {
        let economy_config = crate::systems::EconomyConfig::default();
        crate::systems::run_production_tick(&mut new_state, &economy_config);
        crate::systems::run_taxation_tick(&mut new_state);
        crate::systems::run_manpower_tick(&mut new_state);
        crate::systems::run_expenses_tick(&mut new_state);
        crate::systems::run_mana_tick(&mut new_state);
    }

    // 4. Compute checksum (if enabled)
    if config.checksum_frequency > 0 {
        // Calculate tick number (days since start date)
        // For simplicity, we'll use a simple counter based on date
        // In production, WorldState should track tick count explicitly
        let tick = ((new_state.date.year - 1444) * 365
            + (new_state.date.month as i32 - 1) * 30
            + (new_state.date.day as i32 - 1)) as u64;

        if tick.is_multiple_of(config.checksum_frequency as u64) {
            let checksum = new_state.checksum();
            log::debug!("Tick {}: checksum={:016x}", tick, checksum);
        }
    }

    new_state
}

fn execute_command(
    state: &mut WorldState,
    country_tag: &str,
    cmd: &Command,
    adjacency: Option<&eu4data::adjacency::AdjacencyGraph>,
) -> Result<(), ActionError> {
    match cmd {
        Command::BuildInProvince {
            province: _,
            building: _,
        } => {
            // Stub implementation
            let _country =
                state
                    .countries
                    .get(country_tag)
                    .ok_or(ActionError::CountryNotFound {
                        tag: country_tag.to_string(),
                    })?;

            // Validate Logic (Check cost vs treasury)
            // if country.treasury < cost ...

            // Apply Effect
            log::info!("Player {} building something (stub)", country_tag);

            Ok(())
        }
        Command::Move {
            army_id,
            destination,
        } => {
            // Validate army exists
            let army = state
                .armies
                .get(army_id)
                .ok_or(ActionError::ArmyNotFound { army_id: *army_id })?;

            // Validate ownership
            if army.owner != country_tag {
                return Err(ActionError::ArmyNotOwned {
                    army_id: *army_id,
                    tag: country_tag.to_string(),
                });
            }

            let current_location = army.location;

            // Find path using adjacency graph (if available)
            let path = if let Some(graph) = adjacency {
                use game_pathfinding::AStar;
                let (path_vec, _) = AStar::find_path(graph, current_location, *destination, state)
                    .ok_or(ActionError::NoPathExists {
                        start: current_location,
                        destination: *destination,
                    })?;
                // A* returns [start, p1, p2, end]. We just want [p1, p2, end].
                let mut p = std::collections::VecDeque::from(path_vec);
                if p.front() == Some(&current_location) {
                    p.pop_front();
                }
                p.into()
            } else {
                // Fallback: assume direct adjacency if no graph available
                vec![*destination]
            };

            // Check military access for destination (static check at command time)
            if let Some(province) = state.provinces.get(destination) {
                if let Some(owner) = &province.owner {
                    if owner != country_tag {
                        // Need military access to move through another country's territory
                        if !state.diplomacy.has_military_access(country_tag, owner) {
                            // Exception: can move if at war
                            if !state.diplomacy.are_at_war(country_tag, owner) {
                                return Err(ActionError::NoMilitaryAccess {
                                    province: *destination,
                                    owner: owner.clone(),
                                });
                            }
                        }
                    }
                }
            }

            // Set movement path
            // TODO: Handle edge case where start == destination (empty path).
            // Currently wastes 10 ticks doing nothing. Should skip movement initialization.
            if let Some(army) = state.armies.get_mut(army_id) {
                army.movement = Some(MovementState {
                    path: path.clone().into(),
                    progress: Fixed::ZERO,
                    required_progress: Fixed::from_int(10), // BASE_MOVE_COST
                });
                log::info!(
                    "Army {} pathing from {} to {} via {:?}",
                    army_id,
                    current_location,
                    destination,
                    path
                );
            }

            Ok(())
        }
        Command::DeclareWar { target } => {
            // Validate attacker exists
            if !state.countries.contains_key(country_tag) {
                return Err(ActionError::CountryNotFound {
                    tag: country_tag.to_string(),
                });
            }

            // Validate target exists
            if !state.countries.contains_key(target) {
                return Err(ActionError::CountryNotFound {
                    tag: target.clone(),
                });
            }

            // Cannot declare war on self
            if country_tag == target {
                return Err(ActionError::CannotDeclareWarOnSelf);
            }

            // Check if already at war
            if state.diplomacy.are_at_war(country_tag, target) {
                return Err(ActionError::AlreadyAtWar {
                    target: target.clone(),
                });
            }

            // Create war
            let war_id = state.diplomacy.next_war_id;
            state.diplomacy.next_war_id += 1;

            let war = crate::state::War {
                id: war_id,
                name: format!("{} vs {}", country_tag, target),
                attackers: vec![country_tag.to_string()],
                defenders: vec![target.clone()],
                start_date: state.date,
            };

            state.diplomacy.wars.insert(war_id, war);

            log::info!("{} declared war on {}", country_tag, target);

            Ok(())
        }
        Command::MoveFleet {
            fleet_id,
            destination,
        } => {
            // Validate fleet exists
            let fleet = state
                .fleets
                .get(fleet_id)
                .ok_or(ActionError::FleetNotFound {
                    fleet_id: *fleet_id,
                })?;

            // Validate ownership
            if fleet.owner != country_tag {
                return Err(ActionError::FleetNotOwned {
                    fleet_id: *fleet_id,
                    tag: country_tag.to_string(),
                });
            }

            let current_location = fleet.location;

            // Find path using adjacency graph (if available)
            let path = if let Some(graph) = adjacency {
                use game_pathfinding::AStar;
                let (path_vec, _) = AStar::find_path(graph, current_location, *destination, state)
                    .ok_or(ActionError::NoPathExists {
                        start: current_location,
                        destination: *destination,
                    })?;
                let mut p = std::collections::VecDeque::from(path_vec);
                if p.front() == Some(&current_location) {
                    p.pop_front();
                }
                p.into()
            } else {
                // Fallback: assume direct adjacency if no graph available
                vec![*destination]
            };

            // Set movement path (fleets use same movement_path pattern as armies)
            if let Some(fleet) = state.fleets.get_mut(fleet_id) {
                fleet.movement = Some(MovementState {
                    path: path.clone().into(),
                    progress: Fixed::ZERO,
                    required_progress: Fixed::from_int(10), // BASE_MOVE_COST
                });
                log::info!(
                    "Fleet {} pathing from {} to {} via {:?}",
                    fleet_id,
                    current_location,
                    destination,
                    path
                );
            }

            Ok(())
        }
        Command::Embark { army_id, fleet_id } => {
            // Validate army exists
            let army = state
                .armies
                .get(army_id)
                .ok_or(ActionError::ArmyNotFound { army_id: *army_id })?;

            // Validate ownership
            if army.owner != country_tag {
                return Err(ActionError::ArmyNotOwned {
                    army_id: *army_id,
                    tag: country_tag.to_string(),
                });
            }

            // Validate fleet exists
            let fleet = state
                .fleets
                .get(fleet_id)
                .ok_or(ActionError::FleetNotFound {
                    fleet_id: *fleet_id,
                })?;

            // Validate fleet ownership
            if fleet.owner != country_tag {
                return Err(ActionError::FleetNotOwned {
                    fleet_id: *fleet_id,
                    tag: country_tag.to_string(),
                });
            }

            // Validate same location
            if army.location != fleet.location {
                return Err(ActionError::NotSameLocation);
            }

            // Check capacity (1 regiment = 1 capacity)
            let army_size = army.regiments.len() as u32;
            let current_capacity_used: u32 = fleet
                .embarked_armies
                .iter()
                .filter_map(|aid| state.armies.get(aid))
                .map(|a| a.regiments.len() as u32)
                .sum();

            if current_capacity_used + army_size > fleet.transport_capacity {
                return Err(ActionError::InsufficientCapacity);
            }

            // Embark the army
            if let Some(army) = state.armies.get_mut(army_id) {
                army.embarked_on = Some(*fleet_id);
            }

            if let Some(fleet) = state.fleets.get_mut(fleet_id) {
                fleet.embarked_armies.push(*army_id);
            }

            log::info!("Army {} embarked on fleet {}", army_id, fleet_id);

            Ok(())
        }
        Command::Disembark {
            army_id,
            destination,
        } => {
            // Validate army exists
            let army = state
                .armies
                .get(army_id)
                .ok_or(ActionError::ArmyNotFound { army_id: *army_id })?;

            // Validate ownership
            if army.owner != country_tag {
                return Err(ActionError::ArmyNotOwned {
                    army_id: *army_id,
                    tag: country_tag.to_string(),
                });
            }

            // Validate army is embarked
            let fleet_id = army
                .embarked_on
                .ok_or(ActionError::ArmyNotEmbarked { army_id: *army_id })?;

            let fleet = state
                .fleets
                .get(&fleet_id)
                .ok_or(ActionError::FleetNotFound { fleet_id })?;

            // Validate destination is adjacent to fleet location
            if let Some(graph) = adjacency {
                if !graph.are_adjacent(fleet.location, *destination) {
                    return Err(ActionError::DestinationNotAdjacent {
                        destination: *destination,
                        fleet_location: fleet.location,
                    });
                }
            }

            // Disembark the army
            if let Some(army) = state.armies.get_mut(army_id) {
                army.location = *destination;
                army.embarked_on = None;
            }

            if let Some(fleet) = state.fleets.get_mut(&fleet_id) {
                fleet.embarked_armies.retain(|&id| id != *army_id);
            }

            log::info!(
                "Army {} disembarked from fleet {} to province {}",
                army_id,
                fleet_id,
                destination
            );

            Ok(())
        }
        Command::PurchaseDevelopment { province, dev_type } => {
            const DEV_COST: Fixed = Fixed::from_int(50);

            // Validate country exists
            let country = state
                .countries
                .get_mut(country_tag)
                .ok_or(ActionError::InvalidTag)?;

            // Validate province exists and is owned
            let prov = state
                .provinces
                .get_mut(province)
                .ok_or(ActionError::InvalidProvinceId)?;

            if prov.owner.as_deref() != Some(country_tag) {
                return Err(ActionError::NotOwned);
            }

            // Check mana and apply cost
            match dev_type {
                DevType::Tax => {
                    if country.adm_mana < DEV_COST {
                        return Err(ActionError::InsufficientMana);
                    }
                    country.adm_mana -= DEV_COST;
                    prov.base_tax += Fixed::from_int(1);
                }
                DevType::Production => {
                    if country.dip_mana < DEV_COST {
                        return Err(ActionError::InsufficientMana);
                    }
                    country.dip_mana -= DEV_COST;
                    prov.base_production += Fixed::from_int(1);
                }
                DevType::Manpower => {
                    if country.mil_mana < DEV_COST {
                        return Err(ActionError::InsufficientMana);
                    }
                    country.mil_mana -= DEV_COST;
                    prov.base_manpower += Fixed::from_int(1);
                }
            }

            log::info!(
                "{} purchased {:?} development in province {} for {} mana",
                country_tag,
                dev_type,
                province,
                DEV_COST.to_f32()
            );

            Ok(())
        }
        Command::Quit => Ok(()), // Handled by outer loop usually, but harmless here
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::Date;
    use crate::testing::WorldStateBuilder;

    #[test]
    fn test_step_world_advances_date() {
        let state = WorldStateBuilder::new().date(1444, 11, 11).build();

        let inputs = vec![];
        let new_state = step_world(&state, &inputs, None, &crate::config::SimConfig::default());

        assert_eq!(new_state.date, Date::new(1444, 11, 12));
    }

    #[test]
    fn test_step_world_command_execution() {
        let state = WorldStateBuilder::new()
            .date(1444, 11, 11)
            .with_country("SWE")
            .build();

        let inputs = vec![PlayerInputs {
            country: "SWE".to_string(),
            commands: vec![Command::BuildInProvince {
                province: 1,
                building: "temple".to_string(),
            }],
        }];

        // This should log (we can't easily assert logs without a capture, but we know it runs)
        // Ideally we'd inspect side effects on state, but the stub does nothing yet.
        let _new_state = step_world(&state, &inputs, None, &crate::config::SimConfig::default());

        // Assert no crash and logic ran
    }

    #[test]
    fn test_determinism() {
        let state = WorldStateBuilder::new()
            .date(1444, 1, 1)
            .with_country("SWE")
            .build();

        let inputs = vec![];

        let state_a = step_world(&state, &inputs, None, &crate::config::SimConfig::default());
        let state_b = step_world(&state, &inputs, None, &crate::config::SimConfig::default());

        // Serialize to compare fully or just debug format
        let json_a = serde_json::to_string(&state_a).unwrap();
        let json_b = serde_json::to_string(&state_b).unwrap();

        assert_eq!(json_a, json_b);
    }

    #[test]
    fn test_declare_war_success() {
        let state = WorldStateBuilder::new()
            .date(1444, 11, 11)
            .with_country("SWE")
            .with_country("DEN")
            .build();

        let inputs = vec![PlayerInputs {
            country: "SWE".to_string(),
            commands: vec![Command::DeclareWar {
                target: "DEN".to_string(),
            }],
        }];

        let new_state = step_world(&state, &inputs, None, &crate::config::SimConfig::default());

        // War should be created
        assert_eq!(new_state.diplomacy.wars.len(), 1);

        // Countries should be at war
        assert!(new_state.diplomacy.are_at_war("SWE", "DEN"));
    }

    #[test]
    fn test_declare_war_on_self_fails() {
        let state = WorldStateBuilder::new()
            .date(1444, 11, 11)
            .with_country("SWE")
            .build();

        let inputs = vec![PlayerInputs {
            country: "SWE".to_string(),
            commands: vec![Command::DeclareWar {
                target: "SWE".to_string(),
            }],
        }];

        let new_state = step_world(&state, &inputs, None, &crate::config::SimConfig::default());

        // No war should be created
        assert_eq!(new_state.diplomacy.wars.len(), 0);
    }

    #[test]
    fn test_declare_war_twice_fails() {
        let mut state = WorldStateBuilder::new()
            .date(1444, 11, 11)
            .with_country("SWE")
            .with_country("DEN")
            .build();

        // First war declaration
        let inputs1 = vec![PlayerInputs {
            country: "SWE".to_string(),
            commands: vec![Command::DeclareWar {
                target: "DEN".to_string(),
            }],
        }];

        state = step_world(&state, &inputs1, None, &crate::config::SimConfig::default());
        assert_eq!(state.diplomacy.wars.len(), 1);

        // Second war declaration (should fail)
        let inputs2 = vec![PlayerInputs {
            country: "SWE".to_string(),
            commands: vec![Command::DeclareWar {
                target: "DEN".to_string(),
            }],
        }];

        let new_state = step_world(&state, &inputs2, None, &crate::config::SimConfig::default());

        // Still only one war
        assert_eq!(new_state.diplomacy.wars.len(), 1);
    }

    #[test]
    fn test_declare_war_nonexistent_country() {
        let state = WorldStateBuilder::new()
            .date(1444, 11, 11)
            .with_country("SWE")
            .build();

        let inputs = vec![PlayerInputs {
            country: "SWE".to_string(),
            commands: vec![Command::DeclareWar {
                target: "XXX".to_string(),
            }],
        }];

        let new_state = step_world(&state, &inputs, None, &crate::config::SimConfig::default());

        // No war should be created
        assert_eq!(new_state.diplomacy.wars.len(), 0);
    }

    #[test]
    fn test_dev_purchasing_full_cycle() {
        let mut state = WorldStateBuilder::new()
            .with_country("SWE")
            .with_province_full(1, Some("SWE"), None, Fixed::from_int(5))
            .build();

        // Generate mana (17 months = 51 mana each)
        for _ in 0..17 {
            state.date = state.date.add_days(30);
            crate::systems::run_mana_tick(&mut state);
        }

        // Purchase tax dev
        let cmd = Command::PurchaseDevelopment {
            province: 1,
            dev_type: DevType::Tax,
        };
        execute_command(&mut state, "SWE", &cmd, None).unwrap();

        // Verify state
        let swe = state.countries.get("SWE").unwrap();
        let prov = state.provinces.get(&1).unwrap();

        assert_eq!(swe.adm_mana, Fixed::from_int(1)); // 51 - 50
        assert_eq!(prov.base_tax, Fixed::from_int(2)); // 1 + 1

        // Insufficient mana should fail
        let cmd2 = Command::PurchaseDevelopment {
            province: 1,
            dev_type: DevType::Tax,
        };
        assert!(execute_command(&mut state, "SWE", &cmd2, None).is_err());
    }

    #[test]
    fn test_dev_purchasing_all_types() {
        let mut state = WorldStateBuilder::new()
            .with_country("SWE")
            .with_province_full(1, Some("SWE"), None, Fixed::from_int(5))
            .build();

        // Generate mana (51 months = 153 mana each)
        for _ in 0..51 {
            state.date = state.date.add_days(30);
            crate::systems::run_mana_tick(&mut state);
        }

        let initial_swe = state.countries.get("SWE").unwrap();
        assert_eq!(initial_swe.adm_mana, Fixed::from_int(153));
        assert_eq!(initial_swe.dip_mana, Fixed::from_int(153));
        assert_eq!(initial_swe.mil_mana, Fixed::from_int(153));

        // Purchase all three types
        execute_command(
            &mut state,
            "SWE",
            &Command::PurchaseDevelopment {
                province: 1,
                dev_type: DevType::Tax,
            },
            None,
        )
        .unwrap();

        execute_command(
            &mut state,
            "SWE",
            &Command::PurchaseDevelopment {
                province: 1,
                dev_type: DevType::Production,
            },
            None,
        )
        .unwrap();

        execute_command(
            &mut state,
            "SWE",
            &Command::PurchaseDevelopment {
                province: 1,
                dev_type: DevType::Manpower,
            },
            None,
        )
        .unwrap();

        // Verify all mana types decreased
        let swe = state.countries.get("SWE").unwrap();
        assert_eq!(swe.adm_mana, Fixed::from_int(103)); // 153 - 50
        assert_eq!(swe.dip_mana, Fixed::from_int(103)); // 153 - 50
        assert_eq!(swe.mil_mana, Fixed::from_int(103)); // 153 - 50

        // Verify all dev types increased
        let prov = state.provinces.get(&1).unwrap();
        assert_eq!(prov.base_tax, Fixed::from_int(2)); // 1 + 1
        assert_eq!(prov.base_production, Fixed::from_int(6)); // 5 + 1
        assert_eq!(prov.base_manpower, Fixed::from_int(2)); // 1 + 1
    }

    #[test]
    fn test_dev_purchasing_not_owned() {
        let mut state = WorldStateBuilder::new()
            .with_country("SWE")
            .with_country("DEN")
            .with_province_full(1, Some("DEN"), None, Fixed::from_int(5))
            .build();

        // Give SWE mana
        state.countries.get_mut("SWE").unwrap().adm_mana = Fixed::from_int(100);

        // SWE tries to purchase dev in DEN's province
        let result = execute_command(
            &mut state,
            "SWE",
            &Command::PurchaseDevelopment {
                province: 1,
                dev_type: DevType::Tax,
            },
            None,
        );

        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), ActionError::NotOwned));
    }
}
