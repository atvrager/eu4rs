//! Trade system types and state for EU4 simulation.
//!
//! Trade nodes form a directed acyclic graph (DAG) where value flows from
//! production sources (provinces) through the network to collection points.
//! Countries compete for trade power to control value distribution.
//!
//! Key mechanics:
//! - **Trade Value**: Generated from province production, flows downstream
//! - **Trade Power**: Determines share of node value (provincial, ships, merchants)
//! - **Merchants**: Steer value downstream or collect with efficiency bonus
//! - **Collection Penalty**: -50% power when collecting outside home node

use crate::fixed::Fixed;
use crate::state::Tag;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Unique identifier for a trade node.
///
/// Maps to indices in the topologically-sorted node array for efficient iteration.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct TradeNodeId(pub u16);

/// Runtime state for a single trade node during simulation.
///
/// Updated monthly during trade value propagation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TradeNodeState {
    /// Trade value generated locally (from province production in this node).
    pub local_value: Fixed,

    /// Trade value received from upstream nodes (after steering/power distribution).
    pub incoming_value: Fixed,

    /// Total trade value available for collection/steering (local + incoming).
    pub total_value: Fixed,

    /// Trade power per country in this node (provincial + ships + merchants + buildings).
    pub country_power: HashMap<Tag, Fixed>,

    /// Sum of all country power (cached for efficiency).
    pub total_power: Fixed,

    /// Active merchants in this node.
    pub merchants: Vec<MerchantState>,

    // =========================================================================
    // Stubs for future extensions (D4, D5, D6 from design doc)
    // =========================================================================
    /// Privateer power per country (D4: Privateers - deferred).
    /// When implemented: privateers steal value proportional to their power share.
    #[serde(default)]
    pub privateer_power: HashMap<Tag, Fixed>,

    /// Power propagated from downstream nodes (D6: Upstream propagation - deferred).
    /// When implemented: ~20% of downstream provincial power counts in upstream nodes.
    #[serde(default)]
    pub upstream_power: HashMap<Tag, Fixed>,
}

impl Default for TradeNodeState {
    fn default() -> Self {
        Self {
            local_value: Fixed::ZERO,
            incoming_value: Fixed::ZERO,
            total_value: Fixed::ZERO,
            country_power: HashMap::new(),
            total_power: Fixed::ZERO,
            merchants: Vec::new(),
            privateer_power: HashMap::new(),
            upstream_power: HashMap::new(),
        }
    }
}

/// State of a merchant assigned to a trade node.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MerchantState {
    /// Country that owns this merchant.
    pub owner: Tag,

    /// What the merchant is doing in this node.
    pub action: MerchantAction,
}

/// What a merchant does in a trade node.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum MerchantAction {
    /// Collect trade value with +10% efficiency bonus.
    /// Note: -50% power penalty applies if not in home node.
    Collect,

    /// Steer trade value toward a specific downstream node.
    /// Adds +5 trade power and contributes to value magnification.
    Steer {
        /// Target downstream node to steer value toward.
        target: TradeNodeId,
    },
}

// =============================================================================
// Country-level trade state
// =============================================================================

/// Trade-related state stored in CountryState.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct CountryTradeState {
    /// Number of merchants available to assign.
    pub merchants_available: u8,

    /// Total merchants the country has (from national ideas, buildings, etc.).
    pub merchants_total: u8,

    /// Home trade node (where collection is most efficient).
    pub home_node: Option<TradeNodeId>,

    /// Base trade range (number of hops from owned provinces).
    /// Increases with DIP tech and ideas.
    #[serde(default)]
    pub trade_range: u8,

    /// Countries this nation has embargoed (D5: Embargoes - deferred).
    /// When implemented: reduces embargoed country's power in shared nodes.
    #[serde(default)]
    pub embargoed_by: Vec<Tag>,
}

// =============================================================================
// Province-level trade state
// =============================================================================

/// Trade-related state stored in ProvinceState.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, Default)]
pub struct ProvinceTradeState {
    /// Center of Trade level (0 = none, 1/2/3 = increasing power bonus).
    /// Level 1: +5 power, Level 2: +10 power, Level 3: +25 power.
    pub center_of_trade: u8,

    /// Light ships protecting trade here (each adds +3 power to owner).
    pub protecting_ships: u16,
}

// =============================================================================
// Cached topology (initialized once from game data)
// =============================================================================

/// Pre-computed topological order for trade value propagation.
///
/// Computed once during WorldState initialization using Kahn's algorithm
/// (or Tarjan's if cycle detection is needed). Stored in reverse order
/// so iteration visits source nodes first.
#[derive(Debug, Clone, Default)]
pub struct TradeTopology {
    /// Node IDs in topological order (sources first, sinks last).
    /// Iterate forward for value propagation, backward for collection.
    pub order: Vec<TradeNodeId>,

    /// End nodes (no outgoing edges) - automatic collection points.
    pub end_nodes: Vec<TradeNodeId>,

    /// Outgoing edges: node → downstream nodes it flows to.
    /// Populated from TradeNetwork during initialization.
    pub edges: HashMap<TradeNodeId, Vec<TradeNodeId>>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_trade_node_id_equality() {
        let id1 = TradeNodeId(1);
        let id2 = TradeNodeId(1);
        let id3 = TradeNodeId(2);
        assert_eq!(id1, id2);
        assert_ne!(id1, id3);
    }

    #[test]
    fn test_merchant_action_equality() {
        let collect = MerchantAction::Collect;
        let steer1 = MerchantAction::Steer {
            target: TradeNodeId(5),
        };
        let steer2 = MerchantAction::Steer {
            target: TradeNodeId(5),
        };
        let steer3 = MerchantAction::Steer {
            target: TradeNodeId(6),
        };

        assert_eq!(steer1, steer2);
        assert_ne!(steer1, steer3);
        assert_ne!(collect, steer1);
    }

    #[test]
    fn test_trade_node_state_default() {
        let state = TradeNodeState::default();
        assert_eq!(state.local_value, Fixed::ZERO);
        assert_eq!(state.total_power, Fixed::ZERO);
        assert!(state.merchants.is_empty());
        assert!(state.privateer_power.is_empty());
    }

    #[test]
    fn test_country_trade_state_default() {
        let state = CountryTradeState::default();
        assert_eq!(state.merchants_available, 0);
        assert_eq!(state.home_node, None);
        assert!(state.embargoed_by.is_empty());
    }
}
