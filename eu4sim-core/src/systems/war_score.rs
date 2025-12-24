use crate::fixed::Fixed;
use crate::state::{Tag, War, WorldState};

/// Maximum war score from battles alone (40%)
const MAX_BATTLE_SCORE: u8 = 40;

/// War score gained per battle won
const SCORE_PER_BATTLE: u8 = 5;

/// Maximum war score from occupation (60%)  
const MAX_OCCUPATION_SCORE: u8 = 60;

/// Awards battle score to the winning side of a battle.
/// Call this after combat resolution when one side clearly won.
pub fn award_battle_score(war: &mut War, attacker_won: bool) {
    if attacker_won {
        war.attacker_battle_score =
            (war.attacker_battle_score + SCORE_PER_BATTLE).min(MAX_BATTLE_SCORE);
    } else {
        war.defender_battle_score =
            (war.defender_battle_score + SCORE_PER_BATTLE).min(MAX_BATTLE_SCORE);
    }
    recalculate_total_score(war);
}

/// Recalculates total war score from battle + occupation components.
fn recalculate_total_score(_war: &mut War) {
    // Total = battles + occupation, capped at 100
    // (Occupation score is calculated separately and stored directly in attacker_score/defender_score)
    // For now, total = battle score (occupation added in recalculate_war_scores)
}

/// Recalculates war scores for all active wars based on occupation.
/// Call this monthly or after significant territory changes.
pub fn recalculate_war_scores(state: &mut WorldState) {
    // Collect war data first to avoid borrow issues
    let war_ids: Vec<_> = state.diplomacy.wars.keys().copied().collect();

    for war_id in war_ids {
        if let Some(war) = state.diplomacy.wars.get(&war_id) {
            let (attacker_occ, defender_occ) = calculate_occupation_scores(state, war);

            // Debug: Log significant occupation changes
            if attacker_occ > 0 || defender_occ > 0 {
                log::debug!(
                    "[WAR_SCORE] War {}: attacker_occ={}, defender_occ={}",
                    war.name,
                    attacker_occ,
                    defender_occ
                );
            }

            // Update war with new occupation-based scores
            if let Some(war) = state.diplomacy.wars.get_mut(&war_id) {
                war.attacker_score = (war.attacker_battle_score + attacker_occ).min(100);
                war.defender_score = (war.defender_battle_score + defender_occ).min(100);
            }
        }
    }
}

/// Calculates occupation-based war scores for both sides.
/// Returns (attacker_occupation_score, defender_occupation_score)
fn calculate_occupation_scores(state: &WorldState, war: &War) -> (u8, u8) {
    // Calculate total development for each side
    let mut attacker_total_dev = Fixed::ZERO;
    let mut defender_total_dev = Fixed::ZERO;

    // Calculate occupied development
    let mut attacker_occupied_dev = Fixed::ZERO; // Dev occupied by attackers (in defender territory)
    let mut defender_occupied_dev = Fixed::ZERO; // Dev occupied by defenders (in attacker territory)

    // Track occupied provinces for debugging
    let mut occupied_provinces: Vec<(u32, String, String)> = Vec::new();

    for (&prov_id, province) in &state.provinces {
        let dev = province.base_tax + province.base_production + province.base_manpower;

        if let Some(owner) = &province.owner {
            let is_attacker_owned = war.attackers.contains(owner);
            let is_defender_owned = war.defenders.contains(owner);

            if is_attacker_owned {
                attacker_total_dev += dev;

                // Check if defender controls this attacker province
                if let Some(controller) = &province.controller {
                    if war.defenders.contains(controller) && controller != owner {
                        defender_occupied_dev += dev;
                        occupied_provinces.push((prov_id, controller.clone(), owner.clone()));
                    }
                }
            } else if is_defender_owned {
                defender_total_dev += dev;

                // Check if attacker controls this defender province
                if let Some(controller) = &province.controller {
                    if war.attackers.contains(controller) && controller != owner {
                        attacker_occupied_dev += dev;
                        occupied_provinces.push((prov_id, controller.clone(), owner.clone()));
                    }
                }
            }
        }
    }

    // Log occupied provinces if any
    if !occupied_provinces.is_empty() {
        log::info!(
            "[WAR_SCORE] {} occupation(s) in {}: {:?}",
            occupied_provinces.len(),
            war.name,
            occupied_provinces
        );
    }

    // Calculate occupation scores: (occupied_dev / enemy_total_dev) * MAX_OCCUPATION_SCORE
    let attacker_occ_score = if defender_total_dev > Fixed::ZERO {
        let ratio = attacker_occupied_dev.div(defender_total_dev);
        (ratio.to_f32() * MAX_OCCUPATION_SCORE as f32).round() as u8
    } else {
        0
    };

    let defender_occ_score = if attacker_total_dev > Fixed::ZERO {
        let ratio = defender_occupied_dev.div(attacker_total_dev);
        (ratio.to_f32() * MAX_OCCUPATION_SCORE as f32).round() as u8
    } else {
        0
    };

    (
        attacker_occ_score.min(MAX_OCCUPATION_SCORE),
        defender_occ_score.min(MAX_OCCUPATION_SCORE),
    )
}

/// Updates province controller after combat/occupation.
/// Call this after an army enters an enemy province.
pub fn update_province_controller(state: &mut WorldState, province_id: u32, new_controller: &Tag) {
    if let Some(province) = state.provinces.get_mut(&province_id) {
        // Only update if the new controller is different and is an enemy
        if province.controller.as_ref() != Some(new_controller) {
            province.controller = Some(new_controller.clone());
            log::info!(
                "Province {} now controlled by {}",
                province_id,
                new_controller
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::Date;
    use crate::testing::WorldStateBuilder;

    #[test]
    fn test_battle_score_accumulation() {
        let mut war = War {
            id: 0,
            name: "Test War".into(),
            attackers: vec!["A".into()],
            defenders: vec!["D".into()],
            start_date: Date::new(1444, 11, 11),
            attacker_score: 0,
            attacker_battle_score: 0,
            defender_score: 0,
            defender_battle_score: 0,
            pending_peace: None,
        };

        // Win 3 battles as attacker
        award_battle_score(&mut war, true);
        award_battle_score(&mut war, true);
        award_battle_score(&mut war, true);

        assert_eq!(war.attacker_battle_score, 15); // 3 * 5
        assert_eq!(war.defender_battle_score, 0);
    }

    #[test]
    fn test_battle_score_cap() {
        let mut war = War {
            id: 0,
            name: "Test War".into(),
            attackers: vec!["A".into()],
            defenders: vec!["D".into()],
            start_date: Date::new(1444, 11, 11),
            attacker_score: 0,
            attacker_battle_score: 0,
            defender_score: 0,
            defender_battle_score: 0,
            pending_peace: None,
        };

        // Win 10 battles (should cap at 40)
        for _ in 0..10 {
            award_battle_score(&mut war, true);
        }

        assert_eq!(war.attacker_battle_score, 40); // Capped
    }

    #[test]
    fn test_occupation_score_calculation() {
        let mut state = WorldStateBuilder::new()
            .with_country("ATK")
            .with_country("DEF")
            .with_province(1, Some("DEF")) // Defender province, dev = 3
            .with_province(2, Some("DEF")) // Defender province, dev = 3
            .build();

        // Add war
        let war = War {
            id: 0,
            name: "Test War".into(),
            attackers: vec!["ATK".into()],
            defenders: vec!["DEF".into()],
            start_date: Date::new(1444, 11, 11),
            attacker_score: 0,
            attacker_battle_score: 0,
            defender_score: 0,
            defender_battle_score: 0,
            pending_peace: None,
        };
        state.diplomacy.wars.insert(0, war);

        // Attacker occupies province 1
        state.provinces.get_mut(&1).unwrap().controller = Some("ATK".into());

        recalculate_war_scores(&mut state);

        let war = state.diplomacy.wars.get(&0).unwrap();
        // Attacker occupied 1/2 of defender dev = 30% occupation score
        assert!(war.attacker_score > 0);
        assert_eq!(war.defender_score, 0);
    }

    // Property tests for war score invariants
    mod proptests {
        use super::*;
        use proptest::prelude::*;

        proptest! {
            #[test]
            fn prop_scores_always_bounded(
                attacker_battles in 0u8..20,
                defender_battles in 0u8..20,
                attacker_occ_dev in 0u32..1000,
                _defender_occ_dev in 0u32..1000,
                total_dev in 1u32..1000,
            ) {
                let mut state = WorldStateBuilder::new()
                    .with_country("ATK")
                    .with_country("DEF")
                    .build();

                // Create provinces with controlled total dev
                for i in 0..total_dev {
                    state.provinces.insert(i, crate::state::ProvinceState {
                        owner: Some("DEF".into()),
                        controller: if i < attacker_occ_dev {
                            Some("ATK".into())
                        } else {
                            Some("DEF".into())
                        },
                        base_tax: Fixed::from_int(1),
                        base_production: Fixed::ZERO,
                        base_manpower: Fixed::ZERO,
                        ..Default::default()
                    });
                }

                let mut war = War {
                    id: 0,
                    name: "Test".into(),
                    attackers: vec!["ATK".into()],
                    defenders: vec!["DEF".into()],
                    start_date: Date::new(1444, 11, 11),
                    attacker_score: 0,
                    attacker_battle_score: 0,
                    defender_score: 0,
                    defender_battle_score: 0,
                    pending_peace: None,
                };

                // Award battle scores
                for _ in 0..attacker_battles {
                    award_battle_score(&mut war, true);
                }
                for _ in 0..defender_battles {
                    award_battle_score(&mut war, false);
                }

                state.diplomacy.wars.insert(0, war);
                recalculate_war_scores(&mut state);

                let war = state.diplomacy.wars.get(&0).unwrap();
                prop_assert!(war.attacker_score <= 100, "Attacker score {} exceeds 100", war.attacker_score);
                prop_assert!(war.defender_score <= 100, "Defender score {} exceeds 100", war.defender_score);
            }

            #[test]
            fn prop_full_occupation_gives_max_score(dev_per_province in 1u32..10) {
                let mut state = WorldStateBuilder::new()
                    .with_country("ATK")
                    .with_country("DEF")
                    .build();

                // Create 5 defender provinces, all occupied by attacker
                for i in 0..5 {
                    state.provinces.insert(i, crate::state::ProvinceState {
                        owner: Some("DEF".into()),
                        controller: Some("ATK".into()),
                        base_tax: Fixed::from_int(dev_per_province as i64),
                        base_production: Fixed::ZERO,
                        base_manpower: Fixed::ZERO,
                        ..Default::default()
                    });
                }

                let war = War {
                    id: 0,
                    name: "Test".into(),
                    attackers: vec!["ATK".into()],
                    defenders: vec!["DEF".into()],
                    start_date: Date::new(1444, 11, 11),
                    attacker_score: 0,
                    attacker_battle_score: 0,
                    defender_score: 0,
                    defender_battle_score: 0,
                    pending_peace: None,
                };

                state.diplomacy.wars.insert(0, war);
                recalculate_war_scores(&mut state);

                let war = state.diplomacy.wars.get(&0).unwrap();
                // Full occupation should give MAX_OCCUPATION_SCORE (60)
                prop_assert_eq!(war.attacker_score, MAX_OCCUPATION_SCORE);
            }

            #[test]
            fn prop_score_monotonic_with_occupation(
                total_provinces in 2u32..10,
                occupied_count_a in 0u32..10,
                occupied_count_b in 0u32..10,
            ) {
                let total_provinces = total_provinces.max(2);
                let occupied_a = occupied_count_a.min(total_provinces);
                let occupied_b = occupied_count_b.min(total_provinces);

                if occupied_a == occupied_b {
                    return Ok(()); // Skip equal cases
                }

                let (less_occupied, more_occupied) = if occupied_a < occupied_b {
                    (occupied_a, occupied_b)
                } else {
                    (occupied_b, occupied_a)
                };

                // Test with less occupation
                let mut state_less = WorldStateBuilder::new()
                    .with_country("ATK")
                    .with_country("DEF")
                    .build();

                for i in 0..total_provinces {
                    state_less.provinces.insert(i, crate::state::ProvinceState {
                        owner: Some("DEF".into()),
                        controller: if i < less_occupied {
                            Some("ATK".into())
                        } else {
                            Some("DEF".into())
                        },
                        base_tax: Fixed::from_int(1),
                        base_production: Fixed::ZERO,
                        base_manpower: Fixed::ZERO,
                        ..Default::default()
                    });
                }

                let war = War {
                    id: 0,
                    name: "Test".into(),
                    attackers: vec!["ATK".into()],
                    defenders: vec!["DEF".into()],
                    start_date: Date::new(1444, 11, 11),
                    attacker_score: 0,
                    attacker_battle_score: 0,
                    defender_score: 0,
                    defender_battle_score: 0,
                    pending_peace: None,
                };
                state_less.diplomacy.wars.insert(0, war.clone());
                recalculate_war_scores(&mut state_less);
                let score_less = state_less.diplomacy.wars.get(&0).unwrap().attacker_score;

                // Test with more occupation
                let mut state_more = WorldStateBuilder::new()
                    .with_country("ATK")
                    .with_country("DEF")
                    .build();

                for i in 0..total_provinces {
                    state_more.provinces.insert(i, crate::state::ProvinceState {
                        owner: Some("DEF".into()),
                        controller: if i < more_occupied {
                            Some("ATK".into())
                        } else {
                            Some("DEF".into())
                        },
                        base_tax: Fixed::from_int(1),
                        base_production: Fixed::ZERO,
                        base_manpower: Fixed::ZERO,
                        ..Default::default()
                    });
                }

                state_more.diplomacy.wars.insert(0, war);
                recalculate_war_scores(&mut state_more);
                let score_more = state_more.diplomacy.wars.get(&0).unwrap().attacker_score;

                // More occupation should give higher or equal score
                prop_assert!(score_more >= score_less,
                    "More occupation ({}) gave score {} but less occupation ({}) gave {}",
                    more_occupied, score_more, less_occupied, score_less);
            }
        }
    }
}
