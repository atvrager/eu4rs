# Project Roadmap

This document tracks the implementation status of the eu4rs simulation engine and visualization tools.

## Overview

**Current Focus**: Phase 10 - Generic UI Engine
**Recently Completed**: Phase 10.2 - Generic UI Binder, Phase 10.1 - 9-Slice Rendering, Phase 9 - Celestial Empire

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
- [x] **Modifier System**: Comprehensive modifier implementation (285/313 modifiers, 91% coverage)
  - Combat: discipline, morale, cavalry_power, infantry_power, artillery_power
  - Military: manpower_recovery_speed, land_forcelimit, naval_forcelimit
  - Economy: global_tax_modifier, production_efficiency, trade_efficiency
  - Diplomacy: diplomatic_reputation, improve_relation_modifier, ae_impact
  - Technology: tech_cost modifiers (ADM/DIP/MIL)
  - Leaders: land_morale, naval_morale, leader pips, army_tradition
  - Estates: loyalty/influence modifiers for 14 estates, privilege slots
  - Buildings: province and country modifiers from buildings
  - Wired through `apply_modifier()` with HashMap accumulation pattern
- [x] **Policy System**: Policy management and bonuses (220 lines + data)
  - 72 policies loaded from game files
  - Policy slot calculation (3 free + 1 per 8 ideas)
  - `EnablePolicy`, `DisablePolicy` commands (stubbed)
  - Modifier application from active policies
- [x] **Building Modifier System**: Province and country modifiers from buildings
  - Province modifiers: local_defensiveness, garrison_size, supply_limit, fort_level
  - Country modifiers: global_tax_modifier, production_efficiency, trade_power
  - Recompute on building construction/demolition
- [x] **Estate System**: Full estate mechanics (2,800 lines across 4 files)
  - 15 estate types (3 core + 12 special estates)
  - Government-based estate availability (Pirate Republics, Theocracies, etc.)
  - Loyalty/influence dynamics with monthly decay
  - Privilege management (`GrantPrivilege`, `RevokePrivilege` commands)
  - Crown land management (`SeizeLand`, `SaleLand` commands)
  - 26 estate-specific modifiers (loyalty, influence, privilege slots)
  - Disaster detection (100% influence + <30% loyalty)
- [x] **Advanced Diplomacy Commands**: Full diplomatic relations system (14 commands + integration)
  - **Alliances** (4): `OfferAlliance`, `AcceptAlliance`, `RejectAlliance`, `BreakAlliance`
  - **Royal Marriages** (4): `OfferRoyalMarriage`, `AcceptRoyalMarriage`, `RejectRoyalMarriage`, `BreakRoyalMarriage`
  - **Military Access** (4): `RequestMilitaryAccess`, `GrantMilitaryAccess`, `DenyMilitaryAccess`, `CancelMilitaryAccess`
  - **Rivals** (2): `SetRival`, `RemoveRival`
  - Pending offer tracking (directional offers with acceptance/rejection)
  - Mutual offer auto-acceptance for alliances and marriages
  - Diplomatic cooldown system (one diplomatic action per day)
  - War integration: declarations automatically break alliances, royal marriages, and military access between enemies
  - Royal marriage war penalty: -1 stability when attacking an RM partner
  - AI integration: neighbor-based offer generation, response handling
  - 25+ test cases covering all commands and integration points

### In Progress
- [x] **Alliance Enforcement**: Defensive pact call-to-arms ✅
  - Call-to-arms acceptance and decline mechanics
  - Trust system (bilateral 0-100 tracking)
  - AI acceptance scoring (trust, debt, stability factors)
  - Decline penalties: -25 prestige, alliance break, -10 trust with all allies
  - Accept bonuses: +5 trust with caller
  - Conflict detection (prevents impossible war configurations)
  - AI integration: GreedyBot scoring for `JoinWar` and `DeclineCallToArms`
  - 14 comprehensive tests covering all mechanics
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

## Phase 8: Holy Roman Empire ✅ **COMPLETE**

*Implemented in v0.3.1*

HRE mechanics including emperor election and imperial reforms.

- [x] **HRE State**: Emperor, electors, imperial authority, reform tracking
- [x] **Emperor Election**: 7 electors vote on emperor death
- [x] **Imperial Authority**: Monthly gain from peace, elector relations
- [x] **Imperial Reforms**: 8 sequential reforms with IA costs
- [x] **Free Cities**: Special OPM status with bonuses
- [x] **Imperial Ban**: Emperor can declare war on non-members holding HRE land
- [x] **Ewiger Landfriede**: Reform preventing internal HRE wars
- [x] **Revoke Privilegia**: Capstone reform vassalizing all members

**Key Files**: [hre.rs](../../eu4sim-core/src/systems/hre.rs), [docs](hre-implementation.md)

---

## Phase 9: Celestial Empire ✅ **COMPLETE**

*Implemented in v0.3.2*

Emperor of China mechanics with mandate and meritocracy.

- [x] **Mandate System**: 0-100 yearly value from stability, tributaries, devastation, loans
- [x] **Meritocracy**: -100 to 100 government stat affecting advisor costs, corruption
- [x] **Celestial Reforms**: 21 non-sequential reforms with prerequisites
  - Prerequisite chains (silver standard requires single whip law)
  - Mutually exclusive factions (bureaucratic vs military)
  - Capstone reform (vassalize tributaries) requires 8 reforms
- [x] **Commands**: TakeMandate, PassCelestialReform, IssueCelestialDecree
- [x] **Tributary Integration**: ForceTributary, RequestTributary, RevokeTributary
- [x] **StrengthenGovernment**: 100 MIL for +10 meritocracy
- [x] **Save Hydration**: Extract and hydrate celestial state from saves

**Key Files**: [celestial.rs](../../eu4sim-core/src/systems/celestial.rs), [docs](celestial-implementation.md)

---

## Phase 10: Generic UI Engine 🔄 **IN PROGRESS**

*Target: v0.4.1*

Decoupled UI system for moddable interface management.

- [x] **10.1: 9-Slice Rendering Foundation**: Scalable backgrounds and asset management (Phase 1)
- [x] **10.2: The Generic UI Binder**: Decoupling Rust code from .gui files (Phase 2)
- [ ] **10.3: Macro & Data Binding**: Procedural macros for UI sync (Phase 3)
- [ ] **10.4: Event Handling & Focus**: Interactive controls and focus management (Phase 5)
- [x] **Phase 11: RealTerrain Graphics Improvement**: Authentic EU4 map rendering
  - [x] **11.1: Foundations**: Normal mapping, water colormap, seasonal tints (Phase 1)
  - [x] **11.2: Terrain Splatting**: Detailed textures (forest, desert, etc.) using splat maps (Phase 2)
  - [ ] **11.3: Borders & FX**: High-quality borders, rivers, and seasonal transitions (Phase 3)

**Key Files**: [mod.rs](../eu4game/src/gui/mod.rs), [nine_slice.rs](../eu4game/src/gui/nine_slice.rs), [sprite_cache.rs](../eu4game/src/gui/sprite_cache.rs)

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

### Command Status (38 Total)

**✅ Fully Implemented (23 commands)**:
- Buildings: `BuildInProvince`, `CancelConstruction`, `DemolishBuilding`
- Military: `Move`, `MoveFleet`, `Embark`, `Disembark`, `MergeArmies`
- War: `DeclareWar`, `OfferPeace`, `AcceptPeace`, `RejectPeace`, `JoinWar`, `CallAllyToWar`
- Economy: `DevelopProvince`, `BuyTech`, `EmbraceInstitution`, `Core`
- Ideas: `PickIdeaGroup`, `UnlockIdea`
- Trade: `SendMerchant`, `RecallMerchant`, `UpgradeCenterOfTrade`
- Colonization: `StartColony`, `AbandonColony`
- Generals: `RecruitGeneral`, `AssignGeneral`, `UnassignGeneral`
- Recruitment: `RecruitRegiment`
- Estates: `GrantPrivilege`, `RevokePrivilege`, `SeizeLand`, `SaleLand`

**❌ Stubbed (15 commands)**:
- Diplomacy (12): Alliance/RM offers/responses, military access, rivals
- Religion (3): Missionary assignment, country conversion
- Other: `SplitArmy`, `MoveCapital`

### System Metrics

```
Total LOC:           ~27,000 (eu4sim-core/src, estimated)
System Modules:      28 files
Unit Tests:          520 tests
Estates (4 files):   2,800 lines
Combat System:       1,467 lines
Trade (3 files):     1,622 lines
Buildings:           827 lines
Naval Combat:        846 lines
Siege:               726 lines
Movement:            479 lines
Ideas:               429 lines
Attrition:           412 lines
Policies:            220 lines

Daily Ticks:         4 systems
Monthly Ticks:       20 systems (added estates)
```

### Modifier System Status

**Total Modifiers**: 313 unique modifier types found in game data
**Implemented**: 285 modifiers (91% coverage)
**Remaining**: 28 modifiers (9%)

**Coverage Breakdown**:
- Ideas: 150+ modifiers from idea groups
- Buildings: ~30 province/country modifiers
- Estates: 26 estate-specific modifiers
- Policies: Modifiers from 72 policies
- Core Systems: Combat, economy, diplomacy base modifiers

**Architecture**: Modifiers stored in `GameModifiers` HashMaps, applied via `apply_modifier()` with accumulation pattern

---

## Contributing

Before starting work on a feature:

1. **Check this roadmap** for phase priorities
2. **Review design docs** in [docs/design/](../design/)
3. **Follow property-based testing** workflow ([guide](../development/testing/property-based-testing.md))
4. **Run CI** before committing: `cargo xtask ci`

**Priority**: Phase 6 completion (religion commands, casus belli system). Phase 7 advanced economy systems.

---

## Known Accuracy Issues ⚠️

Simplified formulas that may need refinement. Track via `eu4sim-verify` hydration tests.

| System | Issue | Current Formula | Notes |
|--------|-------|-----------------|-------|
| Force Limit | Simplified calculation | `6 + dev/10` | Missing: gov type, ideas, policies, buildings, subjects |
| Starting Armies | Approximate placement | Top dev provinces | EU4 may use additional factors |

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

*Last updated: 2025-01-01*
