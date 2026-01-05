use crate::fixed::Fixed;
use crate::state::{ProvinceId, ProvinceState, Tag, WorldState};
use eu4data::defines::economy as defines;
use rayon::prelude::*;
use std::collections::HashMap;
use tracing::instrument;

/// Result of calculating taxation for a single province
struct ProvinceTaxResult {
    owner: Tag,
    income: Fixed,
    base_tax: Fixed,
}

/// Calculate tax income for a single province (pure function)
#[instrument(skip_all, name = "province_tax")]
fn calculate_province_tax(
    province_id: ProvinceId,
    province: &ProvinceState,
    owner: &Tag,
    local_mod: Fixed,
    national_mod: Fixed,
    base_autonomy: Fixed,
) -> ProvinceTaxResult {
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

    ProvinceTaxResult {
        owner: owner.clone(),
        income: safe_income,
        base_tax: province.base_tax,
    }
}

/// Runs monthly taxation calculations.
///
/// Formula: (Base Tax) * (1 + National Mod + Local Mod) * (1 - Autonomy) / 12
#[instrument(skip_all, name = "taxation")]
pub fn run_taxation_tick(state: &mut WorldState) {
    // PHASE 1: Extract province data for parallel processing
    let province_inputs: Vec<_> = state
        .provinces
        .iter()
        .filter_map(|(&province_id, province)| {
            province.owner.as_ref().map(|owner| {
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

                let base_autonomy = state
                    .modifiers
                    .province_autonomy
                    .get(&province_id)
                    .copied()
                    .unwrap_or(Fixed::ZERO);

                (
                    province_id,
                    province,
                    owner.clone(),
                    local_mod,
                    national_mod,
                    base_autonomy,
                )
            })
        })
        .collect();

    // PHASE 2: Calculate income in parallel
    let tax_results: Vec<ProvinceTaxResult> = {
        let _span =
            tracing::info_span!("provinces_parallel", count = province_inputs.len()).entered();
        province_inputs
            .into_par_iter()
            .map(
                |(province_id, province, owner, local_mod, national_mod, base_autonomy)| {
                    calculate_province_tax(
                        province_id,
                        province,
                        &owner,
                        local_mod,
                        national_mod,
                        base_autonomy,
                    )
                },
            )
            .collect()
    };

    // PHASE 3: Aggregate results (sequential)
    let mut income_deltas: HashMap<Tag, Fixed> = HashMap::new();
    let mut province_count: HashMap<Tag, usize> = HashMap::new();
    let mut total_base_tax: HashMap<Tag, Fixed> = HashMap::new();

    for result in tax_results {
        *income_deltas
            .entry(result.owner.clone())
            .or_insert(Fixed::ZERO) += result.income;
        *province_count.entry(result.owner.clone()).or_insert(0) += 1;
        *total_base_tax.entry(result.owner).or_insert(Fixed::ZERO) += result.base_tax;
    }

    // 2. Add base income for all countries with provinces
    // Every country gets 1 ducat/month just for existing
    let base_monthly_income = Fixed::ONE;
    for tag in province_count.keys() {
        *income_deltas.entry(tag.clone()).or_insert(Fixed::ZERO) += base_monthly_income;
    }

    // 3. Apply to Treasury and record for display
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
        // Provincial Monthly: 12.0 / 12 = 1.0
        // Base Income: 1.0 (all countries get 1 ducat/month)
        // Expected Total: 2.0
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
        assert_eq!(swe.treasury, Fixed::from_f32(1.75)); // 0.75 provincial + 1.0 base
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
