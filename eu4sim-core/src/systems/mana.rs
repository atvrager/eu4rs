use crate::fixed::Fixed;
use crate::state::WorldState;

/// Generates monarch power for all countries (+3/+3/+3 per month)
pub fn run_mana_tick(state: &mut WorldState) {
    let monthly_gain = Fixed::from_int(3);

    for (tag, country) in state.countries.iter_mut() {
        country.adm_mana += monthly_gain;
        country.dip_mana += monthly_gain;
        country.mil_mana += monthly_gain;

        log::debug!("Mana tick for {}: +3/+3/+3", tag);
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
}

#[cfg(test)]
mod proptests {
    use super::*;
    use crate::testing::WorldStateBuilder;
    use proptest::prelude::*;

    proptest! {
        #[test]
        fn prop_mana_always_increases(months in 1..100usize) {
            let mut state = WorldStateBuilder::new()
                .with_country("SWE")
                .build();

            for _ in 0..months {
                run_mana_tick(&mut state);
            }

            let swe = state.countries.get("SWE").unwrap();
            let expected = Fixed::from_int((months * 3) as i64);

            prop_assert_eq!(swe.adm_mana, expected);
            prop_assert_eq!(swe.dip_mana, expected);
            prop_assert_eq!(swe.mil_mana, expected);
        }

        #[test]
        fn prop_mana_never_negative_after_generation(initial_months in 1..20usize) {
            let mut state = WorldStateBuilder::new()
                .with_country("SWE")
                .build();

            for _ in 0..initial_months {
                run_mana_tick(&mut state);
            }

            let swe = state.countries.get("SWE").unwrap();

            prop_assert!(swe.adm_mana >= Fixed::ZERO);
            prop_assert!(swe.dip_mana >= Fixed::ZERO);
            prop_assert!(swe.mil_mana >= Fixed::ZERO);
        }
    }
}
