use crate::fixed::Fixed;
use crate::input::{Command, DevType, PlayerInputs};
use crate::metrics::SimMetrics;
use crate::state::{MovementState, PeaceTerms, PendingPeace, WorldState};
use std::time::Instant;
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
    #[error("War not found: {war_id}")]
    WarNotFound { war_id: u32 },
    #[error("Country {tag} is not a participant in war {war_id}")]
    NotWarParticipant { tag: String, war_id: u32 },
    #[error("Insufficient war score: required {required}, have {available}")]
    InsufficientWarScore { required: u8, available: u8 },
    #[error("No pending peace offer in war {war_id}")]
    NoPendingPeace { war_id: u32 },
    #[error("Cannot accept own peace offer")]
    CannotAcceptOwnOffer,
}

/// Advance the world by one tick.
pub fn step_world(
    state: &WorldState,
    inputs: &[PlayerInputs],
    adjacency: Option<&eu4data::adjacency::AdjacencyGraph>,
    config: &crate::config::SimConfig,
    mut metrics: Option<&mut SimMetrics>,
) -> WorldState {
    let tick_start = Instant::now();
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
    let move_start = Instant::now();
    crate::systems::run_movement_tick(&mut new_state, adjacency);
    if let Some(m) = metrics.as_mut() {
        m.movement_time += move_start.elapsed();
    }

    // Combat runs daily (whenever armies are engaged)
    let combat_start = Instant::now();
    crate::systems::run_combat_tick(&mut new_state);
    if let Some(m) = metrics.as_mut() {
        m.combat_time += combat_start.elapsed();
    }

    // Update occupation (armies in enemy territory take control)
    let occ_start = Instant::now();
    update_occupation(&mut new_state);
    if let Some(m) = metrics.as_mut() {
        m.occupation_time += occ_start.elapsed();
    }

    // Economic systems run monthly (on 1st of each month)
    if new_state.date.day == 1 {
        let econ_start = Instant::now();
        let economy_config = crate::systems::EconomyConfig::default();

        // Monthly tick ordering:
        // 1. Production → Updates province output values
        // 2. Taxation → Collects from updated production
        // 3. Manpower → Regenerates military capacity
        // 4. Expenses → Deducts costs (uses fresh manpower pool)
        // 5. Mana → Generates monarch points
        // 6. Colonization → Progresses active colonies
        // 7. War scores → Recalculates based on current occupation
        // 8. Auto-peace → Ends stalemate wars (10yr timeout)
        //
        // Order matters for production→taxation. Other systems are independent.
        crate::systems::run_production_tick(&mut new_state, &economy_config);
        crate::systems::run_taxation_tick(&mut new_state);
        crate::systems::run_manpower_tick(&mut new_state);
        crate::systems::run_expenses_tick(&mut new_state);
        crate::systems::run_mana_tick(&mut new_state);
        crate::systems::run_colonization_tick(&mut new_state);

        // Recalculate war scores monthly
        crate::systems::recalculate_war_scores(&mut new_state);

        // Auto-end wars after 10 years (stalemate prevention)
        auto_end_stale_wars(&mut new_state);

        if let Some(m) = metrics.as_mut() {
            m.economy_time += econ_start.elapsed();
        }
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

    if let Some(m) = metrics {
        m.total_ticks += 1;
        m.total_time += tick_start.elapsed();
    }

    new_state
}

/// Updates province controllers based on army presence.
/// If an army is in a province owned by an enemy (during war), the army's owner becomes controller.
fn update_occupation(state: &mut WorldState) {
    // Collect updates first to avoid borrow issues
    let mut updates: Vec<(u32, String)> = Vec::new();

    for army in state.armies.values() {
        let province_id = army.location;
        if let Some(province) = state.provinces.get(&province_id) {
            if let Some(owner) = &province.owner {
                // Check if army owner is at war with province owner
                if owner != &army.owner && state.diplomacy.are_at_war(&army.owner, owner) {
                    // Army is in enemy territory during war - occupy!
                    if province.controller.as_ref() != Some(&army.owner) {
                        updates.push((province_id, army.owner.clone()));
                    }
                }
            }
        }
    }

    // Apply updates
    for (province_id, new_controller) in updates {
        if let Some(province) = state.provinces.get_mut(&province_id) {
            log::info!(
                "Province {} now occupied by {}",
                province_id,
                new_controller
            );
            province.controller = Some(new_controller);
        }
    }
}

/// Auto-ends wars that have been ongoing for 10+ years with white peace.
fn auto_end_stale_wars(state: &mut WorldState) {
    const STALEMATE_YEARS: i32 = 10;

    // Collect wars to end (can't modify while iterating)
    let wars_to_end: Vec<u32> = state
        .diplomacy
        .wars
        .values()
        .filter(|war| {
            let years_at_war = state.date.year - war.start_date.year;
            years_at_war >= STALEMATE_YEARS
        })
        .map(|war| war.id)
        .collect();

    for war_id in wars_to_end {
        // Restore province controllers
        restore_province_controllers(state, war_id);

        // Remove war
        if let Some(war) = state.diplomacy.wars.remove(&war_id) {
            log::info!(
                "War '{}' auto-ended in white peace after {} years of stalemate",
                war.name,
                STALEMATE_YEARS
            );
        }
    }
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
                log::trace!(
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
                attacker_score: 0,
                attacker_battle_score: 0,
                defender_score: 0,
                defender_battle_score: 0,
                pending_peace: None,
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
        Command::OfferPeace { war_id, terms } => {
            // Validate war exists
            let war = state
                .diplomacy
                .wars
                .get(war_id)
                .ok_or(ActionError::WarNotFound { war_id: *war_id })?;

            // Validate country is participant
            let is_attacker = war.attackers.contains(&country_tag.to_string());
            let is_defender = war.defenders.contains(&country_tag.to_string());
            if !is_attacker && !is_defender {
                return Err(ActionError::NotWarParticipant {
                    tag: country_tag.to_string(),
                    war_id: *war_id,
                });
            }

            // Calculate war score cost for terms
            let war_score_cost = calculate_peace_terms_cost(state, terms, war, is_attacker);
            let available_score = if is_attacker {
                war.attacker_score
            } else {
                war.defender_score
            };

            if war_score_cost > available_score {
                return Err(ActionError::InsufficientWarScore {
                    required: war_score_cost,
                    available: available_score,
                });
            }

            // Store peace offer
            let pending = PendingPeace {
                from_attacker: is_attacker,
                terms: terms.clone(),
                offered_on: state.date,
            };

            if let Some(war) = state.diplomacy.wars.get_mut(war_id) {
                war.pending_peace = Some(pending);
            }

            log::info!(
                "{} offered peace in war {} with terms {:?}",
                country_tag,
                war_id,
                terms
            );
            Ok(())
        }
        Command::AcceptPeace { war_id } => {
            // Validate war and pending peace exist
            let war = state
                .diplomacy
                .wars
                .get(war_id)
                .ok_or(ActionError::WarNotFound { war_id: *war_id })?;

            let pending = war
                .pending_peace
                .clone()
                .ok_or(ActionError::NoPendingPeace { war_id: *war_id })?;

            // Validate caller is the recipient (not the offerer)
            let is_attacker = war.attackers.contains(&country_tag.to_string());
            if pending.from_attacker == is_attacker {
                return Err(ActionError::CannotAcceptOwnOffer);
            }

            // Execute peace terms
            execute_peace_terms(state, *war_id, &pending.terms)?;

            // Remove war
            state.diplomacy.wars.remove(war_id);

            log::info!("{} accepted peace in war {}", country_tag, war_id);
            Ok(())
        }
        Command::RejectPeace { war_id } => {
            // Clear pending peace offer
            if let Some(war) = state.diplomacy.wars.get_mut(war_id) {
                war.pending_peace = None;
                log::info!("{} rejected peace in war {}", country_tag, war_id);
            }
            Ok(())
        }

        // ===== STUB COMMANDS (Phase 2+) =====
        // These commands are defined but not yet implemented.
        // They log a warning and return Ok(()) to allow graceful degradation.
        Command::MergeArmies { .. } => {
            log::warn!("MergeArmies not implemented yet");
            Ok(())
        }
        Command::SplitArmy { .. } => {
            log::warn!("SplitArmy not implemented yet");
            Ok(())
        }
        Command::StartColony { province } => {
            let province = *province;
            // Minimal: Validate unowned province, not already a colony, not a sea province.
            if state
                .provinces
                .get(&province)
                .is_none_or(|p| p.owner.is_none())
                && !state.colonies.contains_key(&province)
            {
                if let Some(p) = state.provinces.get(&province) {
                    if !p.is_sea {
                        state.colonies.insert(
                            province,
                            crate::state::Colony {
                                province,
                                owner: country_tag.to_string(),
                                settlers: 0,
                            },
                        );
                        log::info!("{} started a colony in province {}", country_tag, province);
                    }
                }
            }
            Ok(())
        }
        Command::AbandonColony { province } => {
            let province = *province;
            if let Some(colony) = state.colonies.get(&province) {
                if colony.owner == country_tag {
                    state.colonies.remove(&province);
                    log::info!("{} abandoned colony in province {}", country_tag, province);
                }
            }
            Ok(())
        }
        Command::OfferAlliance { .. } => {
            log::warn!("OfferAlliance not implemented yet");
            Ok(())
        }
        Command::BreakAlliance { .. } => {
            log::warn!("BreakAlliance not implemented yet");
            Ok(())
        }
        Command::OfferRoyalMarriage { .. } => {
            log::warn!("OfferRoyalMarriage not implemented yet");
            Ok(())
        }
        Command::BreakRoyalMarriage { .. } => {
            log::warn!("BreakRoyalMarriage not implemented yet");
            Ok(())
        }
        Command::RequestMilitaryAccess { .. } => {
            log::warn!("RequestMilitaryAccess not implemented yet");
            Ok(())
        }
        Command::CancelMilitaryAccess { .. } => {
            log::warn!("CancelMilitaryAccess not implemented yet");
            Ok(())
        }
        Command::SetRival { .. } => {
            log::warn!("SetRival not implemented yet");
            Ok(())
        }
        Command::RemoveRival { .. } => {
            log::warn!("RemoveRival not implemented yet");
            Ok(())
        }
        Command::AcceptAlliance { .. } => {
            log::warn!("AcceptAlliance not implemented yet");
            Ok(())
        }
        Command::RejectAlliance { .. } => {
            log::warn!("RejectAlliance not implemented yet");
            Ok(())
        }
        Command::AcceptRoyalMarriage { .. } => {
            log::warn!("AcceptRoyalMarriage not implemented yet");
            Ok(())
        }
        Command::RejectRoyalMarriage { .. } => {
            log::warn!("RejectRoyalMarriage not implemented yet");
            Ok(())
        }
        Command::GrantMilitaryAccess { .. } => {
            log::warn!("GrantMilitaryAccess not implemented yet");
            Ok(())
        }
        Command::DenyMilitaryAccess { .. } => {
            log::warn!("DenyMilitaryAccess not implemented yet");
            Ok(())
        }
        Command::BuyTech { .. } => {
            log::warn!("BuyTech not implemented yet");
            Ok(())
        }
        Command::EmbraceInstitution { .. } => {
            log::warn!("EmbraceInstitution not implemented yet");
            Ok(())
        }
        Command::AssignMissionary { .. } => {
            log::warn!("AssignMissionary not implemented yet");
            Ok(())
        }
        Command::RecallMissionary { .. } => {
            log::warn!("RecallMissionary not implemented yet");
            Ok(())
        }
        Command::ConvertCountryReligion { .. } => {
            log::warn!("ConvertCountryReligion not implemented yet");
            Ok(())
        }
        Command::DevelopProvince { .. } => {
            log::warn!("DevelopProvince not implemented yet (use PurchaseDevelopment)");
            Ok(())
        }
        Command::MoveCapital { .. } => {
            log::warn!("MoveCapital not implemented yet");
            Ok(())
        }
        Command::Pass => Ok(()), // Explicit no-op

        Command::Quit => Ok(()), // Handled by outer loop usually, but harmless here
    }
}

/// Calculates the war score cost for peace terms.
fn calculate_peace_terms_cost(
    state: &WorldState,
    terms: &PeaceTerms,
    war: &crate::state::War,
    is_attacker: bool,
) -> u8 {
    match terms {
        PeaceTerms::WhitePeace => 0, // Free with 50% war score (AI acceptance logic)
        PeaceTerms::TakeProvinces { provinces } => {
            // Cost = sum of province dev / 2 (simplified)
            let enemy_tags: &[String] = if is_attacker {
                &war.defenders
            } else {
                &war.attackers
            };

            let mut cost = 0u32;
            for &prov_id in provinces {
                if let Some(prov) = state.provinces.get(&prov_id) {
                    // Only count provinces owned by enemy
                    if prov.owner.as_ref().is_some_and(|o| enemy_tags.contains(o)) {
                        let dev = prov.base_tax + prov.base_production + prov.base_manpower;
                        cost += (dev.to_f32() / 2.0).ceil() as u32;
                    }
                }
            }
            cost.min(100) as u8
        }
        PeaceTerms::FullAnnexation => 100, // Requires 100% war score
    }
}

/// Executes peace terms (province transfers, country elimination).
fn execute_peace_terms(
    state: &mut WorldState,
    war_id: u32,
    terms: &PeaceTerms,
) -> Result<(), ActionError> {
    // Get war info before modifying state
    let war = state
        .diplomacy
        .wars
        .get(&war_id)
        .ok_or(ActionError::WarNotFound { war_id })?;

    // Determine winner based on war score
    let attacker_winning = war.attacker_score > war.defender_score;
    let winner_tags: Vec<String> = if attacker_winning {
        war.attackers.clone()
    } else {
        war.defenders.clone()
    };

    match terms {
        PeaceTerms::WhitePeace => {
            // Restore all provinces to original owners
            restore_province_controllers(state, war_id);
        }
        PeaceTerms::TakeProvinces { provinces } => {
            // Transfer provinces to winner (first attacker/defender)
            let new_owner = winner_tags.first().cloned().unwrap_or_default();
            for &prov_id in provinces {
                if let Some(prov) = state.provinces.get_mut(&prov_id) {
                    prov.owner = Some(new_owner.clone());
                    prov.controller = Some(new_owner.clone());
                    log::info!("Province {} transferred to {}", prov_id, new_owner);
                }
            }
        }
        PeaceTerms::FullAnnexation => {
            // Transfer ALL enemy provinces to winner
            let loser_tags: Vec<String> = if attacker_winning {
                war.defenders.clone()
            } else {
                war.attackers.clone()
            };
            let new_owner = winner_tags.first().cloned().unwrap_or_default();

            let province_ids: Vec<u32> = state.provinces.keys().copied().collect();
            for prov_id in province_ids {
                if let Some(prov) = state.provinces.get_mut(&prov_id) {
                    if prov.owner.as_ref().is_some_and(|o| loser_tags.contains(o)) {
                        prov.owner = Some(new_owner.clone());
                        prov.controller = Some(new_owner.clone());
                    }
                }
            }

            // Remove annexed countries
            for tag in &loser_tags {
                state.countries.remove(tag);
                log::info!("Country {} eliminated through full annexation", tag);
            }
        }
    }

    Ok(())
}

/// Restores province controllers to their owners after white peace.
fn restore_province_controllers(state: &mut WorldState, war_id: u32) {
    if let Some(war) = state.diplomacy.wars.get(&war_id) {
        let all_participants: Vec<String> = war
            .attackers
            .iter()
            .chain(war.defenders.iter())
            .cloned()
            .collect();

        let prov_ids: Vec<_> = state.provinces.keys().cloned().collect();
        for prov_id in prov_ids {
            if let Some(prov) = state.provinces.get_mut(&prov_id) {
                if let Some(owner) = &prov.owner {
                    // If controller was a war participant, restore to owner
                    if prov
                        .controller
                        .as_ref()
                        .is_some_and(|c| all_participants.contains(c) && c != owner)
                    {
                        prov.controller = Some(owner.clone());
                    }
                }
            }
        }
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
        let new_state = step_world(
            &state,
            &inputs,
            None,
            &crate::config::SimConfig::default(),
            None,
        );

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
        let _new_state = step_world(
            &state,
            &inputs,
            None,
            &crate::config::SimConfig::default(),
            None,
        );

        // Assert no crash and logic ran
    }

    #[test]
    fn test_determinism() {
        let state = WorldStateBuilder::new()
            .date(1444, 1, 1)
            .with_country("SWE")
            .build();

        let inputs = vec![];

        let state_a = step_world(
            &state,
            &inputs,
            None,
            &crate::config::SimConfig::default(),
            None,
        );
        let state_b = step_world(
            &state,
            &inputs,
            None,
            &crate::config::SimConfig::default(),
            None,
        );

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

        let new_state = step_world(
            &state,
            &inputs,
            None,
            &crate::config::SimConfig::default(),
            None,
        );

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

        let new_state = step_world(
            &state,
            &inputs,
            None,
            &crate::config::SimConfig::default(),
            None,
        );

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

        state = step_world(
            &state,
            &inputs1,
            None,
            &crate::config::SimConfig::default(),
            None,
        );
        assert_eq!(state.diplomacy.wars.len(), 1);

        // Second war declaration (should fail)
        let inputs2 = vec![PlayerInputs {
            country: "SWE".to_string(),
            commands: vec![Command::DeclareWar {
                target: "DEN".to_string(),
            }],
        }];

        let new_state = step_world(
            &state,
            &inputs2,
            None,
            &crate::config::SimConfig::default(),
            None,
        );

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

        let new_state = step_world(
            &state,
            &inputs,
            None,
            &crate::config::SimConfig::default(),
            None,
        );

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

    #[test]
    fn test_colonization_cycle() {
        use crate::testing::WorldStateBuilder;

        let mut state = WorldStateBuilder::new()
            .with_country("SWE")
            .with_province(1, None) // Unowned
            .build();

        // Start colony
        let cmd = Command::StartColony { province: 1 };
        execute_command(&mut state, "SWE", &cmd, None).unwrap();

        assert!(state.colonies.contains_key(&1));
        let colony = state.colonies.get(&1).unwrap();
        assert_eq!(colony.owner, "SWE");
        assert_eq!(colony.settlers, 0);

        // Progress 12 months (1 year)
        for _ in 0..12 {
            state.date = state.date.add_days(30);
            crate::systems::run_colonization_tick(&mut state);
        }

        // 83 * 12 = 996 settlers. Not finished yet.
        assert!(state.colonies.contains_key(&1));
        assert_eq!(state.colonies.get(&1).unwrap().settlers, 996);

        // One more month
        state.date = state.date.add_days(30);
        crate::systems::run_colonization_tick(&mut state);

        // 996 + 83 = 1079 >= 1000. Finished!
        assert!(!state.colonies.contains_key(&1));
        let prov = state.provinces.get(&1).unwrap();
        assert_eq!(prov.owner.as_ref().unwrap(), "SWE");
    }
}
