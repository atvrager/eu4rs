//! Siege system - EU4-authentic dice roll mechanics.
//!
//! Sieges use a dice roll system where:
//! - Roll 1d14 every ~30 days (one siege phase)
//! - Win when: roll + siege_progress + bonuses - fort_level >= 20
//! - Progress increases each failed phase (caps at +12)
//! - Roll of 1 = disease outbreak, roll of 14 = wall breach

use crate::fixed::Fixed;
use crate::state::{ArmyId, ProvinceId, RegimentType, Siege, WorldState};
use eu4data::defines::siege as defines;

// ============================================================================
// Public API
// ============================================================================

/// Run daily siege tick for all active sieges.
/// Called once per day in the simulation tick.
///
/// The adjacency graph is optional - if provided, blockade detection will be enabled.
pub fn run_siege_tick(
    state: &mut WorldState,
    adjacency: Option<&eu4data::adjacency::AdjacencyGraph>,
) {
    // Update blockade status for all sieges
    if let Some(adj) = adjacency {
        update_blockade_status(state, adj);
    }

    let siege_provinces: Vec<ProvinceId> = state.sieges.keys().cloned().collect();

    for province_id in siege_provinces {
        tick_siege_day(state, province_id);
    }

    cleanup_completed_sieges(state);
}

/// Start siege or instant occupation based on fort status.
/// Called when an army enters an enemy province.
pub fn start_occupation(
    state: &mut WorldState,
    province_id: ProvinceId,
    attacker: &str,
    army_id: ArmyId,
) {
    let fort_level = state
        .provinces
        .get(&province_id)
        .map(|p| p.fort_level)
        .unwrap_or(0);

    if fort_level == 0 {
        // Instant occupation for unfortified provinces
        if let Some(province) = state.provinces.get_mut(&province_id) {
            province.controller = Some(attacker.to_string());
        }
        log::info!(
            "Province {} instantly occupied by {}",
            province_id,
            attacker
        );
    } else {
        // Start siege for fortified provinces
        start_siege(state, province_id, attacker, army_id);
    }
}

// ============================================================================
// Daily Siege Tick
// ============================================================================

/// Tick one day for a specific siege.
fn tick_siege_day(state: &mut WorldState, province_id: ProvinceId) {
    let (phase_complete, days_in_phase, progress) = {
        let siege = match state.sieges.get_mut(&province_id) {
            Some(s) => s,
            None => return,
        };
        siege.days_in_phase += 1;
        (
            siege.days_in_phase >= defines::SIEGE_PHASE_DAYS,
            siege.days_in_phase,
            siege.progress_modifier,
        )
    };

    // Log progress periodically (every 10 days)
    if days_in_phase % 10 == 0 {
        log::debug!(
            "[SIEGE] Province {} - day {}/{}, progress_modifier={}",
            province_id,
            days_in_phase,
            defines::SIEGE_PHASE_DAYS,
            progress
        );
    }

    if phase_complete {
        resolve_siege_phase(state, province_id);
    }
}

/// Resolve a completed siege phase (roll dice and check for victory).
fn resolve_siege_phase(state: &mut WorldState, province_id: ProvinceId) {
    // Roll 1d14
    let roll = roll_siege_dice(state);

    // Calculate bonuses and total
    let (total, siege_progress, fort_level, artillery_bonus, general_bonus, blockade_bonus) = {
        let siege = match state.sieges.get(&province_id) {
            Some(s) => s,
            None => return,
        };

        let artillery = count_artillery(state, &siege.besieging_armies)
            .min(defines::ARTILLERY_BONUS_MAX as u32) as i32;
        let general = get_best_siege_general(state, &siege.besieging_armies) as i32
            * defines::GENERAL_SIEGE_PIP_BONUS;
        let blockade = if siege.is_blockaded {
            defines::BLOCKADE_BONUS
        } else {
            0
        };

        let total = roll as i32 + siege.progress_modifier + artillery + general + blockade
            - siege.fort_level as i32;
        (
            total,
            siege.progress_modifier,
            siege.fort_level,
            artillery,
            general,
            blockade,
        )
    };

    // Check for special rolls
    if roll == defines::DISEASE_OUTBREAK_ROLL {
        // Disease outbreak - lose some besieging troops
        apply_disease_casualties(state, province_id);
        log::info!("Siege {}: Disease outbreak! (rolled {})", province_id, roll);
    } else if roll == defines::WALL_BREACH_ROLL {
        // Wall breach - can now assault
        if let Some(siege) = state.sieges.get_mut(&province_id) {
            siege.breached = true;
        }
        log::info!("Siege {}: Wall breach! (rolled {})", province_id, roll);
    }

    // Check for victory
    if total >= defines::SIEGE_WIN_THRESHOLD {
        complete_siege(state, province_id);
        log::info!(
            "Siege {}: Surrendered! (roll {} + progress {} + art {} + gen {} + blk {} - fort {} = {} >= 20)",
            province_id,
            roll,
            siege_progress,
            artillery_bonus,
            general_bonus,
            blockade_bonus,
            fort_level,
            total
        );
    } else {
        // Increase progress modifier (caps at MAX_SIEGE_PROGRESS)
        if let Some(siege) = state.sieges.get_mut(&province_id) {
            siege.progress_modifier =
                (siege.progress_modifier + 1).min(defines::MAX_SIEGE_PROGRESS);
            siege.days_in_phase = 0; // Reset for next phase
        }
        log::trace!(
            "Siege {}: No surrender (total {} < 20), progress now +{}",
            province_id,
            total,
            siege_progress + 1
        );
    }

    // Apply starvation if blockaded
    apply_starvation(state, province_id);
}

// ============================================================================
// Siege Start/Completion
// ============================================================================

/// Start a new siege.
fn start_siege(state: &mut WorldState, province_id: ProvinceId, attacker: &str, army_id: ArmyId) {
    if state.sieges.contains_key(&province_id) {
        // Add army to existing siege
        if let Some(siege) = state.sieges.get_mut(&province_id) {
            if !siege.besieging_armies.contains(&army_id) {
                siege.besieging_armies.push(army_id);
            }
        }
        return;
    }

    let (fort_level, is_mothballed) = state
        .provinces
        .get(&province_id)
        .map(|p| (p.fort_level, p.is_mothballed))
        .unwrap_or((1, false));

    // Mothballed forts fall instantly (garrison = 0)
    if is_mothballed {
        if let Some(province) = state.provinces.get_mut(&province_id) {
            province.controller = Some(attacker.to_string());
        }
        log::info!(
            "Mothballed fort at province {} falls instantly to {}",
            province_id,
            attacker
        );
        return;
    }

    let siege = Siege {
        id: state.next_siege_id,
        province: province_id,
        attacker: attacker.to_string(),
        besieging_armies: vec![army_id],
        fort_level,
        garrison: fort_level as u32 * defines::GARRISON_BASE_SIZE,
        progress_modifier: 0, // Starts at 0, increases each failed phase
        days_in_phase: 0,
        start_date: state.date,
        is_blockaded: false, // Updated each tick by update_blockade_status()
        breached: false,
    };

    state.next_siege_id += 1;
    state.sieges.insert(province_id, siege);
    log::info!(
        "Siege started at province {} (fort level {}) by {}",
        province_id,
        fort_level,
        attacker
    );
}

/// Complete a siege (defender surrendered).
fn complete_siege(state: &mut WorldState, province_id: ProvinceId) {
    if let Some(siege) = state.sieges.get(&province_id) {
        let new_controller = siege.attacker.clone();
        log::info!(
            "Siege complete: {} now controlled by {}",
            province_id,
            new_controller
        );

        if let Some(province) = state.provinces.get_mut(&province_id) {
            province.controller = Some(new_controller);
        }
    }
}

/// Remove completed sieges from the map.
fn cleanup_completed_sieges(state: &mut WorldState) {
    let completed: Vec<ProvinceId> = state
        .sieges
        .iter()
        .filter(|(&prov_id, _)| {
            // Siege is complete if controller changed
            state
                .provinces
                .get(&prov_id)
                .and_then(|p| p.controller.as_ref())
                .map(|ctrl| {
                    state
                        .sieges
                        .get(&prov_id)
                        .map(|s| ctrl == &s.attacker)
                        .unwrap_or(false)
                })
                .unwrap_or(false)
        })
        .map(|(&id, _)| id)
        .collect();

    for prov_id in completed {
        state.sieges.remove(&prov_id);
    }
}

// ============================================================================
// Helper Functions
// ============================================================================

/// Roll a 1d14 siege die.
fn roll_siege_dice(state: &mut WorldState) -> u32 {
    let x = state.random_u64();
    (x % 14 + 1) as u32
}

/// Count artillery regiments in besieging armies.
fn count_artillery(state: &WorldState, army_ids: &[ArmyId]) -> u32 {
    let mut count = 0;
    for &army_id in army_ids {
        if let Some(army) = state.armies.get(&army_id) {
            for reg in &army.regiments {
                if reg.type_ == RegimentType::Artillery && reg.strength > Fixed::ZERO {
                    count += 1;
                }
            }
        }
    }
    count
}

/// Get the best siege pip among besieging generals.
fn get_best_siege_general(state: &WorldState, army_ids: &[ArmyId]) -> u8 {
    let mut best_siege = 0u8;
    for &army_id in army_ids {
        if let Some(army) = state.armies.get(&army_id) {
            if let Some(gen_id) = army.general {
                if let Some(general) = state.generals.get(&gen_id) {
                    best_siege = best_siege.max(general.siege);
                }
            }
        }
    }
    best_siege
}

/// Apply disease casualties to besieging armies (roll of 1).
fn apply_disease_casualties(state: &mut WorldState, province_id: ProvinceId) {
    let army_ids: Vec<ArmyId> = state
        .sieges
        .get(&province_id)
        .map(|s| s.besieging_armies.clone())
        .unwrap_or_default();

    // Apply 5% casualties to each army
    let disease_loss = Fixed::from_f32(0.05);
    for army_id in army_ids {
        if let Some(army) = state.armies.get_mut(&army_id) {
            for reg in &mut army.regiments {
                let loss = reg.strength.mul(disease_loss);
                reg.strength = (reg.strength - loss).max(Fixed::ZERO);
            }
        }
    }
}

/// Apply starvation to garrison if blockaded.
fn apply_starvation(state: &mut WorldState, province_id: ProvinceId) {
    let is_blockaded = state
        .sieges
        .get(&province_id)
        .map(|s| s.is_blockaded)
        .unwrap_or(false);

    if !is_blockaded {
        return;
    }

    // Lose 10% of garrison per month (once per phase = 30 days)
    if let Some(siege) = state.sieges.get_mut(&province_id) {
        let loss = (siege.garrison as f32 * defines::STARVATION_MONTHLY_LOSS_PERCENT / 100.0)
            .floor() as u32;
        siege.garrison = siege.garrison.saturating_sub(loss);

        if siege.garrison < defines::GARRISON_SURRENDER_THRESHOLD {
            // Garrison too low - surrender
            log::info!(
                "Siege {}: Garrison starved to {}, surrendering",
                province_id,
                siege.garrison
            );
            complete_siege(state, province_id);
        }
    }
}

// ============================================================================
// Blockade Detection
// ============================================================================

/// Update blockade status for all active sieges.
fn update_blockade_status(state: &mut WorldState, adjacency: &eu4data::adjacency::AdjacencyGraph) {
    let siege_provinces: Vec<ProvinceId> = state.sieges.keys().cloned().collect();

    for province_id in siege_provinces {
        let is_blockaded = check_blockade(state, adjacency, province_id);
        if let Some(siege) = state.sieges.get_mut(&province_id) {
            siege.is_blockaded = is_blockaded;
        }
    }
}

/// Check if a province is blockaded by attacker fleets.
///
/// A province is blockaded if:
/// 1. It is coastal (has adjacent sea zones)
/// 2. All adjacent sea zones have attacker fleets (or allied fleets)
///
/// Blockades provide +1 siege bonus and cause garrison starvation.
/// Sea control is determined by fleet presence - any fleet in a sea zone controls it.
fn check_blockade(
    state: &WorldState,
    adjacency: &eu4data::adjacency::AdjacencyGraph,
    province_id: ProvinceId,
) -> bool {
    let siege = match state.sieges.get(&province_id) {
        Some(s) => s,
        None => return false,
    };

    let defender = match state
        .provinces
        .get(&province_id)
        .and_then(|p| p.owner.as_ref())
    {
        Some(d) => d,
        None => return false,
    };

    // Get all adjacent sea zones
    let neighbors = adjacency.neighbors(province_id);
    let adjacent_seas: Vec<ProvinceId> = neighbors
        .iter()
        .filter(|&&neighbor_id| {
            state
                .provinces
                .get(&neighbor_id)
                .map(|p| p.is_sea)
                .unwrap_or(false)
        })
        .copied()
        .collect();

    // If no adjacent seas, can't be blockaded
    if adjacent_seas.is_empty() {
        return false;
    }

    // Check if all adjacent sea zones have attacker fleets (or allied fleets at war with defender)
    for sea_zone in adjacent_seas {
        let has_attacker_fleet = state.fleets.values().any(|fleet| {
            fleet.location == sea_zone
                && (fleet.owner == siege.attacker
                    || state.diplomacy.are_at_war(&fleet.owner, defender))
        });

        // If any sea zone lacks attacker fleet presence, not fully blockaded
        if !has_attacker_fleet {
            return false;
        }
    }

    // All adjacent sea zones have attacker fleets - province is blockaded
    true
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::{Army, General, Regiment};
    use crate::testing::WorldStateBuilder;

    fn make_regiment(type_: RegimentType, strength: i64) -> Regiment {
        Regiment {
            type_,
            strength: Fixed::from_int(strength),
            morale: Fixed::from_int(1),
        }
    }

    #[test]
    fn test_instant_occupation_unfortified() {
        let mut state = WorldStateBuilder::new()
            .with_country("ATK")
            .with_province(1, Some("DEF"))
            .build();

        // Province 1 has no fort (fort_level = 0 by default)
        state.provinces.get_mut(&1).unwrap().fort_level = 0;
        state.provinces.get_mut(&1).unwrap().owner = Some("DEF".to_string());

        // Create attacker army
        let army = Army {
            id: 1,
            name: "Test Army".to_string(),
            owner: "ATK".to_string(),
            location: 1,
            previous_location: None,
            regiments: vec![make_regiment(RegimentType::Infantry, 1000)],
            movement: None,
            embarked_on: None,
            general: None,
            in_battle: None,
            infantry_count: 0,
            cavalry_count: 0,
            artillery_count: 0,
        };
        state.armies.insert(1, army);

        start_occupation(&mut state, 1, "ATK", 1);

        // Province should be instantly occupied
        assert_eq!(
            state.provinces.get(&1).unwrap().controller,
            Some("ATK".to_string())
        );
        assert!(state.sieges.is_empty());
    }

    #[test]
    fn test_fortified_province_starts_siege() {
        let mut state = WorldStateBuilder::new()
            .with_country("ATK")
            .with_province(1, Some("DEF"))
            .build();

        // Province 1 has a level 2 fort
        state.provinces.get_mut(&1).unwrap().fort_level = 2;
        state.provinces.get_mut(&1).unwrap().owner = Some("DEF".to_string());

        let army = Army {
            id: 1,
            name: "Test Army".to_string(),
            owner: "ATK".to_string(),
            location: 1,
            previous_location: None,
            regiments: vec![make_regiment(RegimentType::Infantry, 1000)],
            movement: None,
            embarked_on: None,
            general: None,
            in_battle: None,
            infantry_count: 0,
            cavalry_count: 0,
            artillery_count: 0,
        };
        state.armies.insert(1, army);

        start_occupation(&mut state, 1, "ATK", 1);

        // Siege should have started
        assert!(state.sieges.contains_key(&1));
        let siege = state.sieges.get(&1).unwrap();
        assert_eq!(siege.fort_level, 2);
        assert_eq!(siege.garrison, 2000); // 2 * 1000
        assert_eq!(siege.progress_modifier, 0);
    }

    #[test]
    fn test_mothballed_fort_falls_instantly() {
        let mut state = WorldStateBuilder::new()
            .with_country("ATK")
            .with_province(1, Some("DEF"))
            .build();

        state.provinces.get_mut(&1).unwrap().fort_level = 3;
        state.provinces.get_mut(&1).unwrap().is_mothballed = true;
        state.provinces.get_mut(&1).unwrap().owner = Some("DEF".to_string());

        let army = Army {
            id: 1,
            name: "Test Army".to_string(),
            owner: "ATK".to_string(),
            location: 1,
            previous_location: None,
            regiments: vec![make_regiment(RegimentType::Infantry, 1000)],
            movement: None,
            embarked_on: None,
            general: None,
            in_battle: None,
            infantry_count: 0,
            cavalry_count: 0,
            artillery_count: 0,
        };
        state.armies.insert(1, army);

        start_occupation(&mut state, 1, "ATK", 1);

        // Mothballed fort should fall instantly
        assert_eq!(
            state.provinces.get(&1).unwrap().controller,
            Some("ATK".to_string())
        );
        assert!(state.sieges.is_empty());
    }

    #[test]
    fn test_artillery_bonus() {
        let mut state = WorldStateBuilder::new()
            .with_country("ATK")
            .with_province(1, Some("DEF"))
            .build();

        let army = Army {
            id: 1,
            name: "Test Army".to_string(),
            owner: "ATK".to_string(),
            location: 1,
            previous_location: None,
            regiments: vec![
                make_regiment(RegimentType::Infantry, 1000),
                make_regiment(RegimentType::Artillery, 1000),
                make_regiment(RegimentType::Artillery, 1000),
                make_regiment(RegimentType::Artillery, 1000),
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

        let count = count_artillery(&state, &[1]);
        assert_eq!(count, 3);
    }

    #[test]
    fn test_general_siege_pip_bonus() {
        let mut state = WorldStateBuilder::new()
            .with_country("ATK")
            .with_province(1, Some("DEF"))
            .build();

        // Create general with 3 siege pips
        let general = General {
            id: 1,
            name: "Siege Master".to_string(),
            owner: "ATK".to_string(),
            fire: 0,
            shock: 0,
            maneuver: 0,
            siege: 3,
        };
        state.generals.insert(1, general);

        let army = Army {
            id: 1,
            name: "Test Army".to_string(),
            owner: "ATK".to_string(),
            location: 1,
            previous_location: None,
            regiments: vec![make_regiment(RegimentType::Infantry, 1000)],
            movement: None,
            embarked_on: None,
            general: Some(1),
            in_battle: None,
            infantry_count: 0,
            cavalry_count: 0,
            artillery_count: 0,
        };
        state.armies.insert(1, army);

        let bonus = get_best_siege_general(&state, &[1]);
        assert_eq!(bonus, 3);
    }
}
