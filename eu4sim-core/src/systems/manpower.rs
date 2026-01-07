use crate::fixed::Fixed;
use crate::fixed_generic::Mod32;
use crate::state::WorldState;
use eu4data::defines::manpower as defines;
use std::collections::HashMap;
use tracing::instrument;

/// Runs monthly manpower recovery.
///
/// Formula:
/// 1. Calculate Max Manpower = Base(10k) + Sum(Province Manpower * 1000 * (1-Autonomy))
/// 2. Recovery = Max / 120 (10 years to fill)
/// 3. Cap at Max.
#[instrument(skip_all, name = "manpower")]
pub fn run_manpower_tick(state: &mut WorldState) {
    let mut country_max_manpower: HashMap<String, Fixed> = HashMap::default();

    // 1. Calculate Max Manpower from Provinces
    for (&id, province) in &state.provinces {
        if let Some(owner) = &province.owner {
            // Clamp autonomy to [0, 1] to prevent negative contribution
            // Uncored provinces have a 75% autonomy floor
            let base_autonomy = state
                .modifiers
                .province_autonomy
                .get(&id)
                .copied()
                .unwrap_or(Mod32::ZERO);

            // Apply coring-based floor: uncored = max(base, 75%)
            let floor = crate::systems::coring::effective_autonomy(province, owner);
            let raw_autonomy = base_autonomy.max(floor);

            let autonomy = raw_autonomy.clamp(Mod32::ZERO, Mod32::ONE);

            let factor = Mod32::ONE - autonomy;
            let prov_max =
                province.base_manpower * Mod32::from_int(defines::MEN_PER_DEV as i32) * factor;

            *country_max_manpower
                .entry(owner.clone())
                .or_insert(Fixed::ZERO) += prov_max.to_fixed();
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
            let base_max = Fixed::from_int(defines::BASE_MANPOWER) + province_sum;

            // Apply global manpower modifier
            let manpower_mod = state
                .modifiers
                .country_manpower
                .get(&tag)
                .copied()
                .unwrap_or(Mod32::ZERO);
            let max = base_max.mul(Fixed::ONE + manpower_mod.to_fixed());

            // Recovery: Max / 120 (120 months = 10 years)
            let base_recovery = max.div(Fixed::from_int(defines::RECOVERY_MONTHS));

            // Apply manpower recovery speed modifier
            let recovery_speed_mod = state
                .modifiers
                .country_manpower_recovery_speed
                .get(&tag)
                .copied()
                .unwrap_or(Mod32::ZERO);
            let recovery = base_recovery.mul(Fixed::ONE + recovery_speed_mod.to_fixed());

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
        // Setup: 1 province, base manpower 1.0 -> 250 men (MEN_PER_DEV = 250)
        // Base Country = 10000
        // Total Max = 10250
        // Monthly Recovery = 10250 / 120 = 85.4166
        let mut cores = std::collections::HashSet::new();
        cores.insert("SWE".to_string());
        let province = ProvinceState {
            base_manpower: Mod32::from_f32(1.0),
            owner: Some("SWE".to_string()),
            cores,
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
        // 10250 / 120 = 85.4166
        assert!(swe.manpower > Fixed::from_f32(85.4));
        assert!(swe.manpower < Fixed::from_f32(85.5));
    }

    #[test]
    fn test_manpower_cap() {
        let province = ProvinceState {
            base_manpower: Mod32::from_f32(1.0),
            owner: Some("SWE".to_string()),
            ..Default::default()
        };

        let mut state = WorldStateBuilder::new()
            .with_country("SWE")
            .with_province_state(1, province)
            .build();

        // Max is 10250 (10000 base + 250 from province). Set current to 20000.
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
                base_manpower: Mod32::from_f32(10.0), // 10k men
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
            state.modifiers.province_autonomy.insert(1, Mod32::from_f32(autonomy));

            run_manpower_tick(&mut state);

            let swe = state.countries.get("SWE").unwrap();
            // Even if autonomy is 100% or -100%, we have Base Manpower (10k),
            // so recovery should be positive.
            prop_assert!(swe.manpower > Fixed::ZERO);
        }
    }
}
