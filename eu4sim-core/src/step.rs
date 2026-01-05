use crate::fixed::Fixed;
use crate::input::{Command, DevType, PlayerInputs};
use crate::metrics::SimMetrics;
use crate::profiling::{frame_mark_daily, frame_mark_monthly};
use crate::state::{
    ArmyId, DiplomacyState, GeneralId, MovementState, PeaceTerms, PendingPeace, ProvinceId,
    Regiment, RelationType, TechType, WorldState,
};
use std::time::Instant;
use thiserror::Error;
use tracing::instrument;

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
    #[error("Cannot declare war within same realm: {attacker} and {target} share overlord")]
    SameRealmWar { attacker: String, target: String },
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
    #[error("Invalid action: {reason}")]
    InvalidAction { reason: String },
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
    #[error("Invalid command: {message}")]
    InvalidCommand { message: String },
    // Idea errors
    #[error("Invalid idea group: {group_id:?}")]
    InvalidIdeaGroup { group_id: crate::ideas::IdeaGroupId },
    #[error("Cannot pick national idea group: {group_id:?}")]
    CannotPickNationalIdeas { group_id: crate::ideas::IdeaGroupId },
    #[error("Maximum of 8 idea groups already picked")]
    MaxIdeaGroupsReached,
    #[error("Idea group already picked: {group_id:?}")]
    IdeaGroupAlreadyPicked { group_id: crate::ideas::IdeaGroupId },
    #[error("Idea group not yet picked: {group_id:?}")]
    IdeaGroupNotPicked { group_id: crate::ideas::IdeaGroupId },
    #[error("All 7 ideas already unlocked in group: {group_id:?}")]
    AllIdeasUnlocked { group_id: crate::ideas::IdeaGroupId },
    #[error("Insufficient tech for idea: need {required}, have {current}")]
    InsufficientTechForIdea { required: u8, current: u8 },
    #[error("Ewiger Landfriede prohibits wars between HRE members")]
    EwigerLandfriedeActive,
    #[error("Province not found: {province_id}")]
    ProvinceNotFound { province_id: u32 },
    #[error("Invalid destination for unit type: {destination}")]
    InvalidDestination { destination: u32 },
}

/// Advance the world by one tick.
#[instrument(skip_all, name = "step_world")]
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
                // Downgrade to debug - these are often valid simultaneous move conflicts
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
    crate::systems::run_combat_tick(&mut new_state, adjacency);
    if let Some(m) = metrics.as_mut() {
        m.combat_time += combat_start.elapsed();
    }

    // Naval combat runs daily (whenever fleets are engaged)
    let naval_combat_start = Instant::now();
    crate::systems::run_naval_combat_tick(&mut new_state);
    if let Some(m) = metrics.as_mut() {
        m.combat_time += naval_combat_start.elapsed(); // Count as combat time
    }

    // Siege runs daily (progress siege phases and dice rolls)
    let siege_start = Instant::now();
    crate::systems::run_siege_tick(&mut new_state, adjacency);
    if let Some(m) = metrics.as_mut() {
        m.combat_time += siege_start.elapsed(); // Count as combat time
    }

    // Clean up empty armies (0/0/0 strength) after combat/sieges
    cleanup_empty_armies(&mut new_state);

    // Update occupation and sieges (armies in enemy territory start sieges or occupy instantly)
    let occ_start = Instant::now();
    update_occupation(&mut new_state);
    if let Some(m) = metrics.as_mut() {
        m.occupation_time += occ_start.elapsed();
    }

    // Mark end of daily tick for Tracy
    frame_mark_daily();

    // Economic systems run monthly (on 1st of each month)
    if new_state.date.day == 1 {
        // Debug: Log treasury at start of monthly tick
        if let Some(country) = new_state.countries.get("KOR") {
            log::debug!(
                "Monthly tick starting for {} - KOR treasury: {:.2}",
                new_state.date,
                country.treasury.to_f32()
            );
        }

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
        // 10. Estates → Updates loyalty/influence, checks disasters
        // 11. Reformation → Spreads Protestant/Reformed religions
        // 12. War scores → Recalculates based on current occupation
        // 13. Auto-peace → Ends stalemate wars (10yr timeout)
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
        crate::systems::run_attrition_tick(&mut new_state);
        cleanup_empty_armies(&mut new_state); // Attrition can destroy armies
        crate::systems::run_expenses_tick(&mut new_state);
        crate::systems::run_advisor_cost_tick(&mut new_state);
        crate::systems::run_mana_tick(&mut new_state);
        crate::systems::run_stats_tick(&mut new_state);
        crate::systems::run_colonization_tick(&mut new_state);
        crate::systems::run_estate_tick(&mut new_state);
        crate::systems::tick_institution_spread(&mut new_state);
        crate::systems::run_reformation_tick(&mut new_state, adjacency);
        crate::systems::run_hre_tick(&mut new_state);

        // Coring - Progress active coring and complete after 36 months. 🛡️
        crate::systems::tick_coring(&mut new_state);

        // Building construction - Progress and complete buildings
        crate::systems::tick_building_construction(&mut new_state);

        // Recalculate overextension (uncored dev causes OE penalties)
        crate::systems::recalculate_overextension(&mut new_state);

        // Recalculate war scores monthly
        crate::systems::recalculate_war_scores(&mut new_state);

        // Coalition formation and AE decay
        crate::systems::run_coalition_tick(&mut new_state);

        // Yearly systems - run on January 1st
        if new_state.date.month == 1 {
            // Tributary payments happen at the start of each year
            crate::systems::run_tribute_payments(&mut new_state);
            // Celestial Empire mandate tick (yearly, unlike HRE's monthly)
            crate::systems::run_celestial_tick(&mut new_state);
            // Meritocracy tick (yearly, from advisors)
            crate::systems::run_meritocracy_tick(&mut new_state);
        }

        // Auto-end wars after 10 years (stalemate prevention)
        auto_end_stale_wars(&mut new_state);

        if let Some(m) = metrics.as_mut() {
            m.economy_time += econ_start.elapsed();
        }

        // Mark end of monthly tick for Tracy
        frame_mark_monthly();
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
/// If an army is in a province owned by an enemy (during war), start siege or occupy instantly.
/// For unfortified provinces: instant occupation (like before).
/// For fortified provinces or capitals: start/join siege (handled by siege system).
fn update_occupation(state: &mut WorldState) {
    // Collect armies in enemy territory
    let mut enemy_occupations: Vec<(ProvinceId, ArmyId, String)> = Vec::new();

    for (&army_id, army) in state.armies.iter() {
        // Skip armies in combat or embarked
        if army.in_battle.is_some() || army.embarked_on.is_some() {
            continue;
        }

        let province_id = army.location;
        if let Some(province) = state.provinces.get(&province_id) {
            if let Some(owner) = &province.owner {
                // Skip if we already control this province
                if province.controller.as_ref() == Some(&army.owner) {
                    continue;
                }

                // Check if army owner is at war with province owner
                if owner != &army.owner && state.diplomacy.are_at_war(&army.owner, owner) {
                    log::debug!(
                        "Army {} ({}) in enemy province {} owned by {}",
                        army_id,
                        army.owner,
                        province_id,
                        owner
                    );
                    enemy_occupations.push((province_id, army_id, army.owner.clone()));
                }
            }
        }
    }

    // Process occupations: instant for unfortified, sieges for fortified
    for (province_id, army_id, attacker) in enemy_occupations.iter() {
        crate::systems::start_occupation(state, *province_id, attacker, *army_id);
    }

    // Clean up abandoned sieges (no armies left besieging)
    cleanup_abandoned_sieges(state);
}

/// Remove sieges that have no besieging armies (armies withdrew or were destroyed).
fn cleanup_abandoned_sieges(state: &mut WorldState) {
    let mut sieges_to_remove: Vec<ProvinceId> = Vec::new();
    let mut sieges_to_update: Vec<(ProvinceId, Vec<ArmyId>)> = Vec::new();

    for (&province_id, siege) in state.sieges.iter() {
        // Check if any besieging armies still exist and are at the siege location
        let active_armies: Vec<ArmyId> = siege
            .besieging_armies
            .iter()
            .filter(|&&army_id| {
                state
                    .armies
                    .get(&army_id)
                    .map(|a| a.location == province_id && a.in_battle.is_none())
                    .unwrap_or(false)
            })
            .copied()
            .collect();

        if active_armies.is_empty() {
            sieges_to_remove.push(province_id);
            log::debug!(
                "Siege at province {} abandoned (no armies left)",
                province_id
            );
        } else if active_armies.len() < siege.besieging_armies.len() {
            // Some armies left - need to update the list
            sieges_to_update.push((province_id, active_armies));
        }
    }

    // Remove abandoned sieges
    for province_id in sieges_to_remove {
        state.sieges.remove(&province_id);
    }

    // Update besieging army lists for remaining sieges
    for (province_id, active_armies) in sieges_to_update {
        if let Some(siege) = state.sieges.get_mut(&province_id) {
            siege.besieging_armies = active_armies;
        }
    }
}

/// Remove armies that have no regiments or all regiments at zero strength.
/// These are "ghost armies" that should not exist.
fn cleanup_empty_armies(state: &mut WorldState) {
    use crate::fixed::Fixed;

    let armies_to_remove: Vec<ArmyId> = state
        .armies
        .iter()
        .filter(|(_, army)| {
            // Army is empty if it has no regiments
            if army.regiments.is_empty() {
                return true;
            }
            // Or if all regiments have zero strength
            army.regiments.iter().all(|reg| reg.strength <= Fixed::ZERO)
        })
        .map(|(&id, _)| id)
        .collect();

    for army_id in &armies_to_remove {
        if let Some(army) = state.armies.get(army_id) {
            log::debug!(
                "Removing empty army {} '{}' (owner: {})",
                army_id,
                army.name,
                army.owner
            );
        }
        state.armies.remove(army_id);
    }

    if !armies_to_remove.is_empty() {
        log::info!("Cleaned up {} empty armies", armies_to_remove.len());
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

            // Clear peace offer cooldowns and pending call-to-arms for all participants
            for tag in war.attackers.iter().chain(war.defenders.iter()) {
                if let Some(country) = state.countries.get_mut(tag) {
                    country.peace_offer_cooldowns.remove(&war_id);
                    country.pending_call_to_arms.remove(&war_id);
                }
            }
        }

        // Clear pending call-to-arms for all countries
        for (_tag, country) in state.countries.iter_mut() {
            country.pending_call_to_arms.remove(&war_id);
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

/// Call allies to join a war.
///
/// - Defensive allies (defender's allies) auto-join immediately
/// - Offensive allies (attacker's allies) receive a pending call-to-arms to decide
fn call_allies_to_war(
    state: &mut WorldState,
    war_id: crate::state::WarId,
    declarer: &str,
    is_attacker: bool,
) {
    use crate::input::WarSide;
    use crate::state::RelationType;

    // Find all allies of the declarer
    let allies: Vec<String> = state
        .diplomacy
        .relations
        .iter()
        .filter_map(|((a, b), rel)| {
            if *rel == RelationType::Alliance {
                if a == declarer {
                    Some(b.clone())
                } else if b == declarer {
                    Some(a.clone())
                } else {
                    None
                }
            } else {
                None
            }
        })
        .collect();

    for ally in allies {
        if !is_attacker {
            // Defensive war - allies auto-join to defend
            join_war(state, &ally, war_id, WarSide::Defender);
            log::info!("{} auto-joins war {} to defend {}", ally, war_id, declarer);
        } else {
            // Offensive war - create pending call-to-arms (ally chooses)
            if let Some(country) = state.countries.get_mut(&ally) {
                country
                    .pending_call_to_arms
                    .insert(war_id, WarSide::Attacker);
                log::info!(
                    "{} received call-to-arms from {} for war {}",
                    ally,
                    declarer,
                    war_id
                );
            }
        }
    }
}

/// Add a country to a war on the specified side.
/// Subjects auto-join their overlord's wars based on subject type.
///
/// Unlike allies (who get a choice for offensive wars), subjects with
/// `joins_overlords_wars = true` auto-join both offensive and defensive wars.
/// Tributaries (`joins_overlords_wars = false`) never auto-join.
fn call_subjects_to_war(
    state: &mut WorldState,
    war_id: crate::state::WarId,
    overlord: &str,
    side: crate::input::WarSide,
) {
    // Get all subjects of this overlord
    let subjects: Vec<String> = state
        .diplomacy
        .get_subjects(overlord)
        .iter()
        .filter_map(|rel| {
            // Check if this subject type auto-joins wars
            let subject_def = state.subject_types.get(rel.subject_type);
            if subject_def.is_some_and(|def| def.joins_overlords_wars) {
                Some(rel.subject.clone())
            } else {
                None
            }
        })
        .collect();

    for subject in subjects {
        join_war(state, &subject, war_id, side);
        log::info!(
            "{} (subject of {}) auto-joins war {} as {:?}",
            subject,
            overlord,
            war_id,
            side
        );

        // Recursively add subjects of subjects (if any)
        // This handles cases like Austria -> Hungary -> Bohemia (PU chains)
        call_subjects_to_war(state, war_id, &subject, side);
    }
}

fn join_war(
    state: &mut WorldState,
    country: &str,
    war_id: crate::state::WarId,
    side: crate::input::WarSide,
) {
    use crate::input::WarSide;

    if let Some(war) = state.diplomacy.wars.get_mut(&war_id) {
        let list = match side {
            WarSide::Attacker => &mut war.attackers,
            WarSide::Defender => &mut war.defenders,
        };
        if !list.contains(&country.to_string()) {
            list.push(country.to_string());
            log::info!("{} joined war {} as {:?}", country, war_id, side);
        }
    }

    // Clear pending call-to-arms if it exists
    if let Some(country_state) = state.countries.get_mut(country) {
        country_state.pending_call_to_arms.remove(&war_id);
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

            // BuildInProvince - Transform the land with lasting structures.
            let buildable = crate::systems::available_buildings(
                prov,
                country,
                &state.building_defs,
                &state.building_upgraded_by,
            );
            for building_id in buildable {
                if let Some(def) = state.building_defs.get(&building_id) {
                    available.push(Command::BuildInProvince {
                        province: *prov_id,
                        building: def.name.clone(),
                    });
                }
            }

            // CancelConstruction - Halt work and reclaim invested gold.
            if prov.building_construction.is_some() {
                available.push(Command::CancelConstruction { province: *prov_id });
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
                            && !neighbor.is_wasteland
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

    // Recruitment & Generals - The sinews of war. ⚔️
    let manpower_cost = Fixed::from_int(1000);
    if country.manpower >= manpower_cost {
        for (&prov_id, prov) in &state.provinces {
            if prov.owner.as_deref() == Some(country_tag) {
                // Infantry (10g)
                if country.treasury >= Fixed::from_int(10) {
                    available.push(Command::RecruitRegiment {
                        province: prov_id,
                        unit_type: crate::state::RegimentType::Infantry,
                    });
                }
                // Cavalry (25g)
                if country.treasury >= Fixed::from_int(25) {
                    available.push(Command::RecruitRegiment {
                        province: prov_id,
                        unit_type: crate::state::RegimentType::Cavalry,
                    });
                }
                // Artillery (30g + Tech 7)
                if country.treasury >= Fixed::from_int(30)
                    && country.mil_tech >= eu4data::defines::combat::ARTILLERY_TECH_REQUIRED
                {
                    available.push(Command::RecruitRegiment {
                        province: prov_id,
                        unit_type: crate::state::RegimentType::Artillery,
                    });
                }
            }
        }
    }

    // Recruit General (50 MIL)
    if country.mil_mana >= Fixed::from_int(50) {
        available.push(Command::RecruitGeneral);
    }

    // Assign General (Free, but requires general and unled army)
    // Find unassigned generals
    let unassigned_generals: Vec<crate::state::GeneralId> = state
        .generals
        .values()
        .filter(|g| {
            g.owner == country_tag && !state.armies.values().any(|a| a.general == Some(g.id))
        })
        .map(|g| g.id)
        .collect();

    if !unassigned_generals.is_empty() {
        for (army_id, army) in &state.armies {
            if army.owner == country_tag && army.general.is_none() {
                // Offer assigning the first available general (simplification for AI)
                // Listing all combinations would explode the action space
                if let Some(&gen_id) = unassigned_generals.first() {
                    available.push(Command::AssignGeneral {
                        army: *army_id,
                        general: gen_id,
                    });
                }
            }
        }
    }

    // 3. Military Actions - Armies are the shields that guard our truth. 🛡️
    // Build set of armies participating in active sieges (shouldn't move)
    let besieging_armies: std::collections::HashSet<ArmyId> = state
        .sieges
        .values()
        .flat_map(|s| s.besieging_armies.iter().copied())
        .collect();

    if let Some(graph) = adjacency {
        // Move: For each army, check adjacent provinces
        for (army_id, army) in &state.armies {
            // Skip armies that are besieging - they should finish the siege first
            if besieging_armies.contains(army_id) {
                continue;
            }

            // Skip empty armies (0/0/0 strength) - they shouldn't exist but might not be cleaned up yet
            let total_strength: Fixed = army
                .regiments
                .iter()
                .map(|r| r.strength)
                .fold(Fixed::ZERO, |a, b| a + b);
            if total_strength <= Fixed::ZERO {
                continue;
            }

            if army.owner == country_tag && army.movement.is_none() && army.embarked_on.is_none() {
                for neighbor in graph.neighbors(army.location) {
                    if can_army_enter(state, country_tag, neighbor) {
                        // Check Zone of Control - forts block movement through adjacent provinces
                        if state.is_blocked_by_zoc(
                            army.location,
                            neighbor,
                            country_tag,
                            Some(graph),
                        ) {
                            continue; // ZoC blocked
                        }

                        // Check strait blocking - enemy fleets in sea zones block crossing
                        if state.is_strait_blocked(
                            army.location,
                            neighbor,
                            country_tag,
                            Some(graph),
                        ) {
                            continue; // Strait blocked by enemy fleet
                        }

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

    // MergeArmies: When 2+ friendly armies are in the same province
    {
        use std::collections::HashMap;
        let mut armies_by_location: HashMap<ProvinceId, Vec<ArmyId>> = HashMap::new();

        for (army_id, army) in &state.armies {
            // Only consider stationary, non-embarked, non-battling armies owned by this country
            if army.owner == country_tag
                && army.movement.is_none()
                && army.embarked_on.is_none()
                && army.in_battle.is_none()
            {
                armies_by_location
                    .entry(army.location)
                    .or_default()
                    .push(*army_id);
            }
        }

        // Generate MergeArmies for provinces with 2+ armies
        for (_location, army_ids) in armies_by_location {
            if army_ids.len() >= 2 {
                available.push(Command::MergeArmies { army_ids });
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

            // Check Ewiger Landfriede: if active, we cannot attack other HRE members
            let ewiger_landfriede_blocks = state.global.hre.has_ewiger_landfriede()
                && state
                    .global
                    .hre
                    .is_member(&country_tag.to_string(), &state.provinces);

            for target_tag in potential_targets {
                if !state.diplomacy.are_at_war(country_tag, &target_tag)
                    && !state
                        .diplomacy
                        .has_active_truce(country_tag, &target_tag, state.date)
                    && !state.diplomacy.in_same_realm(
                        country_tag,
                        &target_tag,
                        &state.subject_types,
                    )
                {
                    // Skip HRE members if Ewiger Landfriede is active and we're in HRE
                    if ewiger_landfriede_blocks
                        && state.global.hre.is_member(&target_tag, &state.provinces)
                    {
                        continue;
                    }
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
        let is_attacker = war.attackers.contains(&country_tag.to_string());
        let our_score = if is_attacker {
            war.attacker_score
        } else {
            war.defender_score
        };
        let enemies: Vec<&String> = if is_attacker {
            war.defenders.iter().collect()
        } else {
            war.attackers.iter().collect()
        };

        // Find occupied enemy provinces (controller is us, owner is enemy)
        let occupied: Vec<ProvinceId> = state
            .provinces
            .iter()
            .filter(|(_, p)| {
                p.controller.as_ref() == Some(&country_tag.to_string())
                    && p.owner.as_ref().is_some_and(|o| enemies.contains(&o))
            })
            .map(|(&id, _)| id)
            .collect();

        // Check if we occupy at least one enemy fort
        // Fort requirement: can't take provinces without occupying a fort first
        let has_occupied_fort = state.provinces.iter().any(|(_, p)| {
            p.fort_level > 0
                && p.controller.as_ref() == Some(&country_tag.to_string())
                && p.owner.as_ref().is_some_and(|o| enemies.contains(&o))
        });

        // Don't offer peace if there's already a pending offer in this war
        let has_pending_offer = war.pending_peace.is_some();

        // OfferPeace with TakeProvinces if we occupy enemy provinces, can afford it, AND have a fort
        if !has_pending_offer && !occupied.is_empty() && has_occupied_fort {
            let terms = PeaceTerms::TakeProvinces {
                provinces: occupied.clone(),
            };
            let peace_cost = calculate_peace_terms_cost(state, &terms, war, is_attacker);

            if our_score >= peace_cost {
                log::info!(
                    "[PEACE] {} offering TakeProvinces in {} ({} provinces, cost={}, score={})",
                    country_tag,
                    war.name,
                    occupied.len(),
                    peace_cost,
                    our_score,
                );
                available.push(Command::OfferPeace {
                    war_id: war.id,
                    terms,
                });
            } else {
                log::debug!(
                    "[PEACE] {} can't afford TakeProvinces in {}: cost={} > score={}",
                    country_tag,
                    war.name,
                    peace_cost,
                    our_score,
                );
            }
        } else if !occupied.is_empty() && our_score > 0 && !has_occupied_fort {
            // Debug: occupation but no fort
            log::debug!(
                "[PEACE] {} occupies {} provinces but no fort in {}",
                country_tag,
                occupied.len(),
                war.name,
            );
        }

        // WhitePeace - only offer if losing or stalemate (war score <= 10)
        // Also requires 6+ months of war to prevent frivolous early offers
        // AND no pending offer already
        let war_months = state.date.months_since(&war.start_date);
        if !has_pending_offer && war_months >= 6 && our_score <= 10 {
            available.push(Command::OfferPeace {
                war_id: war.id,
                terms: PeaceTerms::WhitePeace,
            });
        }

        // AcceptPeace - Accepting the fate that the stars have written. 🛡️
        if let Some(pending) = &war.pending_peace {
            let caller_is_attacker = war.attackers.contains(&country_tag.to_string());
            if pending.from_attacker != caller_is_attacker {
                available.push(Command::AcceptPeace { war_id: war.id });
                available.push(Command::RejectPeace { war_id: war.id });
            }
        }
    }

    // Pending Call-to-Arms - Allies summon aid in times of war.
    if let Some(country_state) = state.countries.get(country_tag) {
        for (&war_id, &side) in &country_state.pending_call_to_arms {
            available.push(Command::JoinWar { war_id, side });
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

    // 6. Diplomacy - Allies and rivals shape the fabric of power. ✧
    let can_offer_diplomatic = country.last_diplomatic_action != Some(state.date);

    // Accept/Reject pending alliance offers (no cooldown)
    for (offer_key, _date) in &state.diplomacy.pending_alliance_offers {
        let (from, to) = offer_key;
        if to == country_tag {
            available.push(Command::AcceptAlliance { from: from.clone() });
            available.push(Command::RejectAlliance { from: from.clone() });
        }
    }

    // Accept/Reject pending royal marriage offers (no cooldown)
    for (offer_key, _date) in &state.diplomacy.pending_marriage_offers {
        let (from, to) = offer_key;
        if to == country_tag {
            available.push(Command::AcceptRoyalMarriage { from: from.clone() });
            available.push(Command::RejectRoyalMarriage { from: from.clone() });
        }
    }

    if can_offer_diplomatic {
        // Only consider neighbors for alliances and rivalries (same optimization as war declarations)
        if let Some(graph) = adjacency {
            let mut potential_neighbors = std::collections::HashSet::new();

            // Find neighbors of all owned provinces
            for (prov_id, prov) in &state.provinces {
                if prov.owner.as_deref() == Some(country_tag) {
                    for neighbor_id in graph.neighbors(*prov_id) {
                        if let Some(neighbor) = state.provinces.get(&neighbor_id) {
                            if let Some(owner) = &neighbor.owner {
                                if owner != country_tag {
                                    potential_neighbors.insert(owner.clone());
                                }
                            }
                        }
                    }
                }
            }

            // OfferAlliance - for neighbors not at war, not already allied, no pending offer
            for target in &potential_neighbors {
                let key = DiplomacyState::sorted_pair(country_tag, target);
                let offer_key = (country_tag.to_string(), target.clone());

                if !state.diplomacy.are_at_war(country_tag, target)
                    && state.diplomacy.relations.get(&key) != Some(&RelationType::Alliance)
                    && !state
                        .diplomacy
                        .pending_alliance_offers
                        .contains_key(&offer_key)
                {
                    available.push(Command::OfferAlliance {
                        target: target.clone(),
                    });
                }
            }

            // SetRival - only if under the limit of 3
            let current_rival_count = country.rivals.len();
            if current_rival_count < 3 {
                for target in potential_neighbors {
                    let key = DiplomacyState::sorted_pair(country_tag, &target);

                    // Can rival if: not at war, not allied, not already rivals
                    if !state.diplomacy.are_at_war(country_tag, &target)
                        && state.diplomacy.relations.get(&key) != Some(&RelationType::Alliance)
                        && !country.rivals.contains(&target)
                    {
                        available.push(Command::SetRival { target });
                    }
                }
            }
        }

        // RemoveRival - offer to remove each current rival
        for rival in &country.rivals {
            available.push(Command::RemoveRival {
                target: rival.clone(),
            });
        }

        // BreakAlliance - offer to break each current alliance
        for (pair, rel_type) in &state.diplomacy.relations {
            if *rel_type == RelationType::Alliance {
                let (a, b) = pair;
                if a == country_tag {
                    available.push(Command::BreakAlliance { target: b.clone() });
                } else if b == country_tag {
                    available.push(Command::BreakAlliance { target: a.clone() });
                }
            }
        }

        // OfferRoyalMarriage - for neighbors not at war, not already married, no pending offer
        if let Some(graph) = adjacency {
            let mut potential_neighbors = std::collections::HashSet::new();

            // Find neighbors (reuse neighbor finding logic)
            for (prov_id, prov) in &state.provinces {
                if prov.owner.as_deref() == Some(country_tag) {
                    for neighbor_id in graph.neighbors(*prov_id) {
                        if let Some(neighbor) = state.provinces.get(&neighbor_id) {
                            if let Some(owner) = &neighbor.owner {
                                if owner != country_tag {
                                    potential_neighbors.insert(owner.clone());
                                }
                            }
                        }
                    }
                }
            }

            for target in potential_neighbors {
                let key = DiplomacyState::sorted_pair(country_tag, &target);
                let offer_key = (country_tag.to_string(), target.clone());

                if !state.diplomacy.are_at_war(country_tag, &target)
                    && state.diplomacy.relations.get(&key) != Some(&RelationType::RoyalMarriage)
                    && !state
                        .diplomacy
                        .pending_marriage_offers
                        .contains_key(&offer_key)
                {
                    available.push(Command::OfferRoyalMarriage { target });
                }
            }
        }

        // BreakRoyalMarriage - offer to break each current royal marriage
        for (pair, rel_type) in &state.diplomacy.relations {
            if *rel_type == RelationType::RoyalMarriage {
                let (a, b) = pair;
                if a == country_tag {
                    available.push(Command::BreakRoyalMarriage { target: b.clone() });
                } else if b == country_tag {
                    available.push(Command::BreakRoyalMarriage { target: a.clone() });
                }
            }
        }

        // CancelMilitaryAccess - offer to cancel access for each country that has access
        for (access_key, _) in &state.diplomacy.military_access {
            let (granter, requester) = access_key;
            if granter == country_tag {
                available.push(Command::CancelMilitaryAccess {
                    target: requester.clone(),
                });
            }
        }

        // RequestMilitaryAccess - for neighbors without access
        if let Some(graph) = adjacency {
            let mut potential_neighbors = std::collections::HashSet::new();

            // Find neighbors (reuse neighbor finding logic)
            for (prov_id, prov) in &state.provinces {
                if prov.owner.as_deref() == Some(country_tag) {
                    for neighbor_id in graph.neighbors(*prov_id) {
                        if let Some(neighbor) = state.provinces.get(&neighbor_id) {
                            if let Some(owner) = &neighbor.owner {
                                if owner != country_tag {
                                    potential_neighbors.insert(owner.clone());
                                }
                            }
                        }
                    }
                }
            }

            for target in potential_neighbors {
                let access_key = (target.clone(), country_tag.to_string());
                let request_key = (country_tag.to_string(), target.clone());

                // Request if: not at war, no access, no pending request
                if !state.diplomacy.are_at_war(country_tag, &target)
                    && !state.diplomacy.military_access.contains_key(&access_key)
                    && !state
                        .diplomacy
                        .pending_access_requests
                        .contains_key(&request_key)
                {
                    available.push(Command::RequestMilitaryAccess { target });
                }
            }
        }
    }

    // Grant/Deny pending military access requests (no cooldown)
    for (request_key, _date) in &state.diplomacy.pending_access_requests {
        let (from, to) = request_key;
        if to == country_tag {
            available.push(Command::GrantMilitaryAccess { to: from.clone() });
            available.push(Command::DenyMilitaryAccess { to: from.clone() });
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
        Command::BuildInProvince { province, building } => {
            crate::systems::start_construction(state, *province, building, country_tag).map_err(
                |e| ActionError::InvalidAction {
                    reason: e.to_string(),
                },
            )
        }
        Command::CancelConstruction { province } => {
            crate::systems::cancel_construction_manual(state, *province, country_tag)
                .map(|_| ())
                .map_err(|e| ActionError::InvalidAction {
                    reason: e.to_string(),
                })
        }
        Command::DemolishBuilding { province, building } => {
            crate::systems::demolish_building(state, *province, building, country_tag).map_err(
                |e| ActionError::InvalidAction {
                    reason: e.to_string(),
                },
            )
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

            // Calculate max morale with country modifier
            let base_morale = Fixed::from_f32(eu4data::defines::combat::BASE_MORALE);
            let morale_mod = state
                .modifiers
                .country_morale
                .get(country_tag)
                .copied()
                .unwrap_or(Fixed::ZERO);
            let max_morale = base_morale.mul(Fixed::ONE + morale_mod);

            if let Some(army_id) = existing_army_id {
                if let Some(army) = state.armies.get_mut(&army_id) {
                    army.regiments.push(crate::state::Regiment {
                        type_: *unit_type,
                        strength: Fixed::from_int(1000),
                        morale: max_morale,
                    });
                    army.recompute_counts();
                }
            } else {
                // Create new army
                let army_id = state.next_army_id;
                state.next_army_id += 1;
                // Set initial counts based on unit type
                let (inf, cav, art) = match unit_type {
                    crate::state::RegimentType::Infantry => (1, 0, 0),
                    crate::state::RegimentType::Cavalry => (0, 1, 0),
                    crate::state::RegimentType::Artillery => (0, 0, 1),
                };
                state.armies.insert(
                    army_id,
                    crate::state::Army {
                        id: army_id,
                        name: format!("{} Army {}", country_tag, army_id),
                        owner: country_tag.to_string(),
                        location: *province,
                        previous_location: None,
                        regiments: vec![crate::state::Regiment {
                            type_: *unit_type,
                            strength: Fixed::from_int(1000),
                            morale: max_morale,
                        }],
                        movement: None,
                        embarked_on: None,
                        general: None,
                        in_battle: None,
                        infantry_count: inf,
                        cavalry_count: cav,
                        artillery_count: art,
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

            // Check Zone of Control (ZoC) - forts block movement through adjacent provinces
            if state.is_blocked_by_zoc(current_location, *destination, country_tag, adjacency) {
                return Err(ActionError::NoMilitaryAccess {
                    province: *destination,
                    owner: "ZoC blocked".to_string(), // Generic error (could add specific ZoC error later)
                });
            }

            // Check strait blocking - enemy fleets in sea zones block crossing
            if state.is_strait_blocked(current_location, *destination, country_tag, adjacency) {
                return Err(ActionError::NoMilitaryAccess {
                    province: *destination,
                    owner: "Strait blocked by enemy fleet".to_string(),
                });
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

            // Cannot attack your own subjects or overlord (unless they're tributaries)
            if state
                .diplomacy
                .in_same_realm(country_tag, target, &state.subject_types)
            {
                return Err(ActionError::SameRealmWar {
                    attacker: country_tag.to_string(),
                    target: target.clone(),
                });
            }

            // Ewiger Landfriede: cannot attack other HRE members
            if state.global.hre.has_ewiger_landfriede() {
                let attacker_in_hre = state
                    .global
                    .hre
                    .is_member(&country_tag.to_string(), &state.provinces);
                let target_in_hre = state.global.hre.is_member(target, &state.provinces);
                if attacker_in_hre && target_in_hre {
                    log::info!(
                        "{} cannot attack {} due to Ewiger Landfriede",
                        country_tag,
                        target
                    );
                    return Err(ActionError::EwigerLandfriedeActive);
                }
            }

            // Apply Royal Marriage penalty
            let key = DiplomacyState::sorted_pair(country_tag, target);
            let has_royal_marriage =
                state.diplomacy.relations.get(&key) == Some(&RelationType::RoyalMarriage);
            if has_royal_marriage {
                if let Some(country) = state.countries.get_mut(country_tag) {
                    country.stability.add(-1);
                    log::info!(
                        "{} declares war on RM partner {}: -1 stability",
                        country_tag,
                        target
                    );
                }
            }

            // Apply No-CB stability penalty (stacks with RM penalty)
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

            // Call allies to join the war
            call_allies_to_war(state, war_id, country_tag, true); // Attacker's allies
            call_allies_to_war(state, war_id, target, false); // Defender's allies (auto-join)

            // Call subjects to join the war (based on subject type)
            call_subjects_to_war(state, war_id, country_tag, crate::input::WarSide::Attacker);
            call_subjects_to_war(state, war_id, target, crate::input::WarSide::Defender);

            // Break diplomatic relations between enemies (war cleanup)
            // Get the war with updated attacker/defender lists (after allies/subjects joined)
            if let Some(war) = state.diplomacy.wars.get(&war_id) {
                let attackers = war.attackers.clone();
                let defenders = war.defenders.clone();

                for attacker in &attackers {
                    for defender in &defenders {
                        let key = DiplomacyState::sorted_pair(attacker, defender);

                        // Remove alliance if exists
                        if state.diplomacy.relations.get(&key) == Some(&RelationType::Alliance) {
                            state.diplomacy.relations.remove(&key);
                            log::debug!("War broke alliance between {} and {}", attacker, defender);
                        }

                        // Remove royal marriage if exists
                        if state.diplomacy.relations.get(&key) == Some(&RelationType::RoyalMarriage)
                        {
                            state.diplomacy.relations.remove(&key);
                            log::debug!(
                                "War broke royal marriage between {} and {}",
                                attacker,
                                defender
                            );
                        }

                        // Revoke military access (both directions)
                        let access_key_1 = (attacker.clone(), defender.clone());
                        let access_key_2 = (defender.clone(), attacker.clone());

                        if state
                            .diplomacy
                            .military_access
                            .remove(&access_key_1)
                            .is_some()
                        {
                            log::debug!("War revoked military access: {} → {}", attacker, defender);
                        }
                        if state
                            .diplomacy
                            .military_access
                            .remove(&access_key_2)
                            .is_some()
                        {
                            log::debug!("War revoked military access: {} → {}", defender, attacker);
                        }
                    }
                }
            }

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

            // Validate destination is a sea zone
            if let Some(dest_province) = state.provinces.get(destination) {
                if !dest_province.is_sea {
                    log::warn!(
                        "Fleet {} cannot move to land province {}",
                        fleet_id,
                        destination
                    );
                    return Err(ActionError::InvalidDestination {
                        destination: *destination,
                    });
                }
            } else {
                return Err(ActionError::ProvinceNotFound {
                    province_id: *destination,
                });
            }

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
            let army_size = army.regiment_count();
            let current_capacity_used: u32 = fleet
                .embarked_armies
                .iter()
                .filter_map(|aid| state.armies.get(aid))
                .map(|a| a.regiment_count())
                .sum();

            // Calculate transport capacity from transport ships (1 ship = 1 regiment)
            let transport_capacity = fleet
                .ships
                .iter()
                .filter(|s| s.type_ == crate::state::ShipType::Transport)
                .count() as u32;

            if current_capacity_used + army_size > transport_capacity {
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
        Command::PickIdeaGroup { group_id } => {
            use crate::ideas::IdeaCategory;

            // Validate country exists
            let country = state
                .countries
                .get(country_tag)
                .ok_or(ActionError::CountryNotFound {
                    tag: country_tag.to_string(),
                })?;

            // Validate idea group exists
            let group = state
                .idea_groups
                .get(*group_id)
                .ok_or(ActionError::InvalidIdeaGroup {
                    group_id: *group_id,
                })?;

            // Cannot pick national ideas (they are auto-assigned)
            if group.is_national {
                return Err(ActionError::CannotPickNationalIdeas {
                    group_id: *group_id,
                });
            }

            // Cannot pick more than 8 idea groups
            const MAX_IDEA_GROUPS: usize = 8;
            if country.ideas.groups.len() >= MAX_IDEA_GROUPS {
                return Err(ActionError::MaxIdeaGroupsReached);
            }

            // Cannot pick same group twice
            if country.ideas.groups.contains_key(group_id) {
                return Err(ActionError::IdeaGroupAlreadyPicked {
                    group_id: *group_id,
                });
            }

            // Check if we have required tech level (3 + 4 per group)
            let required_tech = 3 + (country.ideas.groups.len() as u8 * 4);
            let current_tech = match group.category {
                Some(IdeaCategory::Adm) => country.adm_tech,
                Some(IdeaCategory::Dip) => country.dip_tech,
                Some(IdeaCategory::Mil) => country.mil_tech,
                None => country.adm_tech, // Fallback to ADM if no category
            };
            if current_tech < required_tech {
                return Err(ActionError::InsufficientTechForIdea {
                    required: required_tech,
                    current: current_tech,
                });
            }

            // Pick the idea group (0 ideas unlocked initially)
            let country =
                state
                    .countries
                    .get_mut(country_tag)
                    .ok_or(ActionError::CountryNotFound {
                        tag: country_tag.to_string(),
                    })?;
            country.ideas.groups.insert(*group_id, 0);

            log::info!(
                "{} picked idea group {} (slot {})",
                country_tag,
                group.name,
                country.ideas.groups.len()
            );

            Ok(())
        }
        Command::UnlockIdea { group_id } => {
            use crate::ideas::IdeaCategory;

            // Validate country exists
            let country = state
                .countries
                .get(country_tag)
                .ok_or(ActionError::CountryNotFound {
                    tag: country_tag.to_string(),
                })?;

            // Validate idea group was picked
            let ideas_unlocked =
                *country
                    .ideas
                    .groups
                    .get(group_id)
                    .ok_or(ActionError::IdeaGroupNotPicked {
                        group_id: *group_id,
                    })?;

            // Cannot unlock more than 7 ideas
            if ideas_unlocked >= 7 {
                return Err(ActionError::AllIdeasUnlocked {
                    group_id: *group_id,
                });
            }

            // Validate idea group exists and get category
            let group = state
                .idea_groups
                .get(*group_id)
                .ok_or(ActionError::InvalidIdeaGroup {
                    group_id: *group_id,
                })?;

            // Check mana cost (400 base per idea)
            const IDEA_COST: Fixed = Fixed::from_int(400);
            let (mana_type, current_mana) = match group.category {
                Some(IdeaCategory::Adm) => ("ADM", country.adm_mana),
                Some(IdeaCategory::Dip) => ("DIP", country.dip_mana),
                Some(IdeaCategory::Mil) => ("MIL", country.mil_mana),
                None => ("ADM", country.adm_mana), // Fallback to ADM
            };

            if current_mana < IDEA_COST {
                return Err(ActionError::InsufficientMana);
            }

            // Spend mana and unlock idea
            let country =
                state
                    .countries
                    .get_mut(country_tag)
                    .ok_or(ActionError::CountryNotFound {
                        tag: country_tag.to_string(),
                    })?;

            match group.category {
                Some(IdeaCategory::Adm) => country.adm_mana -= IDEA_COST,
                Some(IdeaCategory::Dip) => country.dip_mana -= IDEA_COST,
                Some(IdeaCategory::Mil) => country.mil_mana -= IDEA_COST,
                None => country.adm_mana -= IDEA_COST,
            }

            country.ideas.groups.insert(*group_id, ideas_unlocked + 1);

            // Sync national idea progress: unlocks based on total ideas from generic groups
            let total_generic_ideas: u8 = country.ideas.groups.values().copied().sum();
            if country.ideas.national_ideas.is_some() {
                country.ideas.national_ideas_progress = total_generic_ideas.min(7);
            }

            log::info!(
                "{} unlocked idea {}/{} in {} (cost 400 {}, national progress: {}/7)",
                country_tag,
                ideas_unlocked + 1,
                7,
                group.name,
                mana_type,
                country.ideas.national_ideas_progress
            );

            Ok(())
        }
        Command::GrantPrivilege {
            estate_id,
            privilege_id,
        } => {
            let country =
                state
                    .countries
                    .get_mut(country_tag)
                    .ok_or(ActionError::CountryNotFound {
                        tag: country_tag.to_string(),
                    })?;

            crate::systems::grant_privilege(country, *estate_id, *privilege_id, &state.estates)
                .map_err(|e| ActionError::InvalidCommand {
                    message: format!("Failed to grant privilege: {:?}", e),
                })?;

            Ok(())
        }
        Command::RevokePrivilege {
            estate_id,
            privilege_id,
        } => {
            let country =
                state
                    .countries
                    .get_mut(country_tag)
                    .ok_or(ActionError::CountryNotFound {
                        tag: country_tag.to_string(),
                    })?;

            crate::systems::revoke_privilege(country, *estate_id, *privilege_id, &state.estates)
                .map_err(|e| ActionError::InvalidCommand {
                    message: format!("Failed to revoke privilege: {:?}", e),
                })?;

            Ok(())
        }
        Command::SeizeLand { percentage } => {
            let country =
                state
                    .countries
                    .get_mut(country_tag)
                    .ok_or(ActionError::CountryNotFound {
                        tag: country_tag.to_string(),
                    })?;

            crate::systems::seize_land(country, *percentage).map_err(|e| {
                ActionError::InvalidCommand {
                    message: format!("Failed to seize land: {:?}", e),
                }
            })?;

            Ok(())
        }
        Command::SaleLand {
            estate_id,
            percentage,
        } => {
            let country =
                state
                    .countries
                    .get_mut(country_tag)
                    .ok_or(ActionError::CountryNotFound {
                        tag: country_tag.to_string(),
                    })?;

            crate::systems::sale_land(country, *estate_id, *percentage).map_err(|e| {
                ActionError::InvalidCommand {
                    message: format!("Failed to sell land: {:?}", e),
                }
            })?;

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

            log::debug!(
                "[OFFER_DEBUG] {} offering peace in {}: cost={}, available={}",
                country_tag,
                war.name,
                war_score_cost,
                available_score
            );

            if war_score_cost > available_score {
                log::warn!(
                    "[OFFER_FAIL] {} can't afford peace in {}: cost={} > available={}",
                    country_tag,
                    war.name,
                    war_score_cost,
                    available_score
                );
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

            // Clear peace offer cooldowns and pending call-to-arms for all participants
            for tag in war.attackers.iter().chain(war.defenders.iter()) {
                if let Some(country) = state.countries.get_mut(tag) {
                    country.peace_offer_cooldowns.remove(war_id);
                    country.pending_call_to_arms.remove(war_id);
                }
            }

            // Clear pending call-to-arms for all countries (in case non-participants had pending calls)
            for (_tag, country) in state.countries.iter_mut() {
                country.pending_call_to_arms.remove(war_id);
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
        Command::JoinWar { war_id, side } => {
            // Validate war exists
            if !state.diplomacy.wars.contains_key(war_id) {
                return Err(ActionError::WarNotFound { war_id: *war_id });
            }

            // Check if country has a pending call-to-arms for this war
            let has_pending = state
                .countries
                .get(country_tag)
                .and_then(|c| c.pending_call_to_arms.get(war_id))
                .is_some();

            if !has_pending {
                // Can only join if you have a pending call
                return Ok(()); // Silently ignore (not an error, just invalid action)
            }

            // Join the war
            join_war(state, country_tag, *war_id, *side);
            log::info!("{} joined war {} as {:?}", country_tag, war_id, side);

            Ok(())
        }
        Command::CallAllyToWar { ally, war_id } => {
            // Validate war exists
            if !state.diplomacy.wars.contains_key(war_id) {
                return Err(ActionError::WarNotFound { war_id: *war_id });
            }

            // Check if caller is in the war
            let war = state.diplomacy.wars.get(war_id).unwrap();
            let is_attacker = war.attackers.contains(&country_tag.to_string());
            let is_defender = war.defenders.contains(&country_tag.to_string());

            if !is_attacker && !is_defender {
                return Err(ActionError::NotWarParticipant {
                    tag: country_tag.to_string(),
                    war_id: *war_id,
                });
            }

            // Check if ally has an alliance
            use crate::state::RelationType;
            let has_alliance = state.diplomacy.relations.iter().any(|((a, b), rel)| {
                *rel == RelationType::Alliance
                    && ((a == country_tag && b == ally) || (b == country_tag && a == ally))
            });

            if !has_alliance {
                return Ok(()); // Silently ignore - no alliance
            }

            // Create pending call-to-arms for the ally
            if let Some(ally_country) = state.countries.get_mut(ally) {
                let side = if is_attacker {
                    crate::input::WarSide::Attacker
                } else {
                    crate::input::WarSide::Defender
                };
                ally_country.pending_call_to_arms.insert(*war_id, side);
                log::info!(
                    "{} called ally {} to join war {} as {:?}",
                    country_tag,
                    ally,
                    war_id,
                    side
                );
            }

            Ok(())
        }

        // ===== STUB COMMANDS (Phase 2+) =====
        // These commands are defined but not yet implemented.
        // They log a warning and return Ok(()) to allow graceful degradation.
        Command::MergeArmies { army_ids } => {
            // Validation: need at least 2 armies to merge
            if army_ids.len() < 2 {
                return Err(ActionError::InvalidCommand {
                    message: "MergeArmies requires at least 2 armies".to_string(),
                });
            }

            // Validate all armies exist, same owner, same location, not in battle
            let mut location: Option<ProvinceId> = None;
            for &army_id in army_ids {
                let army = state
                    .armies
                    .get(&army_id)
                    .ok_or(ActionError::InvalidCommand {
                        message: format!("Army {} does not exist", army_id),
                    })?;

                if army.owner != country_tag {
                    return Err(ActionError::InvalidCommand {
                        message: format!("Army {} is not owned by {}", army_id, country_tag),
                    });
                }

                if army.in_battle.is_some() {
                    return Err(ActionError::InvalidCommand {
                        message: format!("Army {} is in battle and cannot be merged", army_id),
                    });
                }

                match location {
                    None => location = Some(army.location),
                    Some(loc) if loc != army.location => {
                        return Err(ActionError::InvalidCommand {
                            message: "All armies must be in the same province to merge".to_string(),
                        });
                    }
                    _ => {}
                }
            }

            // Find the best general among all merging armies (highest total pips)
            let best_general: Option<GeneralId> = army_ids
                .iter()
                .filter_map(|&id| state.armies.get(&id)?.general)
                .filter_map(|gid| state.generals.get(&gid).map(|g| (gid, g)))
                .max_by_key(|(_, g)| {
                    g.fire as u16 + g.shock as u16 + g.maneuver as u16 + g.siege as u16
                })
                .map(|(gid, _)| gid);

            // Collect all regiments from armies to be merged (excluding the target)
            let target_id = army_ids[0];
            let mut all_regiments: Vec<Regiment> = Vec::new();

            for &army_id in &army_ids[1..] {
                if let Some(army) = state.armies.get(&army_id) {
                    all_regiments.extend(army.regiments.clone());
                }
            }

            // Remove merged-from armies (all except target)
            for &army_id in &army_ids[1..] {
                state.armies.remove(&army_id);
            }

            // Update target army: add regiments, assign best general
            if let Some(target) = state.armies.get_mut(&target_id) {
                target.regiments.extend(all_regiments);
                target.general = best_general;
                target.recompute_counts();

                log::info!(
                    "{} merged {} armies into army {} ({} regiments total)",
                    country_tag,
                    army_ids.len(),
                    target_id,
                    target.regiment_count()
                );
            }

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
                    if !p.is_sea && !p.is_wasteland {
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
        Command::OfferAlliance { target } => {
            // One diplomatic action per day - check if already acted today
            if let Some(country) = state.countries.get(country_tag) {
                if country.last_diplomatic_action == Some(state.date) {
                    return Err(ActionError::DiplomaticActionCooldown);
                }
            }

            // Validate both countries exist
            if !state.countries.contains_key(country_tag) {
                return Err(ActionError::CountryNotFound {
                    tag: country_tag.to_string(),
                });
            }
            if !state.countries.contains_key(target) {
                return Err(ActionError::CountryNotFound {
                    tag: target.clone(),
                });
            }

            // Cannot ally self
            if country_tag == target {
                return Err(ActionError::InvalidAction {
                    reason: "Cannot ally yourself".to_string(),
                });
            }

            // Cannot ally if at war
            if state.diplomacy.are_at_war(country_tag, target) {
                return Err(ActionError::InvalidAction {
                    reason: "Cannot ally during war".to_string(),
                });
            }

            // Check if already allied
            let key = DiplomacyState::sorted_pair(country_tag, target);
            if state.diplomacy.relations.get(&key) == Some(&RelationType::Alliance) {
                return Ok(()); // Silently succeed
            }

            // Check for mutual offers (auto-accept)
            let reverse_key = (target.clone(), country_tag.to_string());
            if state
                .diplomacy
                .pending_alliance_offers
                .contains_key(&reverse_key)
            {
                // Both want alliance - auto-accept
                state.diplomacy.pending_alliance_offers.remove(&reverse_key);
                state
                    .diplomacy
                    .relations
                    .insert(key.clone(), RelationType::Alliance);

                // Alliance breaks rivalry (both directions)
                if let Some(country) = state.countries.get_mut(country_tag) {
                    country.rivals.remove(target);
                    country.last_diplomatic_action = Some(state.date);
                }
                if let Some(target_country) = state.countries.get_mut(target) {
                    target_country.rivals.remove(country_tag);
                }

                log::info!(
                    "{} auto-accepted {}'s alliance offer (mutual)",
                    country_tag,
                    target
                );
                return Ok(());
            }

            // Create pending offer
            let offer_key = (country_tag.to_string(), target.clone());
            state
                .diplomacy
                .pending_alliance_offers
                .insert(offer_key, state.date);

            if let Some(country) = state.countries.get_mut(country_tag) {
                country.last_diplomatic_action = Some(state.date);
            }

            log::info!("{} offered alliance to {}", country_tag, target);
            Ok(())
        }
        Command::BreakAlliance { target } => {
            // One diplomatic action per day - check if already acted today
            if let Some(country) = state.countries.get(country_tag) {
                if country.last_diplomatic_action == Some(state.date) {
                    return Err(ActionError::DiplomaticActionCooldown);
                }
            }

            // Validate both countries exist
            if !state.countries.contains_key(country_tag) {
                return Err(ActionError::CountryNotFound {
                    tag: country_tag.to_string(),
                });
            }
            if !state.countries.contains_key(target) {
                return Err(ActionError::CountryNotFound {
                    tag: target.clone(),
                });
            }

            // Check if actually allied
            let key = DiplomacyState::sorted_pair(country_tag, target);
            if state.diplomacy.relations.get(&key) != Some(&RelationType::Alliance) {
                return Ok(()); // Silently succeed if not allied
            }

            // Remove alliance
            state.diplomacy.relations.remove(&key);

            // Apply prestige penalty (-25 prestige per break)
            if let Some(country) = state.countries.get_mut(country_tag) {
                country.prestige.add(Fixed::from_int(-25));
                country.last_diplomatic_action = Some(state.date);
            }

            log::info!(
                "{} broke alliance with {} (-25 prestige)",
                country_tag,
                target
            );
            Ok(())
        }
        Command::OfferRoyalMarriage { target } => {
            // One diplomatic action per day - check if already acted today
            if let Some(country) = state.countries.get(country_tag) {
                if country.last_diplomatic_action == Some(state.date) {
                    return Err(ActionError::DiplomaticActionCooldown);
                }
            }

            // Validate both countries exist
            if !state.countries.contains_key(country_tag) {
                return Err(ActionError::CountryNotFound {
                    tag: country_tag.to_string(),
                });
            }
            if !state.countries.contains_key(target) {
                return Err(ActionError::CountryNotFound {
                    tag: target.clone(),
                });
            }

            // Cannot marry self
            if country_tag == target {
                return Err(ActionError::InvalidAction {
                    reason: "Cannot marry yourself".to_string(),
                });
            }

            // Cannot marry if at war
            if state.diplomacy.are_at_war(country_tag, target) {
                return Err(ActionError::InvalidAction {
                    reason: "Cannot marry during war".to_string(),
                });
            }

            // Check if already married
            let key = DiplomacyState::sorted_pair(country_tag, target);
            if state.diplomacy.relations.get(&key) == Some(&RelationType::RoyalMarriage) {
                return Ok(()); // Silently succeed
            }

            // Check for mutual offers (auto-accept)
            let reverse_key = (target.clone(), country_tag.to_string());
            if state
                .diplomacy
                .pending_marriage_offers
                .contains_key(&reverse_key)
            {
                // Both want marriage - auto-accept
                state.diplomacy.pending_marriage_offers.remove(&reverse_key);
                state
                    .diplomacy
                    .relations
                    .insert(key.clone(), RelationType::RoyalMarriage);

                if let Some(country) = state.countries.get_mut(country_tag) {
                    country.last_diplomatic_action = Some(state.date);
                }

                log::info!(
                    "{} auto-accepted {}'s royal marriage offer (mutual)",
                    country_tag,
                    target
                );
                return Ok(());
            }

            // Create pending offer
            let offer_key = (country_tag.to_string(), target.clone());
            state
                .diplomacy
                .pending_marriage_offers
                .insert(offer_key, state.date);

            if let Some(country) = state.countries.get_mut(country_tag) {
                country.last_diplomatic_action = Some(state.date);
            }

            log::info!("{} offered royal marriage to {}", country_tag, target);
            Ok(())
        }
        Command::BreakRoyalMarriage { target } => {
            // One diplomatic action per day - check if already acted today
            if let Some(country) = state.countries.get(country_tag) {
                if country.last_diplomatic_action == Some(state.date) {
                    return Err(ActionError::DiplomaticActionCooldown);
                }
            }

            // Validate both countries exist
            if !state.countries.contains_key(country_tag) {
                return Err(ActionError::CountryNotFound {
                    tag: country_tag.to_string(),
                });
            }
            if !state.countries.contains_key(target) {
                return Err(ActionError::CountryNotFound {
                    tag: target.clone(),
                });
            }

            // Check if actually married
            let key = DiplomacyState::sorted_pair(country_tag, target);
            if state.diplomacy.relations.get(&key) != Some(&RelationType::RoyalMarriage) {
                return Ok(()); // Silently succeed if not married
            }

            // Remove royal marriage (no prestige penalty, unlike breaking alliances)
            state.diplomacy.relations.remove(&key);

            if let Some(country) = state.countries.get_mut(country_tag) {
                country.last_diplomatic_action = Some(state.date);
            }

            log::info!("{} broke royal marriage with {}", country_tag, target);
            Ok(())
        }
        Command::RequestMilitaryAccess { target } => {
            // One diplomatic action per day - check if already acted today
            if let Some(country) = state.countries.get(country_tag) {
                if country.last_diplomatic_action == Some(state.date) {
                    return Err(ActionError::DiplomaticActionCooldown);
                }
            }

            // Validate both countries exist
            if !state.countries.contains_key(country_tag) {
                return Err(ActionError::CountryNotFound {
                    tag: country_tag.to_string(),
                });
            }
            if !state.countries.contains_key(target) {
                return Err(ActionError::CountryNotFound {
                    tag: target.clone(),
                });
            }

            // Cannot request from self
            if country_tag == target {
                return Err(ActionError::InvalidAction {
                    reason: "Cannot request military access from yourself".to_string(),
                });
            }

            // Cannot request if at war
            if state.diplomacy.are_at_war(country_tag, target) {
                return Err(ActionError::InvalidAction {
                    reason: "Cannot request military access during war".to_string(),
                });
            }

            // Check if already has access
            let access_key = (target.clone(), country_tag.to_string());
            if state.diplomacy.military_access.contains_key(&access_key) {
                return Ok(()); // Silently succeed
            }

            // Create pending request
            let request_key = (country_tag.to_string(), target.clone());
            state
                .diplomacy
                .pending_access_requests
                .insert(request_key, state.date);

            if let Some(country) = state.countries.get_mut(country_tag) {
                country.last_diplomatic_action = Some(state.date);
            }

            log::info!("{} requested military access from {}", country_tag, target);
            Ok(())
        }
        Command::CancelMilitaryAccess { target } => {
            // One diplomatic action per day - check if already acted today
            if let Some(country) = state.countries.get(country_tag) {
                if country.last_diplomatic_action == Some(state.date) {
                    return Err(ActionError::DiplomaticActionCooldown);
                }
            }

            // Validate both countries exist
            if !state.countries.contains_key(country_tag) {
                return Err(ActionError::CountryNotFound {
                    tag: country_tag.to_string(),
                });
            }
            if !state.countries.contains_key(target) {
                return Err(ActionError::CountryNotFound {
                    tag: target.clone(),
                });
            }

            // Check if actually has access (country_tag is the granter, target is the one with access)
            let access_key = (country_tag.to_string(), target.clone());
            if !state.diplomacy.military_access.contains_key(&access_key) {
                return Ok(()); // Silently succeed if no access granted
            }

            // Remove military access
            state.diplomacy.military_access.remove(&access_key);

            if let Some(country) = state.countries.get_mut(country_tag) {
                country.last_diplomatic_action = Some(state.date);
            }

            log::info!("{} cancelled military access for {}", country_tag, target);
            Ok(())
        }
        Command::SetRival { target } => {
            // One diplomatic action per day - check if already acted today
            if let Some(country) = state.countries.get(country_tag) {
                if country.last_diplomatic_action == Some(state.date) {
                    return Err(ActionError::DiplomaticActionCooldown);
                }
            }

            // Validate actor exists
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

            // Cannot rival self
            if country_tag == target {
                return Err(ActionError::InvalidAction {
                    reason: "Cannot rival yourself".to_string(),
                });
            }

            // Check max 3 rivals limit
            let current_rivals = state
                .countries
                .get(country_tag)
                .map(|c| c.rivals.len())
                .unwrap_or(0);
            if current_rivals >= 3 {
                return Err(ActionError::InvalidAction {
                    reason: "Already have 3 rivals (maximum)".to_string(),
                });
            }

            // Cannot rival an ally
            let key = DiplomacyState::sorted_pair(country_tag, target);
            if state.diplomacy.relations.get(&key) == Some(&RelationType::Alliance) {
                return Err(ActionError::InvalidAction {
                    reason: "Cannot rival an ally".to_string(),
                });
            }

            // Check if already rivals (silently succeed)
            if state
                .countries
                .get(country_tag)
                .is_some_and(|c| c.rivals.contains(target))
            {
                return Ok(());
            }

            // Mutate state
            if let Some(country) = state.countries.get_mut(country_tag) {
                country.rivals.insert(target.clone());
                country.last_diplomatic_action = Some(state.date);
            }

            log::info!("{} set {} as rival", country_tag, target);
            Ok(())
        }
        Command::RemoveRival { target } => {
            // One diplomatic action per day - check if already acted today
            if let Some(country) = state.countries.get(country_tag) {
                if country.last_diplomatic_action == Some(state.date) {
                    return Err(ActionError::DiplomaticActionCooldown);
                }
            }

            // Validate actor exists
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

            // Cannot de-rival self (though you can't rival yourself anyway)
            if country_tag == target {
                return Err(ActionError::InvalidAction {
                    reason: "Cannot remove rivalry with yourself".to_string(),
                });
            }

            // Check if actually rivals (silently succeed if not)
            if !state
                .countries
                .get(country_tag)
                .is_some_and(|c| c.rivals.contains(target))
            {
                return Ok(());
            }

            // Mutate state
            if let Some(country) = state.countries.get_mut(country_tag) {
                country.rivals.remove(target);
                country.last_diplomatic_action = Some(state.date);
            }

            log::info!("{} removed {} as rival", country_tag, target);
            Ok(())
        }
        Command::AcceptAlliance { from } => {
            // NO cooldown for responses (can accept multiple in one day)

            // Validate both countries exist
            if !state.countries.contains_key(country_tag) {
                return Err(ActionError::CountryNotFound {
                    tag: country_tag.to_string(),
                });
            }
            if !state.countries.contains_key(from) {
                return Err(ActionError::CountryNotFound { tag: from.clone() });
            }

            // Validate offer exists
            let offer_key = (from.clone(), country_tag.to_string());
            if !state
                .diplomacy
                .pending_alliance_offers
                .contains_key(&offer_key)
            {
                return Err(ActionError::InvalidAction {
                    reason: format!("No alliance offer from {}", from),
                });
            }

            // Remove offer, create alliance
            state.diplomacy.pending_alliance_offers.remove(&offer_key);
            let key = DiplomacyState::sorted_pair(country_tag, from);
            state
                .diplomacy
                .relations
                .insert(key, RelationType::Alliance);

            // Alliance breaks rivalry (both directions)
            if let Some(country) = state.countries.get_mut(country_tag) {
                country.rivals.remove(from);
            }
            if let Some(from_country) = state.countries.get_mut(from) {
                from_country.rivals.remove(country_tag);
            }

            log::info!("{} accepted alliance offer from {}", country_tag, from);
            Ok(())
        }
        Command::RejectAlliance { from } => {
            // NO cooldown for responses (can reject multiple in one day)

            // Validate both countries exist
            if !state.countries.contains_key(country_tag) {
                return Err(ActionError::CountryNotFound {
                    tag: country_tag.to_string(),
                });
            }
            if !state.countries.contains_key(from) {
                return Err(ActionError::CountryNotFound { tag: from.clone() });
            }

            // Validate offer exists
            let offer_key = (from.clone(), country_tag.to_string());
            if !state
                .diplomacy
                .pending_alliance_offers
                .contains_key(&offer_key)
            {
                return Ok(()); // Silently succeed if no offer
            }

            // Remove offer
            state.diplomacy.pending_alliance_offers.remove(&offer_key);

            log::info!("{} rejected alliance offer from {}", country_tag, from);
            Ok(())
        }
        Command::AcceptRoyalMarriage { from } => {
            // NO cooldown for responses (can accept multiple in one day)

            // Validate both countries exist
            if !state.countries.contains_key(country_tag) {
                return Err(ActionError::CountryNotFound {
                    tag: country_tag.to_string(),
                });
            }
            if !state.countries.contains_key(from) {
                return Err(ActionError::CountryNotFound { tag: from.clone() });
            }

            // Validate offer exists
            let offer_key = (from.clone(), country_tag.to_string());
            if !state
                .diplomacy
                .pending_marriage_offers
                .contains_key(&offer_key)
            {
                return Err(ActionError::InvalidAction {
                    reason: format!("No royal marriage offer from {}", from),
                });
            }

            // Remove offer, create royal marriage
            state.diplomacy.pending_marriage_offers.remove(&offer_key);
            let key = DiplomacyState::sorted_pair(country_tag, from);
            state
                .diplomacy
                .relations
                .insert(key, RelationType::RoyalMarriage);

            log::info!(
                "{} accepted royal marriage offer from {}",
                country_tag,
                from
            );
            Ok(())
        }
        Command::RejectRoyalMarriage { from } => {
            // NO cooldown for responses (can reject multiple in one day)

            // Validate both countries exist
            if !state.countries.contains_key(country_tag) {
                return Err(ActionError::CountryNotFound {
                    tag: country_tag.to_string(),
                });
            }
            if !state.countries.contains_key(from) {
                return Err(ActionError::CountryNotFound { tag: from.clone() });
            }

            // Validate offer exists
            let offer_key = (from.clone(), country_tag.to_string());
            if !state
                .diplomacy
                .pending_marriage_offers
                .contains_key(&offer_key)
            {
                return Ok(()); // Silently succeed if no offer
            }

            // Remove offer
            state.diplomacy.pending_marriage_offers.remove(&offer_key);

            log::info!(
                "{} rejected royal marriage offer from {}",
                country_tag,
                from
            );
            Ok(())
        }
        Command::GrantMilitaryAccess { to } => {
            // NO cooldown for responses (can grant multiple in one day)

            // Validate both countries exist
            if !state.countries.contains_key(country_tag) {
                return Err(ActionError::CountryNotFound {
                    tag: country_tag.to_string(),
                });
            }
            if !state.countries.contains_key(to) {
                return Err(ActionError::CountryNotFound { tag: to.clone() });
            }

            // Validate request exists
            let request_key = (to.clone(), country_tag.to_string());
            if !state
                .diplomacy
                .pending_access_requests
                .contains_key(&request_key)
            {
                return Err(ActionError::InvalidAction {
                    reason: format!("No military access request from {}", to),
                });
            }

            // Remove request, grant access
            state.diplomacy.pending_access_requests.remove(&request_key);

            // Military access: (granter, requester) -> true
            let access_key = (country_tag.to_string(), to.clone());
            state.diplomacy.military_access.insert(access_key, true);

            log::info!("{} granted military access to {}", country_tag, to);
            Ok(())
        }
        Command::DenyMilitaryAccess { to } => {
            // NO cooldown for responses (can deny multiple in one day)

            // Validate both countries exist
            if !state.countries.contains_key(country_tag) {
                return Err(ActionError::CountryNotFound {
                    tag: country_tag.to_string(),
                });
            }
            if !state.countries.contains_key(to) {
                return Err(ActionError::CountryNotFound { tag: to.clone() });
            }

            // Validate request exists
            let request_key = (to.clone(), country_tag.to_string());
            if !state
                .diplomacy
                .pending_access_requests
                .contains_key(&request_key)
            {
                return Ok(()); // Silently succeed if no request
            }

            // Remove request
            state.diplomacy.pending_access_requests.remove(&request_key);

            log::info!("{} denied military access to {}", country_tag, to);
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

        // Holy Roman Empire commands
        Command::AddProvinceToHRE { province } => {
            // Only emperor can add provinces to HRE
            if state.global.hre.emperor.as_deref() != Some(country_tag) {
                return Err(ActionError::InvalidAction {
                    reason: "Only the emperor can add provinces to the HRE".to_string(),
                });
            }
            let Some(prov) = state.provinces.get_mut(province) else {
                return Err(ActionError::InvalidProvinceId);
            };
            if prov.is_in_hre {
                return Err(ActionError::InvalidAction {
                    reason: format!("Province {} is already in the HRE", province),
                });
            }
            prov.is_in_hre = true;
            log::info!(
                "Province {} added to HRE by emperor {}",
                province,
                country_tag
            );
            Ok(())
        }
        Command::RemoveProvinceFromHRE { province } => {
            // Emperor can remove provinces, or owner can remove their own
            let Some(prov) = state.provinces.get_mut(province) else {
                return Err(ActionError::InvalidProvinceId);
            };
            let is_emperor = state.global.hre.emperor.as_deref() == Some(country_tag);
            let is_owner = prov.owner.as_deref() == Some(country_tag);
            if !is_emperor && !is_owner {
                return Err(ActionError::InvalidAction {
                    reason: "Only emperor or province owner can remove province from HRE"
                        .to_string(),
                });
            }
            if !prov.is_in_hre {
                return Err(ActionError::InvalidAction {
                    reason: format!("Province {} is not in the HRE", province),
                });
            }
            prov.is_in_hre = false;
            log::info!("Province {} removed from HRE by {}", province, country_tag);
            Ok(())
        }
        Command::JoinHRE => {
            // Find country's capital
            let capital = state
                .provinces
                .iter()
                .find(|(_, p)| p.owner.as_deref() == Some(country_tag) && p.is_capital)
                .map(|(id, _)| *id);
            let Some(capital_id) = capital else {
                return Err(ActionError::InvalidAction {
                    reason: format!("{} has no capital province", country_tag),
                });
            };
            // Check not already member
            if state
                .global
                .hre
                .is_member(&country_tag.to_string(), &state.provinces)
            {
                return Err(ActionError::InvalidAction {
                    reason: format!("{} is already an HRE member", country_tag),
                });
            }
            // Add capital to HRE
            if let Some(prov) = state.provinces.get_mut(&capital_id) {
                prov.is_in_hre = true;
            }
            log::info!("{} joins the Holy Roman Empire", country_tag);
            Ok(())
        }
        Command::LeaveHRE => {
            // Check is member
            if !state
                .global
                .hre
                .is_member(&country_tag.to_string(), &state.provinces)
            {
                return Err(ActionError::InvalidAction {
                    reason: format!("{} is not an HRE member", country_tag),
                });
            }
            // Find capital and remove from HRE
            let capital = state
                .provinces
                .iter()
                .find(|(_, p)| p.owner.as_deref() == Some(country_tag) && p.is_capital)
                .map(|(id, _)| *id);
            if let Some(capital_id) = capital {
                if let Some(prov) = state.provinces.get_mut(&capital_id) {
                    prov.is_in_hre = false;
                }
            }
            // Remove from electors if was elector
            state.global.hre.electors.retain(|e| e != country_tag);
            // Remove from free cities if was one
            state.global.hre.free_cities.remove(country_tag);
            log::info!("{} leaves the Holy Roman Empire", country_tag);
            Ok(())
        }
        Command::GrantElectorate { target } => {
            // Only emperor can grant electorates
            if state.global.hre.emperor.as_deref() != Some(country_tag) {
                return Err(ActionError::InvalidAction {
                    reason: "Only the emperor can grant electorates".to_string(),
                });
            }
            // Check max electors
            if state.global.hre.electors.len() >= crate::systems::hre::defines::MAX_ELECTORS {
                return Err(ActionError::InvalidAction {
                    reason: format!(
                        "Maximum number of electors ({}) reached",
                        crate::systems::hre::defines::MAX_ELECTORS
                    ),
                });
            }
            // Target must be HRE member
            if !state.global.hre.is_member(target, &state.provinces) {
                return Err(ActionError::InvalidAction {
                    reason: format!("{} is not an HRE member", target),
                });
            }
            // Can't be free city
            if state.global.hre.is_free_city(target) {
                return Err(ActionError::InvalidAction {
                    reason: format!("{} is a Free City and cannot be an elector", target),
                });
            }
            // Check not already elector
            if state.global.hre.is_elector(target) {
                return Err(ActionError::InvalidAction {
                    reason: format!("{} is already an elector", target),
                });
            }
            state.global.hre.electors.push(target.clone());
            log::info!("{} grants electorate to {}", country_tag, target);
            Ok(())
        }
        Command::RemoveElectorate { target } => {
            // Only emperor can remove electorates
            if state.global.hre.emperor.as_deref() != Some(country_tag) {
                return Err(ActionError::InvalidAction {
                    reason: "Only the emperor can remove electorates".to_string(),
                });
            }
            if !state.global.hre.is_elector(target) {
                return Err(ActionError::InvalidAction {
                    reason: format!("{} is not an elector", target),
                });
            }
            state.global.hre.electors.retain(|e| e != target);
            log::info!("{} removes electorate from {}", country_tag, target);
            Ok(())
        }
        Command::GrantFreeCity { target } => {
            // Only emperor can grant free city status
            if state.global.hre.emperor.as_deref() != Some(country_tag) {
                return Err(ActionError::InvalidAction {
                    reason: "Only the emperor can grant Free City status".to_string(),
                });
            }
            // Check max free cities
            if state.global.hre.free_cities.len() >= crate::systems::hre::defines::MAX_FREE_CITIES {
                return Err(ActionError::InvalidAction {
                    reason: format!(
                        "Maximum number of Free Cities ({}) reached",
                        crate::systems::hre::defines::MAX_FREE_CITIES
                    ),
                });
            }
            // Target must be HRE member
            if !state.global.hre.is_member(target, &state.provinces) {
                return Err(ActionError::InvalidAction {
                    reason: format!("{} is not an HRE member", target),
                });
            }
            // Must be OPM (one province minor)
            let province_count = state
                .provinces
                .values()
                .filter(|p| p.owner.as_deref() == Some(target))
                .count();
            if province_count != 1 {
                return Err(ActionError::InvalidAction {
                    reason: format!(
                        "{} has {} provinces, must be OPM to be Free City",
                        target, province_count
                    ),
                });
            }
            // Can't be an elector
            if state.global.hre.is_elector(target) {
                return Err(ActionError::InvalidAction {
                    reason: format!("{} is an elector and cannot be a Free City", target),
                });
            }
            // Check not already free city
            if state.global.hre.is_free_city(target) {
                return Err(ActionError::InvalidAction {
                    reason: format!("{} is already a Free City", target),
                });
            }
            state.global.hre.free_cities.insert(target.clone());
            log::info!("{} grants Free City status to {}", country_tag, target);
            Ok(())
        }
        Command::RevokeFreeCity { target } => {
            // Only emperor can revoke free city status
            if state.global.hre.emperor.as_deref() != Some(country_tag) {
                return Err(ActionError::InvalidAction {
                    reason: "Only the emperor can revoke Free City status".to_string(),
                });
            }
            if !state.global.hre.is_free_city(target) {
                return Err(ActionError::InvalidAction {
                    reason: format!("{} is not a Free City", target),
                });
            }
            state.global.hre.free_cities.remove(target);
            log::info!("{} revokes Free City status from {}", country_tag, target);
            Ok(())
        }
        Command::PassImperialReform { reform } => {
            // Only emperor can pass reforms
            if state.global.hre.emperor.as_deref() != Some(country_tag) {
                return Err(ActionError::InvalidAction {
                    reason: "Only the emperor can pass imperial reforms".to_string(),
                });
            }
            // Check sufficient IA (50 required)
            let ia_cost = crate::systems::hre::defines::REFORM_IA_COST;
            if state.global.hre.imperial_authority < ia_cost {
                return Err(ActionError::InvalidAction {
                    reason: format!(
                        "Insufficient Imperial Authority: have {:.1}, need {}",
                        state.global.hre.imperial_authority.to_f32(),
                        ia_cost.to_f32()
                    ),
                });
            }
            // Check reform not already passed
            if state.global.hre.reforms_passed.contains(reform) {
                return Err(ActionError::InvalidAction {
                    reason: format!("Reform {:?} already passed", reform),
                });
            }
            // TODO: Check elector majority approval (when reform registry exists)
            // Deduct IA and add reform
            state.global.hre.imperial_authority -= ia_cost;
            state.global.hre.reforms_passed.push(*reform);
            log::info!(
                "Emperor {} passes reform {:?} (IA: {:.1} -> {:.1})",
                country_tag,
                reform,
                (state.global.hre.imperial_authority + ia_cost).to_f32(),
                state.global.hre.imperial_authority.to_f32()
            );

            // Special handling: Revoke Privilegia vassalizes all HRE members
            if *reform == crate::systems::hre::reforms::REVOKE_PRIVILEGIA {
                let emperor = country_tag.to_string();
                let members = state.global.hre.get_members(&state.provinces);
                let vassal_id = state.subject_types.vassal_id;
                let date = state.date;

                for member in members {
                    // Skip the emperor itself
                    if member == emperor {
                        continue;
                    }
                    // Skip if already someone's subject
                    if state.diplomacy.subjects.contains_key(&member) {
                        continue;
                    }
                    // Vassalize the member
                    if let Err(e) = state
                        .diplomacy
                        .add_subject(&emperor, &member, vassal_id, date)
                    {
                        log::warn!(
                            "Failed to vassalize {} under Revoke Privilegia: {}",
                            member,
                            e
                        );
                    } else {
                        log::info!(
                            "{} becomes vassal of {} via Revoke Privilegia",
                            member,
                            emperor
                        );
                    }
                }
            }

            Ok(())
        }
        Command::ImperialBan { target } => {
            // Only emperor can issue bans
            if state.global.hre.emperor.as_deref() != Some(country_tag) {
                return Err(ActionError::InvalidAction {
                    reason: "Only the emperor can issue imperial bans".to_string(),
                });
            }
            // Target must be HRE member
            if !state.global.hre.is_member(target, &state.provinces) {
                return Err(ActionError::InvalidAction {
                    reason: format!("{} is not an HRE member, cannot be banned", target),
                });
            }
            // TODO: Unlock Imperial Ban CB against target
            log::info!("Emperor {} issues imperial ban on {}", country_tag, target);
            Ok(())
        }

        Command::Pass => Ok(()), // Explicit no-op

        Command::Quit => Ok(()), // Handled by outer loop usually, but harmless here

        // ===== CELESTIAL EMPIRE COMMANDS =====
        Command::TakeMandate => {
            // Transfer Mandate of Heaven to this country
            // In practice this would be done via peace deal, but we support direct command
            if state.global.celestial_empire.dismantled {
                return Err(ActionError::InvalidAction {
                    reason: "Celestial Empire has been dismantled".to_string(),
                });
            }

            // Can't take mandate if already emperor
            if state
                .global
                .celestial_empire
                .is_emperor(&country_tag.to_string())
            {
                return Err(ActionError::InvalidAction {
                    reason: "Already Emperor of China".to_string(),
                });
            }

            let old_emperor = state.global.celestial_empire.emperor.clone();

            // Transfer mandate
            state.global.celestial_empire.emperor = Some(country_tag.to_string());
            state.global.celestial_empire.mandate =
                crate::systems::celestial::defines::DEFAULT_MANDATE;
            state.global.celestial_empire.reforms_passed.clear();

            // Reset meritocracy for new emperor
            if let Some(country) = state.countries.get_mut(country_tag) {
                country.meritocracy.set(Fixed::ZERO);
            }

            log::info!(
                "{} takes the Mandate of Heaven from {:?}",
                country_tag,
                old_emperor
            );
            Ok(())
        }
        Command::PassCelestialReform { reform } => {
            // Must be Emperor of China
            if !state
                .global
                .celestial_empire
                .is_emperor(&country_tag.to_string())
            {
                return Err(ActionError::InvalidAction {
                    reason: "Only the Emperor of China can pass celestial reforms".to_string(),
                });
            }

            // Check if already passed
            if state.global.celestial_empire.has_reform(*reform) {
                return Err(ActionError::InvalidAction {
                    reason: "Reform already passed".to_string(),
                });
            }

            // Check prerequisites
            if !state.global.celestial_empire.can_pass_reform(*reform) {
                return Err(ActionError::InvalidAction {
                    reason: "Reform prerequisites not met".to_string(),
                });
            }

            // Check mandate requirement (80+)
            let min_mandate = crate::systems::celestial::defines::REFORM_MIN_MANDATE;
            if state.global.celestial_empire.mandate < min_mandate {
                return Err(ActionError::InvalidAction {
                    reason: format!(
                        "Need {} mandate to pass reform (have {:.1})",
                        min_mandate.to_f32(),
                        state.global.celestial_empire.mandate.to_f32()
                    ),
                });
            }

            // Check stability
            let country = state
                .countries
                .get(country_tag)
                .ok_or(ActionError::InvalidAction {
                    reason: "Country not found".to_string(),
                })?;
            if country.stability.get() < crate::systems::celestial::defines::REFORM_STABILITY_COST {
                return Err(ActionError::InvalidAction {
                    reason: "Need at least 1 stability to pass reform".to_string(),
                });
            }

            // Deduct costs
            let mandate_cost = crate::systems::celestial::defines::REFORM_MANDATE_COST;
            state.global.celestial_empire.mandate -= mandate_cost;
            let country = state.countries.get_mut(country_tag).unwrap();
            country.stability.set(
                country.stability.get() - crate::systems::celestial::defines::REFORM_STABILITY_COST,
            );

            // Pass reform
            state.global.celestial_empire.reforms_passed.insert(*reform);

            log::info!(
                "{} passes celestial reform {:?} (-70 mandate, -1 stability)",
                country_tag,
                reform
            );
            Ok(())
        }
        Command::IssueCelestialDecree { decree } => {
            // Must be Emperor of China
            if !state
                .global
                .celestial_empire
                .is_emperor(&country_tag.to_string())
            {
                return Err(ActionError::InvalidAction {
                    reason: "Only the Emperor of China can issue decrees".to_string(),
                });
            }

            // Check meritocracy cost
            let meritocracy_cost =
                Fixed::from_int(crate::systems::celestial::defines::DECREE_MERITOCRACY_COST as i64);
            let country = state
                .countries
                .get(country_tag)
                .ok_or(ActionError::InvalidAction {
                    reason: "Country not found".to_string(),
                })?;
            if country.meritocracy.get() < meritocracy_cost {
                return Err(ActionError::InsufficientFunds {
                    required: meritocracy_cost.to_f32(),
                    available: country.meritocracy.get().to_f32(),
                });
            }

            // Deduct meritocracy
            let country = state.countries.get_mut(country_tag).unwrap();
            let new_meritocracy = country.meritocracy.get() - meritocracy_cost;
            country.meritocracy.set(new_meritocracy);

            // TODO: Actually apply decree effects when decree system is implemented
            log::info!(
                "{} issues celestial decree '{}' (-20 meritocracy)",
                country_tag,
                decree
            );
            Ok(())
        }
        Command::ForceTributary { target } => {
            // This would normally be done via peace deal
            // For now, create the tributary relationship directly
            if !state.countries.contains_key(target) {
                return Err(ActionError::InvalidAction {
                    reason: format!("Target country {} not found", target),
                });
            }

            // Can't make yourself a tributary
            if country_tag == target {
                return Err(ActionError::InvalidAction {
                    reason: "Cannot make yourself a tributary".to_string(),
                });
            }

            // Check if target is already a subject
            if state.diplomacy.subjects.contains_key(target) {
                return Err(ActionError::InvalidAction {
                    reason: format!("{} is already a subject", target),
                });
            }

            // Create tributary relationship
            // Find the tributary subject type ID
            let tributary_type_id = state
                .subject_types
                .find_tributary_type()
                .unwrap_or(crate::subjects::SubjectTypeId(1));

            state.diplomacy.subjects.insert(
                target.to_string(),
                crate::state::SubjectRelationship {
                    overlord: country_tag.to_string(),
                    subject: target.to_string(),
                    subject_type: tributary_type_id,
                    start_date: state.date,
                    liberty_desire: 0,
                    integration_progress: 0,
                    integrating: false,
                },
            );

            log::info!("{} forces {} to become tributary", country_tag, target);
            Ok(())
        }
        Command::RequestTributary { target } => {
            // Diplomatic request - for now same as force but logged differently
            // In full implementation, this would create a diplomatic offer
            if !state.countries.contains_key(target) {
                return Err(ActionError::InvalidAction {
                    reason: format!("Target country {} not found", target),
                });
            }

            if country_tag == target {
                return Err(ActionError::InvalidAction {
                    reason: "Cannot request yourself as tributary".to_string(),
                });
            }

            if state.diplomacy.subjects.contains_key(target) {
                return Err(ActionError::InvalidAction {
                    reason: format!("{} is already a subject", target),
                });
            }

            // For now, auto-accept (in full impl would be an offer)
            let tributary_type_id = state
                .subject_types
                .find_tributary_type()
                .unwrap_or(crate::subjects::SubjectTypeId(1));

            state.diplomacy.subjects.insert(
                target.to_string(),
                crate::state::SubjectRelationship {
                    overlord: country_tag.to_string(),
                    subject: target.to_string(),
                    subject_type: tributary_type_id,
                    start_date: state.date,
                    liberty_desire: 0,
                    integration_progress: 0,
                    integrating: false,
                },
            );

            log::info!(
                "{} requests {} as tributary (auto-accepted)",
                country_tag,
                target
            );
            Ok(())
        }
        Command::RevokeTributary { target } => {
            // Release a tributary
            if let Some(relationship) = state.diplomacy.subjects.get(target) {
                if relationship.overlord != country_tag {
                    return Err(ActionError::InvalidAction {
                        reason: format!("{} is not your tributary", target),
                    });
                }
            } else {
                return Err(ActionError::InvalidAction {
                    reason: format!("{} is not a subject", target),
                });
            }

            state.diplomacy.subjects.remove(target);
            log::info!("{} releases tributary {}", country_tag, target);
            Ok(())
        }
        Command::StrengthenGovernment => {
            // Must be Emperor of China
            if !state
                .global
                .celestial_empire
                .is_emperor(&country_tag.to_string())
            {
                return Err(ActionError::InvalidAction {
                    reason: "Only the Emperor of China can strengthen government".to_string(),
                });
            }

            let mil_cost = crate::systems::celestial::defines::STRENGTHEN_GOVERNMENT_MIL_COST;
            let meritocracy_gain =
                crate::systems::celestial::defines::STRENGTHEN_GOVERNMENT_MERITOCRACY;

            // Check MIL power
            let country = state
                .countries
                .get(country_tag)
                .ok_or(ActionError::InvalidAction {
                    reason: "Country not found".to_string(),
                })?;
            if country.mil_mana < mil_cost {
                return Err(ActionError::InsufficientFunds {
                    required: mil_cost.to_f32(),
                    available: country.mil_mana.to_f32(),
                });
            }

            // Deduct MIL and add meritocracy
            let country = state.countries.get_mut(country_tag).unwrap();
            country.mil_mana -= mil_cost;
            let new_meritocracy = (country.meritocracy.get() + meritocracy_gain)
                .min(crate::systems::celestial::defines::MAX_MERITOCRACY);
            country.meritocracy.set(new_meritocracy);

            log::info!(
                "{} strengthens government: -100 MIL, +10 meritocracy (now {:.1})",
                country_tag,
                new_meritocracy.to_f32()
            );
            Ok(())
        }
        Command::AbandonMandate => {
            // Must be Emperor of China to abandon mandate
            if !state
                .global
                .celestial_empire
                .is_emperor(&country_tag.to_string())
            {
                return Err(ActionError::InvalidAction {
                    reason: "Only the Emperor of China can abandon the mandate".to_string(),
                });
            }

            // Clear emperor
            state.global.celestial_empire.emperor = None;
            state.global.celestial_empire.mandate = Fixed::ZERO;
            state.global.celestial_empire.reforms_passed.clear();

            // Reset meritocracy
            if let Some(country) = state.countries.get_mut(country_tag) {
                country.meritocracy.set(Fixed::ZERO);
            }

            log::info!("{} abandons the Mandate of Heaven", country_tag);
            Ok(())
        }
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

/// Apply aggressive expansion to all countries when provinces are conquered.
///
/// AE impact:
/// - 1 AE per 1 development conquered
/// - Applied to all countries in the world
/// - Higher impact on neighbors and countries with good relations
fn apply_aggressive_expansion(state: &mut WorldState, conqueror: &str, provinces: &[ProvinceId]) {
    // Calculate total development conquered
    let total_dev: i64 = provinces
        .iter()
        .filter_map(|&prov_id| {
            state
                .provinces
                .get(&prov_id)
                .map(|p| (p.base_tax + p.base_production + p.base_manpower).0)
        })
        .sum();

    if total_dev == 0 {
        return;
    }

    let ae_per_dev = Fixed::ONE; // 1 AE per 1 dev
    let base_ae = Fixed::from_int(total_dev) * ae_per_dev;

    // Apply ae_impact modifier
    let ae_impact_mod = state
        .modifiers
        .country_ae_impact
        .get(conqueror)
        .copied()
        .unwrap_or(Fixed::ZERO);
    let total_ae = base_ae.mul(Fixed::ONE + ae_impact_mod);

    // Apply AE to all countries
    let country_tags: Vec<String> = state.countries.keys().cloned().collect();
    for tag in country_tags {
        if tag == conqueror {
            continue; // Don't apply AE to self
        }

        if let Some(country) = state.countries.get_mut(&tag) {
            let ae = country
                .aggressive_expansion
                .entry(conqueror.to_string())
                .or_insert(Fixed::ZERO);
            *ae += total_ae;

            log::debug!(
                "{} gains {} AE toward {} (total: {})",
                tag,
                total_ae.to_f32(),
                conqueror,
                ae.to_f32()
            );
        }
    }

    log::info!(
        "{} gained {} AE from conquering {} development across {} provinces",
        conqueror,
        total_ae.to_f32(),
        total_dev,
        provinces.len()
    );
}

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

            // Apply aggressive expansion for conquered provinces
            apply_aggressive_expansion(state, &new_owner, provinces);
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
            let mut conquered_provinces = Vec::new();
            for prov_id in province_ids {
                if let Some(prov) = state.provinces.get_mut(&prov_id) {
                    if prov.owner.as_ref().is_some_and(|o| loser_tags.contains(o)) {
                        prov.owner = Some(new_owner.clone());
                        prov.controller = Some(new_owner.clone());
                        conquered_provinces.push(prov_id);
                    }
                }
            }

            // Apply aggressive expansion for all conquered provinces
            apply_aggressive_expansion(state, &new_owner, &conquered_provinces);

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
#[path = "step_tests.rs"]
mod tests;
