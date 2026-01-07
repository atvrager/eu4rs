//! Production income calculation system.
//!
//! Calculates monthly production income for all provinces using the EU4 formula:
//! `goods_produced × goods_price × (1 + efficiency) × (1 - autonomy)`

use crate::fixed::Fixed;
use crate::fixed_generic::Mod32;
use crate::state::{Tag, WorldState};
use std::collections::HashMap;
use tracing::instrument;

/// Configuration for economy simulation.
/// Externalized constants that can be adjusted without recompiling.
#[derive(Debug, Clone)]
pub struct EconomyConfig {
    /// Goods produced per point of base_production (EU4: 0.2)
    pub base_production_multiplier: Mod32,
}

impl Default for EconomyConfig {
    fn default() -> Self {
        Self {
            base_production_multiplier: Mod32::from_f32(
                eu4data::defines::economy::BASE_PRODUCTION_MULTIPLIER,
            ),
        }
    }
}

/// Runs the monthly production tick for all provinces.
///
/// All arithmetic uses [`Fixed`] (i32 scale 10000) for determinism.
/// Call this on the 1st of each month.
///
/// # Formula
/// ```text
/// monthly_income = (goods_produced × goods_price × (1 + efficiency) × (1 - autonomy)) / 12
/// where: goods_produced = base_production × 0.2
/// ```
#[instrument(skip_all, name = "production")]
pub fn run_production_tick(state: &mut WorldState, config: &EconomyConfig) {
    // Aggregate income per country first, then apply
    let mut income_deltas: HashMap<Tag, Fixed> = HashMap::new();

    for (&province_id, province) in state.provinces.iter() {
        // Skip provinces without trade goods or owners
        let Some(goods_id) = province.trade_goods_id else {
            continue;
        };
        let Some(ref owner) = province.owner else {
            continue;
        };

        // Goods produced = base_production × 0.2 (all Mod32)
        let base_goods_produced = province.base_production * config.base_production_multiplier;

        // Apply country goods_produced modifiers (both goods_produced and global_trade_goods_size stack)
        let goods_produced_mod = state
            .modifiers
            .country_goods_produced
            .get(owner)
            .copied()
            .unwrap_or(Mod32::ZERO);
        let trade_goods_size_mod = state
            .modifiers
            .country_trade_goods_size
            .get(owner)
            .copied()
            .unwrap_or(Mod32::ZERO);
        let goods_produced =
            base_goods_produced * (Mod32::ONE + goods_produced_mod + trade_goods_size_mod);

        // Effective price (base + event modifier)
        // TODO(review): Log warning when price is missing to catch data integrity bugs
        let base_price = state
            .base_goods_prices
            .get(&goods_id)
            .map(|f| Mod32::from_fixed(*f))
            .unwrap_or(Mod32::ONE);
        let price = state.modifiers.effective_price(goods_id, base_price);

        // Efficiency: (1 + efficiency_bonus)
        let efficiency = state
            .modifiers
            .province_production_efficiency
            .get(&province_id)
            .copied()
            .unwrap_or(Mod32::ZERO);
        let efficiency_factor = Mod32::ONE + efficiency;

        // Autonomy: (1 - autonomy)
        // Clamp to [0, 1] to prevent negative income
        // Uncored provinces have a 75% autonomy floor
        let base_autonomy = state
            .modifiers
            .province_autonomy
            .get(&province_id)
            .copied()
            .unwrap_or(Mod32::ZERO);

        // Apply coring-based floor: uncored = max(base, 75%)
        let floor = crate::systems::coring::effective_autonomy(province, owner);
        let raw_autonomy = base_autonomy.max(floor);

        let autonomy = raw_autonomy.clamp(Mod32::ZERO, Mod32::ONE);
        let autonomy_factor = Mod32::ONE - autonomy;

        // Yearly production income: goods × price × efficiency × autonomy
        let yearly_income = goods_produced * price * efficiency_factor * autonomy_factor;

        // Monthly income = Yearly / 12
        let monthly_income =
            yearly_income / Mod32::from_int(eu4data::defines::economy::MONTHS_PER_YEAR as i32);

        // Ensure non-negative (production shouldn't reduce treasury)
        let safe_income = monthly_income.max(Mod32::ZERO);

        // Aggregate to owner (convert Mod32 -> Fixed for treasury)
        *income_deltas.entry(owner.clone()).or_insert(Fixed::ZERO) += safe_income.to_fixed();
    }

    // Apply production income to country treasuries
    // Note: In EU4, you get BOTH production income AND trade value from goods.
    // This is intentional "double dipping" - production is valuable because:
    // 1. You get direct production income (here)
    // 2. The goods also add value to trade nodes (trade_value system)
    // 3. You can collect trade income from those nodes (trade_income system)
    for (tag, delta) in income_deltas {
        if let Some(country) = state.countries.get_mut(&tag) {
            country.treasury += delta;
            country.income.production += delta;

            // Debug logging for Korea
            if tag == "KOR" {
                let kor_provinces: Vec<_> = state
                    .provinces
                    .values()
                    .filter(|p| p.owner.as_deref() == Some("KOR"))
                    .collect();
                log::debug!(
                    "Production income for KOR: +{} ducats from {} provinces (avg {:.2}/province)",
                    delta.to_f32(),
                    kor_provinces.len(),
                    delta.to_f32() / kor_provinces.len() as f32
                );
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::modifiers::TradegoodId;
    use crate::state::{CountryState, ProvinceState};
    use proptest::prelude::*;

    fn setup_test_state() -> WorldState {
        let mut state = WorldState::default();

        // Add a country
        state.countries.insert(
            "SWE".to_string(),
            CountryState {
                treasury: Fixed::from_int(100),
                ..Default::default()
            },
        );

        // Add a province with grain (id=0), base_production=5
        // Include a core so it doesn't have the 75% autonomy penalty
        let mut cores = std::collections::HashSet::new();
        cores.insert("SWE".to_string());
        state.provinces.insert(
            1,
            ProvinceState {
                owner: Some("SWE".to_string()),
                trade_goods_id: Some(TradegoodId(0)),
                base_production: Mod32::from_int(5),
                cores,
                ..Default::default()
            },
        );

        // Set grain price to 2.5
        state
            .base_goods_prices
            .insert(TradegoodId(0), Fixed::from_f32(2.5));

        state
    }

    #[test]
    fn test_production_generates_income() {
        let mut state = setup_test_state();
        let config = EconomyConfig::default();

        run_production_tick(&mut state, &config);

        // Yearly: 5 × 0.2 × 2.5 × 1.0 × 1.0 = 2.5
        // Monthly: 2.5 / 12 = ~0.2083
        let expected_income = Fixed::from_f32(2.5 / 12.0);
        let expected_treasury = Fixed::from_int(100) + expected_income;

        assert_eq!(state.countries["SWE"].treasury, expected_treasury);
    }

    #[test]
    fn test_unowned_province_no_income() {
        let mut state = WorldState::default();

        // Province with no owner
        state.provinces.insert(
            1,
            ProvinceState {
                owner: None,
                trade_goods_id: Some(TradegoodId(0)),
                base_production: Mod32::from_int(5),
                ..Default::default()
            },
        );
        state
            .base_goods_prices
            .insert(TradegoodId(0), Fixed::from_f32(2.5));

        let config = EconomyConfig::default();
        run_production_tick(&mut state, &config);

        // No countries should exist or be modified
        assert!(state.countries.is_empty());
    }

    #[test]
    fn test_efficiency_modifier() {
        let mut state = setup_test_state();
        let config = EconomyConfig::default();

        // Add 50% production efficiency to province 1
        state
            .modifiers
            .province_production_efficiency
            .insert(1, Mod32::from_f32(0.5));

        run_production_tick(&mut state, &config);

        // Yearly: 5 × 0.2 × 2.5 × 1.5 × 1.0 = 3.75
        // Monthly: 3.75 / 12 = 0.3125
        let expected_income = Fixed::from_f32(3.75 / 12.0);
        let expected_treasury = Fixed::from_int(100) + expected_income;

        assert_eq!(state.countries["SWE"].treasury, expected_treasury);
    }

    #[test]
    fn test_autonomy_reduces_income() {
        let mut state = setup_test_state();
        let config = EconomyConfig::default();

        // Add 50% autonomy to province 1
        state
            .modifiers
            .province_autonomy
            .insert(1, Mod32::from_f32(0.5));

        run_production_tick(&mut state, &config);

        // Yearly: 5 × 0.2 × 2.5 × 1.0 × 0.5 = 1.25
        // Monthly: 1.25 / 12 = ~0.1042
        // Use Fixed arithmetic for exact match
        let yearly = Fixed::from_f32(1.25);
        let expected_income = yearly.div(Fixed::from_int(12));
        let expected_treasury = Fixed::from_int(100) + expected_income;

        assert_eq!(state.countries["SWE"].treasury, expected_treasury);
    }

    #[test]
    fn test_determinism() {
        let state1 = setup_test_state();
        let state2 = setup_test_state();
        let config = EconomyConfig::default();

        let mut s1 = state1;
        let mut s2 = state2;

        run_production_tick(&mut s1, &config);
        run_production_tick(&mut s2, &config);

        // Must be identical
        assert_eq!(s1.countries["SWE"].treasury, s2.countries["SWE"].treasury);
    }

    proptest! {
        #[test]
        fn prop_production_never_negative(
            autonomy in -2.0..2.0f32,
            efficiency in -2.0..2.0f32
        ) {
            let mut state = setup_test_state();
            let config = EconomyConfig::default();

            // Set random modifiers
            state.modifiers.province_autonomy.insert(1, Mod32::from_f32(autonomy));
            state.modifiers.province_production_efficiency.insert(1, Mod32::from_f32(efficiency));

            run_production_tick(&mut state, &config);

            let swe = state.countries.get("SWE").unwrap();
            // Should not decrease treasury from initial 100
            prop_assert!(swe.treasury >= Fixed::from_int(100),
                "Treasury decreased! {} -> {}", 100, swe.treasury);
        }
    }
}
