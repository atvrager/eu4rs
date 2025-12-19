use crate::fixed::Fixed;
use crate::state::WorldState;
use eu4data::defines::manpower as defines;
use std::collections::HashMap;

/// Runs monthly manpower recovery.
///
/// Formula:
/// 1. Calculate Max Manpower = Base(10k) + Sum(Province Manpower * 1000 * (1-Autonomy))
/// 2. Recovery = Max / 120 (10 years to fill)
/// 3. Cap at Max.
pub fn run_manpower_tick(state: &mut WorldState) {
    let mut country_max_manpower: HashMap<String, Fixed> = HashMap::default();

    // 1. Calculate Max Manpower from Provinces
    for (&id, province) in &state.provinces {
        if let Some(owner) = &province.owner {
            // Clamp autonomy to [0, 1] to prevent negative contribution
            let raw_autonomy = state
                .modifiers
                .province_autonomy
                .get(&id)
                .copied()
                .unwrap_or(Fixed::ZERO);

            let autonomy = raw_autonomy.clamp(Fixed::ZERO, Fixed::ONE);

            let factor = Fixed::ONE - autonomy;
            let prov_max = province
                .base_manpower
                .mul(Fixed::from_int(defines::MEN_PER_DEV))
                .mul(factor);

            *country_max_manpower
                .entry(owner.clone())
                .or_insert(Fixed::ZERO) += prov_max;
        }
    }

    // 2. Apply Recovery
    let country_tags: Vec<String> = state.countries.keys().cloned().collect();
    for tag in country_tags {
        if let Some(country) = state.countries.get_mut(&tag) {
            let province_sum = country_max_manpower
                .get(&tag)
                .copied()
                .unwrap_or(Fixed::ZERO);
            let max = Fixed::from_int(defines::BASE_MANPOWER) + province_sum;

            // Recovery: Max / 120 (120 months = 10 years)
            let recovery = max.div(Fixed::from_int(defines::RECOVERY_MONTHS));

            // Only grant recovery if below max (don't recover while overcapped)
            if country.manpower < max {
                country.manpower += recovery;
                if country.manpower > max {
                    country.manpower = max;
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::ProvinceState;
    use crate::testing::WorldStateBuilder;
    use proptest::prelude::*;

    #[test]
    fn test_manpower_recovery() {
        // Setup: 1 province, base manpower 1.0 -> 1000 men
        // Base Country = 10000
        // Total Max = 11000
        // Monthly Recovery = 11000 / 120 = 91.6666
        let province = ProvinceState {
            base_manpower: Fixed::from_f32(1.0),
            owner: Some("SWE".to_string()),
            ..Default::default()
        };

        let mut state = WorldStateBuilder::new()
            .with_country("SWE") // Starts with 50000 (builder default)
            .with_province_state(1, province)
            .build();

        // Set manpower low to allow recovery
        state.countries.get_mut("SWE").unwrap().manpower = Fixed::ZERO;

        run_manpower_tick(&mut state);

        let swe = state.countries.get("SWE").unwrap();
        // 11000 / 120 = 91.6666 -> 91.6666
        // Fixed: 916666
        // Expected approx 91.6666
        assert!(swe.manpower > Fixed::from_f32(91.6));
        assert!(swe.manpower < Fixed::from_f32(91.7));
    }

    #[test]
    fn test_manpower_cap() {
        let province = ProvinceState {
            base_manpower: Fixed::from_f32(1.0),
            owner: Some("SWE".to_string()),
            ..Default::default()
        };

        let mut state = WorldStateBuilder::new()
            .with_country("SWE")
            .with_province_state(1, province)
            .build();

        // Max is 11000. Set current to 20000.
        state.countries.get_mut("SWE").unwrap().manpower = Fixed::from_int(20000);

        run_manpower_tick(&mut state);

        let swe = state.countries.get("SWE").unwrap();
        // Should remain at 20000 (no recovery granted when overcapped)
        assert_eq!(swe.manpower, Fixed::from_int(20000));
    }

    proptest! {
        #[test]
        fn prop_manpower_recovery_always_positive_base(
            autonomy in -2.0..2.0f32
        ) {
            let province = ProvinceState {
                base_manpower: Fixed::from_f32(10.0), // 10k men
                owner: Some("SWE".to_string()),
                ..Default::default()
            };

            let mut state = WorldStateBuilder::new()
                .with_country("SWE")
                .with_province_state(1, province)
                .build();

            // Start at 0
            state.countries.get_mut("SWE").unwrap().manpower = Fixed::ZERO;

            // Set crazy autonomy
            state.modifiers.province_autonomy.insert(1, Fixed::from_f32(autonomy));

            run_manpower_tick(&mut state);

            let swe = state.countries.get("SWE").unwrap();
            // Even if autonomy is 100% or -100%, we have Base Manpower (10k),
            // so recovery should be positive.
            prop_assert!(swe.manpower > Fixed::ZERO);
        }
    }
}
