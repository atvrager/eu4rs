# Warfare & Diplomacy Systems Design

**Status**: ✅ Implemented (Tiers 1-3 complete)
**Last updated**: 2025-12-23

## Overview

This document describes the warfare and diplomacy systems implemented to align the EU4RS simulation with authentic EU4 mechanics. The implementation follows a three-tier architecture:

- **Tier 1: Warfare Foundation** - Core combat mechanics (general pips, sieges, ZoC, call-to-arms)
- **Tier 2: Balance Mechanics** - Resource constraints (attrition, river crossings)
- **Tier 3: Advanced Systems** - Complex interactions (naval combat, coalitions, straits)

## Architecture

### System Organization

All warfare/diplomacy systems live in `eu4sim-core/src/systems/`:

```
systems/
├── combat.rs          # Land combat with generals, terrain, morale
├── siege.rs           # Fort sieges with dice roll mechanics
├── attrition.rs       # Supply limits and army attrition
├── naval_combat.rs    # Naval battles and ship durability
├── coalitions.rs      # AE tracking and coalition formation
├── movement.rs        # Army movement with ZoC/strait blocking
└── war_score.rs       # Occupation and battle score calculation
```

### Data Flow

```
┌─────────────────────────────────────────────────────────────────┐
│ step_world() - Monthly Tick Orchestrator                        │
├─────────────────────────────────────────────────────────────────┤
│                                                                 │
│  Daily Ticks (30 per month)                                    │
│  ├─ run_movement_tick()      ← Army/fleet movement            │
│  ├─ run_combat_tick()        ← Land battles (general bonuses) │
│  ├─ run_naval_combat_tick()  ← Naval battles (ship damage)    │
│  └─ run_siege_tick()         ← Siege progress (dice rolls)    │
│                                                                 │
│  Monthly Ticks (end of month)                                  │
│  ├─ run_coalition_tick()     ← AE decay + coalition checks    │
│  ├─ run_attrition_tick()     ← Supply limit enforcement       │
│  ├─ recalculate_war_scores() ← Update occupation scores       │
│  └─ check_auto_peace()       ← 100% war score → peace         │
│                                                                 │
└─────────────────────────────────────────────────────────────────┘
```

## Tier 1: Warfare Foundation

### 1. General Pips in Combat

**Location**: `eu4sim-core/src/systems/combat.rs`

Generals now contribute combat bonuses via their pip values:

```rust
fn get_general_bonus(state: &WorldState, army_ids: &[ArmyId], phase: CombatPhase) -> i8 {
    // Returns highest fire/shock pip among participating armies
    // Added to dice roll (effective_dice = dice + general_bonus, capped at 9)
}

fn get_maneuver_bonus(state: &WorldState, army_ids: &[ArmyId]) -> i8 {
    // Returns highest maneuver pip
    // Negates terrain penalties (up to maneuver value)
}
```

**Integration**:
- Fire/shock pips: Added to damage calculation via `calculate_side_damage()`
- Maneuver pips: Reduce terrain penalties in `get_terrain_penalty()`
- Siege pips: Bonus to siege progress in `calculate_siege_bonus()`

**Testing**: 3 tests verify general bonuses (`test_general_fire_pip_bonus`, `test_maneuver_negates_terrain`, `test_maneuver_partial_negation`)

---

### 2. Siege System

**Location**: `eu4sim-core/src/systems/siege.rs`
**Defines**: `eu4data/src/defines.rs::siege`

Implements EU4-authentic siege mechanics:

#### Occupation Rules

- **Unfortified provinces** (`fort_level == 0`): Instant occupation
- **Fortified provinces** (`fort_level > 0`): Dice roll system

#### Siege Dice Roll System

```rust
// Every 30 days (SIEGE_PHASE_DAYS):
let roll = 1d14;  // Roll 1-14
let total = roll + progress_modifier + bonuses - fort_level;

if total >= 20 {
    // Siege wins, province occupied
    complete_siege(state, province_id);
} else {
    // Increase progress for next phase
    progress_modifier = (progress_modifier + 1).min(12);
}
```

#### Bonuses

| Bonus Type | Value | Source |
|------------|-------|--------|
| Artillery | +5 max | 1 per artillery regiment (capped) |
| General Siege Pips | +1 per pip | Best general's siege stat |
| Blockade | +1 | Coastal province + enemy fleet in adjacent sea |

#### Special Rolls

- **Roll 1**: Disease outbreak (attacker casualties)
- **Roll 14**: Wall breach (enables assault, not yet implemented)

#### State Structures

```rust
pub struct Siege {
    pub province: ProvinceId,
    pub attacker: Tag,
    pub besieging_armies: Vec<ArmyId>,
    pub fort_level: u8,              // 1-8 in EU4
    pub garrison: u32,               // fort_level * 1000
    pub progress_modifier: i32,      // 0-12, increases each failed phase
    pub days_in_phase: u32,          // Days since last dice roll
    pub is_blockaded: bool,          // Adjacent sea controlled by enemy
    pub breached: bool,              // Roll 14 occurred (for future assault feature)
}
```

**Integration**:
- `start_occupation()` called when army enters enemy province
- `run_siege_tick()` called daily in step cycle
- Province controller only changes on siege completion (no instant capture)

**Testing**: 5 tests covering siege progression, artillery bonus, general bonus, instant occupation, mothballed forts

---

### 3. Zone of Control (ZoC)

**Location**: `eu4sim-core/src/state.rs::is_blocked_by_zoc()`

Forts project ZoC to adjacent provinces, blocking enemy movement.

#### Rules

```rust
// Movement from A → B blocked if:
// 1. An adjacent province C has an enemy fort (fort_level > 0)
// 2. Both A and B are adjacent to C
// 3. The fort is not mothballed
// 4. Countries are at war

// Exception: Direct movement to fort province C is always allowed (to siege it)
```

#### Implementation

```rust
impl WorldState {
    pub fn is_blocked_by_zoc(
        &self,
        from: ProvinceId,
        to: ProvinceId,
        mover: &str,
        adjacency: Option<&AdjacencyGraph>,
    ) -> bool {
        // Check all provinces adjacent to `from`
        for neighbor in adjacency.neighbors(from) {
            if neighbor == to { continue; } // Direct fort attack allowed

            if has_enemy_fort(neighbor) && adjacency.are_adjacent(neighbor, to) {
                return true; // Blocked by fort at `neighbor`
            }
        }
        false
    }
}
```

**Integration**:
- Movement validation in `available_commands()` - filters ZoC-blocked moves
- Movement execution in `execute_command()` - returns error if ZoC-blocked

**Testing**: 4 tests verify blocking, mothballed forts, war requirement, command filtering

---

### 4. Call-to-Arms & Alliance Enforcement

**Location**: `eu4sim-core/src/systems/alliance.rs`, `eu4sim-core/src/step.rs`
**State**: `eu4sim-core/src/state.rs::CountryState::pending_call_to_arms`, `DiplomacyState::trust`

Complete EU4-authentic alliance enforcement system with trust mechanics, acceptance logic, and decline penalties.

#### Mechanics

```rust
// On war declaration:
fn call_allies_to_war(state: &mut WorldState, war_id: WarId, declarer: &Tag, is_attacker: bool) {
    for ally in get_allies(declarer) {
        if is_attacker {
            // Offensive war: ally gets choice (pending CtA)
            add_pending_call_to_arms(ally, war_id);
        } else {
            // Defensive war: ally auto-joins
            join_war_side(ally, war_id, WarSide::Defender);
        }
    }
}
```

#### Trust System

Bilateral trust tracking (0-100 scale) affects diplomatic decisions:

```rust
// Trust stored in DiplomacyState
pub trust: HashMap<(Tag, Tag), Fixed>

// Trust modifiers:
// - Accept CtA: +5 trust with caller
// - Decline CtA: -10 trust with ALL allies
```

#### AI Acceptance Scoring

```rust
pub fn calculate_cta_acceptance_score(
    state: &WorldState,
    ally: &Tag,
    caller: &Tag,
    war_id: WarId,
) -> i32 {
    // Trust factor (±50 points swing):
    //   Above 50: +0.5 per point
    //   Below 50: -2 per point

    // Debt penalty: -1000 if loans > 0 or treasury < 0

    // Stability penalty: -50 per missing point below 0
}
```

#### Commands

```rust
pub enum Command {
    CallAllyToWar { ally: Tag, war_id: WarId },    // Request ally join
    JoinWar { war_id: WarId, side: WarSide },      // Accept CtA
    DeclineCallToArms { war_id: WarId },           // Decline CtA
}
```

#### Decline Penalties

Per EU4 wiki mechanics:
- **-25 prestige** (significant diplomatic cost)
- **Alliance breaks** with the caller
- **-10 trust** with ALL allies (reputation damage)
- **Pending CtA removed** from queue

#### Accept Bonuses

- **+5 trust** with the caller (strengthens relationship)
- **Join war** on the specified side
- **Pending CtA removed** from queue

#### Conflict Detection

```rust
pub fn would_create_conflicting_war(state: &WorldState, ally: &Tag, war_id: WarId) -> bool {
    // Returns true if:
    // - Ally is already at war with any participant
    // - Ally is allied to anyone on the opposing side
}
```

**Integration**:
- Defensive allies join automatically in `declare_war_command()`
- Offensive allies get pending CtA in `pending_call_to_arms`
- AI can accept via `JoinWar` or decline via `DeclineCallToArms`
- GreedyBot scores both options based on country situation (debt, manpower, trust)

**Testing**: 14 tests verify defensive auto-join, offensive pending, multi-ally, command availability, cleanup, decline penalties, accept bonuses, trust mechanics, conflict detection, and AI scoring

---

## Tier 2: Balance Mechanics

### 5. Attrition System

**Location**: `eu4sim-core/src/systems/attrition.rs`
**Defines**: `eu4data/src/defines.rs::attrition`

Supply limits prevent infinite doom-stacks.

#### Supply Limit Calculation

```rust
fn calculate_supply_limit(state: &WorldState, province_id: ProvinceId) -> u32 {
    let dev = province.base_tax + province.base_production + province.base_manpower;
    (dev.to_f32() * 1.0) as u32  // 1 regiment per 1 development
}
```

#### Attrition Rate

```rust
if total_regiments > supply_limit {
    let over_limit_ratio = (total_regiments - supply_limit) / supply_limit;
    let attrition_percent = 1.0 + (over_limit_ratio * 5.0);  // Base 1% + 5x multiplier

    // Additional bonuses:
    // +1% hostile territory (enemy-owned province)
    // +2% winter (December, January, February)
}
```

#### Exemptions

- Armies in battle (`in_battle.is_some()`)
- Embarked armies (`embarked_on.is_some()`)

**Integration**:
- `run_attrition_tick()` called monthly (end of month)
- Groups armies by province for shared supply check
- Reduces regiment `strength` (morale unaffected)

**Testing**: 6 tests verify supply calculation, over-limit attrition, hostile territory, winter, embarked exemption

---

### 6. River Crossing Penalty

**Location**: `eu4data/src/adjacency.rs`, `eu4sim-core/src/systems/combat.rs`

Rivers apply -1 dice penalty to attackers.

#### Data Source

Parsed from `map/adjacencies.csv`:

```csv
From;To;Type;Through;...
123;124;river;-1;...
```

#### Storage

```rust
pub struct AdjacencyGraph {
    #[serde(skip)]
    pub river_crossings: HashSet<(ProvinceId, ProvinceId)>,  // Bidirectional
}
```

#### Combat Application

```rust
// In calculate_side_damage():
if is_attacker {
    if let Some(origin) = battle.attacker_origin {
        if adjacency.is_river_crossing(origin, battle.province) {
            terrain_mod -= 1;  // -1 dice penalty
        }
    }
}
```

**Integration**:
- River crossings detected during `AdjacencyGraph::generate()`
- Battle tracks `attacker_origin` to check crossing
- Penalty applied in combat damage calculation

**Testing**: 1 test verifies river crossing penalty (`test_river_crossing_penalty`)

---

## Tier 3: Advanced Systems

### 7. Naval Combat & Blockades

**Location**: `eu4sim-core/src/systems/naval_combat.rs`
**Defines**: `eu4data/src/defines.rs::naval`

Full naval battle system with ship types and durability.

#### Ship Types

```rust
pub enum ShipType {
    HeavyShip,   // Best combat, high hull (100), expensive
    LightShip,   // Trade/scouting, low hull (30), weak combat
    Galley,      // Inland seas, medium hull (50), cheap
    Transport,   // Troop transport, low hull (30), no combat value
}

pub struct Ship {
    pub type_: ShipType,
    pub hull: Fixed,        // Current hull (0-max)
    pub durability: Fixed,  // Combat health (depletes during battle)
}
```

#### Combat Mechanics

```rust
// Phase cycle (3 days per phase)
fn run_naval_combat_tick(state: &mut WorldState) {
    for battle in state.naval_battles.values_mut() {
        battle.phase_day += 1;

        if battle.phase_day >= DAYS_PER_PHASE {
            // Roll dice, switch phase
            roll_dice(battle);
            battle.phase = battle.phase.next();
            battle.phase_day = 0;

            // Deal damage to ships
            apply_damage(state, battle);

            // Check for battle end (one side has no ships)
            check_battle_end(state, battle);
        }
    }
}
```

#### Damage Model

```rust
// Each phase:
1. Roll 1d10 for each side
2. Calculate damage = dice_factor * ship_count * DAMAGE_PER_SHIP
3. Distribute damage across enemy ships (durability)
4. Ships sink when durability <= 0
5. Battle ends when one side has 0 ships
```

#### Admiral Bonuses

```rust
fn get_admiral_bonus(state: &WorldState, fleet_ids: &[FleetId], phase: CombatPhase) -> i8 {
    // Returns highest fire/shock pip among participating admirals
    // Added to dice roll (same as land combat generals)
}
```

#### Coastal Blockades

```rust
fn is_province_blockaded(state: &WorldState, province_id: ProvinceId, defender: &Tag) -> bool {
    // Province must be coastal (adjacent to sea zones)
    // All adjacent sea zones must have enemy fleets
    // Returns true if fully blockaded
}
```

**Integration**:
- `run_naval_combat_tick()` called daily
- Battle detection in `detect_naval_battles()` for fleets in same sea zone at war
- Blockade detection used in siege system for +1 bonus and garrison starvation
- Fleet state updated with battle results (ships sunk, victories)

**Testing**: 9 tests verify battle detection, damage application, admiral bonuses, ship sinking, battle end conditions

---

### 8. Coalitions

**Location**: `eu4sim-core/src/systems/coalitions.rs`
**State**: `eu4sim-core/src/state.rs::{CountryState::aggressive_expansion, DiplomacyState::coalitions}`

Aggressive Expansion (AE) tracking and coalition formation.

#### AE Mechanics

```rust
// On peace deal (provinces taken):
fn apply_aggressive_expansion(state: &mut WorldState, conqueror: &str, provinces: &[ProvinceId]) {
    let total_dev = provinces.iter()
        .map(|p| province_development(p))
        .sum();

    let ae_per_country = total_dev * 1.0;  // 1 AE per 1 dev

    // Apply to all countries (except conqueror)
    for country in state.countries.iter_mut() {
        country.aggressive_expansion[conqueror] += ae_per_country;
    }
}
```

#### Coalition Formation

```rust
const COALITION_THRESHOLD: f32 = 50.0;
const MIN_COALITION_MEMBERS: usize = 4;

fn check_coalition_formation(state: &mut WorldState) {
    for (target, _) in state.countries {
        let angry_countries: Vec<Tag> = state.countries.iter()
            .filter(|(tag, c)| {
                c.aggressive_expansion[target] > COALITION_THRESHOLD
            })
            .collect();

        if angry_countries.len() >= MIN_COALITION_MEMBERS {
            form_coalition(state, target, angry_countries);
        }
    }
}
```

#### AE Decay

```rust
const AE_DECAY_PER_YEAR: f32 = 2.0;
const AE_DECAY_PER_MONTH: f32 = 2.0 / 12.0;  // ~0.167

fn decay_aggressive_expansion(state: &mut WorldState) {
    for country in state.countries.values_mut() {
        for (_, ae) in country.aggressive_expansion.iter_mut() {
            *ae = (*ae - AE_DECAY_PER_MONTH).max(0.0);
        }

        // Remove zero entries
        country.aggressive_expansion.retain(|_, ae| *ae > 0.0);
    }
}
```

#### Coalition State

```rust
pub struct Coalition {
    pub target: Tag,
    pub members: Vec<Tag>,
    pub formed_date: Date,
}

// Stored in DiplomacyState:
pub coalitions: HashMap<Tag, Coalition>,  // Keyed by target
```

#### Coalition Maintenance

```rust
fn update_existing_coalitions(state: &mut WorldState) {
    for coalition in state.diplomacy.coalitions.values_mut() {
        // Remove members who dropped below threshold
        coalition.members.retain(|member| {
            state.countries[member].aggressive_expansion[&coalition.target] > COALITION_THRESHOLD
        });

        // Dissolve if below minimum size
        if coalition.members.len() < MIN_COALITION_MEMBERS {
            mark_for_removal(coalition);
        }
    }
}
```

**Integration**:
- `run_coalition_tick()` called monthly
- AE applied in `execute_peace_terms()` for both `TakeProvinces` and `FullAnnexation`
- Coalition wars (special CB) not yet implemented
- AI coalition behavior not yet implemented

**Testing**: 4 tests verify AE decay, coalition formation, minimum member requirement, dissolution

---

### 9. Strait Blocking

**Location**: `eu4data/src/adjacency.rs`, `eu4sim-core/src/state.rs::is_strait_blocked()`

Enemy fleets block land movement across straits.

#### Data Source

Parsed from `map/adjacencies.csv`:

```csv
From;To;Type;Through;...
1;3;sea;2;...    # Strait from 1→3 through sea zone 2
```

#### Storage

```rust
pub struct AdjacencyGraph {
    #[serde(skip)]  // Can't serialize HashMap with tuple keys
    pub straits: HashMap<(ProvinceId, ProvinceId), ProvinceId>,  // (from, to) → sea_zone
}

// Bidirectional tracking:
graph.straits.insert((1, 3), 2);
graph.straits.insert((3, 1), 2);
```

#### Blocking Logic

```rust
impl WorldState {
    pub fn is_strait_blocked(
        &self,
        from: ProvinceId,
        to: ProvinceId,
        mover: &str,
        adjacency: Option<&AdjacencyGraph>,
    ) -> bool {
        // 1. Check if movement crosses a strait
        let sea_zone = adjacency.get_strait_sea_zone(from, to)?;

        // 2. Check if any enemy fleet is in the sea zone
        for fleet in self.fleets.values() {
            if fleet.location == sea_zone
                && self.diplomacy.are_at_war(mover, &fleet.owner) {
                return true;  // Blocked
            }
        }

        false  // Not blocked
    }
}
```

#### Rules

- Only blocks during wartime (`are_at_war()` check)
- Allied fleets don't block
- Neutral fleets don't block
- Both crossing directions blocked (bidirectional straits)

**Integration**:
- Strait detection in `AdjacencyGraph::generate()` during adjacency parsing
- Movement validation in `available_commands()` - filters strait-blocked moves
- Movement execution in `execute_command()` - returns error if strait-blocked

**Testing**: 5 tests verify enemy blocking, no-fleet passage, allied exemption, peacetime exemption, command filtering

---

## Testing Summary

### Coverage by Tier

| Tier | Feature | Tests | Lines of Code |
|------|---------|-------|---------------|
| **T1** | General Pips | 3 | ~150 |
| **T1** | Sieges | 5 | ~450 |
| **T1** | Zone of Control | 4 | ~100 |
| **T1** | Call-to-Arms & Alliance Enforcement | 14 | ~400 |
| **T2** | Attrition | 6 | ~250 |
| **T2** | River Crossings | 1 | ~50 |
| **T3** | Naval Combat | 9 | ~800 |
| **T3** | Coalitions | 4 | ~250 |
| **T3** | Strait Blocking | 5 | ~150 |
| **Total** | **9 Features** | **42 tests** | **~2,400 LoC** |

### Integration Tests

All systems integrate through `step_world()` monthly tick:

```rust
pub fn step_world(state: &mut WorldState, adjacency: Option<&AdjacencyGraph>) -> WorldState {
    // Daily ticks (30 iterations)
    for _ in 0..30 {
        run_movement_tick(&mut new_state);
        run_combat_tick(&mut new_state);
        run_naval_combat_tick(&mut new_state);
        run_siege_tick(&mut new_state);
    }

    // Monthly ticks
    run_coalition_tick(&mut new_state);
    run_attrition_tick(&mut new_state);
    recalculate_war_scores(&mut new_state);
    check_auto_peace(&mut new_state);

    new_state
}
```

**Full test suite**: 345 tests across workspace, all passing ✅

---

## Performance Considerations

### Daily vs Monthly Ticks

**Daily** (run 30 times per month):
- Combat phases (3-day cycles)
- Naval combat phases (3-day cycles)
- Siege phases (30-day cycles)
- Movement progress

**Monthly** (run once per month):
- AE decay
- Coalition formation
- Attrition application
- War score recalculation

### Optimization Opportunities

1. **Spatial Partitioning**: Group armies/fleets by region for battle detection
2. **Coalition Caching**: Only recalculate when AE changes significantly
3. **Siege Batch Updates**: Process all sieges together instead of individually

---

## Future Enhancements

### Not Yet Implemented

1. **Assault Mechanics**: When siege breached (roll 14), option to assault fort
2. **Coalition Wars**: Special CB for coalitions to declare war together
3. **Naval Attrition**: Ships lose durability in hostile waters
4. **Fleet Morale**: Similar to army morale for naval battles
5. **Trade Interdiction**: Blockades reduce trade income
6. **War Exhaustion**: Accumulates during war, affects morale/unrest

### Potential Extensions

1. **Dynamic War Goals**: AI-selected war goals beyond conquest
2. **White Peace Conditions**: Negotiated peace before 100% war score
3. **Separate Peace**: Coalition members peace out individually
4. **Fort Maintenance**: Mothballing reduces cost but weakens fort

---

## References

- **EU4 Wiki - Land Warfare**: https://eu4.paradoxwikis.com/Land_warfare
- **EU4 Wiki - Naval Warfare**: https://eu4.paradoxwikis.com/Naval_warfare
- **Codebase**: `eu4sim-core/src/systems/`
