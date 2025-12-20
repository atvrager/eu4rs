use crate::fixed::Fixed;
use crate::state::WorldState;

/// Generates monarch power for all countries (+3/+3/+3 per month)
pub fn run_mana_tick(state: &mut WorldState) {
    let monthly_gain = Fixed::from_int(3);
    const MAX_MANA: Fixed = Fixed::from_int(999);

    let country_tags: Vec<String> = state.countries.keys().cloned().collect();
    for tag in country_tags {
        if let Some(country) = state.countries.get_mut(&tag) {
            country.adm_mana = (country.adm_mana + monthly_gain).min(MAX_MANA);
            country.dip_mana = (country.dip_mana + monthly_gain).min(MAX_MANA);
            country.mil_mana = (country.mil_mana + monthly_gain).min(MAX_MANA);

            log::debug!("Mana tick for {}: +3/+3/+3", tag);
        }
    }
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
        assert_eq!(swe.adm_mana, Fixed::from_int(3));
        assert_eq!(swe.dip_mana, Fixed::from_int(3));
        assert_eq!(swe.mil_mana, Fixed::from_int(3));
    }

    #[test]
    fn test_mana_accumulation() {
        let mut state = WorldStateBuilder::new().with_country("SWE").build();

        // Run tick 5 times
        for _ in 0..5 {
            run_mana_tick(&mut state);
        }

        let swe = state.countries.get("SWE").unwrap();
        assert_eq!(swe.adm_mana, Fixed::from_int(15));
        assert_eq!(swe.dip_mana, Fixed::from_int(15));
        assert_eq!(swe.mil_mana, Fixed::from_int(15));
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
        // 998 + 3 = 1001 -> capped at 999
        assert_eq!(swe.adm_mana, Fixed::from_int(999));
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
