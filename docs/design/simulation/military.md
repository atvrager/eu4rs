# Military System Design

The military system handles armies, fleets, movement, combat, and recruitment. It is designed to be deterministic, performant, and correctly handle EU4's complex movement rules.

## Architecture

The military logic is primarily contained in `eu4sim-core/src/systems/military.rs` and `movement.rs`, moving units across the `WorldState`.

### Core Components

1.  **Units**:
    *   **Regiments/Ships**: The smallest individual units (1k men or 1 ship).
    *   **Armies/Fleets**: Collections of regiments/ships that move together.
    *   **Leaders**: Generals/Admirals attached to armies/fleets.

2.  **Pathfinding (`game_pathfinding`)**:
    *   A generic A* implementation in a standalone crate.
    *   Uses traits `Graph`, `Node`, and `CostCalculator` to abstract game logic from pathfinding algorithms.
    *   Supports caching and different movement types (Land vs Naval).

3.  **Movement System**:
    *   **Tick-based**: Movement progress accumulates daily.
    *   **Lock-step**: Units move between provinces when accumulated progress >= cost.
    *   **Stateful**: Movement is stored in `MovementState` attached to the unit.

## Data Structures

### Army

```rust
pub struct Army {
    pub id: ArmyId,
    pub owner: Tag,
    pub location: ProvinceId,
    pub regiments: Vec<Regiment>,
    pub movement: Option<MovementState>, // None if stationary
    // ...
}
```

### MovementState

```rust
pub struct MovementState {
    /// The calculated path (excluding current location)
    pub path: VecDeque<ProvinceId>,
    /// Target destination (final node in path)
    pub destination: ProvinceId,
    /// Progress towards the next province (0.0 to 100.0+)
    pub progress: Fixed,
    /// Cost to enter the next province
    pub required_progress: Fixed,
}
```

## Implementation Phases

### Phase 1-3: Foundations (Implied)
*   Basic unit structures defined.
*   Simple stationary units allowed.

### Phase 4: Movement & Pathfinding (Completed)
**Focus**: Deterministic movement, A* pathfinding, and command handling.

*   **A* Implementation**: Generic graph search that handles multiple paths correctly (using `ClosedSet` to prevent cycles/duplication).
*   **Graph Traversal**: `AdjacencyGraph` loaded from game data determines valid neighbors (land/sea connections).
*   **Command Handling**: 
    *   `Command::Move`: Validates ownership and connectivity.
    *   Calculates path from `start` to `dest` at the *moment* the command is issued.
*   **Movement Tick**: 
    *   Daily tick updates `progress` based on movement speed.
    *   When `progress >= cost`, unit pops next province from `path` and updates `location`.
    *   Handles "overshoot" (unused movement isn't wasted, though currently resets on move).
*   **Dynamic Costs**:
    *   Architecture supports `CostCalculator` trait.
    *   *Current Limitation*: Uses `BASE_MOVE_COST` (10.0) pending borrow checker refactor for full dynamic cost calculation.

### Phase 5: Advanced Mechanics (Planned)
*   **Naval Transport**: Loading armies onto fleets (`Command::BoardShip`).
*   **Zone of Control (ZoC)**: Forts restricting movement options.
*   **Attrition**: Monthly losses based on supply limit and terrain.
*   **Terrain Costs**: Dynamic movement costs based on terrain type and rivers.
*   **Combat**: Battle initiation when hostile units meet.

## Key Algorithms

### A* Pathfinding
Design ensures we find the *fastest* path, not just the shortest distance.
*   **Heuristic**: Euclidean distance (admissible for spatial graphs).
*   **Cost**: Time to traverse (1.0 / speed).

### Path Validation
Paths are snapshotted at command time.
*   **Risk**: Requires re-validation if the world changes (e.g., military access revoked, fort built).
*   **Solution**: `run_movement_tick` will eventually check path validity step-by-step.

## Open Design Decisions

1.  **Dynamic Costs & Borrow Checker**:
    *   Calculating costs requires strictly reading `WorldState`.
    *   Moving units requires mutating `WorldState`.
    *   *Plan*: Split movement into "Calculate Costs" (Read) and "Apply Movement" (Write) phases.

2.  **Empty Paths**:
    *   Handling `start == destination` edge cases gracefully.
