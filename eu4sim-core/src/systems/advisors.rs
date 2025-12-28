//! Advisor salary system.
//!
//! Advisors provide monthly monarch points but cost ducats each month.
//! Salaries are deducted from the country's treasury on each monthly tick.

use crate::fixed::Fixed;
use crate::state::WorldState;

/// Calculate and deduct monthly advisor salaries.
///
/// This system runs on the 1st of each month and deducts the total cost of all
/// advisors from each country's treasury.
pub fn run_advisor_cost_tick(state: &mut WorldState) {
    for (tag, country) in state.countries.iter_mut() {
        if country.advisors.is_empty() {
            continue;
        }

        let mut total_cost = Fixed::ZERO;

        for advisor in &country.advisors {
            total_cost += advisor.monthly_cost;
        }

        // Deduct from treasury
        country.treasury -= total_cost;
        country.income.expenses += total_cost;

        log::info!("{} advisor salaries: -{}", tag, total_cost);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::{Advisor, AdvisorType, CountryState, Date, WorldState};

    #[test]
    fn test_advisor_cost_single() {
        let mut state = WorldState::default();
        state.date = Date::new(1444, 11, 11);

        let mut country = CountryState::default();
        country.treasury = Fixed::from_int(100);
        country.advisors = vec![Advisor {
            name: "Test Advisor".to_string(),
            skill: 3,
            advisor_type: AdvisorType::Administrative,
            monthly_cost: Fixed::from_int(5),
        }];
        state.countries.insert("TEST".to_string(), country);

        run_advisor_cost_tick(&mut state);

        assert_eq!(
            state.countries["TEST"].treasury,
            Fixed::from_int(95),
            "Treasury should be reduced by advisor cost"
        );
        assert_eq!(
            state.countries["TEST"].income.expenses,
            Fixed::from_int(5),
            "Expenses should track advisor cost"
        );
    }

    #[test]
    fn test_advisor_cost_multiple() {
        let mut state = WorldState::default();
        state.date = Date::new(1444, 11, 11);

        let mut country = CountryState::default();
        country.treasury = Fixed::from_int(100);
        country.advisors = vec![
            Advisor {
                name: "Admin Advisor".to_string(),
                skill: 5,
                advisor_type: AdvisorType::Administrative,
                monthly_cost: Fixed::from_int(20),
            },
            Advisor {
                name: "Diplo Advisor".to_string(),
                skill: 4,
                advisor_type: AdvisorType::Diplomatic,
                monthly_cost: Fixed::from_int(15),
            },
            Advisor {
                name: "Military Advisor".to_string(),
                skill: 3,
                advisor_type: AdvisorType::Military,
                monthly_cost: Fixed::from_int(10),
            },
        ];
        state.countries.insert("TEST".to_string(), country);

        run_advisor_cost_tick(&mut state);

        // Total: 20 + 15 + 10 = 45
        assert_eq!(
            state.countries["TEST"].treasury,
            Fixed::from_int(55),
            "Treasury should be reduced by sum of all advisor costs"
        );
        assert_eq!(
            state.countries["TEST"].income.expenses,
            Fixed::from_int(45),
            "Expenses should track total advisor cost"
        );
    }

    #[test]
    fn test_advisor_cost_no_advisors() {
        let mut state = WorldState::default();
        state.date = Date::new(1444, 11, 11);

        let mut country = CountryState::default();
        country.treasury = Fixed::from_int(100);
        country.advisors = vec![];
        state.countries.insert("TEST".to_string(), country);

        run_advisor_cost_tick(&mut state);

        assert_eq!(
            state.countries["TEST"].treasury,
            Fixed::from_int(100),
            "Treasury should not change with no advisors"
        );
        assert_eq!(
            state.countries["TEST"].income.expenses,
            Fixed::ZERO,
            "Expenses should be zero with no advisors"
        );
    }

    #[test]
    fn test_advisor_cost_negative_treasury() {
        // Advisors should still be paid even if treasury goes negative (debt)
        let mut state = WorldState::default();
        state.date = Date::new(1444, 11, 11);

        let mut country = CountryState::default();
        country.treasury = Fixed::from_int(5); // Not enough to pay
        country.advisors = vec![Advisor {
            name: "Expensive Advisor".to_string(),
            skill: 5,
            advisor_type: AdvisorType::Administrative,
            monthly_cost: Fixed::from_int(20),
        }];
        state.countries.insert("TEST".to_string(), country);

        run_advisor_cost_tick(&mut state);

        assert_eq!(
            state.countries["TEST"].treasury,
            Fixed::from_int(-15),
            "Treasury can go negative (debt)"
        );
    }
}
