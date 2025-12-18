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

            // TODO(review): Validate that autonomy ∈ [0, 1] to prevent negative income
            let autonomy = state
                .modifiers
                .province_autonomy
                .get(&province_id)
                .copied()
                .unwrap_or(Fixed::ZERO);

            // Efficiency = 100% + National% + Local%
            let efficiency = Fixed::ONE + national_mod + local_mod;
            let autonomy_factor = Fixed::ONE - autonomy;

            // Yearly Income
            let yearly_income = province.base_tax.mul(efficiency).mul(autonomy_factor);

            // Monthly Income = Yearly / 12
            let monthly_income = yearly_income.div(Fixed::from_int(defines::MONTHS_PER_YEAR));

            *income_deltas.entry(owner.clone()).or_insert(Fixed::ZERO) += monthly_income;
        }
    }

    // 2. Apply to Treasury
    for (tag, delta) in income_deltas {
        if let Some(country) = state.countries.get_mut(&tag) {
            country.treasury += delta;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::ProvinceState;
    use crate::testing::WorldStateBuilder;

    #[test]
    fn test_taxation_basic() {
        // Setup: 1 province, base tax 12.0
        // Expected Monthly: 1.0
        let province = ProvinceState {
            base_tax: Fixed::from_f32(12.0),
            owner: Some("SWE".to_string()),
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
        let province = ProvinceState {
            base_tax: Fixed::from_f32(12.0),
            owner: Some("SWE".to_string()),
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

    // TODO(review): Add determinism test (run twice, compare results)
}
