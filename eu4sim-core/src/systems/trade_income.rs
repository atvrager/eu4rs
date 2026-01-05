//! Trade income collection system.
//!
//! Calculates monthly trade income for each country based on their power
//! share in trade nodes where they collect.
//!
//! # Collection Rules
//! - Home node: Automatic collection (no merchant required)
//! - Other nodes: Requires merchant with Collect action
//! - Merchant bonus: +10% collection efficiency
//!
//! # Formula
//! `monthly_income = (node_value × power_share × efficiency) / 12`
//!
//! where:
//! - `power_share = country_power / total_node_power`
//! - `efficiency = 1.0 + merchant_bonus (0.1 if merchant collecting)`

use crate::fixed::Fixed;
use crate::state::{Tag, WorldState};
use crate::trade::{MerchantAction, TradeNodeId};
use std::collections::HashMap;
use tracing::instrument;

/// Merchant collection efficiency bonus (+10%)
const MERCHANT_COLLECTION_BONUS: f32 = 0.1;

/// Runs the monthly trade income collection tick.
///
/// Call this AFTER trade value and trade power ticks.
///
/// # What it does
/// 1. Identifies where each country collects (home node + merchants)
/// 2. Calculates power share in each collecting node
/// 3. Applies efficiency bonuses
/// 4. Adds income to country treasury
#[instrument(skip_all, name = "trade_income")]
pub fn run_trade_income_tick(state: &mut WorldState) {
    // Skip if trade network isn't initialized
    if state.trade_topology.order.is_empty() {
        return;
    }

    // Collect income per country
    let income = calculate_trade_income(state);

    // Apply income to treasuries and record for display
    for (tag, amount) in income {
        if let Some(country) = state.countries.get_mut(&tag) {
            country.treasury += amount;
            country.income.trade += amount;

            if tag == "KOR" {
                log::debug!(
                    "Trade Income: KOR +{:.2} ducats (treasury now: {:.2})",
                    amount.to_f32(),
                    country.treasury.to_f32()
                );
            } else {
                log::trace!("{} collected {} trade income", tag, amount.to_f32());
            }
        }
    }
}

/// Calculate trade income for all countries.
fn calculate_trade_income(state: &WorldState) -> HashMap<Tag, Fixed> {
    let mut income: HashMap<Tag, Fixed> = HashMap::new();

    // Build collection info: (node_id, tag, has_merchant)
    let mut collection_info: Vec<(TradeNodeId, Tag, bool)> = Vec::new();

    // Home node collection (automatic, no merchant)
    for (tag, country) in state.countries.iter() {
        if let Some(home_node) = country.trade.home_node {
            collection_info.push((home_node, tag.clone(), false));
        }
    }

    // Merchant collection
    for (&node_id, node) in state.trade_nodes.iter() {
        for merchant in &node.merchants {
            if matches!(merchant.action, MerchantAction::Collect) {
                // Only add if not already collecting at home (avoid double collection)
                let is_home = state
                    .countries
                    .get(&merchant.owner)
                    .and_then(|c| c.trade.home_node)
                    .map(|h| h == node_id)
                    .unwrap_or(false);

                if is_home {
                    // Mark home collection as having merchant bonus
                    // Find and update existing entry
                    for (_, tag, has_merchant) in collection_info.iter_mut() {
                        if *tag == merchant.owner {
                            *has_merchant = true;
                            break;
                        }
                    }
                } else {
                    // Collecting outside home with merchant
                    collection_info.push((node_id, merchant.owner.clone(), true));
                }
            }
        }
    }

    // Calculate income for each collection point
    for (node_id, tag, has_merchant) in collection_info {
        let Some(node) = state.trade_nodes.get(&node_id) else {
            continue;
        };

        // Need power to collect
        let country_power = node.country_power.get(&tag).copied().unwrap_or(Fixed::ZERO);
        if country_power <= Fixed::ZERO {
            continue;
        }

        // Skip if node has no value or no power
        if node.total_value <= Fixed::ZERO || node.total_power <= Fixed::ZERO {
            continue;
        }

        // Calculate power share
        let power_share = country_power.div(node.total_power);

        // Calculate base income
        let base_income = node.total_value.mul(power_share);

        // Apply merchant efficiency bonus
        let merchant_efficiency = if has_merchant {
            Fixed::ONE + Fixed::from_f32(MERCHANT_COLLECTION_BONUS)
        } else {
            Fixed::ONE
        };

        // Apply country trade efficiency modifier
        let trade_eff_mod = state
            .modifiers
            .country_trade_efficiency
            .get(&tag)
            .copied()
            .unwrap_or(Fixed::ZERO);
        let total_efficiency = merchant_efficiency.mul(Fixed::ONE + trade_eff_mod);

        // Yearly trade income
        let yearly_income = base_income.mul(total_efficiency);

        // Monthly income = Yearly / 12
        let monthly_income =
            yearly_income.div(Fixed::from_int(eu4data::defines::economy::MONTHS_PER_YEAR));

        // Accumulate (country may collect from multiple nodes)
        *income.entry(tag).or_insert(Fixed::ZERO) += monthly_income;
    }

    income
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::CountryState;
    use crate::trade::{CountryTradeState, MerchantState, TradeNodeState, TradeTopology};

    fn setup_trade_income_state() -> WorldState {
        let mut state = WorldState::default();

        // Create trade node with value and power
        let node_a = TradeNodeId(0);
        let mut country_power = HashMap::new();
        country_power.insert("SWE".to_string(), Fixed::from_int(50)); // SWE has 50%

        state.trade_nodes.insert(
            node_a,
            TradeNodeState {
                total_value: Fixed::from_int(10),  // 10 ducats in node
                total_power: Fixed::from_int(100), // 100 total power
                country_power,
                ..Default::default()
            },
        );

        // Topological order (end node, no outgoing edges)
        state.trade_topology = TradeTopology {
            order: vec![node_a],
            end_nodes: vec![node_a],
            edges: std::collections::HashMap::new(),
        };

        // Add country with home node
        state.countries.insert(
            "SWE".to_string(),
            CountryState {
                trade: CountryTradeState {
                    home_node: Some(node_a),
                    ..Default::default()
                },
                ..Default::default()
            },
        );

        state
    }

    #[test]
    fn test_home_node_collection() {
        let mut state = setup_trade_income_state();
        let initial_treasury = state.countries["SWE"].treasury;

        run_trade_income_tick(&mut state);

        // Yearly: 10 value × 50% power share = 5 income
        // Monthly: 5 / 12 = ~0.4167
        let yearly = Fixed::from_int(5);
        let expected_income = yearly.div(Fixed::from_int(12));
        assert_eq!(
            state.countries["SWE"].treasury,
            initial_treasury + expected_income
        );
    }

    #[test]
    fn test_merchant_collection_bonus() {
        let mut state = setup_trade_income_state();

        // Add collecting merchant at home node
        if let Some(node) = state.trade_nodes.get_mut(&TradeNodeId(0)) {
            node.merchants.push(MerchantState {
                owner: "SWE".to_string(),
                action: MerchantAction::Collect,
            });
        }

        let initial_treasury = state.countries["SWE"].treasury;
        run_trade_income_tick(&mut state);

        // Yearly: 10 × 50% × 1.1 (merchant bonus) = 5.5
        // Monthly: 5.5 / 12 = ~0.4583
        let yearly = Fixed::from_f32(5.5);
        let expected_income = yearly.div(Fixed::from_int(12));
        assert_eq!(
            state.countries["SWE"].treasury,
            initial_treasury + expected_income
        );
    }

    #[test]
    fn test_collection_outside_home_requires_merchant() {
        let mut state = setup_trade_income_state();

        // Change home node to different node
        if let Some(country) = state.countries.get_mut("SWE") {
            country.trade.home_node = Some(TradeNodeId(99)); // Different node
        }

        // No merchant at node 0, so no collection there
        let initial_treasury = state.countries["SWE"].treasury;
        run_trade_income_tick(&mut state);

        // No income without merchant at non-home node
        assert_eq!(state.countries["SWE"].treasury, initial_treasury);
    }

    #[test]
    fn test_collection_outside_home_with_merchant() {
        let mut state = setup_trade_income_state();

        // Change home node to different node
        if let Some(country) = state.countries.get_mut("SWE") {
            country.trade.home_node = Some(TradeNodeId(99)); // Different node
        }

        // Add collecting merchant at node 0
        if let Some(node) = state.trade_nodes.get_mut(&TradeNodeId(0)) {
            node.merchants.push(MerchantState {
                owner: "SWE".to_string(),
                action: MerchantAction::Collect,
            });
        }

        let initial_treasury = state.countries["SWE"].treasury;
        run_trade_income_tick(&mut state);

        // Yearly: 10 × 50% × 1.1 = 5.5 (merchant bonus applies)
        // Monthly: 5.5 / 12 = ~0.4583
        // Note: Power was already reduced by -50% in trade_power tick
        // Here we just test collection with the power we have
        let yearly = Fixed::from_f32(5.5);
        let expected_income = yearly.div(Fixed::from_int(12));
        assert_eq!(
            state.countries["SWE"].treasury,
            initial_treasury + expected_income
        );
    }

    #[test]
    fn test_steering_merchant_no_collection() {
        let mut state = setup_trade_income_state();

        // Change home node to different node
        if let Some(country) = state.countries.get_mut("SWE") {
            country.trade.home_node = Some(TradeNodeId(99)); // Different node
        }

        // Add steering merchant (not collecting)
        if let Some(node) = state.trade_nodes.get_mut(&TradeNodeId(0)) {
            node.merchants.push(MerchantState {
                owner: "SWE".to_string(),
                action: MerchantAction::Steer {
                    target: TradeNodeId(99),
                },
            });
        }

        let initial_treasury = state.countries["SWE"].treasury;
        run_trade_income_tick(&mut state);

        // No income from steering
        assert_eq!(state.countries["SWE"].treasury, initial_treasury);
    }

    #[test]
    fn test_multiple_countries_share_node() {
        let mut state = setup_trade_income_state();

        // Add DAN with 30% power
        state.countries.insert(
            "DAN".to_string(),
            CountryState {
                trade: CountryTradeState {
                    home_node: Some(TradeNodeId(0)),
                    ..Default::default()
                },
                ..Default::default()
            },
        );

        if let Some(node) = state.trade_nodes.get_mut(&TradeNodeId(0)) {
            node.country_power
                .insert("DAN".to_string(), Fixed::from_int(30));
        }

        run_trade_income_tick(&mut state);

        // Yearly: SWE: 10 × 50/100 = 5, DAN: 10 × 30/100 = 3
        // Monthly: 5/12 = ~0.4167, 3/12 = 0.25
        let swe_yearly = Fixed::from_int(5);
        let dan_yearly = Fixed::from_int(3);
        assert_eq!(
            state.countries["SWE"].treasury,
            swe_yearly.div(Fixed::from_int(12))
        );
        assert_eq!(
            state.countries["DAN"].treasury,
            dan_yearly.div(Fixed::from_int(12))
        );
    }

    #[test]
    fn test_no_power_no_income() {
        let mut state = setup_trade_income_state();

        // Remove SWE's power
        if let Some(node) = state.trade_nodes.get_mut(&TradeNodeId(0)) {
            node.country_power.remove("SWE");
        }

        let initial_treasury = state.countries["SWE"].treasury;
        run_trade_income_tick(&mut state);

        // No power = no income
        assert_eq!(state.countries["SWE"].treasury, initial_treasury);
    }

    #[test]
    fn test_empty_node_no_income() {
        let mut state = setup_trade_income_state();

        // Set node value to zero
        if let Some(node) = state.trade_nodes.get_mut(&TradeNodeId(0)) {
            node.total_value = Fixed::ZERO;
        }

        let initial_treasury = state.countries["SWE"].treasury;
        run_trade_income_tick(&mut state);

        // No value = no income
        assert_eq!(state.countries["SWE"].treasury, initial_treasury);
    }

    #[test]
    fn test_empty_topology_no_panic() {
        let mut state = WorldState::default();
        run_trade_income_tick(&mut state);
    }

    #[test]
    fn test_collect_from_multiple_nodes() {
        let mut state = setup_trade_income_state();

        // Add second node where SWE also has power
        let node_b = TradeNodeId(1);
        let mut country_power = HashMap::new();
        country_power.insert("SWE".to_string(), Fixed::from_int(25)); // 25%

        state.trade_nodes.insert(
            node_b,
            TradeNodeState {
                total_value: Fixed::from_int(20), // 20 ducats
                total_power: Fixed::from_int(100),
                country_power,
                merchants: vec![MerchantState {
                    owner: "SWE".to_string(),
                    action: MerchantAction::Collect,
                }],
                ..Default::default()
            },
        );

        state.trade_topology.order.push(node_b);

        let initial_treasury = state.countries["SWE"].treasury;
        run_trade_income_tick(&mut state);

        // Yearly:
        //   Node A (home): 10 × 50% = 5
        //   Node B (merchant): 20 × 25% × 1.1 = 5.5
        //   Total: 10.5
        // Monthly: 10.5 / 12 = ~0.875
        let final_treasury = state.countries["SWE"].treasury;
        let income = final_treasury - initial_treasury;

        // Verify income is approximately 10.5/12 (allow for rounding)
        let expected_approx = Fixed::from_f32(10.5 / 12.0);
        let diff = if income > expected_approx {
            income - expected_approx
        } else {
            expected_approx - income
        };
        assert!(
            diff < Fixed::from_f32(0.01),
            "Income {} not close to expected {}",
            income.to_f32(),
            expected_approx.to_f32()
        );
    }
}
