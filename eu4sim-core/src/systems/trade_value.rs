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
/// 2. Calculate retained vs forwarded based on power distribution
/// 3. Distribute forwarded value to downstream nodes
fn propagate_trade_value(state: &mut WorldState) {
    // Process in topological order (sources first)
    // We need to iterate by index since we can't borrow mutably while iterating
    let order = state.trade_topology.order.clone();

    for &node_id in &order {
        // Calculate total value
        let (forwarded, downstream_nodes) = {
            let Some(node) = state.trade_nodes.get(&node_id) else {
                continue;
            };

            // Total = local + incoming
            let total = node.local_value + node.incoming_value;

            // For now (Phase 2), we use a simplified split:
            // - If total power is zero, forward everything
            // - Otherwise, we'll implement proper power-based split in Phase 3
            //
            // Temporary: forward 100% of value for testing
            // Real logic in Phase 3 will calculate retention based on power shares
            let retained = Fixed::ZERO;
            let forwarded = total - retained;

            // Get downstream nodes from the topology (we need to look at TradeNetwork)
            // For now, collect downstream from node definition
            // Note: In Phase 1 we stored outgoing in TradeNodeDef, but not in TradeNodeState
            // We'll need to access the static network data
            // For now, return empty - we'll fix this properly

            (forwarded, Vec::new())
        };

        // Update total value
        if let Some(node) = state.trade_nodes.get_mut(&node_id) {
            node.total_value = node.local_value + node.incoming_value;
        }

        // Distribute to downstream (Phase 3 will add proper steering weights)
        if !downstream_nodes.is_empty() && forwarded > Fixed::ZERO {
            let per_downstream = forwarded.div(Fixed::from_int(downstream_nodes.len() as i64));
            for downstream_id in downstream_nodes {
                if let Some(downstream) = state.trade_nodes.get_mut(&downstream_id) {
                    downstream.incoming_value += per_downstream;
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
        state.trade_topology = TradeTopology {
            order: vec![node_a, node_b],
            end_nodes: vec![node_b],
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
}
