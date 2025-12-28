use crate::fixed::Fixed;
use crate::state::{Tag, WorldState};
use eu4data::defines::economy as defines;
use std::collections::HashMap;

/// Runs monthly taxation calculations.
///
/// Formula: (Base Tax) * (1 + National Mod + Local Mod) * (1 - Autonomy) / 12
pub fn run_taxation_tick(state: &mut WorldState) {
    let mut income_deltas: HashMap<Tag, Fixed> = HashMap::new();

    // 1. Calculate Province Income
    let mut province_count: HashMap<Tag, usize> = HashMap::new();
    let mut total_base_tax: HashMap<Tag, Fixed> = HashMap::new();

    for (&province_id, province) in state.provinces.iter() {
        if let Some(owner) = &province.owner {
            // Modifiers
            let local_mod = state
                .modifiers
                .province_tax_modifier
                .get(&province_id)
                .copied()
                .unwrap_or(Fixed::ZERO);

            let national_mod = state
                .modifiers
                .country_tax_modifier
                .get(owner)
                .copied()
                .unwrap_or(Fixed::ZERO);

            // Clamp autonomy to [0, 1] to prevent negative income or over-production
            // Uncored provinces have a 75% autonomy floor
            let base_autonomy = state
                .modifiers
                .province_autonomy
                .get(&province_id)
                .copied()
                .unwrap_or(Fixed::ZERO);

            // Apply coring-based floor: uncored = max(base, 75%)
            let floor = crate::systems::coring::effective_autonomy(province, owner);
            let raw_autonomy = base_autonomy.max(floor);

            let autonomy = raw_autonomy.clamp(Fixed::ZERO, Fixed::ONE);

            // Efficiency = 100% + National% + Local%
            let efficiency = Fixed::ONE + national_mod + local_mod;
            let autonomy_factor = Fixed::ONE - autonomy;

            // Yearly Income
            let yearly_income = province.base_tax.mul(efficiency).mul(autonomy_factor);

            // Monthly Income = Yearly / 12
            let monthly_income = yearly_income.div(Fixed::from_int(defines::MONTHS_PER_YEAR));

            // Ensure non-negative income just in case efficiency < -100%
            let safe_income = monthly_income.max(Fixed::ZERO);

            *income_deltas.entry(owner.clone()).or_insert(Fixed::ZERO) += safe_income;
            *province_count.entry(owner.clone()).or_insert(0) += 1;
            *total_base_tax.entry(owner.clone()).or_insert(Fixed::ZERO) += province.base_tax;

            // Detailed logging for Korea
            if owner == "KOR" && safe_income > Fixed::ZERO {
                log::trace!(
                    "Province {}: base_tax={:.1}, efficiency={:.2}, autonomy={:.2}, monthly={:.3}",
                    province_id,
                    province.base_tax.to_f32(),
                    efficiency.to_f32(),
                    autonomy.to_f32(),
                    safe_income.to_f32()
                );
            }
        }
    }

    // 2. Apply to Treasury and record for display
    for (tag, delta) in income_deltas {
        if let Some(country) = state.countries.get_mut(&tag) {
            country.treasury += delta;
            country.income.taxation += delta;

            if tag == "KOR" {
                let prov_count = province_count.get(&tag).copied().unwrap_or(0);
                let base_tax_total = total_base_tax.get(&tag).copied().unwrap_or(Fixed::ZERO);
                log::debug!(
                    "Taxation: KOR +{:.2} ducats from {} provinces (total base_tax={:.1}, avg monthly={:.3}/province, treasury now: {:.2})",
                    delta.to_f32(),
                    prov_count,
                    base_tax_total.to_f32(),
                    (delta.to_f32() / prov_count as f32),
                    country.treasury.to_f32()
                );
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
    fn test_taxation_basic() {
        // Setup: 1 province, base tax 12.0
        // Expected Monthly: 1.0
        let mut cores = std::collections::HashSet::new();
        cores.insert("SWE".to_string());
        let province = ProvinceState {
            base_tax: Fixed::from_f32(12.0),
            owner: Some("SWE".to_string()),
            cores,
            ..Default::default()
        };

        let mut state = WorldStateBuilder::new()
            .with_country("SWE")
            .with_province_state(1, province)
            .build();

        // Reset treasury to 0 for clear assertion
        state.countries.get_mut("SWE").unwrap().treasury = Fixed::ZERO;

        run_taxation_tick(&mut state);

        let swe = state.countries.get("SWE").unwrap();
        assert_eq!(swe.treasury, Fixed::from_f32(1.0));
    }

    #[test]
    fn test_taxation_modifiers() {
        // Setup: Base 12, +50% National, -50% Autonomy
        // Yearly: 12 * 1.5 * 0.5 = 9.0
        // Monthly: 0.75
        let mut cores = std::collections::HashSet::new();
        cores.insert("SWE".to_string());
        let province = ProvinceState {
            base_tax: Fixed::from_f32(12.0),
            owner: Some("SWE".to_string()),
            cores,
            ..Default::default()
        };

        let mut state = WorldStateBuilder::new()
            .with_country("SWE")
            .with_province_state(1, province)
            .build();

        // Reset treasury
        state.countries.get_mut("SWE").unwrap().treasury = Fixed::ZERO;

        // Modifiers
        state
            .modifiers
            .country_tax_modifier
            .insert("SWE".to_string(), Fixed::from_f32(0.5));
        state
            .modifiers
            .province_autonomy
            .insert(1, Fixed::from_f32(0.5));

        run_taxation_tick(&mut state);

        let swe = state.countries.get("SWE").unwrap();
        assert_eq!(swe.treasury, Fixed::from_f32(0.75));
    }

    proptest! {
        #[test]
        fn prop_taxation_never_negative(
            autonomy in -2.0..2.0f32,
            efficiency_mod in -2.0..2.0f32
        ) {
            let province = ProvinceState {
                base_tax: Fixed::from_f32(12.0), // Base 12 = 1.0 monthly base
                owner: Some("SWE".to_string()),
                ..Default::default()
            };

            let mut state = WorldStateBuilder::new()
                .with_country("SWE")
                .with_province_state(1, province)
                .build();

            state.countries.get_mut("SWE").unwrap().treasury = Fixed::ZERO;

            state.modifiers.province_autonomy.insert(1, Fixed::from_f32(autonomy));
            state.modifiers.country_tax_modifier.insert("SWE".to_string(), Fixed::from_f32(efficiency_mod));

            run_taxation_tick(&mut state);

            let swe = state.countries.get("SWE").unwrap();
            // Income should never be negative
            prop_assert!(swe.treasury >= Fixed::ZERO);
        }
    }
}
