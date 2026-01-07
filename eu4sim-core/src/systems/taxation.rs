#[cfg(test)]
use crate::fixed::Fixed;
use crate::fixed_generic::Mod32;
use crate::simd::tax32::{calculate_taxes_batch32, TaxInput32, TaxOutput32};
use crate::state::{ProvinceId, Tag, WorldState};
use rayon::prelude::*;
use std::collections::HashMap;
use tracing::instrument;

/// Runs monthly taxation calculations.
///
/// Formula: (Base Tax) * (1 + National Mod + Local Mod) * (1 - Autonomy) / 12
///
/// Uses SIMD-accelerated batch processing when provinces are grouped by owner.
#[instrument(skip_all, name = "taxation")]
pub fn run_taxation_tick(state: &mut WorldState) {
    // PHASE 1: Group provinces by owner for efficient SIMD batching
    let mut provinces_by_owner: HashMap<Tag, Vec<(ProvinceId, TaxInput32, Mod32)>> = HashMap::new();

    for (&province_id, province) in state.provinces.iter() {
        let Some(owner) = province.owner.as_ref() else {
            continue;
        };

        let local_mod = state
            .modifiers
            .province_tax_modifier
            .get(&province_id)
            .copied()
            .unwrap_or(Mod32::ZERO);

        let national_mod = state
            .modifiers
            .country_tax_modifier
            .get(owner)
            .copied()
            .unwrap_or(Mod32::ZERO);

        let base_autonomy = state
            .modifiers
            .province_autonomy
            .get(&province_id)
            .copied()
            .unwrap_or(Mod32::ZERO);

        // Pre-compute effective autonomy including coring floor
        let floor = crate::systems::coring::effective_autonomy(province, owner);
        let effective_autonomy = base_autonomy.max(floor);

        let input = TaxInput32::new(
            province.base_tax,
            national_mod,
            local_mod,
            effective_autonomy,
        );

        provinces_by_owner.entry(owner.clone()).or_default().push((
            province_id,
            input,
            province.base_tax,
        ));
    }

    // PHASE 2: Calculate income per country using SIMD batches in parallel
    let country_results: Vec<(Tag, Mod32, usize, Mod32)> = {
        let _span =
            tracing::info_span!("taxation_simd", countries = provinces_by_owner.len()).entered();

        provinces_by_owner
            .into_par_iter()
            .map(|(tag, province_data)| {
                let province_count = province_data.len();

                // Per-country span for tracing
                let _country_span = tracing::trace_span!(
                    "country_tax",
                    country = %tag,
                    provinces = province_count
                )
                .entered();

                // Extract SIMD inputs and base_tax totals
                let (inputs, base_taxes): (Vec<TaxInput32>, Vec<Mod32>) = province_data
                    .iter()
                    .map(|(_, input, base_tax)| (*input, *base_tax))
                    .unzip();

                // SIMD batch calculation
                let mut outputs = vec![TaxOutput32::default(); inputs.len()];
                calculate_taxes_batch32(&inputs, &mut outputs);

                // Sum results
                let total_income: Mod32 = outputs
                    .iter()
                    .map(|o| Mod32::from_raw(o.monthly_income))
                    .fold(Mod32::ZERO, |acc, x| acc + x);

                let total_base_tax: Mod32 = base_taxes
                    .iter()
                    .copied()
                    .fold(Mod32::ZERO, |acc, x| acc + x);

                (tag, total_income, province_count, total_base_tax)
            })
            .collect()
    };

    // PHASE 3: Apply results to country state
    let base_monthly_income = Mod32::ONE;

    for (tag, provincial_income, prov_count, base_tax_total) in country_results {
        // Add base income (every country with provinces gets 1 ducat/month)
        let total_income = provincial_income + base_monthly_income;

        if let Some(country) = state.countries.get_mut(&tag) {
            let delta_fixed = total_income.to_fixed();
            country.treasury += delta_fixed;
            country.income.taxation += delta_fixed;

            if tag == "KOR" {
                log::debug!(
                    "Taxation: KOR +{:.2} ducats from {} provinces (total base_tax={:.1}, avg monthly={:.3}/province, treasury now: {:.2})",
                    total_income.to_f32(),
                    prov_count,
                    base_tax_total.to_f32(),
                    (total_income.to_f32() / prov_count as f32),
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
        // Provincial Monthly: 12.0 / 12 = 1.0
        // Base Income: 1.0 (all countries get 1 ducat/month)
        // Expected Total: 2.0
        let mut cores = std::collections::HashSet::new();
        cores.insert("SWE".to_string());
        let province = ProvinceState {
            base_tax: Mod32::from_f32(12.0),
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
        assert_eq!(swe.treasury, Fixed::from_f32(2.0)); // 1.0 provincial + 1.0 base
    }

    #[test]
    fn test_taxation_modifiers() {
        // Setup: Base 12, +50% National, -50% Autonomy
        // Yearly: 12 * 1.5 * 0.5 = 9.0
        // Provincial Monthly: 0.75
        // Base Income: 1.0 (all countries get 1 ducat/month)
        // Total: 1.75
        let mut cores = std::collections::HashSet::new();
        cores.insert("SWE".to_string());
        let province = ProvinceState {
            base_tax: Mod32::from_f32(12.0),
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
            .insert("SWE".to_string(), Mod32::from_f32(0.5));
        state
            .modifiers
            .province_autonomy
            .insert(1, Mod32::from_f32(0.5));

        run_taxation_tick(&mut state);

        let swe = state.countries.get("SWE").unwrap();
        assert_eq!(swe.treasury, Fixed::from_f32(1.75)); // 0.75 provincial + 1.0 base
    }

    proptest! {
        #[test]
        fn prop_taxation_never_negative(
            autonomy in -2.0..2.0f32,
            efficiency_mod in -2.0..2.0f32
        ) {
            let province = ProvinceState {
                base_tax: Mod32::from_f32(12.0), // Base 12 = 1.0 monthly base
                owner: Some("SWE".to_string()),
                ..Default::default()
            };

            let mut state = WorldStateBuilder::new()
                .with_country("SWE")
                .with_province_state(1, province)
                .build();

            state.countries.get_mut("SWE").unwrap().treasury = Fixed::ZERO;

            state.modifiers.province_autonomy.insert(1, Mod32::from_f32(autonomy));
            state.modifiers.country_tax_modifier.insert("SWE".to_string(), Mod32::from_f32(efficiency_mod));

            run_taxation_tick(&mut state);

            let swe = state.countries.get("SWE").unwrap();
            // Income should never be negative
            prop_assert!(swe.treasury >= Fixed::ZERO);
        }
    }
}
