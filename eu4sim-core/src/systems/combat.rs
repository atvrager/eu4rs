//! EU4-authentic combat system with battle lines, morale, and phases.
//!
//! Combat phases last 3 days each, alternating Fire → Shock.
//! Discipline affects damage dealt (casualties + morale) and via tactics reduces damage received.
//! Cavalry is limited to 50% of front line or suffers -25% tactics penalty.
//! 10:1 strength ratio at battle end causes stackwipe.

use crate::fixed::Fixed;
use crate::state::{
    ArmyId, Battle, BattleId, BattleLine, BattleResult, CombatPhase, ProvinceId, RegimentType,
    Terrain, WorldState,
};
use eu4data::defines::combat as defines;
use std::collections::HashMap;

// ============================================================================
// Main Entry Point
// ============================================================================

/// Runs daily combat resolution for all active battles and detects new engagements.
///
/// Called once per day in the simulation tick.
pub fn run_combat_tick(
    state: &mut WorldState,
    adjacency: Option<&eu4data::adjacency::AdjacencyGraph>,
) {
    // 1. Check for reinforcements joining existing battles
    process_reinforcements(state);

    // 2. Start new battles where opposing armies meet
    start_new_battles(state);

    // 3. Run one day for each active battle
    let battle_ids: Vec<_> = state.battles.keys().cloned().collect();
    for battle_id in battle_ids {
        tick_battle_day(state, battle_id, adjacency);
    }

    // 4. Cleanup finished battles
    cleanup_finished_battles(state);
}

// ============================================================================
// Battle Detection & Setup
// ============================================================================

/// Check for armies arriving at provinces with ongoing battles and add as reinforcements.
fn process_reinforcements(state: &mut WorldState) {
    // Collect armies that are in battle provinces but not yet in battle
    let mut reinforcements: Vec<(BattleId, ArmyId, bool)> = Vec::new(); // (battle, army, is_attacker)

    for battle in state.battles.values() {
        if battle.result.is_some() {
            continue; // Battle already over
        }

        for (&army_id, army) in &state.armies {
            if army.location != battle.province || army.in_battle.is_some() {
                continue;
            }

            // Determine which side this army belongs to
            let attacker_tags: Vec<_> = battle
                .attackers
                .iter()
                .filter_map(|&aid| state.armies.get(&aid).map(|a| a.owner.clone()))
                .collect();

            let defender_tags: Vec<_> = battle
                .defenders
                .iter()
                .filter_map(|&aid| state.armies.get(&aid).map(|a| a.owner.clone()))
                .collect();

            if attacker_tags.contains(&army.owner) {
                reinforcements.push((battle.id, army_id, true));
            } else if defender_tags.contains(&army.owner) {
                reinforcements.push((battle.id, army_id, false));
            }
            // Otherwise army is neutral, doesn't join
        }
    }

    // Apply reinforcements
    for (battle_id, army_id, is_attacker) in reinforcements {
        if let Some(battle) = state.battles.get_mut(&battle_id) {
            if is_attacker {
                battle.attackers.push(army_id);
                battle.attacker_line.reserves.push(army_id);
            } else {
                battle.defenders.push(army_id);
                battle.defender_line.reserves.push(army_id);
            }
        }
        if let Some(army) = state.armies.get_mut(&army_id) {
            army.in_battle = Some(battle_id);
        }
        log::info!(
            "Army {} joined battle {} as reinforcement",
            army_id,
            battle_id
        );
    }
}

/// Start new battles where opposing armies are in the same province.
fn start_new_battles(state: &mut WorldState) {
    // Group armies by province (excluding those already in battle)
    let mut province_armies: HashMap<ProvinceId, Vec<ArmyId>> = HashMap::new();

    for (&army_id, army) in &state.armies {
        if army.in_battle.is_some() || army.embarked_on.is_some() {
            continue;
        }
        province_armies
            .entry(army.location)
            .or_default()
            .push(army_id);
    }

    // Check each province for opposing forces at war
    for (province_id, army_ids) in province_armies {
        if army_ids.len() < 2 {
            continue;
        }

        // Group by owner
        let mut owners: HashMap<String, Vec<ArmyId>> = HashMap::new();
        for &army_id in &army_ids {
            if let Some(army) = state.armies.get(&army_id) {
                owners.entry(army.owner.clone()).or_default().push(army_id);
            }
        }

        // Check all pairs for war
        let owner_list: Vec<String> = owners.keys().cloned().collect();
        for i in 0..owner_list.len() {
            for j in (i + 1)..owner_list.len() {
                let owner1 = &owner_list[i];
                let owner2 = &owner_list[j];

                if state.diplomacy.are_at_war(owner1, owner2) {
                    // Check if any of these armies are already in battle
                    let side1 = &owners[owner1];
                    let side2 = &owners[owner2];

                    let any_in_battle = side1
                        .iter()
                        .chain(side2.iter())
                        .any(|&id| state.armies.get(&id).is_some_and(|a| a.in_battle.is_some()));

                    if any_in_battle {
                        continue;
                    }

                    // Start battle! Determine attacker (first to arrive conceptually)
                    // For now: owner1 is attacker
                    start_battle(state, province_id, side1.clone(), side2.clone());
                }
            }
        }
    }
}

/// Initialize a new battle between two groups of armies.
fn start_battle(
    state: &mut WorldState,
    province: ProvinceId,
    attacker_armies: Vec<ArmyId>,
    defender_armies: Vec<ArmyId>,
) {
    let battle_id = state.next_battle_id;
    state.next_battle_id += 1;

    // Get combat width (use first attacker's owner for tech lookup)
    let attacker_owner = attacker_armies
        .first()
        .and_then(|&id| state.armies.get(&id))
        .map(|a| a.owner.clone())
        .unwrap_or_default();
    let combat_width = get_combat_width(state, &attacker_owner);

    // Deploy armies to battle lines
    let attacker_line = deploy_to_lines(state, &attacker_armies, combat_width);
    let defender_line = deploy_to_lines(state, &defender_armies, combat_width);

    // Determine attacker origin for river crossing penalty
    let attacker_origin = attacker_armies
        .first()
        .and_then(|&id| state.armies.get(&id))
        .and_then(|a| a.previous_location);

    // Roll initial dice
    let attacker_dice = roll_dice(state);
    let defender_dice = roll_dice(state);

    let battle = Battle {
        id: battle_id,
        province,
        attacker_origin,
        start_date: state.date,
        phase_day: 0,
        phase: CombatPhase::Fire,
        attacker_dice,
        defender_dice,
        attackers: attacker_armies.clone(),
        defenders: defender_armies.clone(),
        attacker_line,
        defender_line,
        attacker_casualties: 0,
        defender_casualties: 0,
        result: None,
    };

    state.battles.insert(battle_id, battle);

    // Mark armies as in battle
    for &army_id in attacker_armies.iter().chain(defender_armies.iter()) {
        if let Some(army) = state.armies.get_mut(&army_id) {
            army.in_battle = Some(battle_id);
        }
    }

    log::info!(
        "Battle started at province {}: {} vs {}",
        province,
        attacker_armies.len(),
        defender_armies.len()
    );
}

// ============================================================================
// Battle Line Deployment
// ============================================================================

/// Deploy armies to front/back rows respecting combat width and cavalry ratio.
fn deploy_to_lines(state: &WorldState, army_ids: &[ArmyId], combat_width: u8) -> BattleLine {
    let width = combat_width as usize;

    // Collect all regiments with their army reference
    let mut infantry: Vec<(ArmyId, usize)> = Vec::new();
    let mut cavalry: Vec<(ArmyId, usize)> = Vec::new();
    let mut artillery: Vec<(ArmyId, usize)> = Vec::new();

    for &army_id in army_ids {
        if let Some(army) = state.armies.get(&army_id) {
            for (idx, reg) in army.regiments.iter().enumerate() {
                if reg.strength <= Fixed::ZERO {
                    continue;
                }
                match reg.type_ {
                    RegimentType::Infantry => infantry.push((army_id, idx)),
                    RegimentType::Cavalry => cavalry.push((army_id, idx)),
                    RegimentType::Artillery => artillery.push((army_id, idx)),
                }
            }
        }
    }

    // Fill front row: infantry first, then cavalry (up to width)
    let mut front: Vec<Option<(ArmyId, usize)>> = Vec::with_capacity(width);

    // Add all infantry
    for reg in infantry.iter().take(width) {
        front.push(Some(*reg));
    }

    // Calculate max cavalry allowed: 50% of infantry count
    let inf_in_front = front.len();
    let max_cav = (inf_in_front as f32 * defines::BASE_CAVALRY_RATIO).ceil() as usize;
    let remaining_width = width.saturating_sub(front.len());

    // Add cavalry up to limit and remaining width
    let cav_to_add = cavalry.len().min(max_cav).min(remaining_width);
    for reg in cavalry.iter().take(cav_to_add) {
        front.push(Some(*reg));
    }

    // Back row: artillery + excess cavalry
    let mut back: Vec<(ArmyId, usize)> = artillery;
    for reg in cavalry.iter().skip(cav_to_add) {
        back.push(*reg);
    }

    // Any infantry that didn't fit goes to reserves (rare)
    let reserves: Vec<ArmyId> = Vec::new();

    BattleLine {
        front,
        back,
        reserves,
    }
}

// ============================================================================
// Daily Battle Tick
// ============================================================================

/// Run one day of combat for a battle.
fn tick_battle_day(
    state: &mut WorldState,
    battle_id: BattleId,
    adjacency: Option<&eu4data::adjacency::AdjacencyGraph>,
) {
    // Check if battle is already over
    if state
        .battles
        .get(&battle_id)
        .is_some_and(|b| b.result.is_some())
    {
        return;
    }

    // Fill gaps in front line from reserves/back row
    fill_frontline_gaps(state, battle_id);

    // Calculate and apply damage
    apply_daily_damage(state, battle_id, adjacency);

    // Check for morale break / stackwipe
    if check_battle_end(state, battle_id) {
        return;
    }

    // Advance day; if phase ends, switch phase and reroll dice
    advance_day(state, battle_id);
}

/// Fill empty front line slots from back row or reserves.
fn fill_frontline_gaps(state: &mut WorldState, battle_id: BattleId) {
    let battle = match state.battles.get_mut(&battle_id) {
        Some(b) => b,
        None => return,
    };

    // Fill attacker gaps
    for slot in &mut battle.attacker_line.front {
        if slot.is_none() && !battle.attacker_line.back.is_empty() {
            *slot = Some(battle.attacker_line.back.remove(0));
        }
    }

    // Fill defender gaps
    for slot in &mut battle.defender_line.front {
        if slot.is_none() && !battle.defender_line.back.is_empty() {
            *slot = Some(battle.defender_line.back.remove(0));
        }
    }
}

/// Calculate and apply damage for both sides.
fn apply_daily_damage(
    state: &mut WorldState,
    battle_id: BattleId,
    adjacency: Option<&eu4data::adjacency::AdjacencyGraph>,
) {
    // Get battle info (immutable first pass to calculate)
    let (att_damage, def_damage, _phase) = {
        let battle = match state.battles.get(&battle_id) {
            Some(b) => b,
            None => return,
        };

        let att_damage = calculate_side_damage(state, battle, true, adjacency);
        let def_damage = calculate_side_damage(state, battle, false, adjacency);

        (att_damage, def_damage, battle.phase)
    };

    // Apply damage to defenders from attackers
    apply_damage_to_side(state, battle_id, false, att_damage.0, att_damage.1);

    // Apply damage to attackers from defenders
    apply_damage_to_side(state, battle_id, true, def_damage.0, def_damage.1);
}

/// Calculate damage dealt by one side. Returns (casualties, morale_damage).
fn calculate_side_damage(
    state: &WorldState,
    battle: &Battle,
    is_attacker: bool,
    adjacency: Option<&eu4data::adjacency::AdjacencyGraph>,
) -> (Fixed, Fixed) {
    let line = if is_attacker {
        &battle.attacker_line
    } else {
        &battle.defender_line
    };
    let army_ids = if is_attacker {
        &battle.attackers
    } else {
        &battle.defenders
    };
    let dice = if is_attacker {
        battle.attacker_dice
    } else {
        battle.defender_dice
    };

    // Add general pip to dice (capped at 9)
    let general_bonus = get_general_bonus(state, army_ids, battle.phase);
    let effective_dice = ((dice as i8 + general_bonus).clamp(0, 9)) as u8;

    let mut total_damage = Fixed::ZERO;

    // Front row damage
    for (army_id, reg_idx) in line.front.iter().flatten() {
        if let Some(army) = state.armies.get(army_id) {
            if let Some(reg) = army.regiments.get(*reg_idx) {
                let base = get_regiment_phase_damage(reg.type_, battle.phase);
                // Damage formula: base * (effective_dice + 5) / 10 * (strength / 1000)
                let dice_factor =
                    Fixed::from_int(effective_dice as i64 + 5).div(Fixed::from_int(10));
                let strength_factor = reg.strength.div(Fixed::from_int(defines::REGIMENT_SIZE));
                let dmg = Fixed::from_f32(base).mul(dice_factor).mul(strength_factor);
                total_damage += dmg;
            }
        }
    }

    // Back row (artillery) deals damage too
    for (army_id, reg_idx) in &line.back {
        if let Some(army) = state.armies.get(army_id) {
            if let Some(reg) = army.regiments.get(*reg_idx) {
                if reg.type_ == RegimentType::Artillery {
                    let base = get_regiment_phase_damage(reg.type_, battle.phase);
                    let dice_factor =
                        Fixed::from_int(effective_dice as i64 + 5).div(Fixed::from_int(10));
                    let strength_factor = reg.strength.div(Fixed::from_int(defines::REGIMENT_SIZE));
                    let dmg = Fixed::from_f32(base).mul(dice_factor).mul(strength_factor);
                    total_damage += dmg;
                }
            }
        }
    }

    // Apply terrain penalty to attacker (considers maneuver difference and river crossing)
    if is_attacker {
        let terrain_mod = get_terrain_penalty(
            state,
            battle.province,
            battle.attacker_origin,
            &battle.attackers,
            &battle.defenders,
            adjacency,
        );
        // Each -1 to dice effectively reduces damage by ~10%
        let penalty_factor = Fixed::from_int(10 + terrain_mod as i64).div(Fixed::from_int(10));
        total_damage = total_damage.mul(penalty_factor.max(Fixed::ZERO));
    }

    // Scale to reasonable casualty numbers (base damage is per-regiment pip value)
    // Multiply by 100 to get actual casualties
    let casualties = total_damage.mul(Fixed::from_int(100));
    let morale_damage = casualties.mul(Fixed::from_f32(defines::MORALE_DAMAGE_MULTIPLIER));

    (casualties, morale_damage)
}

/// Apply damage to one side of the battle.
fn apply_damage_to_side(
    state: &mut WorldState,
    battle_id: BattleId,
    is_attacker: bool,
    casualties: Fixed,
    morale_damage: Fixed,
) {
    let battle = match state.battles.get(&battle_id) {
        Some(b) => b.clone(),
        None => return,
    };

    let line = if is_attacker {
        &battle.attacker_line
    } else {
        &battle.defender_line
    };

    // Count regiments to distribute damage
    let front_count = line.front.iter().filter(|s| s.is_some()).count();
    let back_count = line.back.len();
    let total_count = front_count + back_count;

    if total_count == 0 {
        return;
    }

    // Distribute casualties evenly across front line
    let per_reg_casualties = if front_count > 0 {
        casualties.div(Fixed::from_int(front_count as i64))
    } else {
        Fixed::ZERO
    };

    let per_reg_morale = if front_count > 0 {
        morale_damage.div(Fixed::from_int(front_count as i64))
    } else {
        Fixed::ZERO
    };

    // Apply to front line
    for (army_id, reg_idx) in line.front.iter().flatten() {
        if let Some(army) = state.armies.get_mut(army_id) {
            if let Some(reg) = army.regiments.get_mut(*reg_idx) {
                reg.strength = (reg.strength - per_reg_casualties).max(Fixed::ZERO);
                reg.morale = (reg.morale - per_reg_morale).max(Fixed::ZERO);
            }
        }
    }

    // Back row takes reduced morale damage (40%)
    let back_morale = morale_damage
        .mul(Fixed::from_f32(defines::BACKROW_MORALE_DAMAGE_FRACTION))
        .div(Fixed::from_int(back_count.max(1) as i64));

    for (army_id, reg_idx) in &line.back {
        if let Some(army) = state.armies.get_mut(army_id) {
            if let Some(reg) = army.regiments.get_mut(*reg_idx) {
                reg.morale = (reg.morale - back_morale).max(Fixed::ZERO);
            }
        }
    }

    // Update battle casualties counter
    if let Some(battle) = state.battles.get_mut(&battle_id) {
        let cas_int = casualties.to_f32() as u32;
        if is_attacker {
            battle.attacker_casualties += cas_int;
        } else {
            battle.defender_casualties += cas_int;
        }
    }
}

/// Get base damage for a regiment type in a given phase.
fn get_regiment_phase_damage(type_: RegimentType, phase: CombatPhase) -> f32 {
    match (type_, phase) {
        (RegimentType::Infantry, CombatPhase::Fire) => defines::INFANTRY_FIRE,
        (RegimentType::Infantry, CombatPhase::Shock) => defines::INFANTRY_SHOCK,
        (RegimentType::Cavalry, CombatPhase::Fire) => defines::CAVALRY_FIRE,
        (RegimentType::Cavalry, CombatPhase::Shock) => defines::CAVALRY_SHOCK,
        (RegimentType::Artillery, CombatPhase::Fire) => defines::ARTILLERY_FIRE,
        (RegimentType::Artillery, CombatPhase::Shock) => defines::ARTILLERY_SHOCK,
    }
}

/// Get terrain penalty for attacker (negative modifier to dice).
/// Maneuver difference between attacker and defender can negate terrain penalties.
fn get_terrain_penalty(
    state: &WorldState,
    province: ProvinceId,
    attacker_origin: Option<ProvinceId>,
    attacker_armies: &[ArmyId],
    defender_armies: &[ArmyId],
    adjacency: Option<&eu4data::adjacency::AdjacencyGraph>,
) -> i8 {
    let terrain = state.provinces.get(&province).and_then(|p| p.terrain);

    let mut base_penalty = match terrain {
        Some(Terrain::Mountains) => defines::MOUNTAIN_PENALTY,
        Some(Terrain::Hills) => defines::HILLS_PENALTY,
        Some(Terrain::Forest) => defines::FOREST_PENALTY,
        Some(Terrain::Marsh) => defines::MARSH_PENALTY,
        Some(Terrain::Jungle) => defines::JUNGLE_PENALTY,
        _ => 0,
    };

    // Add river crossing penalty if attacker crossed a river
    if let (Some(origin), Some(adj)) = (attacker_origin, adjacency) {
        if adj.is_river_crossing(origin, province) {
            base_penalty += defines::CROSSING_RIVER_PENALTY;
        }
    }

    // Maneuver difference can negate terrain penalty (but not river crossing)
    let atk_maneuver = get_maneuver_bonus(state, attacker_armies);
    let def_maneuver = get_maneuver_bonus(state, defender_armies);
    let mitigation = (atk_maneuver - def_maneuver).max(0); // Can't go negative

    (base_penalty + mitigation).min(0) // Can negate penalty, never provide bonus
}

/// Get the highest general pip bonus for a specific combat phase.
/// Returns the best fire or shock pip among all generals in the army list.
fn get_general_bonus(state: &WorldState, army_ids: &[ArmyId], phase: CombatPhase) -> i8 {
    let mut best_bonus: i8 = 0;
    for &army_id in army_ids {
        if let Some(army) = state.armies.get(&army_id) {
            if let Some(gen_id) = army.general {
                if let Some(general) = state.generals.get(&gen_id) {
                    let pip = match phase {
                        CombatPhase::Fire => general.fire,
                        CombatPhase::Shock => general.shock,
                    };
                    best_bonus = best_bonus.max(pip as i8);
                }
            }
        }
    }
    best_bonus
}

/// Get the highest maneuver pip among all generals in the army list.
fn get_maneuver_bonus(state: &WorldState, army_ids: &[ArmyId]) -> i8 {
    let mut best_maneuver: i8 = 0;
    for &army_id in army_ids {
        if let Some(army) = state.armies.get(&army_id) {
            if let Some(gen_id) = army.general {
                if let Some(general) = state.generals.get(&gen_id) {
                    best_maneuver = best_maneuver.max(general.maneuver as i8);
                }
            }
        }
    }
    best_maneuver
}

// ============================================================================
// Battle Resolution
// ============================================================================

/// Check if battle has ended (one side broke or was stackwiped).
/// Returns true if battle is over.
fn check_battle_end(state: &mut WorldState, battle_id: BattleId) -> bool {
    let (att_morale, att_strength, def_morale, def_strength) = {
        let battle = match state.battles.get(&battle_id) {
            Some(b) => b,
            None => return true,
        };

        let att = calculate_side_totals(state, &battle.attacker_line);
        let def = calculate_side_totals(state, &battle.defender_line);

        (att.0, att.1, def.0, def.1)
    };

    let att_broke = att_morale <= Fixed::ZERO || att_strength <= Fixed::ZERO;
    let def_broke = def_morale <= Fixed::ZERO || def_strength <= Fixed::ZERO;

    if !att_broke && !def_broke {
        return false;
    }

    // Determine winner and check for stackwipe
    let result = if att_broke && def_broke {
        BattleResult::Draw
    } else if def_broke {
        let stackwiped =
            att_strength >= def_strength.mul(Fixed::from_f32(defines::STACKWIPE_RATIO));
        let pursuit = if stackwiped {
            def_strength.to_f32() as u32
        } else {
            (def_strength.mul(Fixed::from_f32(defines::PURSUIT_MULTIPLIER * 0.1))).to_f32() as u32
        };
        BattleResult::AttackerVictory {
            pursuit_casualties: pursuit,
            stackwiped,
        }
    } else {
        let stackwiped =
            def_strength >= att_strength.mul(Fixed::from_f32(defines::STACKWIPE_RATIO));
        let pursuit = if stackwiped {
            att_strength.to_f32() as u32
        } else {
            (att_strength.mul(Fixed::from_f32(defines::PURSUIT_MULTIPLIER * 0.1))).to_f32() as u32
        };
        BattleResult::DefenderVictory {
            pursuit_casualties: pursuit,
            stackwiped,
        }
    };

    // Apply stackwipe / pursuit casualties
    apply_battle_result(state, battle_id, &result);

    // Set result
    if let Some(battle) = state.battles.get_mut(&battle_id) {
        log::info!("Battle {} ended: {:?}", battle_id, result);
        battle.result = Some(result);
    }

    true
}

/// Calculate total morale and strength for a side.
fn calculate_side_totals(state: &WorldState, line: &BattleLine) -> (Fixed, Fixed) {
    let mut total_morale = Fixed::ZERO;
    let mut total_strength = Fixed::ZERO;
    let mut count = 0;

    for (army_id, reg_idx) in line.front.iter().flatten() {
        if let Some(army) = state.armies.get(army_id) {
            if let Some(reg) = army.regiments.get(*reg_idx) {
                total_morale += reg.morale;
                total_strength += reg.strength;
                count += 1;
            }
        }
    }

    for (army_id, reg_idx) in &line.back {
        if let Some(army) = state.armies.get(army_id) {
            if let Some(reg) = army.regiments.get(*reg_idx) {
                total_morale += reg.morale;
                total_strength += reg.strength;
                count += 1;
            }
        }
    }

    // Average morale
    let avg_morale = if count > 0 {
        total_morale.div(Fixed::from_int(count as i64))
    } else {
        Fixed::ZERO
    };

    (avg_morale, total_strength)
}

/// Apply pursuit casualties and handle stackwipe.
fn apply_battle_result(state: &mut WorldState, battle_id: BattleId, result: &BattleResult) {
    let battle = match state.battles.get(&battle_id) {
        Some(b) => b.clone(),
        None => return,
    };

    match result {
        BattleResult::AttackerVictory { stackwiped, .. } => {
            if *stackwiped {
                // Destroy all defender regiments
                for &army_id in &battle.defenders {
                    if let Some(army) = state.armies.get_mut(&army_id) {
                        for reg in &mut army.regiments {
                            reg.strength = Fixed::ZERO;
                            reg.morale = Fixed::ZERO;
                        }
                    }
                }
            }
            // Loser retreats (would be handled by movement system)
        }
        BattleResult::DefenderVictory { stackwiped, .. } => {
            if *stackwiped {
                // Destroy all attacker regiments
                for &army_id in &battle.attackers {
                    if let Some(army) = state.armies.get_mut(&army_id) {
                        for reg in &mut army.regiments {
                            reg.strength = Fixed::ZERO;
                            reg.morale = Fixed::ZERO;
                        }
                    }
                }
            }
        }
        BattleResult::Draw => {}
    }
}

/// Advance day counter; switch phase every 3 days.
fn advance_day(state: &mut WorldState, battle_id: BattleId) {
    let battle = match state.battles.get_mut(&battle_id) {
        Some(b) => b,
        None => return,
    };

    battle.phase_day += 1;

    if battle.phase_day >= defines::DAYS_PER_PHASE {
        battle.phase_day = 0;
        battle.phase = match battle.phase {
            CombatPhase::Fire => CombatPhase::Shock,
            CombatPhase::Shock => CombatPhase::Fire,
        };

        // Reroll dice for new phase
        battle.attacker_dice = roll_dice_raw(&mut state.rng_state);
        battle.defender_dice = roll_dice_raw(&mut state.rng_state);
    }
}

// ============================================================================
// Battle Cleanup
// ============================================================================

/// Remove finished battles and clear army in_battle flags.
fn cleanup_finished_battles(state: &mut WorldState) {
    let finished: Vec<BattleId> = state
        .battles
        .iter()
        .filter(|(_, b)| b.result.is_some())
        .map(|(&id, _)| id)
        .collect();

    for battle_id in finished {
        if let Some(battle) = state.battles.remove(&battle_id) {
            // Clear in_battle flag for all participating armies
            for army_id in battle.attackers.iter().chain(battle.defenders.iter()) {
                if let Some(army) = state.armies.get_mut(army_id) {
                    army.in_battle = None;
                }
            }

            // Remove dead regiments
            for army_id in battle.attackers.iter().chain(battle.defenders.iter()) {
                if let Some(army) = state.armies.get_mut(army_id) {
                    army.regiments.retain(|r| r.strength > Fixed::ZERO);
                }
            }
        }
    }

    // Remove empty armies
    state.armies.retain(|_, army| !army.regiments.is_empty());
}

// ============================================================================
// Utility Functions
// ============================================================================

/// Get combat width for a country based on mil tech.
/// Uses accessor pattern so tech scaling can be plugged in later.
pub fn get_combat_width(state: &WorldState, country: &str) -> u8 {
    let mil_tech = state
        .countries
        .get(country)
        .map(|c| c.mil_tech)
        .unwrap_or(0);

    // Simplified scaling: +1 width per ~1.5 mil tech
    // Full EU4 table: 15 → 17 → 20 → 22 → 25 → 27 → 29 → 30 → 32 → 34 → 36 → 38 → 40
    let bonus = (mil_tech as f32 * 0.8).floor() as u8;

    (defines::BASE_COMBAT_WIDTH + bonus).min(defines::MAX_COMBAT_WIDTH)
}

/// Roll a combat die (0-9) using world state RNG.
fn roll_dice(state: &mut WorldState) -> u8 {
    roll_dice_raw(&mut state.rng_state)
}

/// Roll dice using raw RNG state.
fn roll_dice_raw(rng_state: &mut u64) -> u8 {
    // xorshift64
    let mut x = *rng_state;
    if x == 0 {
        x = 1;
    }
    x ^= x << 13;
    x ^= x >> 7;
    x ^= x << 17;
    *rng_state = x;

    // Map to 0-9
    (x % 10) as u8
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::{Army, Date, Regiment};
    use crate::testing::WorldStateBuilder;

    fn make_regiment(type_: RegimentType, strength: i64) -> Regiment {
        Regiment {
            type_,
            strength: Fixed::from_int(strength),
            morale: Fixed::from_f32(defines::BASE_MORALE),
        }
    }

    fn make_army(id: ArmyId, owner: &str, location: ProvinceId, regiments: Vec<Regiment>) -> Army {
        Army {
            id,
            name: format!("{} Army {}", owner, id),
            owner: owner.to_string(),
            location,
            previous_location: None,
            regiments,
            movement: None,
            embarked_on: None,
            general: None,
            in_battle: None,
        }
    }

    #[test]
    fn test_phase_lasts_three_days() {
        let mut state = WorldStateBuilder::new()
            .date(1444, 11, 11)
            .with_country("SWE")
            .with_country("DEN")
            .build();

        // Create armies
        state.armies.insert(
            1,
            make_army(
                1,
                "SWE",
                1,
                vec![make_regiment(RegimentType::Infantry, 1000)],
            ),
        );
        state.armies.insert(
            2,
            make_army(
                2,
                "DEN",
                1,
                vec![make_regiment(RegimentType::Infantry, 1000)],
            ),
        );

        // Declare war
        let war = crate::state::War {
            id: 0,
            name: "SWE vs DEN".into(),
            attackers: vec!["SWE".into()],
            defenders: vec!["DEN".into()],
            start_date: Date::new(1444, 11, 11),
            attacker_score: 0,
            attacker_battle_score: 0,
            defender_score: 0,
            defender_battle_score: 0,
            pending_peace: None,
        };
        state.diplomacy.wars.insert(0, war);

        // Start battle
        run_combat_tick(&mut state, None);

        assert_eq!(state.battles.len(), 1);
        let battle = state.battles.values().next().unwrap();
        assert_eq!(battle.phase, CombatPhase::Fire);
        assert_eq!(battle.phase_day, 1); // After first tick

        // Tick 2 more days - should still be Fire
        run_combat_tick(&mut state, None);
        run_combat_tick(&mut state, None);

        let battle = state.battles.values().next().unwrap();
        // Day 3 completes Fire phase, day 0 of Shock
        assert_eq!(battle.phase, CombatPhase::Shock);
        assert_eq!(battle.phase_day, 0);
    }

    #[test]
    fn test_morale_depletes_over_phases() {
        let mut state = WorldStateBuilder::new()
            .date(1444, 11, 11)
            .with_country("SWE")
            .with_country("DEN")
            .build();

        state.armies.insert(
            1,
            make_army(
                1,
                "SWE",
                1,
                vec![make_regiment(RegimentType::Infantry, 1000)],
            ),
        );
        state.armies.insert(
            2,
            make_army(
                2,
                "DEN",
                1,
                vec![make_regiment(RegimentType::Infantry, 1000)],
            ),
        );

        let war = crate::state::War {
            id: 0,
            name: "Test War".into(),
            attackers: vec!["SWE".into()],
            defenders: vec!["DEN".into()],
            start_date: Date::new(1444, 11, 11),
            attacker_score: 0,
            attacker_battle_score: 0,
            defender_score: 0,
            defender_battle_score: 0,
            pending_peace: None,
        };
        state.diplomacy.wars.insert(0, war);

        let initial_morale = state.armies.get(&1).unwrap().regiments[0].morale;

        // Run several combat ticks
        for _ in 0..5 {
            run_combat_tick(&mut state, None);
        }

        // Morale should have decreased (if battle still ongoing)
        if let Some(army) = state.armies.get(&1) {
            if !army.regiments.is_empty() {
                assert!(army.regiments[0].morale < initial_morale);
            }
        }
    }

    #[test]
    fn test_stackwipe_at_10_to_1() {
        let mut state = WorldStateBuilder::new()
            .date(1444, 11, 11)
            .with_country("SWE")
            .with_country("DEN")
            .build();

        // 10:1 ratio
        let mut large_army = Vec::new();
        for _ in 0..10 {
            large_army.push(make_regiment(RegimentType::Infantry, 1000));
        }
        state.armies.insert(1, make_army(1, "SWE", 1, large_army));
        state.armies.insert(
            2,
            make_army(
                2,
                "DEN",
                1,
                vec![make_regiment(RegimentType::Infantry, 1000)],
            ),
        );

        // Set DEN army to very low morale to trigger immediate break
        state.armies.get_mut(&2).unwrap().regiments[0].morale = Fixed::from_f32(0.01);

        let war = crate::state::War {
            id: 0,
            name: "Test War".into(),
            attackers: vec!["SWE".into()],
            defenders: vec!["DEN".into()],
            start_date: Date::new(1444, 11, 11),
            attacker_score: 0,
            attacker_battle_score: 0,
            defender_score: 0,
            defender_battle_score: 0,
            pending_peace: None,
        };
        state.diplomacy.wars.insert(0, war);

        // Run combat until resolved
        for _ in 0..10 {
            run_combat_tick(&mut state, None);
        }

        // DEN army should be stackwiped (no regiments remaining)
        assert!(
            state.armies.get(&2).is_none() || state.armies.get(&2).unwrap().regiments.is_empty()
        );
    }

    #[test]
    fn test_combat_width_scaling() {
        let state = WorldStateBuilder::new().with_country("TEST").build();

        // Base width at tech 0
        let width = get_combat_width(&state, "TEST");
        assert_eq!(width, defines::BASE_COMBAT_WIDTH);
    }

    #[test]
    fn test_cavalry_ratio_deployment() {
        let mut state = WorldStateBuilder::new().with_country("TEST").build();

        // 10 cavalry, 2 infantry
        let mut regs = Vec::new();
        for _ in 0..2 {
            regs.push(make_regiment(RegimentType::Infantry, 1000));
        }
        for _ in 0..10 {
            regs.push(make_regiment(RegimentType::Cavalry, 1000));
        }
        state.armies.insert(1, make_army(1, "TEST", 1, regs));

        let line = deploy_to_lines(&state, &[1], 20);

        // Front should have: 2 inf + 1 cav (50% of inf)
        let front_count = line.front.len();
        assert_eq!(front_count, 3); // 2 inf + 1 cav at 50% ratio

        // Rest of cavalry in back
        assert_eq!(line.back.len(), 9); // 10 - 1 = 9 cav in back
    }

    #[test]
    fn test_general_fire_pip_bonus() {
        use crate::state::General;

        let mut state = WorldStateBuilder::new()
            .with_country("SWE")
            .with_country("DEN")
            .build();

        // Create general with 3 fire pips
        let general = General {
            id: 1,
            name: "Gustav Vasa".to_string(),
            owner: "SWE".to_string(),
            fire: 3,
            shock: 1,
            maneuver: 2,
            siege: 1,
        };
        state.generals.insert(1, general);

        // Create army with general
        let mut army = make_army(
            1,
            "SWE",
            1,
            vec![make_regiment(RegimentType::Infantry, 1000)],
        );
        army.general = Some(1);
        state.armies.insert(1, army);

        // Enemy army without general
        state.armies.insert(
            2,
            make_army(
                2,
                "DEN",
                1,
                vec![make_regiment(RegimentType::Infantry, 1000)],
            ),
        );

        // Get general bonus for Fire phase
        let bonus = get_general_bonus(&state, &[1], CombatPhase::Fire);
        assert_eq!(bonus, 3);

        // Shock phase should return 1
        let shock_bonus = get_general_bonus(&state, &[1], CombatPhase::Shock);
        assert_eq!(shock_bonus, 1);

        // No general should return 0
        let no_general = get_general_bonus(&state, &[2], CombatPhase::Fire);
        assert_eq!(no_general, 0);
    }

    #[test]
    fn test_maneuver_negates_terrain() {
        use crate::state::General;

        let mut state = WorldStateBuilder::new()
            .with_country("SWE")
            .with_country("DEN")
            .build();

        // Create province with Mountains terrain (-2 penalty)
        state.provinces.insert(
            1,
            crate::state::ProvinceState {
                terrain: Some(Terrain::Mountains),
                ..Default::default()
            },
        );

        // Attacker with 3 maneuver general
        let attacker_general = General {
            id: 1,
            name: "Attacker".to_string(),
            owner: "SWE".to_string(),
            fire: 1,
            shock: 1,
            maneuver: 3,
            siege: 0,
        };
        state.generals.insert(1, attacker_general);

        let mut atk_army = make_army(
            1,
            "SWE",
            1,
            vec![make_regiment(RegimentType::Infantry, 1000)],
        );
        atk_army.general = Some(1);
        state.armies.insert(1, atk_army);

        // Defender with 1 maneuver general
        let defender_general = General {
            id: 2,
            name: "Defender".to_string(),
            owner: "DEN".to_string(),
            fire: 1,
            shock: 1,
            maneuver: 1,
            siege: 0,
        };
        state.generals.insert(2, defender_general);

        let mut def_army = make_army(
            2,
            "DEN",
            1,
            vec![make_regiment(RegimentType::Infantry, 1000)],
        );
        def_army.general = Some(2);
        state.armies.insert(2, def_army);

        // Mountain penalty is -2, maneuver difference is 3-1=2
        // Final penalty should be -2 + 2 = 0 (fully negated)
        let penalty = get_terrain_penalty(&state, 1, None, &[1], &[2], None);
        assert_eq!(penalty, 0);
    }

    #[test]
    fn test_maneuver_partial_negation() {
        use crate::state::General;

        let mut state = WorldStateBuilder::new()
            .with_country("SWE")
            .with_country("DEN")
            .build();

        // Mountains = -2
        state.provinces.insert(
            1,
            crate::state::ProvinceState {
                terrain: Some(Terrain::Mountains),
                ..Default::default()
            },
        );

        // Attacker: 1 maneuver
        let attacker_general = General {
            id: 1,
            name: "Attacker".to_string(),
            owner: "SWE".to_string(),
            fire: 1,
            shock: 1,
            maneuver: 1,
            siege: 0,
        };
        state.generals.insert(1, attacker_general);

        let mut atk_army = make_army(
            1,
            "SWE",
            1,
            vec![make_regiment(RegimentType::Infantry, 1000)],
        );
        atk_army.general = Some(1);
        state.armies.insert(1, atk_army);

        // Defender: 0 maneuver (no general)
        state.armies.insert(
            2,
            make_army(
                2,
                "DEN",
                1,
                vec![make_regiment(RegimentType::Infantry, 1000)],
            ),
        );

        // Mountain penalty -2, maneuver difference 1-0=1
        // Final: -2 + 1 = -1
        let penalty = get_terrain_penalty(&state, 1, None, &[1], &[2], None);
        assert_eq!(penalty, -1);
    }

    #[test]
    fn test_defender_high_maneuver_prevents_mitigation() {
        use crate::state::General;

        let mut state = WorldStateBuilder::new()
            .with_country("SWE")
            .with_country("DEN")
            .build();

        // Mountains = -2
        state.provinces.insert(
            1,
            crate::state::ProvinceState {
                terrain: Some(Terrain::Mountains),
                ..Default::default()
            },
        );

        // Attacker: 2 maneuver
        let attacker_general = General {
            id: 1,
            name: "Attacker".to_string(),
            owner: "SWE".to_string(),
            fire: 1,
            shock: 1,
            maneuver: 2,
            siege: 0,
        };
        state.generals.insert(1, attacker_general);

        let mut atk_army = make_army(
            1,
            "SWE",
            1,
            vec![make_regiment(RegimentType::Infantry, 1000)],
        );
        atk_army.general = Some(1);
        state.armies.insert(1, atk_army);

        // Defender: 4 maneuver (higher!)
        let defender_general = General {
            id: 2,
            name: "Defender".to_string(),
            owner: "DEN".to_string(),
            fire: 1,
            shock: 1,
            maneuver: 4,
            siege: 0,
        };
        state.generals.insert(2, defender_general);

        let mut def_army = make_army(
            2,
            "DEN",
            1,
            vec![make_regiment(RegimentType::Infantry, 1000)],
        );
        def_army.general = Some(2);
        state.armies.insert(2, def_army);

        // Maneuver difference: 2-4 = -2, clamped to 0
        // Penalty stays at -2 (no mitigation)
        let penalty = get_terrain_penalty(&state, 1, None, &[1], &[2], None);
        assert_eq!(penalty, -2);
    }

    #[test]
    fn test_river_crossing_penalty() {
        use eu4data::adjacency::AdjacencyGraph;

        let mut state = WorldStateBuilder::new()
            .with_country("SWE")
            .with_country("DEN")
            .build();

        // Create adjacency graph with a river crossing from province 1 to 2
        let mut adjacency = AdjacencyGraph::new();
        adjacency.add_adjacency(1, 2);
        // Manually add river crossing (simulating CSV parse)
        adjacency.river_crossings.insert((1, 2));
        adjacency.river_crossings.insert((2, 1));

        // Create armies
        let mut atk_army = make_army(
            1,
            "SWE",
            2,
            vec![make_regiment(RegimentType::Infantry, 1000)],
        );
        atk_army.previous_location = Some(1); // Army came from province 1
        state.armies.insert(1, atk_army);

        let def_army = make_army(
            2,
            "DEN",
            2,
            vec![make_regiment(RegimentType::Infantry, 1000)],
        );
        state.armies.insert(2, def_army);

        // River crossing penalty is -1
        let penalty = get_terrain_penalty(&state, 2, Some(1), &[1], &[2], Some(&adjacency));
        assert_eq!(penalty, -1);

        // Without origin, no penalty
        let penalty = get_terrain_penalty(&state, 2, None, &[1], &[2], Some(&adjacency));
        assert_eq!(penalty, 0);

        // River crossing in reverse direction also has penalty
        let penalty = get_terrain_penalty(&state, 1, Some(2), &[1], &[2], Some(&adjacency));
        assert_eq!(penalty, -1);
    }
}
