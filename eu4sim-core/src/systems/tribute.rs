//! Yearly tribute payments for tributary states.
//!
//! Tributaries pay a yearly lump sum to their overlord on January 1st.
//!
//! Formula per EU4 wiki:
//! - Ducats: 12.5% of annual income
//! - Manpower: 25% of annual manpower (not yet implemented)
//! - Monarch power: development/33 rounded down, max 12 (not yet implemented)
//!
//! The first year (1444) is prorated based on months elapsed since game start.

use crate::fixed::Fixed;
use crate::state::WorldState;

/// Tribute rate as fraction of annual income (12.5% = 0.125)
const TRIBUTE_INCOME_RATE: f32 = 0.125;

/// Run yearly tribute payments for all tributary subjects.
///
/// Called on January 1st of each year. Transfers ducats from
/// tributary subjects to their overlords.
///
/// Note: The first year (1444) is prorated since the game starts Nov 11.
/// Proration factor = months_elapsed / 12
pub fn run_tribute_payments(state: &mut WorldState) {
    // Collect tribute transfers to apply (to avoid borrow issues)
    let mut transfers: Vec<(String, String, Fixed)> = Vec::new();

    // Calculate proration for short first year
    // Game starts Nov 11, 1444 (game_start) and we're at Jan 1 of current year
    // Proration = months elapsed since game start / 12
    let game_start = crate::state::Date::default(); // Nov 11, 1444
    let current_year_start = crate::state::Date::new(state.date.year, 1, 1);

    // For the first tribute (Jan 1445), we prorate from Nov 11 1444
    // For subsequent years, it's a full 12 months
    let proration = if state.date.year == 1445 {
        // First year proration based on empirical data:
        // Korea pays ~4 ducats of a 14 ducat annual obligation = 0.286
        // This suggests EU4 counts ~3.4 months for Nov 11 - Dec 31.
        // Using days-based calculation: 51 days / 365 × 12 ≈ 1.68 months
        // But EU4 likely rounds up or uses full months (Nov=1, Dec=1, partial Jan=1)
        // Empirical: 4/14 = 0.286, roughly 3.4/12
        let days_elapsed = current_year_start.days_from_epoch() - game_start.days_from_epoch();
        // Use days/365 × 12 to get fractional months, but EU4 seems to use ~1.7× this
        // Empirical adjustment: multiply by 1.7 to match observed 4 ducat payment
        let fractional_months = Fixed::from_int(days_elapsed).div(Fixed::from_int(30));
        let factor = fractional_months.div(Fixed::from_int(12));
        // Apply empirical correction (1.7×) to match EU4's actual proration
        let corrected = factor.mul(Fixed::from_f32(1.7));
        corrected.max(Fixed::ZERO).min(Fixed::ONE)
    } else {
        Fixed::ONE // Full year for subsequent years
    };

    // Find all tributary relationships
    for (subject_tag, relationship) in &state.diplomacy.subjects {
        // Check if this is a tributary type
        if let Some(subject_type) = state.subject_types.get(relationship.subject_type) {
            // Tributaries don't join overlord wars and are voluntary
            if subject_type.is_voluntary && !subject_type.joins_overlords_wars {
                // Get subject's monthly income (from last tick's calculation)
                let monthly_income = state
                    .countries
                    .get(subject_tag)
                    .map(|c| c.income.taxation + c.income.trade + c.income.production)
                    .unwrap_or(Fixed::ZERO);

                // Annual income = monthly × 12
                let annual_income = monthly_income.mul(Fixed::from_int(12));

                // Tribute = 12.5% of annual income, prorated for first year
                let full_tribute = annual_income.mul(Fixed::from_f32(TRIBUTE_INCOME_RATE));
                let tribute = full_tribute.mul(proration);

                if tribute > Fixed::ZERO {
                    log::debug!(
                        "Tribute calc: {} monthly_income={:.2}, annual={:.2}, rate={}, proration={:.2}, tribute={:.2}",
                        subject_tag,
                        monthly_income.to_f32(),
                        annual_income.to_f32(),
                        TRIBUTE_INCOME_RATE,
                        proration.to_f32(),
                        tribute.to_f32()
                    );
                    transfers.push((
                        subject_tag.clone(),
                        relationship.overlord.clone(),
                        tribute,
                    ));
                }
            }
        }
    }

    // Apply transfers
    for (subject, overlord, amount) in transfers {
        // Check if subject has enough treasury
        let subject_treasury = state
            .countries
            .get(&subject)
            .map(|c| c.treasury)
            .unwrap_or(Fixed::ZERO);

        // Take minimum of tribute and available treasury (can't go negative)
        let actual_payment = amount.min(subject_treasury.max(Fixed::ZERO));

        if actual_payment > Fixed::ZERO {
            // Deduct from subject
            if let Some(country) = state.countries.get_mut(&subject) {
                country.treasury -= actual_payment;
                // Note: Don't add to income.expenses - that's for monthly expenses
                // Tribute is a yearly lump sum tracked separately in treasury
                log::info!(
                    "Tribute: {} pays {} ducats to {} (income-based)",
                    subject,
                    actual_payment.to_f32(),
                    overlord
                );
            }

            // Add to overlord
            if let Some(country) = state.countries.get_mut(&overlord) {
                country.treasury += actual_payment;
                // Could add to income.vassals if we had that field
                log::debug!(
                    "Tribute: {} receives {} ducats from {}",
                    overlord,
                    actual_payment.to_f32(),
                    subject
                );
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::{Date, IncomeBreakdown, SubjectRelationship};
    use crate::subjects::{SubjectTypeDef, SubjectTypeRegistry};
    use crate::testing::WorldStateBuilder;

    fn make_tributary_registry() -> SubjectTypeRegistry {
        let mut registry = SubjectTypeRegistry::new();

        // Add tributary type
        registry.add(SubjectTypeDef {
            name: "tributary_state".into(),
            joins_overlords_wars: false,
            is_voluntary: true,
            ..Default::default()
        });

        registry
    }

    #[test]
    fn test_tribute_payment_basic() {
        let mut state = WorldStateBuilder::new()
            .with_country("MNG")
            .with_country("KOR")
            .build();

        // Set up subject type registry
        state.subject_types = make_tributary_registry();
        let tributary_id = state.subject_types.tributary_id;

        // Set date to Jan 1446 for a full year (no proration)
        state.date = Date::new(1446, 1, 1);

        // Give Korea income: 10 ducats/month total
        // Annual = 120, tribute = 120 × 0.125 = 15 ducats
        state.countries.get_mut("KOR").unwrap().income = IncomeBreakdown {
            taxation: Fixed::from_int(5),
            trade: Fixed::from_int(3),
            production: Fixed::from_int(2),
            expenses: Fixed::ZERO,
        };

        // Set initial treasuries
        state.countries.get_mut("KOR").unwrap().treasury = Fixed::from_int(50);
        state.countries.get_mut("MNG").unwrap().treasury = Fixed::from_int(100);

        // Create tributary relationship
        state.diplomacy.subjects.insert(
            "KOR".to_string(),
            SubjectRelationship {
                overlord: "MNG".to_string(),
                subject: "KOR".to_string(),
                subject_type: tributary_id,
                start_date: Date::new(1444, 1, 1),
                liberty_desire: 0,
                integration_progress: 0,
                integrating: false,
            },
        );

        // Run tribute payments
        run_tribute_payments(&mut state);

        // Check treasury changes
        // Monthly income = 5+3+2 = 10, annual = 120, tribute = 120 × 0.125 = 15
        let kor = state.countries.get("KOR").unwrap();
        let mng = state.countries.get("MNG").unwrap();

        let expected_tribute = Fixed::from_int(15);
        assert_eq!(kor.treasury, Fixed::from_int(50) - expected_tribute);
        assert_eq!(mng.treasury, Fixed::from_int(100) + expected_tribute);
    }

    #[test]
    fn test_tribute_prorated_first_year() {
        let mut state = WorldStateBuilder::new()
            .with_country("MNG")
            .with_country("KOR")
            .build();

        state.subject_types = make_tributary_registry();
        let tributary_id = state.subject_types.tributary_id;

        // Set date to Jan 1445 for prorated first year
        state.date = Date::new(1445, 1, 1);

        // Give Korea income: 10 ducats/month
        state.countries.get_mut("KOR").unwrap().income = IncomeBreakdown {
            taxation: Fixed::from_int(5),
            trade: Fixed::from_int(3),
            production: Fixed::from_int(2),
            expenses: Fixed::ZERO,
        };

        state.countries.get_mut("KOR").unwrap().treasury = Fixed::from_int(50);
        state.countries.get_mut("MNG").unwrap().treasury = Fixed::from_int(100);

        state.diplomacy.subjects.insert(
            "KOR".to_string(),
            SubjectRelationship {
                overlord: "MNG".to_string(),
                subject: "KOR".to_string(),
                subject_type: tributary_id,
                start_date: Date::new(1444, 1, 1),
                liberty_desire: 0,
                integration_progress: 0,
                integrating: false,
            },
        );

        run_tribute_payments(&mut state);

        // First year tribute is prorated
        // Full tribute = 120 × 0.125 = 15
        // Proration = ~2 months / 12 = 0.167
        // Prorated tribute = 15 × 0.167 ≈ 2.5
        let kor = state.countries.get("KOR").unwrap();

        // Just verify tribute was paid (less than full amount)
        assert!(kor.treasury < Fixed::from_int(50));
        assert!(kor.treasury > Fixed::from_int(35)); // Should be much less than 15 deducted
    }

    #[test]
    fn test_tribute_capped_by_treasury() {
        let mut state = WorldStateBuilder::new()
            .with_country("MNG")
            .with_country("KOR")
            .build();

        state.subject_types = make_tributary_registry();
        let tributary_id = state.subject_types.tributary_id;

        // Set date for full year
        state.date = Date::new(1446, 1, 1);

        // High income but low treasury
        state.countries.get_mut("KOR").unwrap().income = IncomeBreakdown {
            taxation: Fixed::from_int(20),
            trade: Fixed::from_int(20),
            production: Fixed::from_int(20),
            expenses: Fixed::ZERO,
        };
        // Annual = 720, tribute = 90 ducats, but only 5 available
        state.countries.get_mut("KOR").unwrap().treasury = Fixed::from_int(5);
        state.countries.get_mut("MNG").unwrap().treasury = Fixed::from_int(100);

        state.diplomacy.subjects.insert(
            "KOR".to_string(),
            SubjectRelationship {
                overlord: "MNG".to_string(),
                subject: "KOR".to_string(),
                subject_type: tributary_id,
                start_date: Date::new(1444, 1, 1),
                liberty_desire: 0,
                integration_progress: 0,
                integrating: false,
            },
        );

        run_tribute_payments(&mut state);

        // Korea should pay only what they have (5 ducats)
        let kor = state.countries.get("KOR").unwrap();
        let mng = state.countries.get("MNG").unwrap();

        assert_eq!(kor.treasury, Fixed::ZERO); // Paid all 5
        assert_eq!(mng.treasury, Fixed::from_int(105)); // 100 + 5
    }
}
