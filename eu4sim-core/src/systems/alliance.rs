//! Alliance enforcement mechanics - call-to-arms acceptance and decline.
//!
//! Implements EU4-authentic alliance behavior:
//! - Defensive CTAs: allies auto-join the defender's side
//! - Offensive CTAs: allies get a choice (pending_call_to_arms)
//! - Decline penalties: -25 prestige, alliance breaks, -10 trust with all allies
//! - Acceptance factors: trust, debt, conflicting wars, relations with target

use crate::fixed::Fixed;
use crate::input::WarSide;
use crate::state::{Tag, WarId, WorldState};

/// Calculate AI acceptance score for a call-to-arms.
///
/// Positive score = likely to accept, negative = likely to decline.
/// Based on EU4 wiki mechanics:
/// - Trust: At 50 neutral, +0.5 per point above, -2 per point below
/// - Debt: -1000 if in debt (loans > 0 or negative treasury)
/// - Destabilization: -50 per missing stability point
/// - Other factors: TODO relations with target, war exhaustion
///
/// Returns a score that AI can use to decide whether to honor the CTA.
pub fn calculate_cta_acceptance_score(
    state: &WorldState,
    ally: &Tag,
    caller: &Tag,
    _war_id: WarId,
) -> i32 {
    let mut score = 0;

    // 1. Trust factor (±50 points swing)
    let trust = state.diplomacy.get_trust(ally, caller);
    let trust_delta = trust.to_f32() - 50.0; // Neutral at 50

    if trust_delta > 0.0 {
        // Above 50: +0.5 per point
        score += (trust_delta * 0.5) as i32;
    } else {
        // Below 50: -2 per point
        score += (trust_delta * 2.0) as i32;
    }

    // 2. Debt penalty (-1000 if in debt)
    let ally_country = state.countries.get(ally).expect("Ally country must exist");
    let in_debt = ally_country.loans > 0 || ally_country.treasury.to_f32() < 0.0;
    if in_debt {
        score -= 1000;
    }

    // 3. Destabilization penalty (-50 per missing stability point)
    let stability = ally_country.stability.get();
    if stability < 0 {
        score += stability * 50; // Negative stability = negative score
    }

    // 4. TODO: Relations with war target
    // 5. TODO: War exhaustion
    // 6. TODO: Other modifiers (diplomatic ideas, etc.)

    score
}

/// Check if accepting a CTA would create a conflicting war.
///
/// Returns true if the ally is already at war with any participant on the same side,
/// or is allied to any participant on the opposing side.
pub fn would_create_conflicting_war(state: &WorldState, ally: &Tag, war_id: WarId) -> bool {
    let war = match state.diplomacy.wars.get(&war_id) {
        Some(w) => w,
        None => return false,
    };

    // Check if ally is already at war with any participant
    for participant in war.attackers.iter().chain(war.defenders.iter()) {
        if state.diplomacy.are_at_war(ally, participant) {
            return true; // Already fighting this participant
        }
    }

    // Check if ally is allied to anyone on the opposing side
    // If we'd join attackers, check if allied to any defender
    // If we'd join defenders, check if allied to any attacker
    let pending_side = state
        .countries
        .get(ally)
        .and_then(|c| c.pending_call_to_arms.get(&war_id))
        .copied();

    match pending_side {
        Some(WarSide::Attacker) => {
            // Joining attackers - check if allied to any defender
            for defender in &war.defenders {
                if state.diplomacy.has_alliance(ally, defender) {
                    return true;
                }
            }
        }
        Some(WarSide::Defender) => {
            // Joining defenders - check if allied to any attacker
            for attacker in &war.attackers {
                if state.diplomacy.has_alliance(ally, attacker) {
                    return true;
                }
            }
        }
        None => {
            // No pending CTA - shouldn't happen, but be safe
            return false;
        }
    }

    false
}

/// Decline a call-to-arms.
///
/// Consequences per EU4 wiki:
/// - Lose 25 prestige
/// - Alliance with caller breaks
/// - Lose 10 trust with ALL allies (not just the caller)
/// - Pending CTA is removed
pub fn decline_call_to_arms(state: &mut WorldState, ally: &Tag, war_id: WarId) {
    // 1. Find the caller (who issued the CTA)
    let war = match state.diplomacy.wars.get(&war_id) {
        Some(w) => w,
        None => return, // War ended already
    };

    // Determine who called us (check which side we were called to)
    let side = match state
        .countries
        .get(ally)
        .and_then(|c| c.pending_call_to_arms.get(&war_id))
    {
        Some(s) => *s,
        None => return, // No pending CTA
    };

    // Find caller from the war participants on our side
    let potential_callers: Vec<Tag> = match side {
        WarSide::Attacker => war.attackers.clone(),
        WarSide::Defender => war.defenders.clone(),
    };

    // 2. Get all allies BEFORE breaking alliances (for trust penalty)
    let all_allies = state.diplomacy.get_allies(ally);

    // 3. Apply prestige penalty (-25)
    if let Some(ally_country) = state.countries.get_mut(ally) {
        ally_country.prestige.add(Fixed::from_int(-25));

        // Remove pending CTA
        ally_country.pending_call_to_arms.remove(&war_id);
    }

    // 4. Break alliance with all callers
    for caller in &potential_callers {
        if caller != ally {
            // Only break if actually allied
            if state.diplomacy.has_alliance(ally, caller) {
                state.diplomacy.remove_alliance(ally, caller);
            }
        }
    }

    // 5. Lose 10 trust with ALL allies (including the ones we just broke alliance with)
    for other_ally in all_allies {
        state
            .diplomacy
            .modify_trust(ally, &other_ally, Fixed::from_int(-10));
    }
}

/// Accept a call-to-arms and join the war.
///
/// Effects:
/// - Join the war on the specified side
/// - Gain +5 trust with the caller
/// - Remove pending CTA
pub fn accept_call_to_arms(state: &mut WorldState, ally: &Tag, war_id: WarId, side: WarSide) {
    // 1. Join the war
    if let Some(war) = state.diplomacy.wars.get_mut(&war_id) {
        let participants = match side {
            WarSide::Attacker => &mut war.attackers,
            WarSide::Defender => &mut war.defenders,
        };

        if !participants.contains(&ally.to_string()) {
            participants.push(ally.clone());
        }
    }

    // 2. Grant trust bonus to caller(s)
    let war = match state.diplomacy.wars.get(&war_id) {
        Some(w) => w,
        None => return,
    };

    let callers: Vec<Tag> = match side {
        WarSide::Attacker => war.attackers.clone(),
        WarSide::Defender => war.defenders.clone(),
    };

    for caller in &callers {
        if caller != ally && state.diplomacy.has_alliance(ally, caller) {
            state
                .diplomacy
                .modify_trust(ally, caller, Fixed::from_int(5));
        }
    }

    // 3. Remove pending CTA
    if let Some(ally_country) = state.countries.get_mut(ally) {
        ally_country.pending_call_to_arms.remove(&war_id);
    }
}
