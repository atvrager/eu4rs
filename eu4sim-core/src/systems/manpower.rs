use crate::fixed::Fixed;
use crate::state::WorldState;
use std::collections::HashMap;

/// Runs monthly manpower recovery.
///
/// Formula:
/// 1. Calculate Max Manpower = Base(10k) + Sum(Province Manpower * 1000 * (1-Autonomy))
/// 2. Recovery = Max / 120 (10 years to fill)
/// 3. Cap at Max.
pub fn run_manpower_tick(state: &mut WorldState) {
    let mut country_max_manpower: HashMap<String, Fixed> = HashMap::new();
    let men_per_dev = Fixed::from_int(1000);
    let base_country_manpower = Fixed::from_int(10000);

    // 1. Calculate Max Manpower from Provinces
    for (&id, province) in &state.provinces {
        if let Some(owner) = &province.owner {
            let autonomy = state
                .modifiers
                .province_autonomy
                .get(&id)
                .copied()
                .unwrap_or(Fixed::ZERO);

            let factor = Fixed::ONE - autonomy;
            let prov_max = province.base_manpower.mul(men_per_dev).mul(factor);

            *country_max_manpower
                .entry(owner.clone())
                .or_insert(Fixed::ZERO) += prov_max;
        }
    }

    // 2. Apply Recovery
    for (tag, country) in state.countries.iter_mut() {
        let province_sum = country_max_manpower
            .get(tag)
            .copied()
            .unwrap_or(Fixed::ZERO);
        let max = base_country_manpower + province_sum;

        // Recovery: Max / 120 (120 months = 10 years)
        let recovery = max.div(Fixed::from_int(120));

        country.manpower += recovery;
        if country.manpower > max {
            country.manpower = max;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::ProvinceState;
    use crate::testing::WorldStateBuilder;

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
        // Should be capped at 11000 + recovery?
        // Logic: country.manpower += recovery; if > max { = max }
        // So 20000 + rec > 11000 -> set to 11000.
        // Wait, if it's ALREADY above max, it should probably decrease or stay?
        // EU4 usually caps immediately if Rec > 0.
        // My logic: `country.manpower += recovery; if country.manpower > max`.
        // So it will snap to max.
        assert_eq!(swe.manpower, Fixed::from_int(11000));
    }
}
