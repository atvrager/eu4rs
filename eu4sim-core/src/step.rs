use crate::fixed::Fixed;
use crate::input::{Command, DevType, PlayerInputs};
use crate::metrics::SimMetrics;
use crate::state::{MovementState, PeaceTerms, PendingPeace, ProvinceId, TechType, WorldState};
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
    #[error("Active truce with {target} (expires {expires})")]
    TruceActive {
        target: String,
        expires: crate::state::Date,
    },
    #[error("Institution {institution} already embraced")]
    AlreadyEmbraced { institution: String },
    #[error("Institution {institution} not present in enough development (need 10%)")]
    InstitutionNotPresent { institution: String },
    #[error("Already at max tech level")]
    MaxTechReached,
    #[error("War declarations blocked during first month (tick {tick} < 30)")]
    FirstMonthImmunity { tick: u64 },
    #[error("Already performed a diplomatic action today")]
    DiplomaticActionCooldown,
    #[error("Peace offer on cooldown for war {war_id} until {until}")]
    PeaceOfferOnCooldown {
        war_id: u32,
        until: crate::state::Date,
    },
    #[error("No trade route exists from home node to destination")]
    NoTradeRoute,
    #[error("Country has no home trade node configured")]
    NoHomeNode,
    #[error("Coring failed: {message}")]
    CoringFailed { message: String },
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
                // Downgrade to debug - these are often valid simultaneous move conflicts (e.g. race to war)
                log::debug!(
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

        // Reset income tracking for this month
        let country_tags: Vec<String> = new_state.countries.keys().cloned().collect();
        for tag in country_tags {
            if let Some(country) = new_state.countries.get_mut(&tag) {
                country.income = crate::state::IncomeBreakdown::default();
            }
        }

        // Monthly tick ordering:
        // 1. Production → Updates province output values
        // 2. Trade value → Calculates value in each trade node from production
        // 3. Trade power → Calculates power shares per country
        // 4. Trade income → Countries collect based on power shares
        // 5. Taxation → Collects from updated production
        // 6. Manpower → Regenerates military capacity
        // 7. Expenses → Deducts costs (uses fresh manpower pool)
        // 8. Mana → Generates monarch points
        // 9. Colonization → Progresses active colonies
        // 10. Reformation → Spreads Protestant/Reformed religions
        // 11. War scores → Recalculates based on current occupation
        // 12. Auto-peace → Ends stalemate wars (10yr timeout)
        //
        // Order matters: merchant arrivals → trade power → production → trade value → trade income.
        // Merchants must arrive first so they participate in power calculation.
        // Power must be calculated first so value propagation knows retention.
        // Trade income must come before taxation as both contribute to treasury.
        let trade_start = Instant::now();
        crate::systems::run_merchant_arrivals(&mut new_state);
        crate::systems::run_trade_power_tick(&mut new_state);
        crate::systems::run_production_tick(&mut new_state, &economy_config);
        crate::systems::run_trade_value_tick(&mut new_state);
        crate::systems::run_trade_income_tick(&mut new_state);
        if let Some(m) = metrics.as_mut() {
            m.trade_time += trade_start.elapsed();
        }

        crate::systems::run_taxation_tick(&mut new_state);
        crate::systems::run_manpower_tick(&mut new_state);
        crate::systems::run_expenses_tick(&mut new_state);
        crate::systems::run_mana_tick(&mut new_state);
        crate::systems::run_stats_tick(&mut new_state);
        crate::systems::run_colonization_tick(&mut new_state);
        crate::systems::tick_institution_spread(&mut new_state);
        crate::systems::run_reformation_tick(&mut new_state, adjacency);

        // Coring - Progress active coring and complete after 36 months. 🛡️
        crate::systems::tick_coring(&mut new_state);

        // Recalculate overextension (uncored dev causes OE penalties)
        crate::systems::recalculate_overextension(&mut new_state);

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
        // Create truces before removing war
        if let Some(war) = state.diplomacy.wars.get(&war_id).cloned() {
            create_war_truces(state, &war, state.date);

            // Clear peace offer cooldowns for all participants
            for tag in war.attackers.iter().chain(war.defenders.iter()) {
                if let Some(country) = state.countries.get_mut(tag) {
                    country.peace_offer_cooldowns.remove(&war_id);
                }
            }
        }

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

/// Determines if an army can enter a province. 🛡️
/// The sword may only pass where diplomacy permits or war demands.
fn can_army_enter(state: &WorldState, country_tag: &str, province_id: ProvinceId) -> bool {
    let Some(province) = state.provinces.get(&province_id) else {
        return false; // Province doesn't exist - deny entry
    };

    // Armies cannot walk on water
    if province.is_sea {
        return false;
    }

    match &province.owner {
        None => true,                                // Uncolonized wilderness - anyone may pass
        Some(owner) if owner == country_tag => true, // Home territory
        Some(owner) => {
            // Foreign soil - need diplomatic passage or be at war
            state.diplomacy.has_military_access(country_tag, owner)
                || state.diplomacy.are_at_war(country_tag, owner)
        }
    }
}

/// Returns all valid commands for a country at the current state.
/// This is the wellspring of action, where possibility becomes choice. ✧
pub fn available_commands(
    state: &WorldState,
    country_tag: &str,
    adjacency: Option<&eu4data::adjacency::AdjacencyGraph>,
) -> Vec<Command> {
    let mut available = Vec::new();

    // 1. Basic Validation - One must exist before one can act. 🛡️
    let Some(country) = state.countries.get(country_tag) else {
        return available;
    };

    // 2. Economic Actions - Wealth is the foundation of every empire's fate. ✧
    const DEV_COST: Fixed = Fixed::from_int(50);
    for (prov_id, prov) in &state.provinces {
        if prov.owner.as_deref() == Some(country_tag) {
            // DevelopProvince - Building for a future that will outlast us all.
            if country.adm_mana >= DEV_COST {
                available.push(Command::DevelopProvince {
                    province: *prov_id,
                    dev_type: DevType::Tax,
                });
            }
            if country.dip_mana >= DEV_COST {
                available.push(Command::DevelopProvince {
                    province: *prov_id,
                    dev_type: DevType::Production,
                });
            }
            if country.mil_mana >= DEV_COST {
                available.push(Command::DevelopProvince {
                    province: *prov_id,
                    dev_type: DevType::Manpower,
                });
            }
        }
    }

    // Technology - Knowledge is the key that unlocks the gates of power. ✧
    // Simplified cost calculation for available commands
    let tech_cost = |level: u8| Fixed::from_int(600 + (level as i64 * 60));

    if country.adm_tech < 32 && country.adm_mana >= tech_cost(country.adm_tech) {
        available.push(Command::BuyTech {
            tech_type: TechType::Adm,
        });
    }
    if country.dip_tech < 32 && country.dip_mana >= tech_cost(country.dip_tech) {
        available.push(Command::BuyTech {
            tech_type: TechType::Dip,
        });
    }
    if country.mil_tech < 32 && country.mil_mana >= tech_cost(country.mil_tech) {
        available.push(Command::BuyTech {
            tech_type: TechType::Mil,
        });
    }

    // Institutions - The spirit of innovation spreads across the lands. ✧
    // For mid-term, we only check for "renaissance" if valid manually.
    let institutions = vec!["renaissance".to_string()];
    for inst in institutions {
        if !country.embraced_institutions.contains(&inst) {
            // Check 10% presence and gold (simplified check for available_commands)
            let mut total_dev = Fixed::ZERO;
            let mut present_dev = Fixed::ZERO;
            for prov in state.provinces.values() {
                if prov.owner.as_deref() == Some(country_tag) {
                    let dev = prov.base_tax + prov.base_production + prov.base_manpower;
                    total_dev += dev;
                    if prov.institution_presence.get(&inst).copied().unwrap_or(0.0) >= 100.0 {
                        present_dev += dev;
                    }
                }
            }

            if total_dev > Fixed::ZERO && (present_dev / total_dev) >= Fixed::from_raw(1000) {
                let cost = (total_dev - present_dev) * Fixed::from_int(2);
                if country.treasury >= cost {
                    available.push(Command::EmbraceInstitution { institution: inst });
                }
            }
        }
    }

    // StartColony - Reaching into the unknown, but only where our borders touch the void. ✧
    // Must be adjacent to an owned province to colonize.
    if let Some(graph) = adjacency {
        let mut colonizable: std::collections::HashSet<ProvinceId> =
            std::collections::HashSet::new();
        for (prov_id, prov) in &state.provinces {
            if prov.owner.as_deref() == Some(country_tag) {
                for neighbor_id in graph.neighbors(*prov_id) {
                    if let Some(neighbor) = state.provinces.get(&neighbor_id) {
                        if neighbor.owner.is_none()
                            && !neighbor.is_sea
                            && !state.colonies.contains_key(&neighbor_id)
                        {
                            colonizable.insert(neighbor_id);
                        }
                    }
                }
            }
        }
        for prov_id in colonizable {
            available.push(Command::StartColony { province: prov_id });
        }
    }

    // Core - Establish permanent claim on owned provinces. Unlimited per turn.
    // Available for any owned province without a core, if ADM is sufficient.
    for (&prov_id, prov) in &state.provinces {
        if prov.owner.as_deref() == Some(country_tag)
            && !prov.cores.contains(country_tag)
            && prov.coring_progress.is_none()
        {
            let cost = crate::systems::coring::calculate_coring_cost(prov);
            if country.adm_mana >= cost {
                available.push(Command::Core { province: prov_id });
            }
        }
    }

    // 3. Military Actions - Armies are the shields that guard our truth. 🛡️
    if let Some(graph) = adjacency {
        // Move: For each army, check adjacent provinces
        for (army_id, army) in &state.armies {
            if army.owner == country_tag && army.movement.is_none() && army.embarked_on.is_none() {
                for neighbor in graph.neighbors(army.location) {
                    if can_army_enter(state, country_tag, neighbor) {
                        available.push(Command::Move {
                            army_id: *army_id,
                            destination: neighbor,
                        });
                    }
                }
            }
        }

        // MoveFleet: For each fleet, check adjacent sea provinces
        for (fleet_id, fleet) in &state.fleets {
            if fleet.owner == country_tag && fleet.movement.is_none() {
                for neighbor in graph.neighbors(fleet.location) {
                    if let Some(p) = state.provinces.get(&neighbor) {
                        if p.is_sea {
                            available.push(Command::MoveFleet {
                                fleet_id: *fleet_id,
                                destination: neighbor,
                            });
                        }
                    }
                }
            }
        }
    }

    // 4. Diplomatic Actions - Words can be as sharp as any blade. ✧
    // First month immunity - don't offer war declarations in first 30 days
    const START_DATE_EPOCH: i64 = 310; // 1444.11.11 in days from 1444.01.01
    let tick = (state.date.days_from_epoch() - START_DATE_EPOCH) as u64;
    let can_declare_war = tick >= 30;

    // Optimization: Only consider neighbors for war declaration to avoid O(N^2) scaling
    if can_declare_war {
        if let Some(graph) = adjacency {
            let mut potential_targets = std::collections::HashSet::new();

            // Find neighbors of all owned provinces
            for (prov_id, prov) in &state.provinces {
                if prov.owner.as_deref() == Some(country_tag) {
                    for neighbor_id in graph.neighbors(*prov_id) {
                        if let Some(neighbor) = state.provinces.get(&neighbor_id) {
                            if let Some(owner) = &neighbor.owner {
                                if owner != country_tag {
                                    potential_targets.insert(owner.clone());
                                }
                            }
                        }
                    }
                }
            }

            for target_tag in potential_targets {
                if !state.diplomacy.are_at_war(country_tag, &target_tag)
                    && !state
                        .diplomacy
                        .has_active_truce(country_tag, &target_tag, state.date)
                {
                    // DeclareWar - The ultimate test of an empire's foundation. 🛡️
                    available.push(Command::DeclareWar {
                        target: target_tag,
                        cb: None,
                    });
                }
            }
        }
    }

    // 5. War Resolution - Every conflict must eventually find its truth. ✧
    for war in state.diplomacy.get_wars_for_country(country_tag) {
        // OfferPeace - An olive branch extended through the smoke of battle.
        available.push(Command::OfferPeace {
            war_id: war.id,
            terms: PeaceTerms::default(),
        });

        // AcceptPeace - Accepting the fate that the stars have written. 🛡️
        if let Some(pending) = &war.pending_peace {
            let caller_is_attacker = war.attackers.contains(&country_tag.to_string());
            if pending.from_attacker != caller_is_attacker {
                available.push(Command::AcceptPeace { war_id: war.id });
                available.push(Command::RejectPeace { war_id: war.id });
            }
        }
    }

    // 6. Trade Actions - Merchants chart the course of empire's prosperity. ✧
    if let Some(trade_state) = state.countries.get(country_tag).map(|c| &c.trade) {
        // Only offer trade commands if merchants are available
        if trade_state.merchants_available > 0 {
            // Find nodes where this country has power (potential send targets)
            for (&node_id, node) in &state.trade_nodes {
                if node
                    .country_power
                    .get(country_tag)
                    .copied()
                    .unwrap_or(Fixed::ZERO)
                    > Fixed::ZERO
                {
                    // Check if country already has a merchant here
                    let has_merchant = node.merchants.iter().any(|m| m.owner == country_tag);

                    if !has_merchant {
                        // Offer Collect action
                        available.push(Command::SendMerchant {
                            node: node_id,
                            action: crate::trade::MerchantAction::Collect,
                        });

                        // Offer Steer actions for each downstream node
                        if let Some(downstream) = state.trade_topology.edges.get(&node_id) {
                            for &target in downstream {
                                available.push(Command::SendMerchant {
                                    node: node_id,
                                    action: crate::trade::MerchantAction::Steer { target },
                                });
                            }
                        }
                    }
                }
            }
        }

        // Recall merchant commands for nodes where country has a merchant
        for (&node_id, node) in &state.trade_nodes {
            if node.merchants.iter().any(|m| m.owner == country_tag) {
                available.push(Command::RecallMerchant { node: node_id });
            }
        }
    }

    available
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
            log::info!("Player {} building something (stub)", country_tag);
            Ok(())
        }
        Command::RecruitRegiment {
            province,
            unit_type,
        } => {
            let country =
                state
                    .countries
                    .get_mut(country_tag)
                    .ok_or(ActionError::CountryNotFound {
                        tag: country_tag.to_string(),
                    })?;

            // 1. Costs (Approximate: 10g + 1000 manpower)
            // TODO: Use correct constants from defines/units
            let gold_cost = match unit_type {
                crate::state::RegimentType::Infantry => Fixed::from_int(10),
                crate::state::RegimentType::Cavalry => Fixed::from_int(25),
                crate::state::RegimentType::Artillery => Fixed::from_int(30),
            };
            let manpower_cost = Fixed::from_int(1000);

            if country.treasury < gold_cost {
                return Err(ActionError::InsufficientFunds {
                    required: gold_cost.to_f32(),
                    available: country.treasury.to_f32(),
                });
            }
            // Manpower check omitted for now (allow deficit spending/debt or just negative manpower)
            // if country.manpower < manpower_cost { ... }

            // 2. Tech Check for Artillery
            if *unit_type == crate::state::RegimentType::Artillery {
                let required_tech = eu4data::defines::combat::ARTILLERY_TECH_REQUIRED;
                if country.mil_tech < required_tech {
                    // Fail silently or error? For now, simplistic error
                    log::warn!(
                        "{} tried to recruit artillery without tech {}",
                        country_tag,
                        required_tech
                    );
                    return Ok(()); // Invalid action but don't crash simulation
                }
            }

            // 3. Deduct resources
            country.treasury -= gold_cost;
            country.manpower -= manpower_cost;

            // 4. Create regiment/army
            // Check if there's already an army in this province owned by this country
            let existing_army_id = state.armies.iter().find_map(|(id, army)| {
                if army.owner == country_tag
                    && army.location == *province
                    && army.in_battle.is_none()
                {
                    Some(*id)
                } else {
                    None
                }
            });

            if let Some(army_id) = existing_army_id {
                if let Some(army) = state.armies.get_mut(&army_id) {
                    army.regiments.push(crate::state::Regiment {
                        type_: *unit_type,
                        strength: Fixed::from_int(1000),
                        morale: Fixed::from_f32(eu4data::defines::combat::BASE_MORALE),
                    });
                }
            } else {
                // Create new army
                let army_id = state.next_army_id;
                state.next_army_id += 1;
                state.armies.insert(
                    army_id,
                    crate::state::Army {
                        id: army_id,
                        name: format!("{} Army {}", country_tag, army_id),
                        owner: country_tag.to_string(),
                        location: *province,
                        regiments: vec![crate::state::Regiment {
                            type_: *unit_type,
                            strength: Fixed::from_int(1000),
                            morale: Fixed::from_f32(eu4data::defines::combat::BASE_MORALE),
                        }],
                        movement: None,
                        embarked_on: None,
                        general: None,
                        in_battle: None,
                    },
                );
            }

            Ok(())
        }
        Command::RecruitGeneral => {
            let country =
                state
                    .countries
                    .get_mut(country_tag)
                    .ok_or(ActionError::CountryNotFound {
                        tag: country_tag.to_string(),
                    })?;

            let cost = Fixed::from_int(50);
            if country.mil_mana < cost {
                return Err(ActionError::InsufficientMana);
            }

            country.mil_mana -= cost;

            // Generate General (Simple 1-6 random for now)
            let general_id = state.next_general_id;
            state.next_general_id += 1;

            let general = crate::state::General {
                id: general_id,
                name: format!("General {}", general_id),
                owner: country_tag.to_string(),
                fire: 2, // TODO: Use RNG
                shock: 2,
                maneuver: 2,
                siege: 0,
            };

            state.generals.insert(general_id, general);
            // In a real game, this general would go into a "recruited pool" or be assigned immediately.
            // For now, it just exists. The next command assigns it.
            log::info!("{} recruited General {}", country_tag, general_id);

            Ok(())
        }
        Command::AssignGeneral { general, army } => {
            let _country =
                state
                    .countries
                    .get(country_tag)
                    .ok_or(ActionError::CountryNotFound {
                        tag: country_tag.to_string(),
                    })?;

            // Validate ownership
            let army_entry = state
                .armies
                .get_mut(army)
                .ok_or(ActionError::ArmyNotFound { army_id: *army })?;

            if army_entry.owner != country_tag {
                return Err(ActionError::ArmyNotOwned {
                    army_id: *army,
                    tag: country_tag.to_string(),
                });
            }

            if !state.generals.contains_key(general) {
                // Error: General not found
                return Ok(());
            }

            army_entry.general = Some(*general);
            Ok(())
        }
        Command::UnassignGeneral { army } => {
            let army_entry = state
                .armies
                .get_mut(army)
                .ok_or(ActionError::ArmyNotFound { army_id: *army })?;

            if army_entry.owner != country_tag {
                return Err(ActionError::ArmyNotOwned {
                    army_id: *army,
                    tag: country_tag.to_string(),
                });
            }

            army_entry.general = None;
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
        Command::DeclareWar { target, cb } => {
            // First month immunity - no war declarations in first 30 days
            // (EU4 starts paused on 1444.11.11, giving players time to set up before conflict)
            const START_DATE_EPOCH: i64 = 310; // 1444.11.11 in days from 1444.01.01
            let tick = (state.date.days_from_epoch() - START_DATE_EPOCH) as u64;
            if tick < 30 {
                return Err(ActionError::FirstMonthImmunity { tick });
            }

            // One diplomatic action per day - check if already acted today
            if let Some(country) = state.countries.get(country_tag) {
                if country.last_diplomatic_action == Some(state.date) {
                    return Err(ActionError::DiplomaticActionCooldown);
                }
            }

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

            // Check for active truce
            if state
                .diplomacy
                .has_active_truce(country_tag, target, state.date)
            {
                let key = crate::state::DiplomacyState::sorted_pair(country_tag, target);
                let expiry = state.diplomacy.truces.get(&key).unwrap();
                return Err(ActionError::TruceActive {
                    target: target.clone(),
                    expires: *expiry,
                });
            }

            // Apply No-CB stability penalty
            if cb.is_none() {
                if let Some(country) = state.countries.get_mut(country_tag) {
                    country.stability.add(-2);
                    log::info!(
                        "{} declares no-CB war on {}: -2 stability",
                        country_tag,
                        target
                    );
                }
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

            // Mark diplomatic action cooldown
            if let Some(country) = state.countries.get_mut(country_tag) {
                country.last_diplomatic_action = Some(state.date);
            }

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
        Command::DevelopProvince { province, dev_type } => {
            crate::systems::develop_province(state, country_tag.to_string(), *province, *dev_type)
                .map_err(|e: anyhow::Error| {
                    let msg = e.to_string();
                    if msg.contains("Not enough") {
                        ActionError::InsufficientMana
                    } else if msg.contains("not found") {
                        ActionError::InvalidProvinceId
                    } else if msg.contains("not own") {
                        ActionError::NotOwned
                    } else {
                        // Default to something safe if we can't map precisely
                        ActionError::InsufficientMana
                    }
                })?;

            log::trace!(
                "{} developed province {} ({:?})",
                country_tag,
                province,
                dev_type
            );

            Ok(())
        }
        Command::BuyTech { tech_type } => {
            crate::systems::buy_tech(state, country_tag.to_string(), *tech_type).map_err(
                |e: anyhow::Error| {
                    let msg = e.to_string();
                    if msg.contains("Not enough") {
                        ActionError::InsufficientMana
                    } else if msg.contains("maximum") {
                        ActionError::MaxTechReached
                    } else {
                        ActionError::InsufficientMana
                    }
                },
            )?;

            log::info!("{} bought {:?} tech", country_tag, tech_type);

            Ok(())
        }
        Command::EmbraceInstitution { institution } => {
            crate::systems::embrace_institution(
                state,
                country_tag.to_string(),
                institution.clone(),
            )
            .map_err(|e: anyhow::Error| {
                let msg = e.to_string();
                if msg.contains("already embraced") {
                    ActionError::AlreadyEmbraced {
                        institution: institution.clone(),
                    }
                } else if msg.contains("Not enough gold") {
                    // We don't have a generic InsufficientFunds without specific numbers here easily
                    ActionError::InsufficientFunds {
                        required: 0.0,
                        available: 0.0,
                    }
                } else if msg.contains("Less than 10%") {
                    ActionError::InstitutionNotPresent {
                        institution: institution.clone(),
                    }
                } else {
                    ActionError::InsufficientFunds {
                        required: 0.0,
                        available: 0.0,
                    }
                }
            })?;

            log::info!("{} embraced institution {}", country_tag, institution);

            Ok(())
        }
        Command::OfferPeace { war_id, terms } => {
            // One diplomatic action per day - check if already acted today
            if let Some(country) = state.countries.get(country_tag) {
                if country.last_diplomatic_action == Some(state.date) {
                    return Err(ActionError::DiplomaticActionCooldown);
                }
            }

            // Check peace offer cooldown (30 days after rejection)
            if let Some(country) = state.countries.get(country_tag) {
                if let Some(&cooldown_until) = country.peace_offer_cooldowns.get(war_id) {
                    if cooldown_until > state.date {
                        return Err(ActionError::PeaceOfferOnCooldown {
                            war_id: *war_id,
                            until: cooldown_until,
                        });
                    }
                }
            }

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

            // Mark diplomatic action cooldown
            if let Some(country) = state.countries.get_mut(country_tag) {
                country.last_diplomatic_action = Some(state.date);
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
                .cloned()
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

            // Create truces before removing war
            create_war_truces(state, &war, state.date);

            // Clear peace offer cooldowns for all participants
            for tag in war.attackers.iter().chain(war.defenders.iter()) {
                if let Some(country) = state.countries.get_mut(tag) {
                    country.peace_offer_cooldowns.remove(war_id);
                }
            }

            // Remove war
            state.diplomacy.wars.remove(war_id);

            log::info!("{} accepted peace in war {}", country_tag, war_id);
            Ok(())
        }
        Command::RejectPeace { war_id } => {
            // Get the offerer before clearing the pending peace
            if let Some(war) = state.diplomacy.wars.get(war_id).cloned() {
                if let Some(pending) = &war.pending_peace {
                    // Find the offerer's tag
                    let offerer_tag = if pending.from_attacker {
                        war.attackers.first().cloned()
                    } else {
                        war.defenders.first().cloned()
                    };

                    // Set 30-day cooldown on the offerer
                    if let Some(tag) = offerer_tag {
                        let cooldown_until = state.date.add_days(30);
                        if let Some(country) = state.countries.get_mut(&tag) {
                            country
                                .peace_offer_cooldowns
                                .insert(*war_id, cooldown_until);
                        }
                    }
                }
            }

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
        Command::Core { province } => {
            crate::systems::coring::start_coring(
                state,
                country_tag.to_string(),
                *province,
                state.date,
            )
            .map_err(|e| ActionError::CoringFailed { message: e })?;
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
        Command::AssignMissionary { .. } => {
            log::warn!("AssignMissionary not implemented yet");
            Ok(())
        }
        Command::RecallMissionary { .. } => {
            log::warn!("RecallMissionary not implemented yet");
            Ok(())
        }
        Command::ConvertCountryReligion { religion } => {
            let country =
                state
                    .countries
                    .get_mut(country_tag)
                    .ok_or(ActionError::CountryNotFound {
                        tag: country_tag.to_string(),
                    })?;

            // Stability hit for changing religion (controversial decision)
            country.stability.add(-2);
            country.religion = Some(religion.clone());

            log::info!("{} has converted to {}", country_tag, religion);
            Ok(())
        }
        Command::MoveCapital { .. } => {
            log::warn!("MoveCapital not implemented yet");
            Ok(())
        }

        // Trade commands
        Command::SendMerchant { node, action } => {
            use crate::trade::{MerchantTravel, TradeNodeGraph};
            use game_pathfinding::AStar;

            // Validate country exists and get home node
            let home_node = state
                .countries
                .get(country_tag)
                .ok_or(ActionError::CountryNotFound {
                    tag: country_tag.to_string(),
                })?
                .trade
                .home_node
                .ok_or(ActionError::NoHomeNode)?;

            // Check if merchant is available
            if state.countries[country_tag].trade.merchants_available == 0 {
                return Err(ActionError::InsufficientMana); // Reusing error for now
            }

            // Check if node exists
            if !state.trade_nodes.contains_key(node) {
                return Err(ActionError::InvalidProvinceId); // Reusing error
            }

            // Check if already have a merchant there or en route
            if state.trade_nodes[node]
                .merchants
                .iter()
                .any(|m| m.owner == country_tag)
            {
                log::debug!("{} already has merchant at node {:?}", country_tag, node);
                return Ok(());
            }

            if state.countries[country_tag]
                .trade
                .merchants_en_route
                .iter()
                .any(|t| t.destination == *node)
            {
                log::debug!(
                    "{} already has merchant en route to node {:?}",
                    country_tag,
                    node
                );
                return Ok(());
            }

            // Calculate travel time using A* pathfinding
            let graph = TradeNodeGraph::new(&state.trade_topology);
            let (path, _cost) =
                AStar::find_path(&graph, home_node, *node, &()).ok_or(ActionError::NoTradeRoute)?;

            // Travel time = (hops - 1) * 15 days (path includes start node)
            let hops = path.len().saturating_sub(1);
            let travel_days = (hops * 15) as u32;
            let arrival_date = state.date.add_days(travel_days);

            // Decrement available merchants and queue for travel
            let country = state.countries.get_mut(country_tag).unwrap();
            country.trade.merchants_available -= 1;
            country.trade.merchants_en_route.push(MerchantTravel {
                destination: *node,
                action: action.clone(),
                arrival_date,
            });

            log::info!(
                "{} dispatches merchant to trade node {:?} ({:?}), arriving {}",
                country_tag,
                node,
                action,
                arrival_date
            );
            Ok(())
        }

        Command::RecallMerchant { node } => {
            // Validate country exists
            let country =
                state
                    .countries
                    .get_mut(country_tag)
                    .ok_or(ActionError::CountryNotFound {
                        tag: country_tag.to_string(),
                    })?;

            // First check for merchant en route to this node (cancel travel)
            let en_route_idx = country
                .trade
                .merchants_en_route
                .iter()
                .position(|t| t.destination == *node);

            if let Some(idx) = en_route_idx {
                country.trade.merchants_en_route.remove(idx);
                country.trade.merchants_available += 1;
                log::info!(
                    "{} recalls merchant en route to trade node {:?}",
                    country_tag,
                    node
                );
                return Ok(());
            }

            // Check if node exists and has our merchant (stationed)
            let node_state = state
                .trade_nodes
                .get_mut(node)
                .ok_or(ActionError::InvalidProvinceId)?;

            let merchant_idx = node_state
                .merchants
                .iter()
                .position(|m| m.owner == country_tag);

            if let Some(idx) = merchant_idx {
                node_state.merchants.remove(idx);
                // Re-borrow country after node_state borrow ends
                state
                    .countries
                    .get_mut(country_tag)
                    .unwrap()
                    .trade
                    .merchants_available += 1;
                log::info!(
                    "{} recalls merchant from trade node {:?}",
                    country_tag,
                    node
                );
            } else {
                log::debug!(
                    "{} has no merchant at node {:?} to recall",
                    country_tag,
                    node
                );
            }

            Ok(())
        }

        Command::UpgradeCenterOfTrade { province } => {
            // Validate country owns province
            let prov = state
                .provinces
                .get_mut(province)
                .ok_or(ActionError::InvalidProvinceId)?;

            if prov.owner.as_ref() != Some(&country_tag.to_string()) {
                return Err(ActionError::NotOwned);
            }

            // Check current level and upgrade
            let current_level = prov.trade.center_of_trade;
            if current_level >= 3 {
                log::debug!("Province {} already at max CoT level", province);
                return Ok(());
            }

            // TODO: Check costs (diplo mana + ducats)
            prov.trade.center_of_trade = current_level + 1;
            log::info!(
                "{} upgrades CoT in province {} to level {}",
                country_tag,
                province,
                current_level + 1
            );
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
        .cloned()
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

            // Attacker loses 10 prestige for failing to enforce demands
            for attacker in &war.attackers {
                if let Some(c) = state.countries.get_mut(attacker) {
                    c.prestige.add(Fixed::from_int(-10));
                }
            }
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

            // Winners gain 25 prestige
            for tag in &winner_tags {
                if let Some(c) = state.countries.get_mut(tag) {
                    c.prestige.add(Fixed::from_int(25));
                }
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

/// Creates truces between all attackers and defenders in a war.
fn create_war_truces(
    state: &mut WorldState,
    war: &crate::state::War,
    current_date: crate::state::Date,
) {
    let expiry = current_date.add_years(5);
    for attacker in &war.attackers {
        for defender in &war.defenders {
            state.diplomacy.create_truce(attacker, defender, expiry);
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
            available_commands: vec![],
            visible_state: None,
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
        // Use December 1444 to bypass first-month immunity
        let state = WorldStateBuilder::new()
            .date(1444, 12, 11)
            .with_country("SWE")
            .with_country("DEN")
            .build();

        let inputs = vec![PlayerInputs {
            country: "SWE".to_string(),
            commands: vec![Command::DeclareWar {
                target: "DEN".to_string(),
                cb: None,
            }],
            available_commands: vec![],
            visible_state: None,
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
    fn test_first_month_immunity_blocks_war() {
        // November 1444 (first month) should block all war declarations
        let mut state = WorldStateBuilder::new()
            .date(1444, 11, 11)
            .with_country("SWE")
            .with_country("DEN")
            .build();

        let result = execute_command(
            &mut state,
            "SWE",
            &Command::DeclareWar {
                target: "DEN".to_string(),
                cb: None,
            },
            None,
        );
        assert!(matches!(
            result,
            Err(ActionError::FirstMonthImmunity { .. })
        ));

        // No war should be created
        assert_eq!(state.diplomacy.wars.len(), 0);
    }

    #[test]
    fn test_declare_war_on_self_fails() {
        // Use December 1444 to bypass first-month immunity
        let state = WorldStateBuilder::new()
            .date(1444, 12, 11)
            .with_country("SWE")
            .build();

        let inputs = vec![PlayerInputs {
            country: "SWE".to_string(),
            commands: vec![Command::DeclareWar {
                target: "SWE".to_string(),
                cb: None,
            }],
            available_commands: vec![],
            visible_state: None,
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
        // Use December 1444 to bypass first-month immunity
        let mut state = WorldStateBuilder::new()
            .date(1444, 12, 11)
            .with_country("SWE")
            .with_country("DEN")
            .build();

        // First war declaration
        let inputs1 = vec![PlayerInputs {
            country: "SWE".to_string(),
            commands: vec![Command::DeclareWar {
                target: "DEN".to_string(),
                cb: None,
            }],
            available_commands: vec![],
            visible_state: None,
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
                cb: None,
            }],
            available_commands: vec![],
            visible_state: None,
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
        // Use December 1444 to bypass first-month immunity
        let state = WorldStateBuilder::new()
            .date(1444, 12, 11)
            .with_country("SWE")
            .build();

        let inputs = vec![PlayerInputs {
            country: "SWE".to_string(),
            commands: vec![Command::DeclareWar {
                target: "XXX".to_string(),
                cb: None,
            }],
            available_commands: vec![],
            visible_state: None,
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
        let cmd = Command::DevelopProvince {
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
        let cmd2 = Command::DevelopProvince {
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
            &Command::DevelopProvince {
                province: 1,
                dev_type: DevType::Tax,
            },
            None,
        )
        .unwrap();

        execute_command(
            &mut state,
            "SWE",
            &Command::DevelopProvince {
                province: 1,
                dev_type: DevType::Production,
            },
            None,
        )
        .unwrap();

        execute_command(
            &mut state,
            "SWE",
            &Command::DevelopProvince {
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
            &Command::DevelopProvince {
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

    #[test]
    fn test_truce_blocks_war_declaration() {
        // Use December 1444 to bypass first-month immunity
        let mut state = WorldStateBuilder::new()
            .date(1444, 12, 11)
            .with_country("A")
            .with_country("B")
            .build();

        // Create truce expiring in 5 years
        let expiry = state.date.add_years(5);
        state.diplomacy.create_truce("A", "B", expiry);

        // Declare war should fail
        let result = execute_command(
            &mut state,
            "A",
            &Command::DeclareWar {
                target: "B".into(),
                cb: None,
            },
            None,
        );
        assert!(matches!(result, Err(ActionError::TruceActive { .. })));
    }

    #[test]
    fn test_truce_expires() {
        let mut state = WorldStateBuilder::new()
            .with_country("A")
            .with_country("B")
            .build();

        // Truce at current date is EXPIRED (expires > current_date)
        // So if expiry == state.date, it's NOT active anymore
        state.diplomacy.create_truce("A", "B", state.date);

        // Should not be active
        assert!(!state.diplomacy.has_active_truce("A", "B", state.date));
    }

    #[test]
    fn test_peace_creates_truces() {
        let mut state = WorldStateBuilder::new()
            .with_country("A")
            .with_country("B")
            .build();

        // Start a war
        let war_id = 0;
        state.diplomacy.wars.insert(
            war_id,
            crate::state::War {
                id: war_id,
                name: "A vs B".to_string(),
                attackers: vec!["A".to_string()],
                defenders: vec!["B".to_string()],
                start_date: state.date,
                attacker_score: 0,
                attacker_battle_score: 0,
                defender_score: 0,
                defender_battle_score: 0,
                pending_peace: None,
            },
        );

        // Offer and accept peace
        let terms = PeaceTerms::WhitePeace;
        execute_command(
            &mut state,
            "A",
            &Command::OfferPeace {
                war_id,
                terms: terms.clone(),
            },
            None,
        )
        .unwrap();
        execute_command(&mut state, "B", &Command::AcceptPeace { war_id }, None).unwrap();

        // Verify truce exists
        assert!(state.diplomacy.has_active_truce("A", "B", state.date));
    }
}
