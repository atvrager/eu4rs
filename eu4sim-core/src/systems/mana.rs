use crate::fixed::Fixed;
use crate::state::{Advisor, AdvisorType, WorldState};
use tracing::instrument;

/// Generates monarch power for all countries based on ruler stats and advisors.
///
/// Each month, countries gain monarch power:
/// - Base: +3 for each category
/// - Ruler: +ruler_stat (0-6) for each category
/// - Advisor: +advisor_skill (1-5) per hired advisor of matching type
/// - Total: base + ruler + advisor = 3 to 14 per month per category (vanilla)
///
/// Power is capped at 999 by default (can be higher with unembraced institutions).
#[instrument(skip_all, name = "mana")]
pub fn run_mana_tick(state: &mut WorldState) {
    // Default cap is 999, but increases with unembraced institutions
    // TODO: Calculate dynamic cap based on tech penalty from institutions
    const MAX_MANA: Fixed = Fixed::from_int(999);
    const BASE_GAIN: i64 = 3;

    let country_tags: Vec<String> = state.countries.keys().cloned().collect();
    for tag in country_tags {
        if let Some(country) = state.countries.get_mut(&tag) {
            // Sum advisor skill levels by type (skill level = mana contribution)
            let (adm_skill, dip_skill, mil_skill) = sum_advisor_skills(&country.advisors);

            let adm_gain = Fixed::from_int(BASE_GAIN + country.ruler_adm as i64 + adm_skill);
            let dip_gain = Fixed::from_int(BASE_GAIN + country.ruler_dip as i64 + dip_skill);
            let mil_gain = Fixed::from_int(BASE_GAIN + country.ruler_mil as i64 + mil_skill);

            country.adm_mana = (country.adm_mana + adm_gain).min(MAX_MANA);
            country.dip_mana = (country.dip_mana + dip_gain).min(MAX_MANA);
            country.mil_mana = (country.mil_mana + mil_gain).min(MAX_MANA);

            log::trace!(
                "Mana tick for {}: +{}/+{}/+{} (base 3 + ruler {}/{}/{} + advisor {}/{}/{})",
                tag,
                adm_gain,
                dip_gain,
                mil_gain,
                country.ruler_adm,
                country.ruler_dip,
                country.ruler_mil,
                adm_skill,
                dip_skill,
                mil_skill
            );
        }
    }
}

/// Returns (adm_skill, dip_skill, mil_skill) - sum of skill levels by advisor type.
/// In EU4, each hired advisor contributes their skill level to monthly mana generation.
fn sum_advisor_skills(advisors: &[Advisor]) -> (i64, i64, i64) {
    let mut adm = 0;
    let mut dip = 0;
    let mut mil = 0;
    for advisor in advisors {
        let skill = advisor.skill as i64;
        match advisor.advisor_type {
            AdvisorType::Administrative => adm += skill,
            AdvisorType::Diplomatic => dip += skill,
            AdvisorType::Military => mil += skill,
        }
    }
    (adm, dip, mil)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testing::WorldStateBuilder;

    #[test]
    fn test_mana_generation() {
        let mut state = WorldStateBuilder::new().with_country("SWE").build();

        run_mana_tick(&mut state);

        let swe = state.countries.get("SWE").unwrap();
        // Default ruler has 3/3/3 stats, plus base 3 = +6/+6/+6 per month
        assert_eq!(swe.adm_mana, Fixed::from_int(6));
        assert_eq!(swe.dip_mana, Fixed::from_int(6));
        assert_eq!(swe.mil_mana, Fixed::from_int(6));
    }

    #[test]
    fn test_mana_accumulation() {
        let mut state = WorldStateBuilder::new().with_country("SWE").build();

        // Run tick 5 times: 5 * 6 = 30
        for _ in 0..5 {
            run_mana_tick(&mut state);
        }

        let swe = state.countries.get("SWE").unwrap();
        assert_eq!(swe.adm_mana, Fixed::from_int(30));
        assert_eq!(swe.dip_mana, Fixed::from_int(30));
        assert_eq!(swe.mil_mana, Fixed::from_int(30));
    }

    #[test]
    fn test_mana_cap() {
        let mut state = WorldStateBuilder::new().with_country("SWE").build();

        // Give near-max mana
        if let Some(c) = state.countries.get_mut("SWE") {
            c.adm_mana = Fixed::from_int(998);
        }

        run_mana_tick(&mut state);

        let swe = state.countries.get("SWE").unwrap();
        // 998 + 6 (base 3 + ruler 3) = 1004 -> capped at 999
        assert_eq!(swe.adm_mana, Fixed::from_int(999));
    }

    #[test]
    fn test_mana_with_advisor() {
        use crate::state::{Advisor, AdvisorType};

        let mut state = WorldStateBuilder::new().with_country("SWE").build();

        // Add a skill-3 ADM advisor
        if let Some(c) = state.countries.get_mut("SWE") {
            c.advisors.push(Advisor {
                name: "Test Philosopher".to_string(),
                skill: 3,
                advisor_type: AdvisorType::Administrative,
                monthly_cost: Fixed::from_int(10),
            });
        }

        run_mana_tick(&mut state);

        let swe = state.countries.get("SWE").unwrap();
        // Base 3 + ruler 3 + advisor skill 3 = 9 for ADM
        assert_eq!(swe.adm_mana, Fixed::from_int(9));
        // Base 3 + ruler 3 = 6 for DIP/MIL (no advisor)
        assert_eq!(swe.dip_mana, Fixed::from_int(6));
        assert_eq!(swe.mil_mana, Fixed::from_int(6));
    }

    #[test]
    fn test_mana_with_skill5_advisor() {
        use crate::state::{Advisor, AdvisorType};

        let mut state = WorldStateBuilder::new().with_country("SWE").build();

        // Add a skill-5 DIP advisor (max skill)
        if let Some(c) = state.countries.get_mut("SWE") {
            c.advisors.push(Advisor {
                name: "Master Diplomat".to_string(),
                skill: 5,
                advisor_type: AdvisorType::Diplomatic,
                monthly_cost: Fixed::from_int(25),
            });
        }

        run_mana_tick(&mut state);

        let swe = state.countries.get("SWE").unwrap();
        // Base 3 + ruler 3 = 6 for ADM/MIL (no advisor)
        assert_eq!(swe.adm_mana, Fixed::from_int(6));
        // Base 3 + ruler 3 + advisor skill 5 = 11 for DIP
        assert_eq!(swe.dip_mana, Fixed::from_int(11));
        assert_eq!(swe.mil_mana, Fixed::from_int(6));
    }

    #[test]
    fn test_mana_with_all_advisors() {
        use crate::state::{Advisor, AdvisorType};

        let mut state = WorldStateBuilder::new().with_country("SWE").build();

        // Add all three advisors with different skills
        if let Some(c) = state.countries.get_mut("SWE") {
            c.advisors.push(Advisor {
                name: "Admin".to_string(),
                skill: 2,
                advisor_type: AdvisorType::Administrative,
                monthly_cost: Fixed::from_int(5),
            });
            c.advisors.push(Advisor {
                name: "Diplo".to_string(),
                skill: 3,
                advisor_type: AdvisorType::Diplomatic,
                monthly_cost: Fixed::from_int(10),
            });
            c.advisors.push(Advisor {
                name: "Mil".to_string(),
                skill: 4,
                advisor_type: AdvisorType::Military,
                monthly_cost: Fixed::from_int(16),
            });
        }

        run_mana_tick(&mut state);

        let swe = state.countries.get("SWE").unwrap();
        // Base 3 + ruler 3 + advisor skills
        assert_eq!(swe.adm_mana, Fixed::from_int(8)); // 3 + 3 + 2
        assert_eq!(swe.dip_mana, Fixed::from_int(9)); // 3 + 3 + 3
        assert_eq!(swe.mil_mana, Fixed::from_int(10)); // 3 + 3 + 4
    }
}

#[cfg(test)]
mod proptests {
    use super::*;
    use crate::testing::WorldStateBuilder;
    use proptest::prelude::*;

    proptest! {
        #[test]
        fn prop_mana_always_increases_or_caps(months in 1..100usize) {
            let mut state = WorldStateBuilder::new()
                .with_country("SWE")
                .build();

            let mut prev_mana = state.countries.get("SWE").unwrap().adm_mana;

            for _ in 0..months {
                run_mana_tick(&mut state);
                let current_mana = state.countries.get("SWE").unwrap().adm_mana;
                prop_assert!(current_mana >= prev_mana);
                prop_assert!(current_mana <= Fixed::from_int(999));
                prev_mana = current_mana;
            }
        }

        #[test]
        fn prop_mana_never_exceeds_cap(initial_months in 1..500usize) {
            let mut state = WorldStateBuilder::new()
                .with_country("SWE")
                .build();

            for _ in 0..initial_months {
                run_mana_tick(&mut state);
            }

            let swe = state.countries.get("SWE").unwrap();
            let cap = Fixed::from_int(999);

            prop_assert!(swe.adm_mana <= cap);
            prop_assert!(swe.dip_mana <= cap);
            prop_assert!(swe.mil_mana <= cap);
        }
    }
}
