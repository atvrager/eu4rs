//! Yearly tribute payments for tributary states.
//!
//! Tributaries pay a yearly lump sum to their overlord on January 1st.
//!
//! Formula per EU4 defines.lua:
//! - Gold: 12.5% of annual income
//! - Manpower: 25% of annual manpower
//! - Monarch power (ADM/DIP/MIL): 3% of total development, max 12
//!
//! The first year (1444) is prorated based on months elapsed since game start.

use crate::fixed::Fixed;
use crate::state::{TributeType, WorldState};

/// Tribute rate as fraction of annual income (12.5%)
const TRIBUTE_INCOME_RATE: f32 = 0.125;

/// Tribute rate as fraction of annual manpower (25%)
const TRIBUTE_MANPOWER_RATE: f32 = 0.25;

/// Tribute rate for monarch power (3% of development)
const TRIBUTE_MONARCH_POWER_RATE: f32 = 0.03;

/// Maximum monarch power tribute per year
const TRIBUTE_MAX_MONARCH_POWER: i32 = 12;

/// Types of tribute transfers
enum TributeTransfer {
    Gold {
        subject: String,
        overlord: String,
        amount: Fixed,
    },
    Manpower {
        subject: String,
        overlord: String,
        amount: Fixed,
    },
    MonarchPower {
        subject: String,
        overlord: String,
        amount: i32,
        power_type: TributeType, // ADM, DIP, or MIL
    },
}

/// Run yearly tribute payments for all tributary subjects.
///
/// Called on January 1st of each year. Transfers resources from
/// tributary subjects to their overlords based on tribute type.
///
/// Note: The first year (1444) is prorated since the game starts Nov 11.
/// Proration factor = months_elapsed / 12
pub fn run_tribute_payments(state: &mut WorldState) {
    // Collect tribute transfers to apply (to avoid borrow issues)
    let mut transfers: Vec<TributeTransfer> = Vec::new();

    // Calculate proration for short first year
    let game_start = crate::state::Date::default(); // Nov 11, 1444
    let current_year_start = crate::state::Date::new(state.date.year, 1, 1);

    let proration = if state.date.year == 1445 {
        // First year proration based on empirical data
        let days_elapsed = current_year_start.days_from_epoch() - game_start.days_from_epoch();
        let fractional_months = Fixed::from_int(days_elapsed).div(Fixed::from_int(30));
        let factor = fractional_months.div(Fixed::from_int(12));
        // Empirical correction (1.7×) to match EU4's actual proration
        let corrected = factor.mul(Fixed::from_f32(1.7));
        corrected.max(Fixed::ZERO).min(Fixed::ONE)
    } else {
        Fixed::ONE
    };

    // Find all tributary relationships
    for (subject_tag, relationship) in &state.diplomacy.subjects {
        if let Some(subject_type) = state.subject_types.get(relationship.subject_type) {
            // Tributaries don't join overlord wars and are voluntary
            if subject_type.is_voluntary && !subject_type.joins_overlords_wars {
                let subject_country = match state.countries.get(subject_tag) {
                    Some(c) => c,
                    None => continue,
                };

                // Get the tribute type (default to Gold if not set)
                let tribute_type = subject_country.tribute_type.unwrap_or(TributeType::Gold);

                match tribute_type {
                    TributeType::Gold => {
                        let monthly_income = subject_country.income.taxation
                            + subject_country.income.trade
                            + subject_country.income.production;
                        let annual_income = monthly_income.mul(Fixed::from_int(12));
                        let full_tribute = annual_income.mul(Fixed::from_f32(TRIBUTE_INCOME_RATE));
                        let tribute = full_tribute.mul(proration);

                        if tribute > Fixed::ZERO {
                            log::trace!(
                                "Tribute (Gold): {} annual_income={:.2}, tribute={:.2}",
                                subject_tag,
                                annual_income.to_f32(),
                                tribute.to_f32()
                            );
                            transfers.push(TributeTransfer::Gold {
                                subject: subject_tag.clone(),
                                overlord: relationship.overlord.clone(),
                                amount: tribute,
                            });
                        }
                    }
                    TributeType::Manpower => {
                        // Calculate annual manpower recovery
                        // Manpower is stored as raw men, annual recovery is complex
                        // Simplified: assume current manpower pool represents ~1 year's recovery
                        // This is a rough approximation - actual calculation would need
                        // base manpower from provinces + modifiers
                        let annual_manpower = calculate_annual_manpower(state, subject_tag);
                        let full_tribute =
                            annual_manpower.mul(Fixed::from_f32(TRIBUTE_MANPOWER_RATE));
                        let tribute = full_tribute.mul(proration);

                        if tribute > Fixed::ZERO {
                            log::trace!(
                                "Tribute (Manpower): {} annual={:.0}, tribute={:.0}",
                                subject_tag,
                                annual_manpower.to_f32(),
                                tribute.to_f32()
                            );
                            transfers.push(TributeTransfer::Manpower {
                                subject: subject_tag.clone(),
                                overlord: relationship.overlord.clone(),
                                amount: tribute,
                            });
                        }
                    }
                    TributeType::Adm | TributeType::Dip | TributeType::Mil => {
                        // Calculate total development for monarch power tribute
                        let total_dev = calculate_total_development(state, subject_tag);
                        let raw_tribute = (total_dev.to_f32() * TRIBUTE_MONARCH_POWER_RATE) as i32;
                        let full_tribute = raw_tribute.min(TRIBUTE_MAX_MONARCH_POWER);
                        // Prorate for first year (round down)
                        let tribute = ((full_tribute as f32) * proration.to_f32()) as i32;

                        if tribute > 0 {
                            log::trace!(
                                "Tribute ({:?}): {} dev={:.0}, tribute={}",
                                tribute_type,
                                subject_tag,
                                total_dev.to_f32(),
                                tribute
                            );
                            transfers.push(TributeTransfer::MonarchPower {
                                subject: subject_tag.clone(),
                                overlord: relationship.overlord.clone(),
                                amount: tribute,
                                power_type: tribute_type,
                            });
                        }
                    }
                }
            }
        }
    }

    // Apply transfers
    for transfer in transfers {
        match transfer {
            TributeTransfer::Gold {
                subject,
                overlord,
                amount,
            } => {
                apply_gold_tribute(state, &subject, &overlord, amount);
            }
            TributeTransfer::Manpower {
                subject,
                overlord,
                amount,
            } => {
                apply_manpower_tribute(state, &subject, &overlord, amount);
            }
            TributeTransfer::MonarchPower {
                subject,
                overlord,
                amount,
                power_type,
            } => {
                apply_monarch_power_tribute(state, &subject, &overlord, amount, power_type);
            }
        }
    }
}

/// Calculate annual manpower recovery for a country.
/// Based on sum of base_manpower from owned provinces × 1000 (men per dev).
fn calculate_annual_manpower(state: &WorldState, tag: &str) -> Fixed {
    let mut total_base_manpower = Fixed::ZERO;

    for province in state.provinces.values() {
        if province.owner.as_deref() == Some(tag) {
            total_base_manpower += province.base_manpower;
        }
    }

    // Convert base manpower (development) to actual men
    // Each point of manpower dev = 250 men/year base (before modifiers)
    total_base_manpower.mul(Fixed::from_int(250))
}

/// Calculate total development for a country.
fn calculate_total_development(state: &WorldState, tag: &str) -> Fixed {
    let mut total_dev = Fixed::ZERO;

    for province in state.provinces.values() {
        if province.owner.as_deref() == Some(tag) {
            total_dev += province.base_tax + province.base_production + province.base_manpower;
        }
    }

    total_dev
}

/// Apply gold tribute transfer.
fn apply_gold_tribute(state: &mut WorldState, subject: &str, overlord: &str, amount: Fixed) {
    let subject_treasury = state
        .countries
        .get(subject)
        .map(|c| c.treasury)
        .unwrap_or(Fixed::ZERO);

    // Can't pay more than available treasury
    let actual_payment = amount.min(subject_treasury.max(Fixed::ZERO));

    if actual_payment > Fixed::ZERO {
        if let Some(country) = state.countries.get_mut(subject) {
            country.treasury -= actual_payment;
            log::trace!(
                "Tribute: {} pays {:.2} ducats to {}",
                subject,
                actual_payment.to_f32(),
                overlord
            );
        }

        if let Some(country) = state.countries.get_mut(overlord) {
            country.treasury += actual_payment;
        }
    }
}

/// Apply manpower tribute transfer.
fn apply_manpower_tribute(state: &mut WorldState, subject: &str, overlord: &str, amount: Fixed) {
    let subject_manpower = state
        .countries
        .get(subject)
        .map(|c| c.manpower)
        .unwrap_or(Fixed::ZERO);

    // Can't pay more than available manpower
    let actual_payment = amount.min(subject_manpower.max(Fixed::ZERO));

    if actual_payment > Fixed::ZERO {
        if let Some(country) = state.countries.get_mut(subject) {
            country.manpower -= actual_payment;
            log::trace!(
                "Tribute: {} transfers {:.0} manpower to {}",
                subject,
                actual_payment.to_f32(),
                overlord
            );
        }

        if let Some(country) = state.countries.get_mut(overlord) {
            country.manpower += actual_payment;
        }
    }
}

/// Apply monarch power tribute transfer.
fn apply_monarch_power_tribute(
    state: &mut WorldState,
    subject: &str,
    overlord: &str,
    amount: i32,
    power_type: TributeType,
) {
    // Tribute is always paid in full (unlike gold which is capped by treasury)
    // Countries can go into monarch power debt from tribute payments
    let actual_payment = amount;

    if actual_payment > 0 {
        let payment_fixed = Fixed::from_int(actual_payment as i64);

        if let Some(country) = state.countries.get_mut(subject) {
            match power_type {
                TributeType::Adm => country.adm_mana -= payment_fixed,
                TributeType::Dip => country.dip_mana -= payment_fixed,
                TributeType::Mil => country.mil_mana -= payment_fixed,
                _ => {}
            }
            log::trace!(
                "Tribute: {} pays {} {:?} to {}",
                subject,
                actual_payment,
                power_type,
                overlord
            );
        }

        if let Some(country) = state.countries.get_mut(overlord) {
            match power_type {
                TributeType::Adm => country.adm_mana += payment_fixed,
                TributeType::Dip => country.dip_mana += payment_fixed,
                TributeType::Mil => country.mil_mana += payment_fixed,
                _ => {}
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

    #[test]
    fn test_tribute_manpower() {
        let mut state = WorldStateBuilder::new()
            .with_country("MNG")
            .with_country("KOR")
            .build();

        state.subject_types = make_tributary_registry();
        let tributary_id = state.subject_types.tributary_id;

        // Set date for full year
        state.date = Date::new(1446, 1, 1);

        // Give Korea provinces with manpower development
        // Add a province owned by Korea with 10 base manpower
        let prov_id: crate::state::ProvinceId = 1;
        state.provinces.insert(
            prov_id,
            crate::state::ProvinceState {
                owner: Some("KOR".to_string()),
                base_manpower: Fixed::from_int(10),
                ..Default::default()
            },
        );

        // Set Korea to pay manpower tribute
        state.countries.get_mut("KOR").unwrap().tribute_type = Some(TributeType::Manpower);
        state.countries.get_mut("KOR").unwrap().manpower = Fixed::from_int(10000);
        state.countries.get_mut("MNG").unwrap().manpower = Fixed::from_int(50000);

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

        // Annual manpower = 10 base × 250 = 2500 men
        // Tribute = 2500 × 0.25 = 625 men
        let kor = state.countries.get("KOR").unwrap();
        let mng = state.countries.get("MNG").unwrap();

        // Korea should have lost manpower, Ming gained it
        assert!(kor.manpower < Fixed::from_int(10000));
        assert!(mng.manpower > Fixed::from_int(50000));

        // The expected tribute is 625 men (10 dev × 250 × 0.25)
        let expected_tribute = Fixed::from_int(625);
        assert_eq!(kor.manpower, Fixed::from_int(10000) - expected_tribute);
        assert_eq!(mng.manpower, Fixed::from_int(50000) + expected_tribute);
    }

    #[test]
    fn test_tribute_monarch_power_mil() {
        let mut state = WorldStateBuilder::new()
            .with_country("MNG")
            .with_country("KOR")
            .build();

        state.subject_types = make_tributary_registry();
        let tributary_id = state.subject_types.tributary_id;

        // Set date for full year
        state.date = Date::new(1446, 1, 1);

        // Give Korea provinces with 145 total development (like real Korea)
        // 145 dev × 0.03 = 4.35, so tribute should be 4 MIL
        let prov_id: crate::state::ProvinceId = 1;
        state.provinces.insert(
            prov_id,
            crate::state::ProvinceState {
                owner: Some("KOR".to_string()),
                base_tax: Fixed::from_int(50),
                base_production: Fixed::from_int(45),
                base_manpower: Fixed::from_int(50),
                ..Default::default()
            },
        );

        // Set Korea to pay MIL tribute
        state.countries.get_mut("KOR").unwrap().tribute_type = Some(TributeType::Mil);
        state.countries.get_mut("KOR").unwrap().mil_mana = Fixed::from_int(200);
        state.countries.get_mut("MNG").unwrap().mil_mana = Fixed::from_int(100);

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

        // Dev = 50+45+50 = 145
        // Tribute = 145 × 0.03 = 4.35 → 4 MIL (rounded down)
        let kor = state.countries.get("KOR").unwrap();
        let mng = state.countries.get("MNG").unwrap();

        // Korea should have lost 4 MIL, Ming gained 4
        assert_eq!(kor.mil_mana, Fixed::from_int(196));
        assert_eq!(mng.mil_mana, Fixed::from_int(104));
    }

    #[test]
    fn test_tribute_monarch_power_capped_at_12() {
        let mut state = WorldStateBuilder::new()
            .with_country("MNG")
            .with_country("KOR")
            .build();

        state.subject_types = make_tributary_registry();
        let tributary_id = state.subject_types.tributary_id;

        // Set date for full year
        state.date = Date::new(1446, 1, 1);

        // Give Korea 500 total development
        // 500 × 0.03 = 15, but capped at 12
        let prov_id: crate::state::ProvinceId = 1;
        state.provinces.insert(
            prov_id,
            crate::state::ProvinceState {
                owner: Some("KOR".to_string()),
                base_tax: Fixed::from_int(200),
                base_production: Fixed::from_int(150),
                base_manpower: Fixed::from_int(150),
                ..Default::default()
            },
        );

        state.countries.get_mut("KOR").unwrap().tribute_type = Some(TributeType::Adm);
        state.countries.get_mut("KOR").unwrap().adm_mana = Fixed::from_int(200);
        state.countries.get_mut("MNG").unwrap().adm_mana = Fixed::from_int(100);

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

        // Should pay exactly 12 (capped)
        let kor = state.countries.get("KOR").unwrap();
        let mng = state.countries.get("MNG").unwrap();

        assert_eq!(kor.adm_mana, Fixed::from_int(188)); // 200 - 12
        assert_eq!(mng.adm_mana, Fixed::from_int(112)); // 100 + 12
    }
}
