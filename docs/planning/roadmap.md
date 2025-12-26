# Project Roadmap

This document tracks the implementation status of the eu4rs simulation engine and visualization tools.

## Overview

**Current Focus**: Phase 6 - Modifier System & Advanced Diplomacy
**Next Up**: Phase 7 - Advanced Economy

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

- [x] **War Declaration**: `DeclareWar` command with attacker/defender coalitions
- [x] **Combat System**: Daily combat resolution when hostile armies meet (1,467 lines)
  - EU4-authentic battle phases (Fire/Shock alternation every 3 days)
  - Discipline affects damage, cavalry ratio penalties
  - 10:1 stackwipe mechanics
  - River crossing penalties, terrain modifiers
- [x] **Combat Power**: Type-based modifiers (Infantry: 1.0, Cavalry: 1.5, Artillery: 1.2)
- [x] **Stability & Prestige System**: Bounded value types for stability (-3 to +3), prestige, army tradition
  - Monthly decay for prestige and tradition (~5%/year)
  - No-CB war penalty (-2 stability)
  - Peace term effects (White Peace: -10 prestige, Full Annexation: +25)
- [x] **Peace System**: War resolution with land transfer
  - TakeProvinces peace terms with occupied enemy provinces
  - Fort requirement: must occupy a fort to take provinces
  - War score validation for peace term costs
- [x] **Truce System**: 5-year cooling off period between warring parties

**Key Files**: [combat.rs](../../eu4sim-core/src/systems/combat.rs), [step.rs](../../eu4sim-core/src/step.rs), [bounded.rs](../../eu4sim-core/src/bounded.rs)

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

**Key Files**: [movement.rs](../../eu4sim-core/src/systems/movement.rs), [game_pathfinding](../../game_pathfinding/)

---

## Phase 5: Advanced Military ✅ **COMPLETE**

*Target: v0.2.0*

Enhanced combat and military management.

- [x] **Siege System**: Fort siege mechanics (726 lines)
  - Siege progress calculation (30-day phases, RNG-based resolution)
  - Fort level affects siege difficulty
  - Armies persist until siege completes
- [x] **Naval Combat**: Ship combat mechanics (846 lines)
- [x] **Generals**: Leader recruitment and assignment
  - `RecruitGeneral` command (50 MIL)
  - `AssignGeneral` / `UnassignGeneral` commands
  - Fire, shock, maneuver, siege stats
- [x] **Truce Enforcement**: 5-year cooling period after peace
- [x] **Zone of Control**: Fort logic restricting movement
- [x] **Strait Blocking**: Enemy fleets block sea crossings
- [x] **Attrition System**: Supply limits and monthly losses (412 lines)

**Key Files**: [siege.rs](../../eu4sim-core/src/systems/siege.rs), [naval_combat.rs](../../eu4sim-core/src/systems/naval_combat.rs), [attrition.rs](../../eu4sim-core/src/systems/attrition.rs)

---

## Phase 6: AI & Economy 🔄 **IN PROGRESS**

*Target: v0.3.0*

Expanded AI capabilities and economic depth.

### Completed
- [x] **AI Crate Refactor**: Extract `eu4sim-ai` crate from `eu4sim-core/src/ai/`
- [x] **GreedyBot**: Heuristic AI that makes locally-optimal decisions (714 lines)
  - Economy: Prioritize high-ROI buildings, develop best provinces
  - Military: Attack weak neighbors, siege forts, persist sieges
  - Action ranking logic (reusable for ML prompt filtering later)
  - Serves as training data generator for learned AI
- [x] **Available Commands API**: `fn available_commands(&WorldState, &Tag) -> Vec<Command>`
  - Enumerates all legal actions for a country this tick
  - 600+ lines of validation logic
- [x] **LLM AI**: Candle inference integration (v0.1.9)
  - SmolLM2-360M base model with LoRA adapters
  - 600-1000ms inference time (CPU F32)
  - Cap'n Proto training data format
- [x] **Trade System**: Trade node mechanics (1,622 lines across 3 files)
  - Trade power calculation
  - Value steering and collection
  - Merchant assignment (`SendMerchant`, `RecallMerchant`)
- [x] **Buildings**: Province buildings with construction queue (827 lines)
  - 26 building types from EU4 data
  - Construction costs and time
  - Economic/military bonuses
  - `BuildInProvince`, `CancelConstruction`, `DemolishBuilding` commands
- [x] **Technology**: Tech groups and advancement (62 lines)
  - ADM/DIP/MIL tech levels (1-32)
  - Linear cost formula: 600 + (level × 60)
  - `BuyTech` command
- [x] **Institutions**: Institution spread mechanics (121 lines)
  - Monthly spread based on development
  - `EmbraceInstitution` command
  - 10% presence requirement
- [x] **Ideas System**: National and generic idea groups (429 lines + data)
  - 50 generic groups + 400 national idea sets
  - 7 ideas per group + completion bonuses
  - `PickIdeaGroup`, `UnlockIdea` commands
- [x] **Colonization**: Standing order colonies (50 lines)
  - `StartColony` / `AbandonColony` commands
  - Fixed growth rate (~1000 settlers/year)
- [x] **Development**: Province development purchasing (60 lines)
  - `DevelopProvince` command (50 mana/click for Tax/Production/Manpower)
- [x] **Coring System**: Province coring to reduce overextension (343 lines)
  - `Core` command (10 ADM per dev, 36 months)
- [x] **Subjects & Vassals**: Relationship tracking
  - Data structures for vassal/subject types
- [x] **Coalitions**: Aggressive expansion tracking (271 lines)
  - Coalition formation based on AE threshold
  - War participation tracking
- [x] **Reformation**: Religion spread system (254 lines)
  - Simplified spread logic via adjacency

### In Progress
- [ ] **Modifier System Wiring**: Connect idea modifiers to actual mechanics
  - **Current Status**: Only 4/400+ modifiers implemented
    - `global_tax_modifier` ✓
    - `land_maintenance_modifier` ✓
    - `fort_maintenance_modifier` ✓
    - `production_efficiency` ✓
  - **Next**: Wire top 20 modifiers by frequency (discipline, cavalry_power, goods_produced, etc.)
  - **Blocker**: Ideas parse correctly but 96% of modifiers have no gameplay effect
- [ ] **Alliance Enforcement**: Defensive pact call-to-arms
  - Data structures exist, `CallAllyToWar` / `JoinWar` commands defined
  - Pending diplomacy queue implemented
  - **Missing**: Acceptance logic, honor penalty
- [ ] **Advanced Diplomacy Commands**: 12 stubbed commands need implementation
  - `OfferAlliance`, `BreakAlliance`, `AcceptAlliance`, `RejectAlliance`
  - `OfferRoyalMarriage`, `BreakRoyalMarriage`, `AcceptRoyalMarriage`, `RejectRoyalMarriage`
  - `RequestMilitaryAccess`, `CancelMilitaryAccess`, `GrantMilitaryAccess`, `DenyMilitaryAccess`
  - `SetRival`, `RemoveRival`
- [ ] **Religion Commands**: 3 stubbed commands need implementation
  - `AssignMissionary`, `RecallMissionary`, `ConvertCountryReligion`
- [ ] **Casus Belli System**: War justification mechanics beyond no-CB

---

## Phase 7: Advanced Economy 📋 **PLANNED**

*Target: v0.4.0*

Trade expansion and economic complexity.

- [ ] **Trade Companies**: Asia/Africa trade posts
- [ ] **Privateering**: Disrupting enemy trade
- [ ] **Mercantilism**: Trade policy mechanics
- [ ] **Building Effects**: Wire building bonuses to production/tax/manpower
- [ ] **Tech Effects**: Apply tech bonuses to units, economy, institutions
- [ ] **Modifier Stacking**: Complete implementation of remaining 396+ modifiers

---

## Visualization & Tools

Parallel development track for rendering and debugging.

### eu4viz (Visualizer) 🔄 **ACTIVE**

- [x] **Map Rendering**: Province polygons with Vulkan/WGPU
- [x] **Political Map Mode**: Country colors
- [x] **Terrain Rendering**: Terrain type visualization
- [x] **Camera Controls**: Pan, zoom, rotation
- [x] **Timeline Replay**: Event log visualization with time slider
  - Sparse ownership changes for memory-efficient state reconstruction
  - Drag-to-scrub with 367ms map regeneration (16x optimized)
  - Date display with fallback computation
- [ ] **Unit Visualization**: Army/fleet icons on map
- [ ] **UI Overlays**: Info panels, tooltips
- [ ] **Map Modes**: Diplomatic, trade, development views

### Developer Tools

- [x] **Property-Based Testing**: SystemVerilog Assertion analogy ([docs](../development/testing/property-based-testing.md))
- [x] **Code Coverage**: >75% target with `llvm-cov` ([docs](../development/testing/coverage.md))
- [x] **Auto-Codegen**: Type generation from EU4 schemas ([docs](../development/code-generation.md))
- [x] **Personalization System**: AI agent personas via MyAnimeList integration
- [x] **Profiling**: FPS counter, timing instrumentation ([docs](../development/performance.md))
- [x] **Replay System**: Timeline replay from event log

---

## Version History

| Version | Date | Highlights |
|---------|------|------------|
| **0.2.0** | 2025-12-24 | War resolution with province transfers, siege system, gp-only AI mode |
| **0.1.9** | 2025-12-21 | LLM AI with LoRA inference, hybrid mode integration, training pipeline |
| **0.1.8** | 2025-12-20 | Timeline replay with event log, 367ms map regeneration, FPS profiling |
| **0.1.7** | 2025-12-19 | Truce system, AI war declaration filtering, checksum integration |
| **0.1.6** | 2025-12-19 | Personalization System, agent personas, Claude Code protocols |
| **0.1.5** | 2025-12-19 | Stability & prestige system, bounded value types |
| **0.1.4** | 2025-12-17 | Property testing, movement pathfinding |
| **0.1.3** | 2025-12 | Movement system, naval transport |
| **0.1.2** | 2025-11 | Combat system, war declarations |
| **0.1.1** | 2025-11 | Manpower, expenses, military units |
| **0.1.0** | 2025-11 | Production, taxation, core state |

---

## Implementation Reality Check

### Command Status (34 Total)

**✅ Fully Implemented (19 commands)**:
- Buildings: `BuildInProvince`, `CancelConstruction`, `DemolishBuilding`
- Military: `Move`, `MoveFleet`, `Embark`, `Disembark`, `MergeArmies`
- War: `DeclareWar`, `OfferPeace`, `AcceptPeace`, `RejectPeace`, `JoinWar`, `CallAllyToWar`
- Economy: `DevelopProvince`, `BuyTech`, `EmbraceInstitution`, `Core`
- Ideas: `PickIdeaGroup`, `UnlockIdea`
- Trade: `SendMerchant`, `RecallMerchant`, `UpgradeCenterOfTrade`
- Colonization: `StartColony`, `AbandonColony`
- Generals: `RecruitGeneral`, `AssignGeneral`, `UnassignGeneral`
- Recruitment: `RecruitRegiment`

**❌ Stubbed (15 commands)**:
- Diplomacy (12): Alliance/RM offers/responses, military access, rivals
- Religion (3): Missionary assignment, country conversion
- Other: `SplitArmy`, `MoveCapital`

### System Metrics

```
Total LOC:           22,546 (eu4sim-core/src)
System Modules:      25 files (9,229 LOC)
Unit Tests:          494 tests
Combat System:       1,467 lines
Buildings:           827 lines
Naval Combat:        846 lines
Siege:              726 lines
Trade (3 files):    1,622 lines
Movement:            479 lines
Ideas:              429 lines
Attrition:          412 lines

Daily Ticks:         4 systems
Monthly Ticks:       19 systems
```

### Modifier System Reality

**Architecture**: `ModifierStubTracker` tracks 400+ modifier types from ideas data
**Applied**: Only 4 modifiers actually affect gameplay
**Impact**: Ideas can be picked and unlocked, but ~96% have no mechanical effect

**Next Steps**: Wire top 20 modifiers by frequency to close this gap

---

## Contributing

Before starting work on a feature:

1. **Check this roadmap** for phase priorities
2. **Review design docs** in [docs/design/](../design/)
3. **Follow property-based testing** workflow ([guide](../development/testing/property-based-testing.md))
4. **Run CI** before committing: `cargo xtask ci`

**Priority**: Phase 6 modifier wiring. Focus on connecting idea modifiers to actual mechanics.

---

## Future Explorations 💡

Ideas worth exploring but not on the critical path:

### Learned AI (LLM-Trained) ✅ **PHASE 1-2 COMPLETE**

Train small language models (360M-2B params) to play EU4. See [Learned AI Design](../design/simulation/learned-ai.md).

**Completed** (v0.1.9):
- ✅ `eu4sim-ai` crate with Candle inference
- ✅ SmolLM2-360M base model loading from HuggingFace
- ✅ LoRA adapter merging (160 weight pairs)
- ✅ Integrated into hybrid mode (1 LlmAi + N GreedyAIs)
- ✅ Training pipeline: Colab notebook with CUDA training
- ✅ Cap'n Proto binary format for training data

**Performance** (CPU, F32):
- Model load: ~1.0s (with LoRA merge)
- Inference: 600-1000ms per prompt
- Prompt size: ~220-340 tokens

**Next Steps**:
- [ ] RL improvement via self-play (Python + Rust game env)
- [ ] Quantized models (4-bit) for faster inference
- [ ] Multiple personality adapters (aggressive, diplomatic, etc.)

### TUI Rendering Mode

Play EU4 in your terminal! A text-based interface using `ratatui` or similar:
- ASCII map with country colors
- Command-line input for orders
- Turn-based or simplified real-time
- Perfect for SSH sessions or low-resource environments

### Performance Instrumentation

- `--benchmark` flag to measure simulation speed
- Phase-by-phase timing breakdown
- Track progress toward sub-10-minute full game goal

---

*Last updated: 2025-12-26*
