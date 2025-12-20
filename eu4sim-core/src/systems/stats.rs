use crate::fixed::Fixed;
use crate::state::WorldState;

/// Monthly decay rates (EU4 approximations)
/// EU4 Standard: 5% yearly decay
/// Yearly factor: 0.95
/// Monthly factor: 0.95^(1/12) ≈ 0.99574
/// Monthly decay rate: 1 - 0.9957 = 0.00426
///
/// 42 / 10000 = 0.0042
const DECAY_RATE: Fixed = Fixed::from_raw(42);

/// Run monthly country stat updates.
/// Call on the 1st of each month.
///
/// Everything in the world eventually decays toward its foundation. ✧
/// Pride fades into history (prestige) and strength returns to the soil (tradition). 🛡️
pub fn run_stats_tick(state: &mut WorldState) {
    let tags: Vec<String> = state.countries.keys().cloned().collect();
    for tag in tags {
        if let Some(country) = state.countries.get_mut(&tag) {
            // Prestige decays toward 0 - Fame is but a shadow that shrinks as the sun moves.
            country.prestige.decay_toward(Fixed::ZERO, DECAY_RATE);

            // Army tradition decays toward 0 - Even the sharpest blade rusts if it is not used in battle.
            country.army_tradition.decay_toward(Fixed::ZERO, DECAY_RATE);

            // Stability does NOT decay (only events change it) - Peace is a fragile truth that must be broken to change.
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bounded::new_prestige;
    use crate::state::CountryState;

    #[test]
    fn test_prestige_decay() {
        let mut country = CountryState {
            prestige: new_prestige(),
            ..Default::default()
        };
        country.prestige.set(Fixed::from_int(100)); // Max prestige

        let mut state = WorldState::default();
        state.countries.insert("TAG".to_string(), country);

        // Run one tick
        run_stats_tick(&mut state);

        let updated = state.countries.get("TAG").unwrap();
        // Should be less than 100
        assert!(updated.prestige.get() < Fixed::from_int(100));
        // Should be around 100 - (100 * 0.0042) = 99.58
        // 100 * 42 = 4200 (raw)
        // 1000000 - 4200 = 995800 raw -> 99.58
        assert_eq!(
            updated.prestige.get(),
            Fixed::from_int(100) - Fixed::from_f32(0.42)
        );
    }
}
