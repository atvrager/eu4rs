//! Tests for alliance enforcement mechanics (call-to-arms acceptance/decline).
//!
//! These tests verify EU4-authentic alliance behavior:
//! - Defensive CTAs: auto-join
//! - Offensive CTAs: choice with consequences
//! - Decline penalties: -25 prestige, alliance break, trust loss
//! - Acceptance factors: trust, debt, conflicting wars

use super::*;
use crate::fixed::Fixed;
use crate::input::{Command, WarSide};
use crate::testing::WorldStateBuilder;

// ============================================================================
// Decline Penalty Tests
// ============================================================================

#[test]
fn test_decline_cta_loses_prestige() {
    use crate::state::RelationType;

    // SWE and NOR are allies. SWE declares war on DEN.
    let mut state = WorldStateBuilder::new()
        .date(1444, 12, 11)
        .with_country("SWE")
        .with_country("DEN")
        .with_country("NOR")
        .build();

    // Set NOR's prestige to 50 to track the penalty
    state
        .countries
        .get_mut("NOR")
        .unwrap()
        .prestige
        .set(Fixed::from_int(50));

    // Create SWE-NOR alliance
    state.diplomacy.relations.insert(
        ("NOR".to_string(), "SWE".to_string()),
        RelationType::Alliance,
    );

    // SWE declares war on DEN (creates pending CTA for NOR)
    execute_command(
        &mut state,
        "SWE",
        &Command::DeclareWar {
            target: "DEN".to_string(),
            cb: None,
        },
        None,
    )
    .unwrap();

    let war_id = *state.diplomacy.wars.keys().next().unwrap();

    // NOR declines by doing nothing (or explicitly declining)
    // For now, we'll implement decline as a separate command or timeout
    // But let's test explicit decline command
    execute_command(
        &mut state,
        "NOR",
        &Command::DeclineCallToArms { war_id },
        None,
    )
    .unwrap();

    // NOR should lose 25 prestige (was 50, now 25)
    let nor_prestige = state.countries.get("NOR").unwrap().prestige.get();
    assert_eq!(
        nor_prestige,
        Fixed::from_int(25),
        "Declining CTA should lose 25 prestige"
    );
}

#[test]
fn test_decline_cta_breaks_alliance() {
    use crate::state::RelationType;

    let mut state = WorldStateBuilder::new()
        .date(1444, 12, 11)
        .with_country("SWE")
        .with_country("DEN")
        .with_country("NOR")
        .build();

    // Create SWE-NOR alliance
    let alliance_key = ("NOR".to_string(), "SWE".to_string());
    state
        .diplomacy
        .relations
        .insert(alliance_key.clone(), RelationType::Alliance);

    // SWE declares war on DEN
    execute_command(
        &mut state,
        "SWE",
        &Command::DeclareWar {
            target: "DEN".to_string(),
            cb: None,
        },
        None,
    )
    .unwrap();

    let war_id = *state.diplomacy.wars.keys().next().unwrap();

    // NOR declines the call-to-arms
    execute_command(
        &mut state,
        "NOR",
        &Command::DeclineCallToArms { war_id },
        None,
    )
    .unwrap();

    // Alliance should be broken
    assert!(
        !state.diplomacy.relations.contains_key(&alliance_key),
        "Alliance should break when declining CTA"
    );
}

#[test]
fn test_decline_cta_loses_trust_with_all_allies() {
    use crate::state::RelationType;

    // NOR is allied with both SWE and FIN.
    // SWE declares war, NOR declines.
    // NOR should lose trust with both SWE and FIN.
    let mut state = WorldStateBuilder::new()
        .date(1444, 12, 11)
        .with_country("SWE")
        .with_country("DEN")
        .with_country("NOR")
        .with_country("FIN")
        .build();

    // Create SWE-NOR and FIN-NOR alliances
    state.diplomacy.relations.insert(
        ("NOR".to_string(), "SWE".to_string()),
        RelationType::Alliance,
    );
    state.diplomacy.relations.insert(
        ("FIN".to_string(), "NOR".to_string()),
        RelationType::Alliance,
    );

    // Set initial trust values (50 with both)
    state.diplomacy.set_trust("NOR", "SWE", Fixed::from_int(50));
    state.diplomacy.set_trust("NOR", "FIN", Fixed::from_int(50));

    // SWE declares war on DEN
    execute_command(
        &mut state,
        "SWE",
        &Command::DeclareWar {
            target: "DEN".to_string(),
            cb: None,
        },
        None,
    )
    .unwrap();

    let war_id = *state.diplomacy.wars.keys().next().unwrap();

    // NOR declines
    execute_command(
        &mut state,
        "NOR",
        &Command::DeclineCallToArms { war_id },
        None,
    )
    .unwrap();

    // NOR should lose 10 trust with both SWE and FIN
    assert_eq!(
        state.diplomacy.get_trust("NOR", "SWE").to_f32(),
        40.0,
        "Should lose 10 trust with caller"
    );
    assert_eq!(
        state.diplomacy.get_trust("NOR", "FIN").to_f32(),
        40.0,
        "Should lose 10 trust with all allies"
    );
}

#[test]
fn test_decline_cta_clears_pending() {
    use crate::state::RelationType;

    let mut state = WorldStateBuilder::new()
        .date(1444, 12, 11)
        .with_country("SWE")
        .with_country("DEN")
        .with_country("NOR")
        .build();

    state.diplomacy.relations.insert(
        ("NOR".to_string(), "SWE".to_string()),
        RelationType::Alliance,
    );

    // SWE declares war on DEN
    execute_command(
        &mut state,
        "SWE",
        &Command::DeclareWar {
            target: "DEN".to_string(),
            cb: None,
        },
        None,
    )
    .unwrap();

    let war_id = *state.diplomacy.wars.keys().next().unwrap();

    // Verify pending CTA exists
    assert!(state
        .countries
        .get("NOR")
        .unwrap()
        .pending_call_to_arms
        .contains_key(&war_id));

    // NOR declines
    execute_command(
        &mut state,
        "NOR",
        &Command::DeclineCallToArms { war_id },
        None,
    )
    .unwrap();

    // Pending CTA should be cleared
    assert!(
        !state
            .countries
            .get("NOR")
            .unwrap()
            .pending_call_to_arms
            .contains_key(&war_id),
        "Pending CTA should be cleared after declining"
    );
}

// ============================================================================
// Accept Bonus Tests
// ============================================================================

#[test]
fn test_accept_cta_joins_war() {
    use crate::state::RelationType;

    let mut state = WorldStateBuilder::new()
        .date(1444, 12, 11)
        .with_country("SWE")
        .with_country("DEN")
        .with_country("NOR")
        .build();

    state.diplomacy.relations.insert(
        ("NOR".to_string(), "SWE".to_string()),
        RelationType::Alliance,
    );

    // SWE declares war on DEN
    execute_command(
        &mut state,
        "SWE",
        &Command::DeclareWar {
            target: "DEN".to_string(),
            cb: None,
        },
        None,
    )
    .unwrap();

    let war_id = *state.diplomacy.wars.keys().next().unwrap();

    // NOR accepts (via JoinWar command)
    execute_command(
        &mut state,
        "NOR",
        &Command::JoinWar {
            war_id,
            side: WarSide::Attacker,
        },
        None,
    )
    .unwrap();

    // NOR should be in the war as attacker
    let war = state.diplomacy.wars.get(&war_id).unwrap();
    assert!(
        war.attackers.contains(&"NOR".to_string()),
        "NOR should join war after accepting CTA"
    );

    // Pending CTA should be cleared
    assert!(
        !state
            .countries
            .get("NOR")
            .unwrap()
            .pending_call_to_arms
            .contains_key(&war_id),
        "Pending CTA should be cleared after accepting"
    );
}

#[test]
fn test_accept_cta_grants_trust_bonus() {
    use crate::state::RelationType;

    let mut state = WorldStateBuilder::new()
        .date(1444, 12, 11)
        .with_country("SWE")
        .with_country("DEN")
        .with_country("NOR")
        .build();

    state.diplomacy.relations.insert(
        ("NOR".to_string(), "SWE".to_string()),
        RelationType::Alliance,
    );

    // Set initial trust
    state.diplomacy.set_trust("NOR", "SWE", Fixed::from_int(50));

    // SWE declares war on DEN
    execute_command(
        &mut state,
        "SWE",
        &Command::DeclareWar {
            target: "DEN".to_string(),
            cb: None,
        },
        None,
    )
    .unwrap();

    let war_id = *state.diplomacy.wars.keys().next().unwrap();

    // NOR accepts
    execute_command(
        &mut state,
        "NOR",
        &Command::JoinWar {
            war_id,
            side: WarSide::Attacker,
        },
        None,
    )
    .unwrap();

    // NOR should gain +5 trust with SWE
    assert_eq!(
        state.diplomacy.get_trust("NOR", "SWE").to_f32(),
        55.0,
        "Should gain trust with caller for honoring CTA"
    );
}

// ============================================================================
// Manual CallAllyToWar Command Tests
// ============================================================================

#[test]
fn test_call_ally_to_war_creates_pending_cta() {
    use crate::state::RelationType;

    let mut state = WorldStateBuilder::new()
        .date(1444, 12, 11)
        .with_country("SWE")
        .with_country("DEN")
        .with_country("NOR")
        .build();

    // Create SWE-NOR alliance
    state.diplomacy.relations.insert(
        ("NOR".to_string(), "SWE".to_string()),
        RelationType::Alliance,
    );

    // SWE declares war on DEN (NOR gets pending CTA automatically)
    execute_command(
        &mut state,
        "SWE",
        &Command::DeclareWar {
            target: "DEN".to_string(),
            cb: None,
        },
        None,
    )
    .unwrap();

    let war_id = *state.diplomacy.wars.keys().next().unwrap();

    // Verify NOR has pending CTA
    assert!(state
        .countries
        .get("NOR")
        .unwrap()
        .pending_call_to_arms
        .contains_key(&war_id));
}

#[test]
fn test_call_ally_to_war_requires_alliance() {
    let mut state = WorldStateBuilder::new()
        .date(1444, 12, 11)
        .with_country("SWE")
        .with_country("DEN")
        .with_country("NOR")
        .build();

    // No alliance between SWE and NOR

    // SWE declares war on DEN
    execute_command(
        &mut state,
        "SWE",
        &Command::DeclareWar {
            target: "DEN".to_string(),
            cb: None,
        },
        None,
    )
    .unwrap();

    let war_id = *state.diplomacy.wars.keys().next().unwrap();

    // SWE tries to manually call NOR (not an ally)
    let result = execute_command(
        &mut state,
        "SWE",
        &Command::CallAllyToWar {
            ally: "NOR".to_string(),
            war_id,
        },
        None,
    );

    assert!(result.is_err(), "Cannot call non-ally to war");
}

#[test]
fn test_call_ally_to_war_requires_participation() {
    use crate::state::RelationType;

    let mut state = WorldStateBuilder::new()
        .date(1444, 12, 11)
        .with_country("SWE")
        .with_country("DEN")
        .with_country("NOR")
        .with_country("FIN")
        .build();

    // Create SWE-NOR alliance
    state.diplomacy.relations.insert(
        ("NOR".to_string(), "SWE".to_string()),
        RelationType::Alliance,
    );

    // SWE declares war on DEN
    execute_command(
        &mut state,
        "SWE",
        &Command::DeclareWar {
            target: "DEN".to_string(),
            cb: None,
        },
        None,
    )
    .unwrap();

    let war_id = *state.diplomacy.wars.keys().next().unwrap();

    // FIN tries to call NOR to war (but FIN isn't in the war)
    let result = execute_command(
        &mut state,
        "FIN",
        &Command::CallAllyToWar {
            ally: "NOR".to_string(),
            war_id,
        },
        None,
    );

    assert!(
        result.is_err(),
        "Cannot call ally to war if not participating"
    );
}

// ============================================================================
// Conflicting War Detection Tests
// ============================================================================

#[test]
fn test_conflicting_war_prevents_cta() {
    use crate::state::RelationType;

    // NOR is allied with both SWE and DEN.
    // SWE declares war on DEN.
    // NOR should NOT get a CTA since joining either side conflicts.
    let mut state = WorldStateBuilder::new()
        .date(1444, 12, 11)
        .with_country("SWE")
        .with_country("DEN")
        .with_country("NOR")
        .build();

    // Create SWE-NOR and DEN-NOR alliances
    state.diplomacy.relations.insert(
        ("NOR".to_string(), "SWE".to_string()),
        RelationType::Alliance,
    );
    state.diplomacy.relations.insert(
        ("DEN".to_string(), "NOR".to_string()),
        RelationType::Alliance,
    );

    // SWE declares war on DEN
    execute_command(
        &mut state,
        "SWE",
        &Command::DeclareWar {
            target: "DEN".to_string(),
            cb: None,
        },
        None,
    )
    .unwrap();

    let war_id = *state.diplomacy.wars.keys().next().unwrap();

    // NOR should NOT have a pending CTA (conflicting alliances)
    assert!(
        !state
            .countries
            .get("NOR")
            .unwrap()
            .pending_call_to_arms
            .contains_key(&war_id),
        "Should not create CTA when ally is allied to both sides"
    );
}

#[test]
#[ignore] // TODO: Requires integration with DeclareWar to check conflicts when auto-creating CTAs
fn test_conflicting_war_already_fighting() {
    use crate::state::RelationType;

    // NOR is already at war with DEN.
    // SWE (allied to NOR) declares war on FIN (allied to DEN).
    // NOR cannot join SWE's side because that would put them on same side as DEN.
    let mut state = WorldStateBuilder::new()
        .date(1444, 12, 11)
        .with_country("SWE")
        .with_country("DEN")
        .with_country("NOR")
        .with_country("FIN")
        .build();

    // Create alliances: SWE-NOR, FIN-DEN
    state.diplomacy.relations.insert(
        ("NOR".to_string(), "SWE".to_string()),
        RelationType::Alliance,
    );
    state.diplomacy.relations.insert(
        ("DEN".to_string(), "FIN".to_string()),
        RelationType::Alliance,
    );

    // NOR declares war on DEN (War 1: NOR vs DEN)
    execute_command(
        &mut state,
        "NOR",
        &Command::DeclareWar {
            target: "DEN".to_string(),
            cb: None,
        },
        None,
    )
    .unwrap();

    // SWE declares war on FIN (War 2: SWE vs FIN)
    // DEN joins defensively on FIN's side
    execute_command(
        &mut state,
        "SWE",
        &Command::DeclareWar {
            target: "FIN".to_string(),
            cb: None,
        },
        None,
    )
    .unwrap();

    // Find war 2
    let war2_id = state
        .diplomacy
        .wars
        .values()
        .find(|w| w.attackers.contains(&"SWE".to_string()))
        .map(|w| w.id)
        .unwrap();

    // NOR should NOT get a CTA for war 2 (already fighting against DEN in war 1)
    assert!(
        !state
            .countries
            .get("NOR")
            .unwrap()
            .pending_call_to_arms
            .contains_key(&war2_id),
        "Should not create CTA when ally would fight against existing enemy"
    );
}

// ============================================================================
// Acceptance Logic Tests (AI behavior)
// ============================================================================

#[test]
fn test_ai_accepts_with_high_trust() {
    // This will be tested via AI scoring in greedy.rs
    // For now, just verify the acceptance scoring function exists
    use crate::state::RelationType;
    use crate::systems::calculate_cta_acceptance_score;

    let mut state = WorldStateBuilder::new()
        .date(1444, 12, 11)
        .with_country("SWE")
        .with_country("DEN")
        .with_country("NOR")
        .build();

    state.diplomacy.relations.insert(
        ("NOR".to_string(), "SWE".to_string()),
        RelationType::Alliance,
    );

    // High trust (80)
    state.diplomacy.set_trust("NOR", "SWE", Fixed::from_int(80));

    // SWE declares war on DEN
    execute_command(
        &mut state,
        "SWE",
        &Command::DeclareWar {
            target: "DEN".to_string(),
            cb: None,
        },
        None,
    )
    .unwrap();

    let war_id = *state.diplomacy.wars.keys().next().unwrap();

    // Calculate acceptance score
    let score =
        calculate_cta_acceptance_score(&state, &"NOR".to_string(), &"SWE".to_string(), war_id);

    // High trust should give positive score (80 - 50 = 30, * 0.5 = +15)
    assert!(
        score > 0,
        "High trust should result in positive acceptance score"
    );
}

#[test]
fn test_ai_declines_with_low_trust() {
    use crate::state::RelationType;
    use crate::systems::calculate_cta_acceptance_score;

    let mut state = WorldStateBuilder::new()
        .date(1444, 12, 11)
        .with_country("SWE")
        .with_country("DEN")
        .with_country("NOR")
        .build();

    state.diplomacy.relations.insert(
        ("NOR".to_string(), "SWE".to_string()),
        RelationType::Alliance,
    );

    // Low trust (20)
    state.diplomacy.set_trust("NOR", "SWE", Fixed::from_int(20));

    // SWE declares war on DEN
    execute_command(
        &mut state,
        "SWE",
        &Command::DeclareWar {
            target: "DEN".to_string(),
            cb: None,
        },
        None,
    )
    .unwrap();

    let war_id = *state.diplomacy.wars.keys().next().unwrap();

    // Calculate acceptance score
    let score =
        calculate_cta_acceptance_score(&state, &"NOR".to_string(), &"SWE".to_string(), war_id);

    // Low trust should give negative score (20 - 50 = -30, * -2 = -60)
    assert!(
        score < 0,
        "Low trust should result in negative acceptance score"
    );
}

#[test]
fn test_ai_declines_when_in_debt() {
    use crate::state::RelationType;
    use crate::systems::calculate_cta_acceptance_score;

    let mut state = WorldStateBuilder::new()
        .date(1444, 12, 11)
        .with_country("SWE")
        .with_country("DEN")
        .with_country("NOR")
        .build();

    state.diplomacy.relations.insert(
        ("NOR".to_string(), "SWE".to_string()),
        RelationType::Alliance,
    );

    // NOR is in debt
    state.countries.get_mut("NOR").unwrap().loans = 5;
    state.countries.get_mut("NOR").unwrap().treasury = Fixed::from_int(-100); // Negative treasury

    // SWE declares war on DEN
    execute_command(
        &mut state,
        "SWE",
        &Command::DeclareWar {
            target: "DEN".to_string(),
            cb: None,
        },
        None,
    )
    .unwrap();

    let war_id = *state.diplomacy.wars.keys().next().unwrap();

    // Calculate acceptance score
    let score =
        calculate_cta_acceptance_score(&state, &"NOR".to_string(), &"SWE".to_string(), war_id);

    // Debt should give massive penalty (-1000)
    assert!(
        score < -500,
        "Debt should result in very negative acceptance score"
    );
}

// ============================================================================
// Multiple Allies Test
// ============================================================================

#[test]
fn test_multiple_allies_some_accept_some_decline() {
    use crate::state::RelationType;

    // SWE declares war on DEN.
    // NOR (allied) accepts.
    // FIN (allied) declines.
    let mut state = WorldStateBuilder::new()
        .date(1444, 12, 11)
        .with_country("SWE")
        .with_country("DEN")
        .with_country("NOR")
        .with_country("FIN")
        .build();

    // Create alliances: SWE-NOR, SWE-FIN
    state.diplomacy.relations.insert(
        ("NOR".to_string(), "SWE".to_string()),
        RelationType::Alliance,
    );
    state.diplomacy.relations.insert(
        ("FIN".to_string(), "SWE".to_string()),
        RelationType::Alliance,
    );

    // SWE declares war on DEN
    execute_command(
        &mut state,
        "SWE",
        &Command::DeclareWar {
            target: "DEN".to_string(),
            cb: None,
        },
        None,
    )
    .unwrap();

    let war_id = *state.diplomacy.wars.keys().next().unwrap();

    // Both should have pending CTAs
    assert!(state
        .countries
        .get("NOR")
        .unwrap()
        .pending_call_to_arms
        .contains_key(&war_id));
    assert!(state
        .countries
        .get("FIN")
        .unwrap()
        .pending_call_to_arms
        .contains_key(&war_id));

    // NOR accepts
    execute_command(
        &mut state,
        "NOR",
        &Command::JoinWar {
            war_id,
            side: WarSide::Attacker,
        },
        None,
    )
    .unwrap();

    // FIN declines
    execute_command(
        &mut state,
        "FIN",
        &Command::DeclineCallToArms { war_id },
        None,
    )
    .unwrap();

    // Verify war state
    let war = state.diplomacy.wars.get(&war_id).unwrap();
    assert!(war.attackers.contains(&"NOR".to_string()), "NOR joined");
    assert!(!war.attackers.contains(&"FIN".to_string()), "FIN declined");

    // Verify alliance states
    assert!(
        state
            .diplomacy
            .relations
            .contains_key(&("NOR".to_string(), "SWE".to_string())),
        "SWE-NOR alliance preserved"
    );
    assert!(
        !state
            .diplomacy
            .relations
            .contains_key(&("FIN".to_string(), "SWE".to_string())),
        "SWE-FIN alliance broken"
    );
}
