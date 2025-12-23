# Trade System Design

*Last updated: 2025-12-22*

## Overview

The EU4 trade system models the flow of wealth through a network of trade nodes. Provinces generate trade value from production, which accumulates in their trade node. Countries compete for shares of this value through trade power, and can employ merchants to steer value downstream or collect it as income.

This document specifies the eu4rs implementation of the full EU4 trade system.

## Core Concepts

### Trade Nodes

Trade nodes are fixed locations where trade value accumulates and gets distributed. EU4 has ~80 trade nodes forming a **directed acyclic graph (DAG)** with value flowing from source nodes (e.g., Malacca, Canton) toward end nodes (e.g., English Channel, Venice, Genoa).

```
Value Flow Example:
  Malacca → Bengal → Gujarat → Aden → Alexandria → Venice (end)
                  ↘ Coromandel → Cape → Ivory Coast → Caribbean → Seville
```

Key properties:
- **End nodes**: Have no outgoing connections (Venice, Genoa, English Channel). Value can only be collected here.
- **Inland nodes**: Have both incoming and outgoing connections. Value can be steered or collected.
- **Member provinces**: Each province belongs to exactly one trade node.

### Trade Value

Trade value represents the wealth flowing through the trade network.

**Generation**: Each province generates trade value from production:
```
trade_value = goods_produced × goods_price
goods_produced = production_dev × 0.2
```

**Accumulation**: Value flows into the province's trade node:
```
node.local_value = Σ province_trade_value (for all provinces in node)
node.incoming_value = Σ forwarded_value (from upstream nodes)
node.total_value = local_value + incoming_value
```

### Trade Power

Trade power determines a country's share of a node's value.

**Sources of trade power**:

| Source | Formula | Notes |
|--------|---------|-------|
| Provincial | `dev × 0.2` | For each owned province in node |
| Center of Trade | +5 / +10 / +25 | Level 1 / 2 / 3 |
| Merchant (present) | +2 | Base bonus for having a merchant |
| Merchant (steering) | +5 | Additional bonus when steering |
| Light Ships | +0.5 per ship | "Protect Trade" mission |
| Buildings | +2 (Marketplace), +5 (Trade Depot) | Provincial buildings |
| Modifiers | Variable | Ideas, events, advisors |

**Critical penalty - Non-home collection**:
```
If collecting outside home node: -50% trade power in that node
```
This penalty is essential to prevent "collect everywhere" degeneracy and preserve the "steer to home" gameplay loop.

**Power share calculation**:
```
power_share = country_power / total_node_power
```

### Merchants

Each country has a limited pool of merchants (typically 2-5, increased by diplo tech and ideas).

**Merchant actions**:

| Action | Effect |
|--------|--------|
| **Collect** | Collects trade income from node. +10% collection efficiency. |
| **Steer** | Increases value flow toward a chosen downstream node. +5 power. |

**Collection rules**:
- **Home node**: Automatic collection without a merchant
- **Other nodes**: Requires a merchant to collect (with penalty vs home node)
- **End nodes**: Collection only (no steering possible)

### Trade Income

Trade income is the actual ducats a country receives from trade.

**Collection formula**:
```
income = node_value × power_share × efficiency

efficiency:
  - Base: 0.5 (50%)
  - Home node: +0.1 (+10%)
  - Merchant collecting: +0.1 (+10%)
  - Trade efficiency modifier: variable
  - Collection from non-home without merchant: Not allowed
```

**Example**: Venice (end node) with 100 total value, 60% power share, home node:
```
income = 100 × 0.6 × (0.5 + 0.1) = 36 ducats/month
```

### Value Steering

Countries with merchants steering in a node influence where value flows downstream.

**Steering mechanics**:
1. Each merchant steering toward a destination adds weight to that specific edge only
2. Base weight is 1.0 per connection, +1.0 per merchant steering toward that edge
3. Value distributed proportionally by weight
4. **Steering magnifies value**: Each steering merchant adds +5% to forwarded value (rewards long trade chains)

**Value magnification formula**:
```
merchant_count = count of all steering merchants in node
multiplier = 1.0 + (0.05 × merchant_count)
forwarded_with_boost = forwarded × multiplier
```

**Example**: Node A has two outgoing (→B, →C), with 100 forwarded value:
- 1 merchant steers toward B (specifically toward B, not C)
- No merchants steer toward C
- Magnification: 100 × 1.05 = 105 total forwarded
- Weights: B=2.0, C=1.0, total=3.0
- B receives 70.0, C receives 35.0

## Design Decisions

### D1: Production vs Trade Value (Option A)

**Decision**: Trade value is generated from production *instead of* production income.

**Rationale**: This matches EU4's actual model where production feeds trade, not treasury directly. The simplified approach (production + trade separately) would double-count wealth.

**Implementation**: The production system generates `trade_value` that accumulates in nodes. Production *income* is removed; all province income flows through trade.

**Implication**: Countries without trade presence (landlocked with no nodes) still get income through:
1. Their provinces are in *some* trade node (all provinces are)
2. They have power in that node (from owning provinces)
3. Their home node collects automatically

### D2: Simplified Steering

**Decision**: Steering adds +1.0 weight per merchant toward target. No complex transfer mechanics.

**Rationale**: EU4's steering is more complex (transfer bonus, efficiency, etc.). The simplified model captures the essential "guide value toward your home node" strategy.

### D3: Fixed Trade Node Graph

**Decision**: Trade node connections are static (loaded from game data). No dynamic node creation.

**Rationale**: EU4's trade node network is historically designed and fixed. Node creation/destruction is not a game mechanic.

### D4: No Privateers (Initially)

**Decision**: Defer privateering to a later phase.

**Rationale**: Privateers add interesting strategic depth but are not core to the trade loop. The base system (value, power, merchants, collection) is sufficient for meaningful trade decisions.

**Stub for extension**: `TradeNodeState` will include `privateer_power: HashMap<Tag, Fixed>` (default empty). Power calculation will check this map and apply privateer effects when populated. Adding privateers later just means:
1. Adding `SendPrivateer` command
2. Populating the map when privateers are assigned
3. The existing power calculation already handles the data

### D5: No Trade Embargoes (Initially)

**Decision**: Defer embargoes to a later phase.

**Rationale**: Embargoes modify power (-50% to target in nodes where you have presence). Important for diplomatic/trade warfare but not core to the flow mechanics.

**Stub for extension**: `CountryState` will include `embargoed_by: Vec<Tag>` (default empty). Power calculation will check: `if embargoed_by.contains(rival) { power *= 0.5 }`. Adding embargoes later just means:
1. Adding `Embargo` / `LiftEmbargo` commands
2. Populating the vec when embargoes are declared
3. The existing power calculation already handles the data

### D6: No Upstream Power Propagation (Initially)

**Decision**: Defer upstream power propagation to a later phase.

**Rationale**: EU4 propagates ~20% of provincial power upstream. This allows downstream powers (e.g., Britain in English Channel) to have influence in upstream nodes (Ivory Coast) without ships. This adds recursive complexity requiring an iterative solver. The core system works without it; downstream collection still functions.

**Stub for extension**: `TradeNodeState` will include `upstream_power: HashMap<Tag, Fixed>` (default empty). Power calculation sums `provincial_power + upstream_power`. Adding upstream propagation later just means:
1. Running an iterative solver before the main power calculation
2. Populating `upstream_power` with propagated values
3. The existing power summation already handles the data

### D7: Trade Range for Merchants

**Decision**: Implement trade range check for merchant placement.

**Rationale**: Countries cannot send merchants to nodes they have no connection to. A country needs provinces in the node, or diplomatic trade range (from diplo tech). This prevents AI from sending merchants to unreachable nodes.

## Implementation Phases

### Phase 1: Foundation (Types + Data)

**New types** (`eu4sim-core/src/trade.rs`):

```rust
/// Type-safe trade node identifier
#[derive(Hash, Eq, PartialEq, Clone, Copy, Debug, Serialize, Deserialize)]
pub struct TradeNodeId(pub u16);

/// Static trade node definition (from game data)
pub struct TradeNodeDef {
    pub id: TradeNodeId,
    pub name: String,
    pub outgoing: Vec<TradeNodeId>,  // Downstream connections
    pub provinces: Vec<ProvinceId>,   // Member provinces
    pub is_end_node: bool,            // No outgoing = end node
}

/// Runtime state for a trade node
#[derive(Default)]
pub struct TradeNodeState {
    pub local_value: Fixed,           // From member province production
    pub incoming_value: Fixed,        // From upstream nodes
    pub total_value: Fixed,           // local + incoming
    pub country_power: HashMap<Tag, Fixed>,
    pub total_power: Fixed,
    pub merchants: Vec<MerchantState>,
    // Stubs for future extensions (default empty)
    pub privateer_power: HashMap<Tag, Fixed>,  // D4: Privateers
    pub upstream_power: HashMap<Tag, Fixed>,   // D6: Upstream propagation
}

/// A merchant assignment
pub struct MerchantState {
    pub owner: Tag,
    pub action: MerchantAction,
}

#[derive(Clone, Copy)]
pub enum MerchantAction {
    Collect,
    Steer { target: TradeNodeId },
}
```

**State additions** (`state.rs`):

```rust
pub struct WorldState {
    // ... existing ...
    pub trade_nodes: HashMap<TradeNodeId, TradeNodeState>,
    pub province_trade_node: HashMap<ProvinceId, TradeNodeId>,  // Static after init
}

pub struct CountryState {
    // ... existing ...
    pub merchants_available: u8,
    pub max_merchants: u8,
    pub home_trade_node: Option<TradeNodeId>,
    // Stub for future extensions (default empty)
    pub embargoed_by: Vec<Tag>,  // D5: Embargoes
}

pub struct ProvinceState {
    // ... existing ...
    pub center_of_trade: u8,  // 0-3
}
```

**Data loading** (`eu4data/src/tradenodes.rs`):
- Parse `common/tradenodes/*.txt`
- Build node → provinces mapping
- Build province → node mapping
- Identify end nodes
- **Cycle detection**: Run Tarjan's algorithm on load. Panic if cycles found (indicates corrupt mod data)
- **Cache topological order**: Compute once, store as `Vec<TradeNodeId>` for monthly tick reuse
- Return `HashMap<TradeNodeId, TradeNodeDef>` + `Vec<TradeNodeId>` (topo order)

### Phase 2: Trade Value System

**New system** (`systems/trade_value.rs`):

```rust
/// Monthly trade value calculation.
/// 1. Reset node values
/// 2. Calculate local value from province production
/// 3. Propagate through network (topological sort)
pub fn run_trade_value_tick(state: &mut WorldState);
```

**Algorithm** (topological propagation):

```rust
fn propagate_trade_value(state: &mut WorldState, node_defs: &[TradeNodeDef]) {
    // Process nodes in topological order (upstream → downstream)
    let order = topological_sort(node_defs);

    for node_id in order {
        let node = &mut state.trade_nodes[&node_id];
        node.total_value = node.local_value + node.incoming_value;

        // Determine retained vs forwarded (based on collector presence)
        let retained = calculate_retained_value(node, state);
        let forwarded = node.total_value - retained;

        // Distribute forwarded to downstream
        distribute_downstream(node_id, forwarded, state, node_defs);
    }
}
```

### Phase 3: Trade Power System

**New system** (`systems/trade_power.rs`):

```rust
/// Recalculate trade power for all nodes.
/// Must run before value distribution.
pub fn run_trade_power_tick(state: &mut WorldState);
```

**Power calculation** per country per node:

```rust
fn calculate_country_power(
    tag: &Tag,
    node_id: TradeNodeId,
    state: &WorldState,
) -> Fixed {
    let mut power = Fixed::ZERO;
    let country = &state.countries[tag];
    let is_home = country.home_trade_node == Some(node_id);

    // Provincial power
    for &prov_id in &state.trade_node_provinces[&node_id] {
        if state.provinces[&prov_id].owner == Some(*tag) {
            let dev = state.provinces[&prov_id].total_dev();
            power += dev * Fixed::from_f32(0.2);

            // Center of trade bonus
            let cot = state.provinces[&prov_id].center_of_trade;
            power += match cot {
                1 => Fixed::from_int(5),
                2 => Fixed::from_int(10),
                3 => Fixed::from_int(25),
                _ => Fixed::ZERO,
            };
        }
    }

    // Merchant bonus
    for merchant in &state.trade_nodes[&node_id].merchants {
        if merchant.owner == *tag {
            power += Fixed::from_int(2); // Base merchant bonus
            if matches!(merchant.action, MerchantAction::Steer { .. }) {
                power += Fixed::from_int(5); // Steering bonus
            }
        }
    }

    // Upstream power propagation (stub - empty by default)
    if let Some(upstream) = state.trade_nodes[&node_id].upstream_power.get(tag) {
        power += *upstream;
    }

    // CRITICAL: -50% penalty for collecting outside home node
    let is_collecting = state.trade_nodes[&node_id].merchants.iter()
        .any(|m| m.owner == *tag && matches!(m.action, MerchantAction::Collect));
    if is_collecting && !is_home {
        power *= Fixed::from_f32(0.5);
    }

    // TODO: Light ships, buildings, modifiers, embargoes

    power
}
```

### Phase 4: Merchants and Collection

**New commands** (`input.rs`):

```rust
pub enum Command {
    // ... existing ...

    /// Assign a merchant to a trade node
    SendMerchant {
        node: TradeNodeId,
        action: MerchantAction,
    },

    /// Recall a merchant from a node
    RecallMerchant {
        node: TradeNodeId,
    },

    /// Upgrade center of trade (costs gold)
    UpgradeCenterOfTrade {
        province: ProvinceId,
    },
}
```

**Collection system** (`systems/trade_income.rs`):

```rust
/// Apply trade income to all countries.
pub fn run_trade_income_tick(state: &mut WorldState) {
    for (tag, country) in &mut state.countries {
        let income = calculate_trade_income(tag, state);
        country.treasury += income;
    }
}

fn calculate_trade_income(tag: &Tag, state: &WorldState) -> Fixed {
    let mut total = Fixed::ZERO;

    for (node_id, node) in &state.trade_nodes {
        let power_share = node.country_power.get(tag)
            .copied()
            .unwrap_or(Fixed::ZERO) / node.total_power;

        if power_share == Fixed::ZERO {
            continue;
        }

        let is_home = state.countries[tag].home_trade_node == Some(*node_id);
        let has_merchant = node.merchants.iter().any(|m|
            m.owner == *tag && matches!(m.action, MerchantAction::Collect)
        );

        // Can only collect from home node or with merchant
        if !is_home && !has_merchant {
            continue;
        }

        let mut efficiency = Fixed::from_f32(0.5);  // Base 50%
        if is_home { efficiency += Fixed::from_f32(0.1); }
        if has_merchant { efficiency += Fixed::from_f32(0.1); }

        total += node.total_value * power_share * efficiency;
    }

    total
}
```

### Phase 5: Steering Mechanics

**Steering weight calculation** (with value magnification):

```rust
fn distribute_downstream(
    node_id: TradeNodeId,
    forwarded: Fixed,
    state: &mut WorldState,
    node_defs: &[TradeNodeDef],
) {
    let node_def = &node_defs[node_id.0 as usize];
    let node = &state.trade_nodes[&node_id];

    if node_def.outgoing.is_empty() {
        return; // End node
    }

    // Count steering merchants for value magnification
    let steering_count = node.merchants.iter()
        .filter(|m| matches!(m.action, MerchantAction::Steer { .. }))
        .count() as i32;

    // Value magnification: +5% per steering merchant
    let multiplier = Fixed::ONE + (Fixed::from_f32(0.05) * Fixed::from_int(steering_count));
    let forwarded_boosted = forwarded * multiplier;

    // Calculate weights per downstream (steering only affects specific edge)
    let mut weights: HashMap<TradeNodeId, Fixed> = HashMap::new();
    for &downstream_id in &node_def.outgoing {
        let base = Fixed::from_int(1);
        // Only count merchants steering to THIS specific downstream
        let steering_bonus: Fixed = node.merchants.iter()
            .filter(|m| matches!(m.action, MerchantAction::Steer { target } if target == downstream_id))
            .map(|_| Fixed::from_int(1))
            .sum();
        weights.insert(downstream_id, base + steering_bonus);
    }

    let total_weight: Fixed = weights.values().copied().sum();

    // Distribute boosted value
    for (downstream_id, weight) in weights {
        let share = forwarded_boosted * weight / total_weight;
        state.trade_nodes.get_mut(&downstream_id).unwrap().incoming_value += share;
    }
}
```

### Phase 6: AI Integration

**Available commands** (`step.rs`):

```rust
// In available_commands():
if country.merchants_available > 0 {
    for (node_id, node) in &state.trade_nodes {
        // Check if we already have a merchant here
        let has_merchant = node.merchants.iter().any(|m| m.owner == country_tag);
        if has_merchant {
            continue;
        }

        // Can send merchant if:
        // 1. We have power in the node (own provinces there), OR
        // 2. Node is within trade range (adjacent to a node where we have power)
        // This allows expansion into new markets (the "Expansion Paradox" fix)
        let has_power = node.country_power.get(&country_tag)
            .copied().unwrap_or(Fixed::ZERO) > Fixed::ZERO;
        let in_trade_range = is_node_in_trade_range(*node_id, country_tag, state);

        if has_power || in_trade_range {
            // Can send to collect
            available.push(Command::SendMerchant {
                node: *node_id,
                action: MerchantAction::Collect,
            });

            // Can steer to any downstream (if not end node)
            for &downstream in &node_def.outgoing {
                available.push(Command::SendMerchant {
                    node: *node_id,
                    action: MerchantAction::Steer { target: downstream },
                });
            }
        }
    }
}

// Recall existing merchants
for (node_id, node) in &state.trade_nodes {
    if node.merchants.iter().any(|m| m.owner == country_tag) {
        available.push(Command::RecallMerchant { node: *node_id });
    }
}

/// Check if a node is within trade range for a country.
/// MVP: Node is reachable if it's adjacent to any node where we have power.
fn is_node_in_trade_range(node_id: TradeNodeId, tag: &Tag, state: &WorldState) -> bool {
    // Get all nodes where we have power
    let powered_nodes: Vec<TradeNodeId> = state.trade_nodes.iter()
        .filter(|(_, n)| n.country_power.get(tag).copied().unwrap_or(Fixed::ZERO) > Fixed::ZERO)
        .map(|(id, _)| *id)
        .collect();

    // Check if target node is adjacent (upstream or downstream) to any powered node
    for powered_id in powered_nodes {
        let powered_def = &node_defs[powered_id.0 as usize];
        // Downstream from powered node
        if powered_def.outgoing.contains(&node_id) {
            return true;
        }
        // Upstream from powered node (check if target flows into powered)
        let target_def = &node_defs[node_id.0 as usize];
        if target_def.outgoing.contains(&powered_id) {
            return true;
        }
    }
    false
}
```

**Greedy AI scoring** (`ai/greedy.rs`):

```rust
Command::SendMerchant { node, action } => {
    let node_value = state.trade_nodes[node].total_value;
    let power_share = /* calculate */;
    let potential_income = node_value * power_share;

    match action {
        MerchantAction::Collect => {
            // Directly increases income
            (potential_income * Fixed::from_f32(0.1)).to_int() as i32
        }
        MerchantAction::Steer { target } => {
            // Value if steering toward home node chain
            if on_path_to_home(*target, country.home_trade_node, node_defs) {
                (potential_income * Fixed::from_f32(0.15)).to_int() as i32
            } else {
                (potential_income * Fixed::from_f32(0.05)).to_int() as i32
            }
        }
    }
}
```

## Monthly Tick Integration

Trade systems run monthly, after production:

```rust
// In step_world(), monthly tick:
if new_state.date.day == 1 {
    // ... existing ticks ...

    // Trade system (order matters)
    crate::systems::trade_power::run_trade_power_tick(&mut new_state);
    crate::systems::trade_value::run_trade_value_tick(&mut new_state);
    crate::systems::trade_income::run_trade_income_tick(&mut new_state);
}
```

## Testing Strategy

### Unit Tests

```rust
#[test]
fn test_trade_value_generation() {
    // Province with 10 production dev, grain (price 2.5)
    // Expected: 10 * 0.2 * 2.5 = 5.0 trade value
}

#[test]
fn test_trade_power_from_dev() {
    // Province with 15 total dev
    // Expected: 15 * 0.2 = 3.0 power
}

#[test]
fn test_non_home_collection_penalty() {
    // Country collecting outside home node
    // Expected: 50% reduction in trade power
}

#[test]
fn test_collection_efficiency() {
    // Home node with merchant
    // Expected: 50% + 10% + 10% = 70% efficiency
}

#[test]
fn test_steering_weight_distribution() {
    // Node with 2 outgoing, 1 merchant steering to first
    // Expected: 66.7% to first, 33.3% to second (after magnification)
}

#[test]
fn test_steering_value_magnification() {
    // 2 merchants steering in node, 100 forwarded value
    // Expected: 100 * 1.10 = 110 total forwarded
}
```

### Property Tests

```rust
proptest! {
    #[test]
    fn prop_value_magnification_bounded(merchants in 0..20) {
        // Steering magnification is bounded: 1.0 + 0.05 * merchants
        // Max reasonable: 1.0 + 0.05 * 20 = 2.0 (100% increase)
    }

    #[test]
    fn prop_collection_bounded(power_shares in ...) {
        // Total collected <= total node value
    }

    #[test]
    fn prop_power_positive(devs in ...) {
        // Trade power is always non-negative
    }

    #[test]
    fn prop_power_penalty_applied(is_home in bool, is_collecting in bool) {
        // -50% penalty only when collecting AND not home
    }
}
```

### Integration Tests

```rust
#[test]
fn test_full_trade_cycle() {
    // Setup: 3-node chain (A → B → C end)
    // Venice controls C, Turkey controls B, generic controls A
    // Run monthly tick
    // Assert: Venice collects most (end node advantage)
}
```

## Future Extensions

| Feature | Description | Priority |
|---------|-------------|----------|
| Privateers | Raid trade for hostile income reduction | Medium |
| Embargoes | -50% power to target country | Medium |
| Trade Conflicts | War over trade power | Low |
| Trade Companies | Special colonial regions | Low |
| Trade Leagues | Merchant republic mechanics | Low |

## Files Summary

| File | Action | Size |
|------|--------|------|
| `docs/design/simulation/trade-system.md` | NEW | This doc |
| `eu4sim-core/src/trade.rs` | NEW | Types |
| `eu4data/src/tradenodes.rs` | NEW/MODIFY | Data loading |
| `eu4sim-core/src/state.rs` | MODIFY | State additions |
| `eu4sim-core/src/systems/trade_power.rs` | NEW | Power calc |
| `eu4sim-core/src/systems/trade_value.rs` | NEW | Value propagation |
| `eu4sim-core/src/systems/trade_income.rs` | NEW | Collection |
| `eu4sim-core/src/input.rs` | MODIFY | Commands |
| `eu4sim-core/src/step.rs` | MODIFY | Tick integration |
| `eu4sim-core/src/ai/greedy.rs` | MODIFY | AI scoring |
