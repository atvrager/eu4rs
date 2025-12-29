# Celestial Empire (Emperor of China) System

**Status**: Implemented (Phase 9)
**Last Updated**: 2025-12-29

## Overview

The Celestial Empire is China's unique political system where the Emperor of China maintains the Mandate of Heaven. Unlike the HRE's election system, the mandate can be taken by conquest. Reforms are non-sequential and provide bonuses to the emperor and tributaries.

## Core Mechanics

| Component | Description |
|-----------|-------------|
| **Emperor** | Holds Mandate of Heaven. Can be transferred via war or `TakeMandate` command |
| **Mandate** | 0-100 value. Updated yearly based on stability, tributaries, devastation, loans |
| **Meritocracy** | -100 to 100 government stat. Affects advisor costs and corruption |
| **Celestial Reforms** | 21 non-sequential reforms, passed at 80+ mandate (costs 70 mandate + 1 stability) |
| **Tributaries** | Subject type that provides mandate growth (+0.15 per 100 development) |

## Mandate System

### Yearly Mandate Formula

```
mandate_delta = 0
+ (stability × 0.4)                    // Per positive stability point
+ (tributary_dev / 100 × 0.15)         // Per 100 tributary development
- (devastated_dev / 100 × 12.0)        // Per 100 devastated development
- (loans / 5 × 0.60)                   // Per 5 active loans
```

### Implementation

The mandate tick runs on January 1st of each year via `run_celestial_tick()` in `systems/celestial.rs`:

```rust
// Stability bonus
if stability > 0 {
    mandate_delta += MANDATE_PER_STABILITY * stability;
}

// Tributary development bonus
let tributary_dev = calculate_tributary_development(state, emperor_tag);
mandate_delta += tributary_dev / 100 * MANDATE_PER_100_TRIBUTARY_DEV;

// Devastation penalty (dev-weighted)
let devastated_dev = calculate_devastated_development(state, emperor_tag);
mandate_delta -= devastated_dev / 100 * MANDATE_PER_100_DEVASTATION;

// Loan penalty
mandate_delta -= (loans / 5) * MANDATE_PER_5_LOANS;
```

### Constants

Located in `systems/celestial.rs::defines`:

| Constant | Value | Description |
|----------|-------|-------------|
| `DEFAULT_MANDATE` | 80 | Starting mandate for new emperors |
| `REFORM_MIN_MANDATE` | 80 | Minimum mandate to pass reforms |
| `REFORM_MANDATE_COST` | 70 | Mandate cost per reform |
| `MANDATE_PER_STABILITY` | 0.4 | Yearly gain per stability |
| `MANDATE_PER_100_TRIBUTARY_DEV` | 0.15 | Yearly gain per 100 tributary dev |
| `MANDATE_PER_100_DEVASTATION` | 12.0 | Yearly loss per 100 devastated dev |
| `MANDATE_PER_5_LOANS` | 0.60 | Yearly loss per 5 loans |

## Meritocracy System

Meritocracy is a -100 to 100 value tracked per country (stored in `CountryState::meritocracy`).

### Yearly Meritocracy Gain

Each advisor contributes `skill_level × 0.5` meritocracy per year:

```rust
for advisor in emperor.advisors {
    advisor_bonus += advisor.skill * MERITOCRACY_PER_ADVISOR_LEVEL; // 0.5
}
```

### Meritocracy Effects

Linear interpolation from 0 to 100:

| Meritocracy | Advisor Cost Modifier | Corruption Reduction |
|-------------|----------------------|---------------------|
| 0 | +25% | 0 |
| 50 | 0% | -0.1/year |
| 100 | -25% | -0.2/year |

### Implementation

```rust
pub fn calculate_advisor_cost_modifier(meritocracy: Fixed) -> Fixed {
    // Linear: +0.25 at 0, -0.25 at 100
    let ratio = meritocracy / 100;
    ADVISOR_COST_AT_ZERO - ratio * (ADVISOR_COST_AT_ZERO - ADVISOR_COST_AT_MAX)
}

pub fn calculate_corruption_reduction(meritocracy: Fixed) -> Fixed {
    // Linear: 0 at 0, 0.2 at 100
    if meritocracy <= 0 { return 0; }
    (meritocracy / 100) * CORRUPTION_REDUCTION_AT_MAX
}
```

## Celestial Reforms

21 reforms defined in `systems/celestial.rs::reforms`. Unlike HRE, reforms can be passed in any order (with prerequisites).

### Reform Prerequisites

| Reform | Requires |
|--------|----------|
| `CODIFY_SINGLE_WHIP_LAW` | `REFORM_LAND_TAX` |
| `ESTABLISH_SILVER_STANDARD` | `CODIFY_SINGLE_WHIP_LAW` |
| `UNIFIED_TRADE_MARKET` | `SEABAN` + `KANHE_CERTIFICATE` |
| `VASSALIZE_TRIBUTARIES` | 8 reforms passed |
| `PROMOTE_BUREAUCRATIC` | Not `PROMOTE_MILITARY` (mutually exclusive) |
| `PROMOTE_MILITARY` | Not `PROMOTE_BUREAUCRATIC` (mutually exclusive) |

### Passing Reforms

Requirements:
- Must be Emperor of China
- Mandate >= 80
- Stability >= 1
- Prerequisites met
- Reform not already passed

Costs:
- 70 mandate
- 1 stability

## Commands

| Command | Description | Validation |
|---------|-------------|------------|
| `TakeMandate` | Claim Emperor of China status | Resets reforms, sets mandate to 80 |
| `PassCelestialReform { reform }` | Pass a celestial reform | 80+ mandate, prerequisites |
| `IssueCelestialDecree { decree }` | Issue decree | 20 meritocracy cost |
| `ForceTributary { target }` | Force tributary status | Target not already subject |
| `RequestTributary { target }` | Request tributary status | Target not already subject |
| `RevokeTributary { target }` | Release tributary | Must be your tributary |
| `StrengthenGovernment` | Spend 100 MIL for +10 meritocracy | Must be emperor, 100+ MIL |
| `AbandonMandate` | Give up Celestial Empire | Must be emperor |

## Data Structures

### CelestialEmpireState

```rust
pub struct CelestialEmpireState {
    pub emperor: Option<Tag>,
    pub mandate: Fixed,
    pub dismantled: bool,
    pub reforms_passed: HashSet<CelestialReformId>,
}
```

### CelestialReformId

```rust
#[derive(Hash, Eq, PartialEq, Clone, Copy, Debug, Serialize, Deserialize)]
pub struct CelestialReformId(pub u16);
```

## Save File Hydration

Celestial empire state is extracted from save files via:
- Text parsing: `extract_celestial_empire_from_text()` in `parse.rs`
- Hydration: `map_celestial_reform_name_to_id()` in `hydrate.rs`

## Differences from HRE

| Aspect | HRE | Celestial Empire |
|--------|-----|------------------|
| Succession | Election by 7 electors | Conquest (TakeMandate) |
| Authority | Imperial Authority (monthly) | Mandate (yearly) |
| Reforms | Sequential order required | Any order (with prerequisites) |
| Members | Provinces with `is_in_hre` | Tributaries only |
| Special mechanic | Free Cities, Ewiger Landfriede | Meritocracy |

## File Locations

| File | Contents |
|------|----------|
| `eu4sim-core/src/systems/celestial.rs` | Core system logic, defines, reforms |
| `eu4sim-core/src/state.rs` | `CelestialEmpireState`, `CelestialReformId` |
| `eu4sim-core/src/step.rs` | Command implementations |
| `eu4sim-verify/src/hydrate.rs` | Save file hydration |
| `eu4sim-verify/src/parse.rs` | Save file extraction |

## Test Coverage

25 tests in `systems/celestial.rs::tests`:
- Mandate calculation (stability, loans, devastation, tributaries)
- Mandate bounds (0-100 clamping)
- Meritocracy effects (advisor cost, corruption reduction)
- Meritocracy bounds (-100 to 100)
- Reform prerequisites (chains, mutual exclusion)
- Integration tests (tributary development bonus)

## Future Work

Deferred to future phases:
- Unguarded Nomadic Frontier disaster
- Detailed Mandate of Heaven CB mechanics
- Take Mandate peace term scoring
- AI decision-making for Celestial Empire
- Decree effect application (currently stub)
- Prosperity-based mandate bonus

## References

- [EU4 Wiki: Emperor of China](https://eu4.paradoxwikis.com/Emperor_of_China)
- [EU4 Wiki: Mandate of Heaven](https://eu4.paradoxwikis.com/Mandate_of_Heaven)
- Game file: `common/imperial_reforms/01_china.txt`
- Game file: `common/defines.lua`
