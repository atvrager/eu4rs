//! Trade power calculation system.
//!
//! Calculates trade power for each country in each trade node from:
//! - Provincial power: development × 0.2 for owned provinces
//! - Centers of Trade: +5/+10/+25 for level 1/2/3
//! - Light ships: +3 per ship protecting trade (future)
//! - Merchant bonus: +2 base power when merchant is present
//! - Trade buildings: Marketplace +2, Trade Depot +5 (future)
//!
//! # Critical: Collection Penalty (D2 from design doc)
//! When collecting outside home node, country receives -50% power penalty.
//! This creates the "steer to home" gameplay loop.

use crate::fixed::Fixed;
use crate::fixed_generic::Mod32;
use crate::state::{Tag, WorldState};
use crate::trade::{MerchantAction, TradeNodeId};
use std::collections::HashMap;

/// Provincial power per development point (EU4: 0.2)
const DEV_POWER_MULTIPLIER: f32 = 0.2;

/// Power bonus from merchant presence
const MERCHANT_POWER_BONUS: i64 = 2;

/// Collection penalty when not in home node (-50%)
const NON_HOME_COLLECTION_PENALTY: f32 = 0.5;

/// Power bonus from Centers of Trade
const COT_LEVEL_1_BONUS: i64 = 5;
const COT_LEVEL_2_BONUS: i64 = 10;
const COT_LEVEL_3_BONUS: i64 = 25;

/// Processes merchant arrivals for all countries.
///
/// Merchants that were dispatched earlier arrive at their destination
/// when the current date reaches their arrival_date. Should be called
/// BEFORE trade power tick so arriving merchants participate in trade.
pub fn run_merchant_arrivals(state: &mut WorldState) {
    use crate::trade::MerchantState;

    // Collect arrivals first to avoid borrow conflicts
    let mut arrivals: Vec<(
        String,
        crate::trade::TradeNodeId,
        crate::trade::MerchantAction,
    )> = Vec::new();

    for (tag, country) in state.countries.iter() {
        for travel in &country.trade.merchants_en_route {
            if state.date >= travel.arrival_date {
                arrivals.push((tag.clone(), travel.destination, travel.action.clone()));
            }
        }
    }

    // Place arrived merchants at their destinations
    for (owner, node_id, action) in arrivals {
        if let Some(node) = state.trade_nodes.get_mut(&node_id) {
            node.merchants.push(MerchantState {
                owner: owner.clone(),
                action,
            });
            log::info!(
                "Merchant from {} arrives at trade node {:?}",
                owner,
                node_id
            );
        }
    }

    // Remove arrived merchants from en_route lists
    let current_date = state.date;
    let country_tags: Vec<_> = state.countries.keys().cloned().collect();
    for tag in country_tags {
        if let Some(country) = state.countries.get_mut(&tag) {
            country
                .trade
                .merchants_en_route
                .retain(|t| current_date < t.arrival_date);
        }
    }
}

/// Runs the monthly trade power tick.
///
/// Call this BEFORE trade value tick so that value propagation can
/// calculate retention based on collection power.
///
/// # What it does
/// 1. Resets all country power in all nodes
/// 2. Calculates provincial power for each country
/// 3. Adds merchant bonuses
/// 4. Applies collection penalty for non-home nodes
/// 5. Recalculates total node power
pub fn run_trade_power_tick(state: &mut WorldState) {
    // Skip if trade network isn't initialized
    if state.trade_topology.order.is_empty() {
        return;
    }

    // 1. Reset all country power
    let node_ids: Vec<_> = state.trade_nodes.keys().copied().collect();
    for node_id in &node_ids {
        if let Some(node) = state.trade_nodes.get_mut(node_id) {
            node.country_power.clear();
            node.total_power = Fixed::ZERO;
        }
    }

    // 2. Calculate provincial power
    calculate_provincial_power(state);

    // 3. Add merchant bonuses and apply collection penalty
    apply_merchant_modifiers(state);

    // 4. Recalculate total power per node
    for node_id in &node_ids {
        if let Some(node) = state.trade_nodes.get_mut(node_id) {
            node.total_power = node.country_power.values().fold(Fixed::ZERO, |a, &b| a + b);
        }
    }
}

/// Calculate trade power from provinces for each country in each node.
fn calculate_provincial_power(state: &mut WorldState) {
    let dev_mult = Mod32::from_f32(DEV_POWER_MULTIPLIER);

    for (&province_id, province) in state.provinces.iter() {
        // Skip unowned provinces
        let Some(ref owner) = province.owner else {
            continue;
        };

        // Get trade node for this province
        let Some(&node_id) = state.province_trade_node.get(&province_id) else {
            continue;
        };

        // Calculate base power from development
        // Total dev = tax + production + manpower
        let total_dev = province.base_tax + province.base_production + province.base_manpower;
        let mut power = total_dev * dev_mult;

        // Add Center of Trade bonus
        let cot_bonus = match province.trade.center_of_trade {
            1 => Mod32::from_int(COT_LEVEL_1_BONUS as i32),
            2 => Mod32::from_int(COT_LEVEL_2_BONUS as i32),
            3 => Mod32::from_int(COT_LEVEL_3_BONUS as i32),
            _ => Mod32::ZERO,
        };
        power += cot_bonus;

        // Apply country global trade power modifier
        let trade_power_mod = state
            .modifiers
            .country_trade_power
            .get(owner)
            .copied()
            .unwrap_or(Mod32::ZERO);
        let modified_power = power * (Mod32::ONE + trade_power_mod);

        // Accumulate power to country in node
        if let Some(node) = state.trade_nodes.get_mut(&node_id) {
            *node
                .country_power
                .entry(owner.clone())
                .or_insert(Fixed::ZERO) += modified_power.to_fixed();
        }
    }
}

/// Add merchant bonuses and apply collection penalty.
fn apply_merchant_modifiers(state: &mut WorldState) {
    // Collect merchants and home nodes first to avoid borrow issues
    let mut merchant_info: Vec<(TradeNodeId, Tag, MerchantAction)> = Vec::new();
    let mut home_nodes: HashMap<Tag, TradeNodeId> = HashMap::new();

    // Iterate with node_id to properly associate merchants
    let node_ids: Vec<_> = state.trade_nodes.keys().copied().collect();
    for &node_id in &node_ids {
        if let Some(node) = state.trade_nodes.get(&node_id) {
            for merchant in &node.merchants {
                merchant_info.push((node_id, merchant.owner.clone(), merchant.action.clone()));
            }
        }
    }

    // Get home nodes from country state
    for (tag, country) in state.countries.iter() {
        if let Some(home) = country.trade.home_node {
            home_nodes.insert(tag.clone(), home);
        }
    }

    // Apply merchant modifiers
    for (node_id, owner, action) in merchant_info {
        if let Some(node) = state.trade_nodes.get_mut(&node_id) {
            // Add merchant power bonus (+2)
            *node
                .country_power
                .entry(owner.clone())
                .or_insert(Fixed::ZERO) += Fixed::from_int(MERCHANT_POWER_BONUS);

            // Apply collection penalty if collecting outside home node
            if matches!(action, MerchantAction::Collect) {
                let home = home_nodes.get(&owner);
                let is_home = home.map(|&h| h == node_id).unwrap_or(false);

                if !is_home {
                    // Apply -50% penalty
                    if let Some(power) = node.country_power.get_mut(&owner) {
                        *power = power.mul(Fixed::from_f32(NON_HOME_COLLECTION_PENALTY));
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::{CountryState, ProvinceState};
    use crate::trade::{
        CountryTradeState, MerchantState, ProvinceTradeState, TradeNodeState, TradeTopology,
    };

    fn setup_trade_power_state() -> WorldState {
        let mut state = WorldState::default();

        // Create trade node
        let node_a = TradeNodeId(0);
        state.trade_nodes.insert(node_a, TradeNodeState::default());

        // Topological order (end node, no outgoing edges)
        state.trade_topology = TradeTopology {
            order: vec![node_a],
            end_nodes: vec![node_a],
            edges: std::collections::HashMap::new(),
        };

        // Map province 1 to node A
        state.province_trade_node.insert(1, node_a);

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

        // Add province with 3/3/3 development (total 9)
        state.provinces.insert(
            1,
            ProvinceState {
                owner: Some("SWE".to_string()),
                base_tax: Mod32::from_int(3),
                base_production: Mod32::from_int(3),
                base_manpower: Mod32::from_int(3),
                trade: ProvinceTradeState::default(),
                ..Default::default()
            },
        );

        state
    }

    #[test]
    fn test_provincial_power_calculation() {
        let mut state = setup_trade_power_state();

        run_trade_power_tick(&mut state);

        // 9 dev × 0.2 = 1.8 power
        let node = &state.trade_nodes[&TradeNodeId(0)];
        assert_eq!(node.country_power["SWE"], Fixed::from_f32(1.8));
        assert_eq!(node.total_power, Fixed::from_f32(1.8));
    }

    #[test]
    fn test_center_of_trade_bonus() {
        let mut state = setup_trade_power_state();

        // Add level 2 CoT
        if let Some(prov) = state.provinces.get_mut(&1) {
            prov.trade.center_of_trade = 2;
        }

        run_trade_power_tick(&mut state);

        // 9 × 0.2 + 10 = 1.8 + 10 = 11.8
        let node = &state.trade_nodes[&TradeNodeId(0)];
        assert_eq!(node.country_power["SWE"], Fixed::from_f32(11.8));
    }

    #[test]
    fn test_merchant_power_bonus() {
        let mut state = setup_trade_power_state();

        // Add collecting merchant
        if let Some(node) = state.trade_nodes.get_mut(&TradeNodeId(0)) {
            node.merchants.push(MerchantState {
                owner: "SWE".to_string(),
                action: MerchantAction::Collect,
            });
        }

        run_trade_power_tick(&mut state);

        // 9 × 0.2 + 2 (merchant) = 1.8 + 2 = 3.8
        // No penalty since this is home node
        let node = &state.trade_nodes[&TradeNodeId(0)];
        assert_eq!(node.country_power["SWE"], Fixed::from_f32(3.8));
    }

    #[test]
    fn test_collection_penalty_outside_home() {
        let mut state = setup_trade_power_state();

        // Change home node to different node
        if let Some(country) = state.countries.get_mut("SWE") {
            country.trade.home_node = Some(TradeNodeId(99)); // Different node
        }

        // Add collecting merchant
        if let Some(node) = state.trade_nodes.get_mut(&TradeNodeId(0)) {
            node.merchants.push(MerchantState {
                owner: "SWE".to_string(),
                action: MerchantAction::Collect,
            });
        }

        run_trade_power_tick(&mut state);

        // (9 × 0.2 + 2) × 0.5 = 3.8 × 0.5 = 1.9
        let node = &state.trade_nodes[&TradeNodeId(0)];
        assert_eq!(node.country_power["SWE"], Fixed::from_f32(1.9));
    }

    #[test]
    fn test_steering_no_penalty() {
        let mut state = setup_trade_power_state();

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

        run_trade_power_tick(&mut state);

        // 9 × 0.2 + 2 = 3.8 (no penalty for steering)
        let node = &state.trade_nodes[&TradeNodeId(0)];
        assert_eq!(node.country_power["SWE"], Fixed::from_f32(3.8));
    }

    #[test]
    fn test_multiple_countries() {
        let mut state = setup_trade_power_state();

        // Add second country
        state
            .countries
            .insert("DAN".to_string(), CountryState::default());

        // Add province for DAN
        state.province_trade_node.insert(2, TradeNodeId(0));
        state.provinces.insert(
            2,
            ProvinceState {
                owner: Some("DAN".to_string()),
                base_tax: Mod32::from_int(5),
                base_production: Mod32::from_int(5),
                base_manpower: Mod32::from_int(5),
                trade: ProvinceTradeState::default(),
                ..Default::default()
            },
        );

        run_trade_power_tick(&mut state);

        let node = &state.trade_nodes[&TradeNodeId(0)];

        // SWE: 9 × 0.2 = 1.8
        assert_eq!(node.country_power["SWE"], Fixed::from_f32(1.8));
        // DAN: 15 × 0.2 = 3.0
        assert_eq!(node.country_power["DAN"], Fixed::from_f32(3.0));
        // Total: 4.8
        assert_eq!(node.total_power, Fixed::from_f32(4.8));
    }

    #[test]
    fn test_power_resets_each_tick() {
        let mut state = setup_trade_power_state();

        // First tick
        run_trade_power_tick(&mut state);
        let first_power = state.trade_nodes[&TradeNodeId(0)].country_power["SWE"];

        // Second tick should reset and recalculate
        run_trade_power_tick(&mut state);
        let second_power = state.trade_nodes[&TradeNodeId(0)].country_power["SWE"];

        assert_eq!(first_power, second_power);
    }

    #[test]
    fn test_unowned_province_no_power() {
        let mut state = setup_trade_power_state();

        // Remove province owner
        if let Some(prov) = state.provinces.get_mut(&1) {
            prov.owner = None;
        }

        run_trade_power_tick(&mut state);

        let node = &state.trade_nodes[&TradeNodeId(0)];
        assert!(!node.country_power.contains_key("SWE"));
        assert_eq!(node.total_power, Fixed::ZERO);
    }

    #[test]
    fn test_empty_topology_no_panic() {
        let mut state = WorldState::default();
        run_trade_power_tick(&mut state);
    }
}
