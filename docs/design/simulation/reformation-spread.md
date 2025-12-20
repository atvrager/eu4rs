# Reformation Spread System Design

*Last updated: 2025-12-19*
*Design by: Armin Arlert (Opus session)*

## Overview

This document specifies the "Medium" tier religion system upgrade, focusing on the Protestant/Reformed Reformation spread mechanic. The goal is historical flavor without full EU4 complexity.

## Design Goals

1. **Reformation fires circa 1517** - Protestant, then Reformed (~1536)
2. **Centers of Reformation** - Special provinces that actively convert neighbors
3. **Province religion changes** - Spread via adjacency + development weighting
4. **Country religion tracking** - Add missing `religion` field to CountryState
5. **Religious unity** - Affects stability (future: unrest)

## State Changes Required

### 1. CountryState (state.rs)

Add religion field:

```rust
pub struct CountryState {
    // ... existing fields ...

    /// State religion (e.g., "catholic", "protestant", "reformed")
    pub religion: Option<String>,
}
```

### 2. ReformationState (new in state.rs)

```rust
/// Tracks the global state of the Reformation.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ReformationState {
    /// Has the Protestant Reformation fired?
    pub protestant_reformation_fired: bool,

    /// Has the Reformed movement fired?
    pub reformed_reformation_fired: bool,

    /// Active Centers of Reformation: province_id -> religion
    /// Centers convert adjacent provinces of the parent religion
    pub centers_of_reformation: HashMap<ProvinceId, String>,

    /// When each center was created (for expiry after ~100 years)
    pub center_creation_dates: HashMap<ProvinceId, Date>,
}
```

### 3. GlobalState Update (state.rs)

```rust
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct GlobalState {
    pub reformation: ReformationState,
}
```

## Mechanics

### Reformation Firing

| Religion | Trigger Date | Parent Religion | Spawn Region |
|----------|--------------|-----------------|--------------|
| Protestant | 1517.10.31 | Catholic | HRE (Germany) |
| Reformed | 1536.1.1 | Catholic | Switzerland/France |

**Spawn Logic:**
1. Find all Catholic provinces in the target region
2. Sort by development (highest first)
3. Create 3 Centers of Reformation in top-dev provinces
4. Set province religion to the new religion
5. Log the event

### Center of Reformation Behavior

Each center runs monthly and attempts to convert adjacent Catholic provinces:

```
For each center C with religion R:
  For each adjacent province P:
    If P.religion == "catholic":
      roll = random(0.0, 1.0)
      threshold = base_chance * dev_modifier(P)
      If roll < threshold:
        P.religion = R
        log conversion
```

**Parameters:**
- `base_chance`: 0.02 (2% per month per adjacent province)
- `dev_modifier`: `1.0 / (1.0 + province_dev / 10.0)` (higher dev = slower conversion)
- Centers expire after 100 years OR if center province religion changes

### Country Religion

- Countries do NOT automatically convert when provinces flip
- Country conversion is a deliberate action (existing `ConvertCountryReligion` command)
- For minimal implementation: countries keep starting religion unless command issued
- AI strategy for conversion: out of scope for this design

### Religious Unity

```rust
/// Calculate religious unity for a country (0.0 to 1.0)
fn religious_unity(state: &WorldState, tag: &str) -> f32 {
    let country_religion = state.countries.get(tag)?.religion.as_ref()?;

    let owned_provinces: Vec<_> = state.provinces.iter()
        .filter(|(_, p)| p.owner.as_ref() == Some(&tag.to_string()))
        .collect();

    if owned_provinces.is_empty() {
        return 1.0;
    }

    let matching = owned_provinces.iter()
        .filter(|(_, p)| p.religion.as_ref() == Some(country_religion))
        .count();

    matching as f32 / owned_provinces.len() as f32
}
```

**Effects (future):**
- Religious unity < 100% causes stability decay
- Low unity increases unrest (when unrest system exists)
- For now: just calculate and log, no mechanical effects

## System Integration

### New System: `run_reformation_tick`

Location: `eu4sim-core/src/systems/reformation.rs`

```rust
pub fn run_reformation_tick(state: &mut WorldState, adjacency: Option<&AdjacencyGraph>) {
    // 1. Check if reformation should fire
    check_reformation_trigger(state);

    // 2. Process each center of reformation
    process_centers(state, adjacency);

    // 3. Expire old centers (100 years)
    expire_centers(state);
}
```

**Frequency:** Monthly (check `state.date.day == 1`)

### Integration in step.rs

Add to the monthly systems block:

```rust
if new_state.date.day == 1 {
    // ... existing monthly systems ...
    crate::systems::run_reformation_tick(&mut new_state, adjacency);
}
```

## Region Detection

For spawn region detection, use a simplified approach:

**HRE (Germany) provinces** - hardcoded list or provinces with:
- Culture group: germanic
- Or specific province IDs: 50 (Brandenburg), 65 (Saxony), 67 (Thuringia), etc.

**Swiss/French provinces** - hardcoded list:
- Province IDs: 165 (Bern), 166 (Zurich), 196 (Geneva), etc.

For MVP: use province ID lists. Region detection can be improved later.

## Checksum Updates

Add to `WorldState::checksum()`:

```rust
// Reformation state
self.global.reformation.protestant_reformation_fired.hash(&mut hasher);
self.global.reformation.reformed_reformation_fired.hash(&mut hasher);
// Centers sorted by province ID
let mut center_ids: Vec<_> = self.global.reformation.centers_of_reformation.keys().collect();
center_ids.sort();
for &id in center_ids {
    id.hash(&mut hasher);
    self.global.reformation.centers_of_reformation[&id].hash(&mut hasher);
}
```

## Loader Updates

In `eu4sim/src/loader.rs`, load country religion from history:

```rust
// When building CountryState from HistoryCountries:
let country_state = CountryState {
    // ... existing fields ...
    religion: history.religion.clone(),
};
```

## Testing Strategy

1. **Unit tests:**
   - `test_reformation_fires_at_correct_date`
   - `test_center_converts_adjacent_province`
   - `test_center_expires_after_100_years`
   - `test_religious_unity_calculation`

2. **Integration test:**
   - Run simulation from 1444 to 1600
   - Verify Protestant/Reformed religions appear in Europe
   - Verify Catholic provinces decrease over time

## Open Questions (Deferred)

1. **Counter-Reformation**: Centers can be destroyed by Catholic countries? (Skip for MVP)
2. **Peace of Westphalia**: End religious wars mechanic? (Skip)
3. **Defender of the Faith**: Bonus for religious leader? (Skip)
4. **AI conversion strategy**: When should AI countries convert? (Skip - random or never)

## Summary

| Component | File | Changes |
|-----------|------|---------|
| CountryState.religion | state.rs | Add field |
| ReformationState | state.rs | New struct |
| GlobalState.reformation | state.rs | Add field |
| run_reformation_tick | systems/reformation.rs | New file |
| systems/mod.rs | mod.rs | Export new system |
| step.rs | step.rs | Call reformation tick |
| loader.rs | loader.rs | Load country religion |
| checksum | state.rs | Hash reformation state |