//! Naval combat system - ship-to-ship battles in sea zones.
//!
//! Naval battles use the same phase system as land combat but are simpler:
//! - All ships fight simultaneously (no battle lines)
//! - Ships sink when durability reaches 0
//! - Admirals provide pip bonuses like generals

use crate::fixed::Fixed;
use crate::state::{CombatPhase, FleetId, NavalBattle, NavalBattleId, ShipType, WorldState};
use eu4data::defines::naval as defines;
use std::collections::HashMap;

/// Run daily naval combat tick - execute one combat day for all ongoing naval battles.
pub fn run_naval_combat_tick(state: &mut WorldState) {
    // 1. Check for reinforcements joining existing naval battles
    process_reinforcements(state);

    // 2. Start new naval battles where opposing fleets meet
    start_new_naval_battles(state);

    // 3. Run one day for each active naval battle
    let battle_ids: Vec<_> = state.naval_battles.keys().cloned().collect();
    for battle_id in battle_ids {
        tick_naval_battle_day(state, battle_id);
    }

    // 4. Cleanup finished battles
    cleanup_finished_naval_battles(state);
}

/// Check for friendly fleets arriving at ongoing battles and add them as reinforcements.
fn process_reinforcements(state: &mut WorldState) {
    // For each ongoing battle, check if any fleets just arrived at the sea zone
    let battle_sea_zones: Vec<_> = state
        .naval_battles
        .values()
        .map(|b| (b.id, b.sea_zone, b.attackers[0], b.defenders[0]))
        .collect();

    for (battle_id, sea_zone, _attacker_sample, _defender_sample) in battle_sea_zones {
        let fleets_in_zone: Vec<FleetId> = state
            .fleets
            .iter()
            .filter(|(_, f)| f.location == sea_zone && f.in_battle.is_none())
            .map(|(&id, _)| id)
            .collect();

        for fleet_id in fleets_in_zone {
            let fleet_owner = state.fleets.get(&fleet_id).map(|f| f.owner.clone());
            if let Some(owner) = fleet_owner {
                let battle = match state.naval_battles.get(&battle_id) {
                    Some(b) => b,
                    None => continue,
                };

                // Check if this fleet's owner is on one of the sides
                let attacker_owners: Vec<String> = battle
                    .attackers
                    .iter()
                    .filter_map(|&id| state.fleets.get(&id))
                    .map(|f| f.owner.clone())
                    .collect();

                let defender_owners: Vec<String> = battle
                    .defenders
                    .iter()
                    .filter_map(|&id| state.fleets.get(&id))
                    .map(|f| f.owner.clone())
                    .collect();

                if attacker_owners.contains(&owner) {
                    // Join attacker side
                    if let Some(battle) = state.naval_battles.get_mut(&battle_id) {
                        battle.attackers.push(fleet_id);
                    }
                    if let Some(fleet) = state.fleets.get_mut(&fleet_id) {
                        fleet.in_battle = Some(battle_id);
                    }
                } else if defender_owners.contains(&owner) {
                    // Join defender side
                    if let Some(battle) = state.naval_battles.get_mut(&battle_id) {
                        battle.defenders.push(fleet_id);
                    }
                    if let Some(fleet) = state.fleets.get_mut(&fleet_id) {
                        fleet.in_battle = Some(battle_id);
                    }
                }
            }
        }
    }
}

/// Detect new naval battles where opposing fleets meet in the same sea zone.
fn start_new_naval_battles(state: &mut WorldState) {
    // Group fleets by sea zone
    let mut sea_zone_fleets: HashMap<u32, Vec<FleetId>> = HashMap::new();
    for (&fleet_id, fleet) in &state.fleets {
        if fleet.in_battle.is_none() {
            sea_zone_fleets
                .entry(fleet.location)
                .or_default()
                .push(fleet_id);
        }
    }

    // Check each sea zone for conflicts
    for (sea_zone, fleet_ids) in sea_zone_fleets {
        // Group by owner
        let mut owners: HashMap<String, Vec<FleetId>> = HashMap::new();
        for &fleet_id in &fleet_ids {
            if let Some(fleet) = state.fleets.get(&fleet_id) {
                owners
                    .entry(fleet.owner.clone())
                    .or_default()
                    .push(fleet_id);
            }
        }

        // Check all pairs for war
        let owner_list: Vec<String> = owners.keys().cloned().collect();
        for i in 0..owner_list.len() {
            for j in (i + 1)..owner_list.len() {
                let owner1 = &owner_list[i];
                let owner2 = &owner_list[j];

                if state.diplomacy.are_at_war(owner1, owner2) {
                    let side1 = &owners[owner1];
                    let side2 = &owners[owner2];

                    // Check if any of these fleets are already in battle
                    let any_in_battle = side1
                        .iter()
                        .chain(side2.iter())
                        .any(|&id| state.fleets.get(&id).is_some_and(|f| f.in_battle.is_some()));

                    if any_in_battle {
                        continue;
                    }

                    // Start naval battle!
                    start_naval_battle(state, sea_zone, side1.clone(), side2.clone());
                }
            }
        }
    }
}

/// Initialize a new naval battle between two groups of fleets.
fn start_naval_battle(
    state: &mut WorldState,
    sea_zone: u32,
    attacker_fleets: Vec<FleetId>,
    defender_fleets: Vec<FleetId>,
) {
    let battle_id = state.next_naval_battle_id;
    state.next_naval_battle_id += 1;

    // Roll initial dice
    let attacker_dice = roll_dice(state);
    let defender_dice = roll_dice(state);

    let battle = NavalBattle {
        id: battle_id,
        sea_zone,
        start_date: state.date,
        phase_day: 0,
        phase: CombatPhase::Fire,
        attacker_dice,
        defender_dice,
        attackers: attacker_fleets.clone(),
        defenders: defender_fleets.clone(),
        attacker_losses: 0,
        defender_losses: 0,
        result: None,
    };

    state.naval_battles.insert(battle_id, battle);

    // Mark fleets as in battle
    for &fleet_id in attacker_fleets.iter().chain(defender_fleets.iter()) {
        if let Some(fleet) = state.fleets.get_mut(&fleet_id) {
            fleet.in_battle = Some(battle_id);
        }
    }

    log::info!(
        "Naval battle started at sea zone {}: {} fleets vs {} fleets",
        sea_zone,
        attacker_fleets.len(),
        defender_fleets.len()
    );
}

/// Run one day of naval combat for a battle.
fn tick_naval_battle_day(state: &mut WorldState, battle_id: NavalBattleId) {
    // Check if battle is already over
    if state
        .naval_battles
        .get(&battle_id)
        .is_some_and(|b| b.result.is_some())
    {
        return;
    }

    // Calculate and apply damage
    apply_daily_naval_damage(state, battle_id);

    // Check for battle end (one side has no ships left)
    if check_naval_battle_end(state, battle_id) {
        return;
    }

    // Advance day; if phase ends, switch phase and reroll dice
    advance_naval_day(state, battle_id);
}

/// Calculate and apply damage for both sides.
fn apply_daily_naval_damage(state: &mut WorldState, battle_id: NavalBattleId) {
    // Get battle info (immutable first pass to calculate)
    let (att_damage, def_damage) = {
        let battle = match state.naval_battles.get(&battle_id) {
            Some(b) => b,
            None => return,
        };

        let att_damage = calculate_fleet_damage(state, battle, true);
        let def_damage = calculate_fleet_damage(state, battle, false);

        (att_damage, def_damage)
    };

    // Apply damage to defenders from attackers
    apply_damage_to_fleets(state, battle_id, false, att_damage.0, att_damage.1);

    // Apply damage to attackers from defenders
    apply_damage_to_fleets(state, battle_id, true, def_damage.0, def_damage.1);
}

/// Calculate damage dealt by one side. Returns (hull_damage, durability_damage).
fn calculate_fleet_damage(
    state: &WorldState,
    battle: &NavalBattle,
    is_attacker: bool,
) -> (Fixed, Fixed) {
    let fleet_ids = if is_attacker {
        &battle.attackers
    } else {
        &battle.defenders
    };
    let dice = if is_attacker {
        battle.attacker_dice
    } else {
        battle.defender_dice
    };

    // Add admiral pip to dice (capped at 9)
    let admiral_bonus = get_admiral_bonus(state, fleet_ids, battle.phase);
    let effective_dice = ((dice as i8 + admiral_bonus).clamp(0, 9)) as u8;

    let mut total_damage = Fixed::ZERO;

    // All ships fire simultaneously
    for &fleet_id in fleet_ids {
        if let Some(fleet) = state.fleets.get(&fleet_id) {
            for ship in &fleet.ships {
                let base = get_ship_phase_damage(ship.type_, battle.phase);
                // Damage formula: base * (effective_dice + 5) / 10 * (hull / hull_size)
                let dice_factor =
                    Fixed::from_int(effective_dice as i64 + 5).div(Fixed::from_int(10));
                let hull_size = Fixed::from_int(get_ship_hull_size(ship.type_) as i64);
                let hull_factor = ship.hull.div(hull_size);
                let dmg = Fixed::from_f32(base).mul(dice_factor).mul(hull_factor);
                total_damage += dmg;
            }
        }
    }

    // Scale to reasonable hull damage numbers
    let hull_damage = total_damage.mul(Fixed::from_int(100));
    let durability_damage = hull_damage.mul(Fixed::from_f32(defines::DURABILITY_DAMAGE_MULTIPLIER));

    (hull_damage, durability_damage)
}

/// Apply damage to one side of the battle.
fn apply_damage_to_fleets(
    state: &mut WorldState,
    battle_id: NavalBattleId,
    is_attacker: bool,
    hull_damage: Fixed,
    durability_damage: Fixed,
) {
    let battle = match state.naval_battles.get(&battle_id) {
        Some(b) => b.clone(),
        None => return,
    };

    let fleet_ids = if is_attacker {
        &battle.attackers
    } else {
        &battle.defenders
    };

    // Count ships to distribute damage
    let total_ships: usize = fleet_ids
        .iter()
        .filter_map(|&id| state.fleets.get(&id))
        .map(|f| f.ships.len())
        .sum();

    if total_ships == 0 {
        return;
    }

    // Distribute damage evenly across all ships
    let hull_per_ship = hull_damage.div(Fixed::from_int(total_ships as i64));
    let durability_per_ship = durability_damage.div(Fixed::from_int(total_ships as i64));

    let mut ships_sunk = 0;

    for &fleet_id in fleet_ids {
        if let Some(fleet) = state.fleets.get_mut(&fleet_id) {
            fleet.ships.retain_mut(|ship| {
                ship.hull = (ship.hull - hull_per_ship).max(Fixed::ZERO);
                ship.durability = (ship.durability - durability_per_ship).max(Fixed::ZERO);

                // Ship sinks if durability reaches 0
                if ship.durability <= Fixed::ZERO {
                    ships_sunk += 1;
                    false
                } else {
                    true
                }
            });
        }
    }

    // Update battle casualties
    if let Some(battle) = state.naval_battles.get_mut(&battle_id) {
        if is_attacker {
            battle.attacker_losses += ships_sunk;
        } else {
            battle.defender_losses += ships_sunk;
        }
    }

    if ships_sunk > 0 {
        log::trace!(
            "Naval battle {}: {} ships sunk on {} side",
            battle_id,
            ships_sunk,
            if is_attacker { "attacker" } else { "defender" }
        );
    }
}

/// Check if the naval battle is over (one side has no ships).
fn check_naval_battle_end(state: &mut WorldState, battle_id: NavalBattleId) -> bool {
    let battle = match state.naval_battles.get(&battle_id) {
        Some(b) => b,
        None => return false,
    };

    let attacker_ships: usize = battle
        .attackers
        .iter()
        .filter_map(|&id| state.fleets.get(&id))
        .map(|f| f.ships.len())
        .sum();

    let defender_ships: usize = battle
        .defenders
        .iter()
        .filter_map(|&id| state.fleets.get(&id))
        .map(|f| f.ships.len())
        .sum();

    if attacker_ships == 0 || defender_ships == 0 {
        let result = if attacker_ships > 0 {
            crate::state::BattleResult::AttackerVictory {
                pursuit_casualties: 0, // Naval combat has no pursuit
                stackwiped: false,     // Naval combat has no stackwipe
            }
        } else {
            crate::state::BattleResult::DefenderVictory {
                pursuit_casualties: 0,
                stackwiped: false,
            }
        };

        log::info!("Naval battle {} ended: {:?}", battle_id, result);

        if let Some(battle) = state.naval_battles.get_mut(&battle_id) {
            battle.result = Some(result);
        }
        true
    } else {
        false
    }
}

/// Advance the battle day counter and handle phase transitions.
fn advance_naval_day(state: &mut WorldState, battle_id: NavalBattleId) {
    // Check if phase will end
    let (should_switch_phase, current_phase_day) = {
        let battle = match state.naval_battles.get(&battle_id) {
            Some(b) => b,
            None => return,
        };
        (
            battle.phase_day + 1 >= defines::DAYS_PER_PHASE,
            battle.phase_day,
        )
    };

    // Roll dice if switching phase (before mutable borrow)
    let (attacker_dice, defender_dice) = if should_switch_phase {
        (roll_dice(state), roll_dice(state))
    } else {
        (0, 0) // Unused
    };

    // Now update battle state
    let battle = match state.naval_battles.get_mut(&battle_id) {
        Some(b) => b,
        None => return,
    };

    battle.phase_day = current_phase_day + 1;

    if should_switch_phase {
        // Switch phase
        battle.phase = match battle.phase {
            CombatPhase::Fire => CombatPhase::Shock,
            CombatPhase::Shock => CombatPhase::Fire,
        };
        battle.phase_day = 0;
        battle.attacker_dice = attacker_dice;
        battle.defender_dice = defender_dice;
    }
}

/// Cleanup battles that have ended.
fn cleanup_finished_naval_battles(state: &mut WorldState) {
    let finished_battles: Vec<NavalBattleId> = state
        .naval_battles
        .iter()
        .filter(|(_, b)| b.result.is_some())
        .map(|(&id, _)| id)
        .collect();

    for battle_id in finished_battles {
        if let Some(battle) = state.naval_battles.remove(&battle_id) {
            // Release fleets from battle
            for &fleet_id in battle.attackers.iter().chain(battle.defenders.iter()) {
                if let Some(fleet) = state.fleets.get_mut(&fleet_id) {
                    fleet.in_battle = None;
                }
            }
        }
    }
}

/// Roll a combat dice (0-9).
fn roll_dice(state: &mut WorldState) -> u8 {
    (state.random_u64() % (defines::DICE_MAX as u64 + 1)) as u8
}

/// Get ship's damage value for a combat phase.
fn get_ship_phase_damage(ship_type: ShipType, phase: CombatPhase) -> f32 {
    match (ship_type, phase) {
        (ShipType::HeavyShip, CombatPhase::Fire) => defines::HEAVY_SHIP_FIRE,
        (ShipType::HeavyShip, CombatPhase::Shock) => defines::HEAVY_SHIP_SHOCK,
        (ShipType::LightShip, CombatPhase::Fire) => defines::LIGHT_SHIP_FIRE,
        (ShipType::LightShip, CombatPhase::Shock) => defines::LIGHT_SHIP_SHOCK,
        (ShipType::Galley, CombatPhase::Fire) => defines::GALLEY_FIRE,
        (ShipType::Galley, CombatPhase::Shock) => defines::GALLEY_SHOCK,
        (ShipType::Transport, CombatPhase::Fire) => defines::TRANSPORT_FIRE,
        (ShipType::Transport, CombatPhase::Shock) => defines::TRANSPORT_SHOCK,
    }
}

/// Get ship's hull size.
fn get_ship_hull_size(ship_type: ShipType) -> u32 {
    match ship_type {
        ShipType::HeavyShip => defines::HEAVY_SHIP_HULL_SIZE,
        ShipType::LightShip => defines::LIGHT_SHIP_HULL_SIZE,
        ShipType::Galley => defines::GALLEY_HULL_SIZE,
        ShipType::Transport => defines::TRANSPORT_HULL_SIZE,
    }
}

/// Get the highest admiral pip bonus for a specific combat phase.
fn get_admiral_bonus(state: &WorldState, fleet_ids: &[FleetId], phase: CombatPhase) -> i8 {
    let mut best_bonus: i8 = 0;
    for &fleet_id in fleet_ids {
        if let Some(fleet) = state.fleets.get(&fleet_id) {
            if let Some(admiral_id) = fleet.admiral {
                if let Some(admiral) = state.admirals.get(&admiral_id) {
                    let pip = match phase {
                        CombatPhase::Fire => admiral.fire,
                        CombatPhase::Shock => admiral.shock,
                    };
                    best_bonus = best_bonus.max(pip as i8);
                }
            }
        }
    }
    best_bonus
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::{Admiral, Fleet, ProvinceId, Ship, War};
    use crate::testing::WorldStateBuilder;

    fn make_ship(type_: ShipType) -> Ship {
        let hull_size = get_ship_hull_size(type_);
        Ship {
            type_,
            hull: Fixed::from_int(hull_size as i64),
            durability: Fixed::from_f32(defines::BASE_DURABILITY),
        }
    }

    fn make_fleet(id: FleetId, owner: &str, location: ProvinceId, ships: Vec<Ship>) -> Fleet {
        Fleet {
            id,
            name: format!("Fleet {}", id),
            owner: owner.to_string(),
            location,
            ships,
            embarked_armies: vec![],
            movement: None,
            admiral: None,
            in_battle: None,
        }
    }

    #[test]
    fn test_ship_damage_values() {
        // Heavy ships should be strong in fire phase
        let fire_dmg = get_ship_phase_damage(ShipType::HeavyShip, CombatPhase::Fire);
        let shock_dmg = get_ship_phase_damage(ShipType::HeavyShip, CombatPhase::Shock);
        assert_eq!(fire_dmg, 1.0);
        assert_eq!(shock_dmg, 0.0);

        // Galleys should be strong in shock phase
        let galley_fire = get_ship_phase_damage(ShipType::Galley, CombatPhase::Fire);
        let galley_shock = get_ship_phase_damage(ShipType::Galley, CombatPhase::Shock);
        assert_eq!(galley_fire, 0.1);
        assert_eq!(galley_shock, 0.8);

        // Transports have no combat value
        let transport_fire = get_ship_phase_damage(ShipType::Transport, CombatPhase::Fire);
        let transport_shock = get_ship_phase_damage(ShipType::Transport, CombatPhase::Shock);
        assert_eq!(transport_fire, 0.0);
        assert_eq!(transport_shock, 0.0);
    }

    #[test]
    fn test_ship_hull_sizes() {
        assert_eq!(get_ship_hull_size(ShipType::HeavyShip), 50);
        assert_eq!(get_ship_hull_size(ShipType::LightShip), 10);
        assert_eq!(get_ship_hull_size(ShipType::Galley), 20);
        assert_eq!(get_ship_hull_size(ShipType::Transport), 5);
    }

    #[test]
    fn test_battle_detection_same_sea_zone() {
        let mut state = WorldStateBuilder::new()
            .with_country("ENG")
            .with_country("FRA")
            .build();

        // Create war between ENG and FRA
        let war = War {
            id: 1,
            name: "English-French War".to_string(),
            attackers: vec!["ENG".to_string()],
            defenders: vec!["FRA".to_string()],
            start_date: state.date,
            attacker_score: 0,
            attacker_battle_score: 0,
            defender_score: 0,
            defender_battle_score: 0,
            pending_peace: None,
        };
        state.diplomacy.wars.insert(1, war);

        // Create sea zone (province 100)
        state.provinces.insert(
            100,
            crate::state::ProvinceState {
                owner: None,
                controller: None,
                religion: None,
                culture: None,
                trade_goods_id: None,
                base_production: Fixed::ONE,
                base_tax: Fixed::ONE,
                base_manpower: Fixed::ONE,
                fort_level: 0,
                is_capital: false,
                is_mothballed: false,
                is_sea: true,
                terrain: None,
                institution_presence: Default::default(),
                trade: Default::default(),
                cores: Default::default(),
                coring_progress: None,
            },
        );

        // Create fleets in same sea zone
        let eng_fleet = make_fleet(
            1,
            "ENG",
            100,
            vec![
                make_ship(ShipType::HeavyShip),
                make_ship(ShipType::HeavyShip),
            ],
        );
        let fra_fleet = make_fleet(
            2,
            "FRA",
            100,
            vec![make_ship(ShipType::HeavyShip), make_ship(ShipType::Galley)],
        );

        state.fleets.insert(1, eng_fleet);
        state.fleets.insert(2, fra_fleet);

        // Debug: verify war exists
        assert_eq!(state.diplomacy.wars.len(), 1);
        assert!(state.diplomacy.are_at_war("ENG", "FRA"));

        // Debug: verify fleets exist
        assert_eq!(state.fleets.len(), 2);
        assert_eq!(state.fleets.get(&1).unwrap().location, 100);
        assert_eq!(state.fleets.get(&2).unwrap().location, 100);

        // Run naval combat tick
        run_naval_combat_tick(&mut state);

        // A battle should have been created (or already finished if combat was decisive)
        // Check that at least one fleet took damage
        let eng_fleet = state.fleets.get(&1).unwrap();
        let fra_fleet = state.fleets.get(&2).unwrap();

        let eng_total_durability = eng_fleet
            .ships
            .iter()
            .fold(Fixed::ZERO, |acc, s| acc + s.durability);
        let fra_total_durability = fra_fleet
            .ships
            .iter()
            .fold(Fixed::ZERO, |acc, s| acc + s.durability);

        let base_durability = Fixed::from_f32(defines::BASE_DURABILITY);
        let eng_expected = base_durability * Fixed::from_int(2); // 2 heavy ships
        let fra_expected = base_durability * Fixed::from_int(2); // 1 heavy + 1 galley

        // At least one side should have taken damage (battle occurred)
        assert!(
            eng_total_durability < eng_expected || fra_total_durability < fra_expected,
            "Naval battle should have dealt damage to at least one side"
        );
    }

    #[test]
    fn test_admiral_fire_bonus() {
        let mut state = WorldStateBuilder::new().with_country("ENG").build();

        // Create admiral with 3 fire pips
        let admiral = Admiral {
            id: 1,
            name: "Admiral Nelson".to_string(),
            owner: "ENG".to_string(),
            fire: 3,
            shock: 1,
            maneuver: 2,
            siege: 0,
        };
        state.admirals.insert(1, admiral);

        // Create fleet with admiral
        let mut fleet = make_fleet(1, "ENG", 100, vec![make_ship(ShipType::HeavyShip)]);
        fleet.admiral = Some(1);
        state.fleets.insert(1, fleet);

        // Check fire bonus
        let fire_bonus = get_admiral_bonus(&state, &[1], CombatPhase::Fire);
        assert_eq!(fire_bonus, 3);

        // Check shock bonus
        let shock_bonus = get_admiral_bonus(&state, &[1], CombatPhase::Shock);
        assert_eq!(shock_bonus, 1);
    }

    #[test]
    fn test_battle_ends_when_side_loses_all_ships() {
        let mut state = WorldStateBuilder::new()
            .with_country("ENG")
            .with_country("FRA")
            .build();

        // Create war
        let war = War {
            id: 1,
            name: "English-French War".to_string(),
            attackers: vec!["ENG".to_string()],
            defenders: vec!["FRA".to_string()],
            start_date: state.date,
            attacker_score: 0,
            attacker_battle_score: 0,
            defender_score: 0,
            defender_battle_score: 0,
            pending_peace: None,
        };
        state.diplomacy.wars.insert(1, war);

        // Create sea zone
        state.provinces.insert(
            100,
            crate::state::ProvinceState {
                owner: None,
                controller: None,
                religion: None,
                culture: None,
                trade_goods_id: None,
                base_production: Fixed::ONE,
                base_tax: Fixed::ONE,
                base_manpower: Fixed::ONE,
                fort_level: 0,
                is_capital: false,
                is_mothballed: false,
                is_sea: true,
                terrain: None,
                institution_presence: Default::default(),
                trade: Default::default(),
                cores: Default::default(),
                coring_progress: None,
            },
        );

        // Create fleets - FRA has only transports (no combat value)
        let eng_fleet = make_fleet(1, "ENG", 100, vec![make_ship(ShipType::HeavyShip)]);
        let fra_fleet = make_fleet(2, "FRA", 100, vec![make_ship(ShipType::Transport)]);

        state.fleets.insert(1, eng_fleet);
        state.fleets.insert(2, fra_fleet);

        // Debug: verify war exists
        assert_eq!(state.diplomacy.wars.len(), 1);
        assert!(state.diplomacy.are_at_war("ENG", "FRA"));

        // Run naval combat - battle will start, fight, and possibly end quickly
        for _ in 0..30 {
            run_naval_combat_tick(&mut state);
        }

        // Battle should have ended (FRA has no ships left)
        // Battles are cleaned up once they finish, so check fleet state instead
        assert_eq!(
            state.naval_battles.len(),
            0,
            "Battle should be finished and cleaned up"
        );

        // Verify FRA fleet has lost all ships (destroyed)
        if let Some(fra_fleet) = state.fleets.get(&2) {
            let ships_alive = fra_fleet
                .ships
                .iter()
                .filter(|s| s.durability > Fixed::ZERO)
                .count();
            assert_eq!(ships_alive, 0, "FRA's transport should be destroyed");
        }
    }

    #[test]
    fn test_no_battle_between_allies() {
        let mut state = WorldStateBuilder::new()
            .with_country("ENG")
            .with_country("FRA")
            .build();

        // No war between ENG and FRA

        // Create sea zone
        state.provinces.insert(
            100,
            crate::state::ProvinceState {
                owner: None,
                controller: None,
                religion: None,
                culture: None,
                trade_goods_id: None,
                base_production: Fixed::ONE,
                base_tax: Fixed::ONE,
                base_manpower: Fixed::ONE,
                fort_level: 0,
                is_capital: false,
                is_mothballed: false,
                is_sea: true,
                terrain: None,
                institution_presence: Default::default(),
                trade: Default::default(),
                cores: Default::default(),
                coring_progress: None,
            },
        );

        // Create fleets in same sea zone
        let eng_fleet = make_fleet(1, "ENG", 100, vec![make_ship(ShipType::HeavyShip)]);
        let fra_fleet = make_fleet(2, "FRA", 100, vec![make_ship(ShipType::HeavyShip)]);

        state.fleets.insert(1, eng_fleet);
        state.fleets.insert(2, fra_fleet);

        // Run naval combat tick
        run_naval_combat_tick(&mut state);

        // No battle should start (not at war)
        assert_eq!(state.naval_battles.len(), 0);
    }
}
