# Phase 9: Emperor of China (Celestial Empire) Implementation

**Status**: Complete
**Implemented**: 2025-12-29
**Design Doc**: [celestial-empire.md](../design/simulation/celestial-empire.md)

**Scope**: Core mechanics (mandate, meritocracy, celestial reforms, tributaries)
**Deferred**: Unguarded Nomadic Frontier disaster, Mandate of Heaven CB details, decree effects

## Overview

The Celestial Empire is China's unique political system where the Emperor of China maintains the Mandate of Heaven. Unlike HRE's elections, the mandate can be taken by conquest. Reforms are non-sequential and provide bonuses to emperor and tributaries.

## Key Mechanics

| Component | Description |
|-----------|-------------|
| **Emperor** | Holds Mandate of Heaven. Can be taken via war ("Take Mandate" CB) |
| **Mandate** | 0-100 value. Above 50 = positive modifiers, below 50 = negative |
| **Meritocracy** | -100 to 100 government stat. Affects advisor costs, corruption |
| **Celestial Reforms** | Non-sequential reforms, can be passed at 80+ mandate (costs 70 mandate) |
| **Tributaries** | Subject type that provides mandate growth (+0.15/100 dev) |

## Mandate Formula (Yearly)

```
Base yearly change = 0
+ (stability × 0.4)                    # Per positive stability
+ (prosperous_states × 0.04)           # Per prosperous state
+ (tributary_dev / 100 × 0.15)         # Per 100 tributary dev
- (devastation_dev × 0.12)             # Per 100 devastated dev (scaled)
- (loans / 5 × 0.60)                   # Per 5 loans
```

**Special Events:**
- Defending title successfully: +5 mandate
- Refusing tributary CtA: -10 mandate
- New emperor starts at: 80 mandate

## Meritocracy Effects

| Value | Advisor Cost | Spy Detection | Yearly Corruption |
|-------|--------------|---------------|-------------------|
| 0 | +25% | -50% | 0 |
| 100 | -25% | +50% | -0.2 |

## Celestial Reforms (21 total from game files)

Unlike HRE, reforms can be passed in any order (with some prerequisites).

### Core Reforms
| Reform | Emperor Effect | Member Effect | Prerequisites |
|--------|---------------|---------------|---------------|
| `seaban_decision` | +1 diplomat, +5% trade eff | - | None |
| `establish_gaituguiliu_decision` | +0.5 meritocracy | +1 advisor pool | No corruption |
| `reform_land_tax_decision` | -5% autonomy | -25% state maint | 35% crown land OR adm 5 |
| `military_governors_decision` | +10% nobles loyalty, -10% core | -20% autonomy change | mil 6 OR MIL focus |
| `centralizing_top_government_decision` | +1 ADM/month | -5% estate influence | 65% crown land |

### Capstone Reform
| Reform | Emperor Effect | Prerequisites |
|--------|---------------|---------------|
| `vassalize_tributaries_decision` | +0.05 mandate, -33% liberty desire | **8 reforms passed** |

### Additional Reforms (1.35+)
- `codify_single_whip_law_decision` - Requires `reform_land_tax_decision`
- `establish_silver_standard_decision` - Requires `codify_single_whip_law_decision`
- `promote_bureaucratic_faction_decision` / `promote_military_faction_decision` - Mutually exclusive
- `unifed_trade_market_decision` - Requires `seaban_decision` + `kanhe_certificate_decision`
- `kanhe_certificate_decision` - Trade efficiency + merchants
- `new_keju_formats_decision` - Governing capacity + reform progress
- `inclusive_monarchy_decision` - Tolerance of heathens
- `reform_the_military_branch_decision` - Army professionalism + movement
- `modernize_the_banners_decision` - Cavalry cost + power
- `study_foreign_ship_designs_decision` - Ship cost + heavy ship power
- `tributary_embassies_decision` - Diplomatic upkeep + favor
- `new_world_discovery_decision` - Colonial growth + colonist
- `reign_in_estates_decision` - Absolutism + admin efficiency
- `reform_civil_registration_decision` - Tax + dev cost

## Commands (8 new)

| Command | Description |
|---------|-------------|
| `TakeMandate` | Claim Mandate of Heaven (via peace deal or decision) |
| `PassCelestialReform { reform }` | Pass reform (80+ mandate, costs 70 mandate + 1 stab) |
| `IssueCelestialDecree { decree }` | Issue decree (costs 20 meritocracy, lasts 10 years, parsed from game files) |
| `ForceTributary { target }` | Force nation to become tributary (war goal) |
| `RequestTributary { target }` | Diplomatic request for tributary status |
| `RevokeTributary { target }` | Release tributary |
| `StrengthenGovernment` | Spend 100 MIL for +10 meritocracy |
| `AbandonMandate` | Give up Celestial Empire status |

## Data Structures

### CelestialEmpireState (add to GlobalState)
```rust
pub struct CelestialEmpireState {
    pub emperor: Option<Tag>,
    pub mandate: Fixed,                    // 0-100
    pub reforms_passed: HashSet<CelestialReformId>,
    pub active_decree: Option<CelestialDecree>,
    pub decree_expires: Option<Date>,
}
```

### CountryState additions
```rust
pub meritocracy: BoundedFixed<-100, 100>,  // Only for celestial emperor
```

### CelestialReformId
```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct CelestialReformId(pub u16);

pub mod celestial_reforms {
    pub const SEABAN: CelestialReformId = CelestialReformId(1);
    pub const GAITUGUILIU: CelestialReformId = CelestialReformId(2);
    pub const REFORM_LAND_TAX: CelestialReformId = CelestialReformId(3);
    pub const MILITARY_GOVERNORS: CelestialReformId = CelestialReformId(4);
    pub const CENTRALIZING_GOVERNMENT: CelestialReformId = CelestialReformId(5);
    pub const VASSALIZE_TRIBUTARIES: CelestialReformId = CelestialReformId(6);
    pub const CODIFY_SINGLE_WHIP_LAW: CelestialReformId = CelestialReformId(7);
    pub const ESTABLISH_SILVER_STANDARD: CelestialReformId = CelestialReformId(8);
    pub const KANHE_CERTIFICATE: CelestialReformId = CelestialReformId(9);
    pub const NEW_KEJU_FORMATS: CelestialReformId = CelestialReformId(10);
    pub const INCLUSIVE_MONARCHY: CelestialReformId = CelestialReformId(11);
    pub const PROMOTE_BUREAUCRATIC: CelestialReformId = CelestialReformId(12);
    pub const PROMOTE_MILITARY: CelestialReformId = CelestialReformId(13);
    pub const UNIFIED_TRADE_MARKET: CelestialReformId = CelestialReformId(14);
    pub const REFORM_MILITARY_BRANCH: CelestialReformId = CelestialReformId(15);
    pub const MODERNIZE_BANNERS: CelestialReformId = CelestialReformId(16);
    pub const STUDY_FOREIGN_SHIPS: CelestialReformId = CelestialReformId(17);
    pub const TRIBUTARY_EMBASSIES: CelestialReformId = CelestialReformId(18);
    pub const NEW_WORLD_DISCOVERY: CelestialReformId = CelestialReformId(19);
    pub const REIGN_IN_ESTATES: CelestialReformId = CelestialReformId(20);
    pub const REFORM_CIVIL_REGISTRATION: CelestialReformId = CelestialReformId(21);
}
```

## Differences from HRE

| Aspect | HRE | Celestial Empire |
|--------|-----|------------------|
| Succession | Election by 7 electors | Conquest (Take Mandate CB) |
| Authority | Imperial Authority (monthly) | Mandate (yearly) |
| Reforms | Sequential order required | Any order (with prerequisites) |
| Members | Provinces with `is_in_hre` | Tributaries only |
| Special mechanic | Free Cities, Ewiger Landfriede | Meritocracy, Decrees |

## Implementation Steps

### Step 0: Prerequisites
- [x] Add `meritocracy` field to `CountryState` (BoundedFixed<-100, 100>)
- [x] Verify tributary system works for Celestial Empire context

### Step 1: Foundation
- [x] Create `CelestialEmpireState` struct in `state.rs`
- [x] Add `celestial_empire` to `GlobalState`
- [x] Create `eu4sim-core/src/systems/celestial.rs`
- [x] Add `CelestialReformId` type
- [x] Add 8 Celestial Empire commands to `input.rs`

### Step 2: Mandate System
- [x] Implement yearly mandate tick in `systems/celestial.rs`
- [x] Calculate tributary development contribution
- [x] Calculate devastation penalty
- [x] Calculate stability bonus
- [x] Calculate loan penalty
- [x] Add defines module with constants
- [x] Unit tests for mandate mechanics

### Step 3: Meritocracy
- [x] Implement meritocracy effects (advisor cost, spy detection)
- [x] Yearly meritocracy from advisors
- [x] Corruption reduction at high meritocracy
- [x] `StrengthenGovernment` command (100 MIL → +10 meritocracy)

### Step 4: Commands
- [x] `PassCelestialReform` (check 80+ mandate, deduct 70, -1 stab)
- [x] `TakeMandate` (transfer emperor, reset reforms, set mandate to 80)
- [x] `ForceTributary` / `RequestTributary` / `RevokeTributary`
- [x] `IssueCelestialDecree` (20 meritocracy, 10 year duration)
- [x] `AbandonMandate`

### Step 4.5: Decree System (deferred)
- [ ] Create `eu4data/src/decrees.rs` parser for `common/decrees/*.txt`
- [ ] Load decrees using existing `Decrees` schema from `generated/types/decrees.rs`
- [ ] Create `DecreeRegistry` with ID lookup
- [ ] Apply decree modifiers (including `imperial_mandate` effect)

### Step 5: Reform System (All 21 Reforms)
- [x] Add all 21 reform constants from `01_china.txt`
- [x] Implement prerequisite chains:
  - `codify_single_whip_law` requires `reform_land_tax`
  - `establish_silver_standard` requires `codify_single_whip_law`
  - `unifed_trade_market` requires `seaban` + `kanhe_certificate`
  - `vassalize_tributaries` requires 8 reforms passed
- [x] Mutually exclusive handling (`promote_bureaucratic` vs `promote_military`)
- [ ] Reform trigger conditions (crown land %, stability, etc.) - deferred
- [ ] Reform modifier application (emperor + member effects) - deferred

### Step 6: Integration
- [x] Tributary dev calculation for mandate growth (reuse `tribute.rs`)
- [ ] "Take Mandate" peace term handling - deferred
- [x] Save hydration for celestial empire state
- [ ] `available_commands()` validation - deferred

### Step 7: Testing
- [x] Mandate calculation tests (10 tests)
- [x] Meritocracy effect tests (9 tests)
- [x] Reform prerequisite tests (4 tests)
- [x] Command validation tests
- [x] Integration tests with tributary system (2 tests)

## Files to Create/Modify

### New Files
| File | Purpose |
|------|---------|
| `eu4sim-core/src/systems/celestial.rs` | Core system logic |
| `eu4data/src/decrees.rs` | Decree parser (per 2B review) |

### Modified Files
| File | Changes |
|------|---------|
| `eu4sim-core/src/state.rs` | Add `CelestialEmpireState`, `meritocracy` field |
| `eu4sim-core/src/input.rs` | Add 8 Celestial Empire commands |
| `eu4sim-core/src/step.rs` | Wire commands + yearly tick |
| `eu4sim-core/src/systems/mod.rs` | Export celestial module |
| `eu4sim-verify/src/hydrate.rs` | Hydrate celestial state from saves |

## Constants (defines.rs additions)

```rust
pub mod celestial {
    pub const REFORM_MANDATE_COST: i32 = 70;
    pub const REFORM_STABILITY_COST: i32 = 1;
    pub const REFORM_MIN_MANDATE: i32 = 80;
    pub const DEFAULT_MANDATE: i32 = 80;
    pub const MODIFIER_THRESHOLD: i32 = 50;

    // Yearly mandate changes
    pub const MANDATE_PER_STABILITY: f32 = 0.4;
    pub const MANDATE_PER_PROSPEROUS_STATE: f32 = 0.04;
    pub const MANDATE_PER_100_TRIBUTARY_DEV: f32 = 0.15;
    pub const MANDATE_PER_100_DEVASTATION: f32 = -12.0;
    pub const MANDATE_PER_5_LOANS: f32 = -0.60;
    pub const MANDATE_DEFENDING_SUCCESS: i32 = 5;
    pub const MANDATE_REFUSED_TRIBUTARY_CTA: i32 = -10;

    // Meritocracy
    pub const STRENGTHEN_GOVERNMENT_MERITOCRACY: i32 = 10;
    pub const DECREE_MERITOCRACY_COST: i32 = 20;
    pub const DECREE_DURATION_YEARS: i32 = 10;
}
```

## Reusable Code from HRE

| Component | HRE Location | Reusability |
|-----------|--------------|-------------|
| State structure pattern | `state.rs:1324` | Copy structure, adapt fields |
| Reform ID type | `state.rs:ReformId` | Create `CelestialReformId` |
| Monthly/yearly tick | `systems/hre.rs` | Adapt for yearly mandate |
| Command patterns | `step.rs` | Copy validation patterns |
| Helper methods | `systems/hre.rs` | Adapt `has_reform()` etc. |

## Reusable Code from Tributaries

| Component | Location | Reusability |
|-----------|----------|-------------|
| Tributary detection | `subjects.rs:is_tributary()` | 100% reuse |
| Development calculation | `tribute.rs` | Adapt for mandate contribution |
| Subject type checks | `SubjectTypeRegistry` | 100% reuse |

## Out of Scope (Phase 10+)

- Unguarded Nomadic Frontier disaster
- Detailed Mandate of Heaven CB mechanics
- Take Mandate peace term scoring
- AI decision-making for Celestial Empire

## References

- [EU4 Wiki: Emperor of China](https://eu4.paradoxwikis.com/Emperor_of_China)
- Game file: `common/imperial_reforms/01_china.txt`
- Game file: `common/defines.lua` (celestial defines)
- [EU4 Wiki: Mandate of Heaven](https://eu4.paradoxwikis.com/Mandate_of_Heaven)
