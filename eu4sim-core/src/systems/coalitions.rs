//! Coalition system - countries gang up against aggressive expanders.
//!
//! Coalitions form when multiple countries (4+) accumulate high AE (>50) toward a target.
//! AE decays slowly over time (~2 per year).

use crate::fixed::Fixed;
use crate::state::{Coalition, WorldState};

/// Threshold for coalition membership (countries with this much AE can join)
const COALITION_THRESHOLD: f32 = 50.0;

/// Minimum number of countries required to form a coalition
const MIN_COALITION_MEMBERS: usize = 4;

/// AE decay per year
const AE_DECAY_PER_YEAR: f32 = 2.0;

/// AE decay per month (yearly decay / 12)
const AE_DECAY_PER_MONTH: f32 = AE_DECAY_PER_YEAR / 12.0;

/// Run monthly coalition tick (formation check and AE decay).
pub fn run_coalition_tick(state: &mut WorldState) {
    // First decay AE for all countries
    decay_aggressive_expansion(state);

    // Then check for new coalition formations
    check_coalition_formation(state);

    // Update existing coalitions (remove members below threshold)
    update_existing_coalitions(state);
}

/// Decay aggressive expansion for all countries.
///
/// AE decays at ~2 per year (1/6 per month).
fn decay_aggressive_expansion(state: &mut WorldState) {
    let decay = Fixed::from_f32(AE_DECAY_PER_MONTH);

    for (_tag, country) in state.countries.iter_mut() {
        // Decay AE toward each target
        for (_target, ae) in country.aggressive_expansion.iter_mut() {
            *ae = (*ae - decay).max(Fixed::ZERO);
        }

        // Remove zero entries to keep the map clean
        country
            .aggressive_expansion
            .retain(|_, ae| *ae > Fixed::ZERO);
    }
}

/// Check for new coalition formations against aggressive countries.
fn check_coalition_formation(state: &mut WorldState) {
    // Find all potential targets (countries with high AE against them)
    let country_tags: Vec<String> = state.countries.keys().cloned().collect();

    for target in &country_tags {
        // Skip if coalition already exists
        if state.diplomacy.coalitions.contains_key(target) {
            continue;
        }

        // Count countries with high AE against this target
        let angry_countries: Vec<String> = country_tags
            .iter()
            .filter(|&tag| {
                if tag == target {
                    return false;
                }

                state
                    .countries
                    .get(tag)
                    .and_then(|c| c.aggressive_expansion.get(target))
                    .map(|ae| ae.to_f32() >= COALITION_THRESHOLD)
                    .unwrap_or(false)
            })
            .cloned()
            .collect();

        // Form coalition if enough countries are angry
        if angry_countries.len() >= MIN_COALITION_MEMBERS {
            let coalition = Coalition {
                target: target.clone(),
                members: angry_countries.clone(),
                formed_date: state.date,
            };

            state.diplomacy.coalitions.insert(target.clone(), coalition);

            log::info!(
                "Coalition formed against {} with {} members: {:?}",
                target,
                angry_countries.len(),
                angry_countries
            );
        }
    }
}

/// Update existing coalitions (remove members who no longer qualify).
fn update_existing_coalitions(state: &mut WorldState) {
    let coalition_targets: Vec<String> = state.diplomacy.coalitions.keys().cloned().collect();

    for target in coalition_targets {
        let should_remove = {
            if let Some(coalition) = state.diplomacy.coalitions.get_mut(&target) {
                // Filter out members who no longer have high AE
                coalition.members.retain(|member| {
                    state
                        .countries
                        .get(member)
                        .and_then(|c| c.aggressive_expansion.get(&target))
                        .map(|ae| ae.to_f32() >= COALITION_THRESHOLD)
                        .unwrap_or(false)
                });

                // Dissolve coalition if too few members remain
                if coalition.members.len() < MIN_COALITION_MEMBERS {
                    log::info!(
                        "Coalition against {} dissolved (only {} members remain)",
                        target,
                        coalition.members.len()
                    );
                    true
                } else {
                    false
                }
            } else {
                false
            }
        };

        if should_remove {
            state.diplomacy.coalitions.remove(&target);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testing::WorldStateBuilder;

    #[test]
    fn test_ae_decay() {
        let mut state = WorldStateBuilder::new()
            .with_country("ATK")
            .with_country("VIC")
            .build();

        // Set initial AE
        state
            .countries
            .get_mut("VIC")
            .unwrap()
            .aggressive_expansion
            .insert("ATK".to_string(), Fixed::from_int(100));

        // Run monthly decay
        decay_aggressive_expansion(&mut state);

        // Should have decayed by AE_DECAY_PER_MONTH
        let ae = state
            .countries
            .get("VIC")
            .unwrap()
            .aggressive_expansion
            .get("ATK")
            .unwrap();

        let expected = Fixed::from_int(100) - Fixed::from_f32(AE_DECAY_PER_MONTH);
        assert_eq!(*ae, expected);
    }

    #[test]
    fn test_coalition_formation() {
        let mut state = WorldStateBuilder::new()
            .with_country("ATK")
            .with_country("VIC1")
            .with_country("VIC2")
            .with_country("VIC3")
            .with_country("VIC4")
            .build();

        // Give all victims high AE against attacker
        for i in 1..=4 {
            let tag = format!("VIC{}", i);
            state
                .countries
                .get_mut(&tag)
                .unwrap()
                .aggressive_expansion
                .insert("ATK".to_string(), Fixed::from_int(60));
        }

        // Check coalition formation
        check_coalition_formation(&mut state);

        // Coalition should exist
        assert!(state.diplomacy.coalitions.contains_key("ATK"));
        let coalition = state.diplomacy.coalitions.get("ATK").unwrap();
        assert_eq!(coalition.members.len(), 4);
    }

    #[test]
    fn test_coalition_requires_minimum_members() {
        let mut state = WorldStateBuilder::new()
            .with_country("ATK")
            .with_country("VIC1")
            .with_country("VIC2")
            .with_country("VIC3")
            .build();

        // Only 3 countries with high AE (below minimum of 4)
        for i in 1..=3 {
            let tag = format!("VIC{}", i);
            state
                .countries
                .get_mut(&tag)
                .unwrap()
                .aggressive_expansion
                .insert("ATK".to_string(), Fixed::from_int(60));
        }

        // Check coalition formation
        check_coalition_formation(&mut state);

        // Coalition should NOT exist (need 4+ members)
        assert!(!state.diplomacy.coalitions.contains_key("ATK"));
    }

    #[test]
    fn test_coalition_dissolution_on_ae_decay() {
        let mut state = WorldStateBuilder::new()
            .with_country("ATK")
            .with_country("VIC1")
            .with_country("VIC2")
            .with_country("VIC3")
            .with_country("VIC4")
            .build();

        // Form coalition
        for i in 1..=4 {
            let tag = format!("VIC{}", i);
            state
                .countries
                .get_mut(&tag)
                .unwrap()
                .aggressive_expansion
                .insert("ATK".to_string(), Fixed::from_int(60));
        }

        check_coalition_formation(&mut state);
        assert!(state.diplomacy.coalitions.contains_key("ATK"));

        // Drop AE below threshold for one member
        state
            .countries
            .get_mut("VIC4")
            .unwrap()
            .aggressive_expansion
            .insert("ATK".to_string(), Fixed::from_int(40));

        // Update coalitions
        update_existing_coalitions(&mut state);

        // Coalition should be dissolved (only 3 members remain)
        assert!(!state.diplomacy.coalitions.contains_key("ATK"));
    }
}
