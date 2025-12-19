# Project Roadmap

This document tracks the implementation status of the eu4rs simulation engine and visualization tools.

## Overview

**Current Focus**: Phase 5 - Advanced Military Features
**Next Up**: Phase 6 - Diplomacy & AI

---

## Phase 1: Economic Foundation ✅ **COMPLETE**

*Implemented in v0.1.0*

Core economic systems with deterministic fixed-point arithmetic.

- [x] **Production System**: Province-based goods production with trade value
  - Formula: `income = (base_production × 0.2) × goods_price × (1 + efficiency) × (1 - autonomy)`
  - Monthly tick, deterministic `Fixed` arithmetic
- [x] **Taxation System**: Base tax collection with efficiency and autonomy modifiers
  - Formula: `tax = (base_tax + efficiency) × (1 - autonomy)`
  - Monthly revenue to country treasury
- [x] **WorldState**: Core simulation state structure
- [x] **Deterministic Testing**: Integration tests for reproducibility

**Key Files**: [production.rs](../../eu4sim-core/src/systems/production.rs), [taxation.rs](../../eu4sim-core/src/systems/taxation.rs)

---

## Phase 2: Military & Expenses ✅ **COMPLETE**

*Implemented in v0.1.1*

Military unit management and recurring expenses.

- [x] **Manpower System**: Country-level manpower pools with monthly recovery
  - Max manpower based on province base_manpower development
  - Recovery over 120 months (10 years)
- [x] **Regiment Structure**: Infantry, Cavalry, Artillery unit types
  - 1000 men per regiment, deterministic strength tracking
- [x] **Expense System**: Monthly costs for armies and forts
  - Army: 0.2 ducats/regiment/month
  - Forts: 1.0 ducats/fort/month
- [x] **Auto-initialization**: Armies generated from manpower at game start

**Key Files**: [manpower.rs](../../eu4sim-core/src/systems/manpower.rs), [expenses.rs](../../eu4sim-core/src/systems/expenses.rs)

---

## Phase 3: Diplomacy & War ✅ **COMPLETE**

*Implemented in v0.1.2*

Basic war system with combat resolution.

- [x] **Diplomatic Relations**: Alliance, Rival relationship tracking
- [x] **War Declaration**: `DeclareWar` command with attacker/defender coalitions
- [x] **Combat System**: Daily combat resolution when hostile armies meet
  - Power-based casualty calculation
  - Regiment destruction when strength reaches zero
  - Army removal when all regiments destroyed
- [x] **Combat Power**: Type-based modifiers (Infantry: 1.0, Cavalry: 1.5, Artillery: 1.2)

**Key Files**: [combat.rs](../../eu4sim-core/src/systems/combat.rs), [step.rs](../../eu4sim-core/src/step.rs)

**Limitations**: Alliances not enforced, no peace treaties yet

---

## Phase 4: Movement & Pathfinding ✅ **COMPLETE**

*Implemented in v0.1.3-0.1.4*

Deterministic army/fleet movement with A* pathfinding.

- [x] **A* Pathfinding**: Generic graph search in `game_pathfinding` crate
  - Heuristic-based shortest path calculation
  - Closed-set cycle prevention
- [x] **Movement Commands**: `Command::Move` for armies and fleets
- [x] **Tick-based Progress**: Daily movement tick with progress accumulation
  - Progress resets on province transition
  - Movement state stored per unit
- [x] **Naval Transport**: Basic embarked army tracking
  - Armies follow fleet location when embarked
  - Boarding/disembarking mechanics implemented
- [x] **Property Tests**: Movement monotonicity verification
- [x] **Dynamic Costs**: Terrain-based movement costs
  - Resolves borrow checker blocker via two-pass pattern
- [ ] **Zone of Control**: Fort logic restricting movement
- [ ] **Attrition**: Supply limit calculations and monthly losses

**Key Files**: [movement.rs](../../eu4sim-core/src/systems/movement.rs), [game_pathfinding](../../game_pathfinding/)

**Next Steps**: Dynamic movement costs, zone of control

---

## Phase 5: Advanced Military 📋 **PLANNED**

*Target: v0.2.0*

Enhanced combat and military management.

- [ ] **Terrain Effects**: Movement costs and combat modifiers
  - River crossings, mountain penalties
  - Terrain-specific combat bonuses
- [ ] **Siege System**: Fort siege mechanics
  - Siege progress calculation
  - Garrison attrition
  - Assault/breach mechanics
- [ ] **Leaders**: General/Admiral stats and bonuses
  - Command, fire, shock, maneuver
  - Leader assignment to armies/fleets
- [ ] **Morale System**: Unit morale and rout mechanics
- [ ] **Supply Lines**: Attrition based on supply limit
  - Province supply capacity
  - Distance from capital/ports

---

## Phase 6: Diplomacy & AI 📋 **PLANNED**

*Target: v0.3.0*

Expanded diplomatic actions and basic AI.

- [ ] **Peace Treaties**: War resolution with land transfer
- [ ] **Alliance Enforcement**: Defensive pact call-to-arms
- [ ] **Casus Belli System**: War justification mechanics
- [ ] **Basic AI**: Simple decision-making for countries
  - Economic management
  - Military deployment
  - Diplomatic actions

---

## Phase 7: Advanced Economy 📋 **PLANNED**

*Target: v0.4.0*

Trade, buildings, and economic complexity.

- [ ] **Trade System**: Trade node mechanics
  - Trade power calculation
  - Value steering and collection
- [ ] **Buildings**: Province buildings with effects
  - Construction costs and time
  - Economic/military bonuses
- [ ] **Technology**: Tech groups and advancement
- [ ] **Institutions**: Institution spread mechanics

---

## Visualization & Tools

Parallel development track for rendering and debugging.

### eu4viz (Visualizer) 🔄 **ACTIVE**

- [x] **Map Rendering**: Province polygons with Vulkan/WGPU
- [x] **Political Map Mode**: Country colors
- [x] **Terrain Rendering**: Terrain type visualization
- [x] **Camera Controls**: Pan, zoom, rotation
- [ ] **Unit Visualization**: Army/fleet icons on map
- [ ] **UI Overlays**: Info panels, tooltips
- [ ] **Map Modes**: Diplomatic, trade, development views

### Developer Tools

- [x] **Property-Based Testing**: SystemVerilog Assertion analogy ([docs](../development/testing/property-based-testing.md))
- [x] **Code Coverage**: >75% target with `llvm-cov` ([docs](../development/testing/coverage.md))
- [x] **Auto-Codegen**: Type generation from EU4 schemas ([docs](../development/code-generation.md))
- [ ] **Profiling**: Performance analysis tools
- [ ] **Replay System**: Deterministic replay from command log

---

## Version History

| Version | Date | Highlights |
|---------|------|------------|
| **0.1.4** | 2025-12-17 | Property testing, movement pathfinding |
| **0.1.3** | 2025-12 | Movement system, naval transport |
| **0.1.2** | 2025-11 | Combat system, war declarations |
| **0.1.1** | 2025-11 | Manpower, expenses, military units |
| **0.1.0** | 2025-11 | Production, taxation, core state |

---

## Contributing

Before starting work on a feature:

1. **Check this roadmap** for phase priorities
2. **Review design docs** in [docs/design/](../design/)
3. **Follow property-based testing** workflow ([guide](../development/testing/property-based-testing.md))
4. **Run CI** before committing: `cargo xtask ci`

**Priority**: Focus on completing Phase 4 before starting Phase 5.

---

*Last updated: 2025-12-19*
