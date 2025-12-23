//! Trade value calculation and propagation system.
//!
//! Calculates trade value from province production and propagates it
//! through the trade node network from sources to sinks.
//!
//! # Flow
//! 1. Province production → local trade value
//! 2. Local value aggregates to trade nodes
//! 3. Value propagates downstream (topological order)
//! 4. At each node: retained + forwarded = total
//!
//! # Design Decision D1
//! Production feeds into trade (not additive). Countries collect income
//! from trade nodes, not directly from production.

use crate::fixed::Fixed;
use crate::state::WorldState;

/// Runs the monthly trade value tick.
///
/// Call this on the 1st of each month, BEFORE trade power and collection.
///
/// # What it does
/// 1. Resets all node values to zero
/// 2. Calculates local value from each province's production
/// 3. Aggregates to trade nodes
/// 4. Propagates through network (sources → sinks)
pub fn run_trade_value_tick(state: &mut WorldState) {
    // Skip if trade network isn't initialized
    if state.trade_topology.order.is_empty() {
        return;
    }

    // 1. Reset all node values
    let node_ids: Vec<_> = state.trade_nodes.keys().copied().collect();
    for node_id in node_ids {
        if let Some(node) = state.trade_nodes.get_mut(&node_id) {
            node.local_value = Fixed::ZERO;
            node.incoming_value = Fixed::ZERO;
            node.total_value = Fixed::ZERO;
        }
    }

    // 2. Calculate local value from provinces
    calculate_local_values(state);

    // 3. Propagate through network (sources first, sinks last)
    propagate_trade_value(state);
}

/// Calculate local trade value from each province's production.
///
/// Formula: `trade_value = goods_produced × goods_price`
/// where: `goods_produced = base_production × 0.2`
fn calculate_local_values(state: &mut WorldState) {
    // Base production multiplier (EU4: 0.2)
    let base_mult = Fixed::from_f32(eu4data::defines::economy::BASE_PRODUCTION_MULTIPLIER);

    for (&province_id, province) in state.provinces.iter() {
        // Skip provinces without trade goods
        let Some(goods_id) = province.trade_goods_id else {
            continue;
        };

        // Get trade node for this province
        let Some(&node_id) = state.province_trade_node.get(&province_id) else {
            continue;
        };

        // Calculate goods produced
        let goods_produced = province.base_production.mul(base_mult);

        // Get effective price
        let base_price = state
            .base_goods_prices
            .get(&goods_id)
            .copied()
            .unwrap_or(Fixed::ONE);
        let price = state.modifiers.effective_price(goods_id, base_price);

        // Trade value = goods × price
        let trade_value = goods_produced.mul(price);

        // Accumulate to node's local value
        if let Some(node) = state.trade_nodes.get_mut(&node_id) {
            node.local_value += trade_value;
        }
    }
}

/// Propagate trade value through the network in topological order.
///
/// For each node (sources → sinks):
/// 1. total_value = local_value + incoming_value
/// 2. Calculate retained vs forwarded based on collection power
/// 3. Distribute forwarded value to downstream nodes (with steering magnification)
///
/// Steering mechanics (EU4):
/// - Merchants steering toward a downstream node add +5% value magnification
/// - Value is weighted toward nodes with more steering power
fn propagate_trade_value(state: &mut WorldState) {
    use crate::trade::MerchantAction;

    let order = state.trade_topology.order.clone();
    let edges = state.trade_topology.edges.clone();

    for &node_id in &order {
        // Collect node data (avoiding borrow issues)
        let (total_value, downstream_nodes, merchants, country_power, total_power) = {
            let Some(node) = state.trade_nodes.get(&node_id) else {
                continue;
            };
            let total = node.local_value + node.incoming_value;
            let downstream = edges.get(&node_id).cloned().unwrap_or_default();
            let merchants = node.merchants.clone();
            let country_power = node.country_power.clone();
            let total_power = node.total_power;
            (total, downstream, merchants, country_power, total_power)
        };

        // Update total value
        if let Some(node) = state.trade_nodes.get_mut(&node_id) {
            node.total_value = total_value;
        }

        // Skip if nothing to forward
        if downstream_nodes.is_empty() || total_value == Fixed::ZERO {
            continue;
        }

        // Count steering merchants toward each downstream target
        let mut steering_count: std::collections::HashMap<crate::trade::TradeNodeId, u32> =
            std::collections::HashMap::new();
        for merchant in &merchants {
            if let MerchantAction::Steer { target } = &merchant.action {
                if downstream_nodes.contains(target) {
                    *steering_count.entry(*target).or_insert(0) += 1;
                }
            }
        }

        // Calculate retention: countries collecting at this node retain their share.
        // Collection = home node OR merchant with Collect action.
        // Forwarded value = total × (1 - collection_power / total_power).
        let mut collection_power = Fixed::ZERO;

        for (tag, &power) in &country_power {
            if power <= Fixed::ZERO {
                continue;
            }

            // Check if this country is collecting here
            let is_home = state
                .countries
                .get(tag)
                .and_then(|c| c.trade.home_node)
                .map(|h| h == node_id)
                .unwrap_or(false);

            let has_collect_merchant = merchants
                .iter()
                .any(|m| m.owner == *tag && matches!(m.action, MerchantAction::Collect));

            if is_home || has_collect_merchant {
                collection_power += power;
            }
        }

        // Forwarded = total × (1 - retention_ratio)
        // If 100% power is collecting (e.g., end node), nothing is forwarded.
        let forwarded = if total_power > Fixed::ZERO {
            let retention_ratio = collection_power.div(total_power);
            total_value.mul(Fixed::ONE - retention_ratio)
        } else {
            // No power at this node, forward everything
            total_value
        };

        // Apply steering magnification: +5% per steering merchant
        // Total magnified = forwarded × (1 + 0.05 × total_steering_merchants)
        let total_steering: u32 = steering_count.values().sum();
        let magnification = Fixed::ONE + Fixed::from_f32(0.05 * total_steering as f32);
        let magnified_value = forwarded.mul(magnification);

        // Distribute to downstream nodes
        // Weight by steering: nodes with steering get proportionally more
        if total_steering > 0 {
            // Weighted distribution based on steering
            let base_weight = 1u32; // Each node gets at least 1 weight
            let total_weight: u32 = downstream_nodes
                .iter()
                .map(|id| base_weight + steering_count.get(id).copied().unwrap_or(0))
                .sum();

            for target_id in &downstream_nodes {
                let weight = base_weight + steering_count.get(target_id).copied().unwrap_or(0);
                let share = magnified_value
                    .mul(Fixed::from_int(weight as i64))
                    .div(Fixed::from_int(total_weight as i64));

                if let Some(target_node) = state.trade_nodes.get_mut(target_id) {
                    target_node.incoming_value += share;
                }
            }
        } else {
            // Equal distribution when no steering
            let per_downstream =
                magnified_value.div(Fixed::from_int(downstream_nodes.len() as i64));
            for target_id in &downstream_nodes {
                if let Some(target_node) = state.trade_nodes.get_mut(target_id) {
                    target_node.incoming_value += per_downstream;
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::modifiers::TradegoodId;
    use crate::state::{CountryState, ProvinceState};
    use crate::trade::{ProvinceTradeState, TradeNodeId, TradeNodeState, TradeTopology};

    fn setup_simple_trade_state() -> WorldState {
        let mut state = WorldState::default();

        // Create two trade nodes: A (source) → B (sink)
        let node_a = TradeNodeId(0);
        let node_b = TradeNodeId(1);

        state.trade_nodes.insert(node_a, TradeNodeState::default());
        state.trade_nodes.insert(node_b, TradeNodeState::default());

        // Topological order: A before B
        // Edge: A → B
        let mut edges = std::collections::HashMap::new();
        edges.insert(node_a, vec![node_b]);

        state.trade_topology = TradeTopology {
            order: vec![node_a, node_b],
            end_nodes: vec![node_b],
            edges,
        };

        // Map province 1 to node A
        state.province_trade_node.insert(1, node_a);

        // Add country
        state
            .countries
            .insert("SWE".to_string(), CountryState::default());

        // Add province with grain (id=0), base_production=5
        state.provinces.insert(
            1,
            ProvinceState {
                owner: Some("SWE".to_string()),
                trade_goods_id: Some(TradegoodId(0)),
                base_production: Fixed::from_int(5),
                trade: ProvinceTradeState::default(),
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
    fn test_local_value_calculation() {
        let mut state = setup_simple_trade_state();

        run_trade_value_tick(&mut state);

        // 5 × 0.2 × 2.5 = 2.5 trade value
        let node_a = &state.trade_nodes[&TradeNodeId(0)];
        assert_eq!(node_a.local_value, Fixed::from_f32(2.5));
        assert_eq!(node_a.total_value, Fixed::from_f32(2.5));
    }

    #[test]
    fn test_no_trade_goods_no_value() {
        let mut state = setup_simple_trade_state();

        // Remove trade goods from province
        if let Some(prov) = state.provinces.get_mut(&1) {
            prov.trade_goods_id = None;
        }

        run_trade_value_tick(&mut state);

        let node_a = &state.trade_nodes[&TradeNodeId(0)];
        assert_eq!(node_a.local_value, Fixed::ZERO);
    }

    #[test]
    fn test_unmapped_province_no_value() {
        let mut state = setup_simple_trade_state();

        // Remove province from trade node mapping
        state.province_trade_node.remove(&1);

        run_trade_value_tick(&mut state);

        // Node A should have no local value
        let node_a = &state.trade_nodes[&TradeNodeId(0)];
        assert_eq!(node_a.local_value, Fixed::ZERO);
    }

    #[test]
    fn test_empty_topology_no_panic() {
        let mut state = WorldState::default();

        // Should not panic with empty topology
        run_trade_value_tick(&mut state);
    }

    #[test]
    fn test_multiple_provinces_aggregate() {
        let mut state = setup_simple_trade_state();

        // Add second province to same node
        state.province_trade_node.insert(2, TradeNodeId(0));
        state.provinces.insert(
            2,
            ProvinceState {
                owner: Some("SWE".to_string()),
                trade_goods_id: Some(TradegoodId(0)), // grain
                base_production: Fixed::from_int(10),
                trade: ProvinceTradeState::default(),
                ..Default::default()
            },
        );

        run_trade_value_tick(&mut state);

        // Province 1: 5 × 0.2 × 2.5 = 2.5
        // Province 2: 10 × 0.2 × 2.5 = 5.0
        // Total: 7.5
        let node_a = &state.trade_nodes[&TradeNodeId(0)];
        assert_eq!(node_a.local_value, Fixed::from_f32(7.5));
    }

    #[test]
    fn test_value_resets_each_tick() {
        let mut state = setup_simple_trade_state();

        // First tick
        run_trade_value_tick(&mut state);
        let first_value = state.trade_nodes[&TradeNodeId(0)].local_value;

        // Second tick should reset and recalculate
        run_trade_value_tick(&mut state);
        let second_value = state.trade_nodes[&TradeNodeId(0)].local_value;

        assert_eq!(first_value, second_value);
    }

    #[test]
    fn test_price_modifiers_affect_value() {
        let mut state = setup_simple_trade_state();

        // Add price modifier (+0.5 additive to grain)
        state
            .modifiers
            .goods_price_mods
            .insert(TradegoodId(0), Fixed::from_f32(0.5));

        run_trade_value_tick(&mut state);

        // Base: 5 × 0.2 × 2.5 = 2.5
        // With +0.5 price: 5 × 0.2 × 3.0 = 3.0
        // Note: goods_price_mods is additive (base + modifier)
        let node_a = &state.trade_nodes[&TradeNodeId(0)];
        assert_eq!(node_a.local_value, Fixed::from_f32(3.0));
    }

    #[test]
    fn test_value_propagates_downstream() {
        let mut state = setup_simple_trade_state();

        run_trade_value_tick(&mut state);

        // Node A has local value 2.5, which flows to node B
        let node_b = &state.trade_nodes[&TradeNodeId(1)];
        assert_eq!(node_b.incoming_value, Fixed::from_f32(2.5));
        assert_eq!(node_b.total_value, Fixed::from_f32(2.5));
    }

    #[test]
    fn test_steering_magnification() {
        use crate::trade::{MerchantAction, MerchantState};

        let mut state = setup_simple_trade_state();

        // Add a merchant steering toward node B
        if let Some(node_a) = state.trade_nodes.get_mut(&TradeNodeId(0)) {
            node_a.merchants.push(MerchantState {
                owner: "SWE".to_string(),
                action: MerchantAction::Steer {
                    target: TradeNodeId(1),
                },
            });
        }

        run_trade_value_tick(&mut state);

        // Local value 2.5, magnified by +5% = 2.5 × 1.05 = 2.625
        let node_b = &state.trade_nodes[&TradeNodeId(1)];
        assert_eq!(node_b.incoming_value, Fixed::from_f32(2.625));
    }

    #[test]
    fn test_steering_weighted_distribution() {
        use crate::trade::{MerchantAction, MerchantState};

        let mut state = WorldState::default();

        // Create three nodes: A → B, A → C (two downstream options)
        let node_a = TradeNodeId(0);
        let node_b = TradeNodeId(1);
        let node_c = TradeNodeId(2);

        state.trade_nodes.insert(node_a, TradeNodeState::default());
        state.trade_nodes.insert(node_b, TradeNodeState::default());
        state.trade_nodes.insert(node_c, TradeNodeState::default());

        // A has edges to both B and C
        let mut edges = std::collections::HashMap::new();
        edges.insert(node_a, vec![node_b, node_c]);

        state.trade_topology = TradeTopology {
            order: vec![node_a, node_b, node_c],
            end_nodes: vec![node_b, node_c],
            edges,
        };

        // Set local value directly for testing
        if let Some(node) = state.trade_nodes.get_mut(&node_a) {
            node.local_value = Fixed::from_int(100);
            // Add 2 merchants steering toward B, none toward C
            node.merchants.push(MerchantState {
                owner: "SWE".to_string(),
                action: MerchantAction::Steer { target: node_b },
            });
            node.merchants.push(MerchantState {
                owner: "FRA".to_string(),
                action: MerchantAction::Steer { target: node_b },
            });
        }

        // Simulate propagation phase only (skip local value calculation)
        propagate_trade_value(&mut state);

        // 2 steering merchants = +10% magnification = 100 × 1.10 = 110
        // Weights: B gets 1+2=3, C gets 1+0=1, total=4
        // B's share: 110 × 3/4 = 82.5
        // C's share: 110 × 1/4 = 27.5
        let node_b_state = &state.trade_nodes[&node_b];
        let node_c_state = &state.trade_nodes[&node_c];

        assert_eq!(node_b_state.incoming_value, Fixed::from_f32(82.5));
        assert_eq!(node_c_state.incoming_value, Fixed::from_f32(27.5));
    }

    #[test]
    fn test_no_steering_equal_distribution() {
        let mut state = WorldState::default();

        // Create three nodes: A → B, A → C
        let node_a = TradeNodeId(0);
        let node_b = TradeNodeId(1);
        let node_c = TradeNodeId(2);

        state.trade_nodes.insert(node_a, TradeNodeState::default());
        state.trade_nodes.insert(node_b, TradeNodeState::default());
        state.trade_nodes.insert(node_c, TradeNodeState::default());

        let mut edges = std::collections::HashMap::new();
        edges.insert(node_a, vec![node_b, node_c]);

        state.trade_topology = TradeTopology {
            order: vec![node_a, node_b, node_c],
            end_nodes: vec![node_b, node_c],
            edges,
        };

        // Set local value directly, no merchants
        if let Some(node) = state.trade_nodes.get_mut(&node_a) {
            node.local_value = Fixed::from_int(100);
        }

        propagate_trade_value(&mut state);

        // No steering = equal distribution: 100 / 2 = 50 each
        let node_b_state = &state.trade_nodes[&node_b];
        let node_c_state = &state.trade_nodes[&node_c];

        assert_eq!(node_b_state.incoming_value, Fixed::from_int(50));
        assert_eq!(node_c_state.incoming_value, Fixed::from_int(50));
    }

    #[test]
    fn test_collection_power_reduces_forwarding() {
        use crate::trade::CountryTradeState;

        let mut state = WorldState::default();

        // Create two nodes: A → B
        let node_a = TradeNodeId(0);
        let node_b = TradeNodeId(1);

        // SWE has 50% of power, FRA has 50%
        let mut country_power = std::collections::HashMap::new();
        country_power.insert("SWE".to_string(), Fixed::from_int(50));
        country_power.insert("FRA".to_string(), Fixed::from_int(50));

        let node_a_state = TradeNodeState {
            local_value: Fixed::from_int(100),
            country_power,
            total_power: Fixed::from_int(100),
            ..Default::default()
        };

        state.trade_nodes.insert(node_a, node_a_state);
        state.trade_nodes.insert(node_b, TradeNodeState::default());

        let mut edges = std::collections::HashMap::new();
        edges.insert(node_a, vec![node_b]);

        state.trade_topology = TradeTopology {
            order: vec![node_a, node_b],
            end_nodes: vec![node_b],
            edges,
        };

        // SWE's home is node A (collecting 50%), FRA's home is elsewhere (not collecting)
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
        state.countries.insert(
            "FRA".to_string(),
            CountryState {
                trade: CountryTradeState {
                    home_node: Some(node_b), // FRA collects at B, not A
                    ..Default::default()
                },
                ..Default::default()
            },
        );

        propagate_trade_value(&mut state);

        // SWE collects 50% at node A → retention = 50%
        // Forwarded = 100 × (1 - 0.5) = 50
        let node_b_state = &state.trade_nodes[&node_b];
        assert_eq!(node_b_state.incoming_value, Fixed::from_int(50));
    }

    #[test]
    fn test_full_collection_no_forwarding() {
        use crate::trade::CountryTradeState;

        let mut state = WorldState::default();

        // Create two nodes: A → B
        let node_a = TradeNodeId(0);
        let node_b = TradeNodeId(1);

        // Only SWE has power, and they're collecting
        let mut country_power = std::collections::HashMap::new();
        country_power.insert("SWE".to_string(), Fixed::from_int(100));

        let node_a_state = TradeNodeState {
            local_value: Fixed::from_int(100),
            country_power,
            total_power: Fixed::from_int(100),
            ..Default::default()
        };

        state.trade_nodes.insert(node_a, node_a_state);
        state.trade_nodes.insert(node_b, TradeNodeState::default());

        let mut edges = std::collections::HashMap::new();
        edges.insert(node_a, vec![node_b]);

        state.trade_topology = TradeTopology {
            order: vec![node_a, node_b],
            end_nodes: vec![node_b],
            edges,
        };

        // SWE's home is node A (collecting 100%)
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

        propagate_trade_value(&mut state);

        // SWE collects 100% at node A → nothing forwarded
        let node_b_state = &state.trade_nodes[&node_b];
        assert_eq!(node_b_state.incoming_value, Fixed::ZERO);
    }
}
