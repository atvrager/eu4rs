# Code Review: Evening Sprint (Dec 18-19, 2025)

**Reviewer**: Dr. Johan (Code Review Bot)
**Date**: 2025-12-19
**Commits**: `bc17873..efe0ca8` (16 commits, ~2,500 lines)
**Scope**: Core simulation systems, AI, UI, performance infrastructure

---

## Summary

This is a substantial implementation sprint covering 7 hours of work, adding core simulation systems (mana, war scores, peace deals, AI), performance metrics, and interactive UI controls. The work achieves the mid-term performance goal (1.0 years/sec → 6.3 min for full game) and implements most of the "complete game" simulation loop.

**Code quality**: High—well-tested, documented, and architecturally sound.

**Critical issue**: Float arithmetic in war score calculation breaks determinism and must be fixed before merge.

---

## Blocking Issues 🚫

### 1. Float arithmetic breaks determinism

**Location**: `eu4sim-core/src/systems/war_score.rs:94-95, 101-102`

**Current code**:
```rust
let attacker_occ_score = if defender_total_dev > Fixed::ZERO {
    let ratio = attacker_occupied_dev.div(defender_total_dev);
    (ratio.to_f32() * MAX_OCCUPATION_SCORE as f32).round() as u8  // ❌ Non-deterministic
} else {
    0
};
```

**Problem**: Using `f32` multiplication and rounding introduces non-determinism. Different CPUs/compilers may round differently (especially ARM vs x86), breaking replay and networked multiplayer. This violates the core deterministic simulation invariant.

**Fix**: Keep calculations in `Fixed` throughout:
```rust
let attacker_occ_score = if defender_total_dev > Fixed::ZERO {
    let ratio = attacker_occupied_dev.div(defender_total_dev);
    let score_fixed = ratio.mul(Fixed::from_int(MAX_OCCUPATION_SCORE as i64));
    score_fixed.min(Fixed::from_int(MAX_OCCUPATION_SCORE as i64)).to_int() as u8
} else {
    0
};
```

**Impact**: Current code will cause checksum mismatches across platforms.

**Files to update**:
- `eu4sim-core/src/systems/war_score.rs`: Lines 92-102 (both attacker and defender calculations)

---

### 2. Missing documentation for monthly tick ordering

**Location**: `eu4sim-core/src/step.rs:115-126`

**Current code**:
```rust
if new_state.date.day == 1 {
    crate::systems::run_production_tick(&mut new_state, &economy_config);
    crate::systems::run_taxation_tick(&mut new_state);
    crate::systems::run_manpower_tick(&mut new_state);
    crate::systems::run_expenses_tick(&mut new_state);
    crate::systems::run_mana_tick(&mut new_state);
    crate::systems::recalculate_war_scores(&mut new_state);
    auto_end_stale_wars(&mut new_state);
}
```

**Problem**: No documentation explaining whether this order matters. Dependencies between systems are unclear.

**Fix**: Add comment block explaining ordering rationale:
```rust
// Monthly tick ordering:
// 1. Production → Updates province output values
// 2. Taxation → Collects from updated production
// 3. Manpower → Regenerates military capacity
// 4. Expenses → Deducts costs (uses fresh manpower pool)
// 5. Mana → Generates monarch points
// 6. War scores → Recalculates based on current occupation
// 7. Auto-peace → Ends stalemate wars (10yr timeout)
//
// Order matters for production→taxation. Other systems are independent.
```

**Impact**: Future maintainers may inadvertently break economy by reordering.

---

## Concerns ⚠️

### 3. AI performance dominates tick time (81%)

**Location**: `eu4sim/src/main.rs:282-343`

**Measured**: 2.34ms / 2.87ms total tick time

**Problem**:
1. **Line 295**: Clones entire `CountryState` for every AI call (600+ countries × ~100 bytes)
2. **Lines 304-331**: Enumerates all valid moves (armies × neighbors) every tick
3. **Sequential loop**: No parallelism despite `im::HashMap` being `Send + Sync`

**Optimization paths** (in order of ROI):

**A. Eliminate CountryState clone** (5-10% speedup):
```rust
// Change VisibleWorldState to use reference:
pub struct VisibleWorldState<'a> {
    pub own_country: &'a CountryState,  // Was: CountryState
    // ...
}
```

**B. Parallelize AI loop with rayon** (4-5x speedup):
```rust
use rayon::prelude::*;
let inputs: Vec<_> = ais.par_iter_mut().filter_map(|(tag, ai)| {
    // ... existing logic ...
    Some(PlayerInputs { country: tag.clone(), commands: cmds })
}).collect();
```

**C. Cache available commands** (2-3x speedup on command generation):
- Only regenerate when armies move or diplomatic status changes
- Store `HashMap<Tag, Vec<Command>>` and invalidate on state change

**Priority**: Medium. Performance goal is met (6.3 min < 10 min), but 5x improvement is available for <50 lines of code.

---

### 4. War score scales poorly with conflicts

**Location**: `eu4sim-core/src/systems/war_score.rs:33-48`

**Current approach**: Monthly recalculation iterates all provinces for every war.

**Complexity**: O(wars × provinces) = 20 wars × 3000 provinces = 60,000 reads/month

**Better approach**: Maintain scores incrementally:
```rust
// In step.rs:update_occupation(), when province changes hands:
fn update_occupation(state: &mut WorldState) {
    // ... existing code ...
    if province.controller changed {
        update_war_scores_for_province(state, province_id);  // Only affected wars
    }
}
```

**Impact**: Currently negligible (observer mode has few wars). Critical for late-game with 20+ simultaneous conflicts.

---

### 5. AI RNG seeding reduces test variance

**Location**: `eu4sim/src/main.rs:117`

**Current code**:
```rust
.map(|tag| (tag.clone(), eu4sim_core::ai::RandomAi::new(12345)))
```

**Problem**: All countries initialized with same seed. While each AI maintains independent RNG state (so they diverge), identical seeds mean countries in identical situations make identical first choices. Reduces test coverage.

**Fix**: Hash country tag into seed:
```rust
let base_seed = 12345u64;
let tag_hash: u64 = tag.as_bytes().iter().map(|&b| b as u64).sum();
let seed = base_seed.wrapping_add(tag_hash);
eu4sim_core::ai::RandomAi::new(seed)
```

**Priority**: Low. Current approach is deterministic (correct), just not optimal for diversity.

---

## Suggestions 💡

### 6. Lock Command API with stub variants

**Location**: `eu4sim-core/src/input.rs`

**Rationale**: Design doc (`complete-game-target.md:536-571`) recommends defining all 34 command variants upfront to prevent refactoring when adding networking.

**Current state**: ~10 variants implemented

**Action**: Add stub variants now:
```rust
pub enum Command {
    // IMPLEMENTED
    Move { army_id: u32, destination: u32 },
    DeclareWar { target: String, cb: CasusBelli },
    // ... existing variants ...

    // STUB - Phase 2
    StartColony { province: u32 },
    OfferAlliance { target: String },
    AcceptAlliance { from: String },
    BuyTech { tech_group: TechGroup },
    // ... (add remaining 24 variants) ...
}

impl Command {
    pub fn execute(&self, ...) -> Result<()> {
        match self {
            Command::StartColony { .. } => {
                log::warn!("Colonization not implemented yet");
                Ok(())  // Graceful no-op
            },
            // ...
        }
    }
}
```

**Benefits**:
- API locked early → no networking refactor
- `available_commands()` can return unimplemented commands → AI sees future features
- Game doesn't crash on stub commands

---

### 7. Add property tests for war score invariants

**Location**: `eu4sim-core/src/systems/war_score.rs`

**Missing coverage**:
```rust
#[cfg(test)]
mod proptests {
    use super::*;
    use proptest::prelude::*;

    proptest! {
        #[test]
        fn prop_scores_always_bounded(/* ... */) {
            let mut state = /* ... */;
            recalculate_war_scores(&mut state);

            for war in state.diplomacy.wars.values() {
                prop_assert!(war.attacker_score <= 100);
                prop_assert!(war.defender_score <= 100);
            }
        }

        #[test]
        fn prop_full_occupation_gives_max_score(/* ... */) {
            // If attacker occupies 100% of defender territory:
            // attacker_occupation_score == MAX_OCCUPATION_SCORE
        }

        #[test]
        fn prop_score_monotonic_with_occupation(/* ... */) {
            // More occupied dev → higher score
        }
    }
}
```

**Impact**: Would catch the float rounding bug automatically.

---

### 8. Profile with flame graphs

**Current**: `SimMetrics` shows phase-level times (AI: 2.34ms, Movement: 0.1ms)

**Missing**: Function-level profiling to identify specific allocation hotspots

**Action**: Demonstrate `cargo flamegraph` workflow:
```bash
# Install samply (modern alternative to flamegraph)
cargo install samply

# Profile observer run
samply record cargo run -p eu4sim --release -- --observer --ticks 1000

# Opens interactive flame graph in browser
```

Already mentioned in `docs/development/performance.md:65` but not demonstrated.

---

### 9. Parallelize AI loop (HIGH ROI)

**Impact**: Estimated 4-5x speedup (2.34ms → ~0.5ms on 4-core machine)

**Effort**: Trivial (5 lines of code)

**Code**:
```rust
use rayon::prelude::*;

// Replace: for (tag, ai) in &mut ais
let inputs: Vec<_> = ais.par_iter_mut()
    .filter_map(|(tag, ai)| {
        // ... existing AI logic ...
        if !cmds.is_empty() {
            Some(PlayerInputs { country: tag.clone(), commands: cmds })
        } else {
            None
        }
    })
    .collect();
```

**Why this works**: Migration to `im::HashMap` made `WorldState` `Send + Sync`. Each AI only reads state (no writes), so perfect for parallelism.

**Priority**: Low for mid-term (goal met), High for late-game scaling.

---

## Questions ❓

### 10. Simplified calendar vs. Gregorian

**Location**: `eu4sim-core/src/state.rs:22-46`

**Current**: 30-day months (360-day year)
```rust
while d > 30 {
    d -= 30;
    m += 1;
    // ...
}
```

**Question**: Is this intentional simplification or placeholder?

EU4 uses Gregorian calendar (28-31 day months). Simplified calendar causes:
- Date drift (e.g., "1444.12.31" doesn't exist)
- Confusion when comparing against EU4 save files
- Event triggers may fire on wrong dates

**Impact**: Low for observer mode, Medium for historical accuracy.

---

### 11. Metrics overhead in hot path

**Location**: `eu4sim-core/src/step.rs:94-150`

**Current pattern**:
```rust
let start = Instant::now();
run_system(&mut state);
if let Some(m) = metrics.as_mut() { m.system_time += start.elapsed(); }
```

**Cost**: Each `Instant::now()` is a syscall (~50ns). For 10 systems × 135,000 ticks = ~67ms overhead (~2% of 6.3min run).

**Question**: Is `Option<&mut SimMetrics>` pattern acceptable, or should we use compile-time feature flags?

**Alternative**:
```rust
#[cfg(feature = "metrics")]
let start = Instant::now();

run_system(&mut state);

#[cfg(feature = "metrics")]
m.system_time += start.elapsed();
```

Zero cost when metrics disabled, but clutters code.

---

## Praise 🎉

**Exceptional work on**:

1. **State management migration** (`im::HashMap`): Textbook functional core pattern. O(1) clones unlock trivial multithreading.

2. **Testing discipline**: Every system has unit + property tests. `WorldStateBuilder` pattern is elegant.

3. **Documentation**: `state-management.md` and `performance.md` are publication-quality. Explain *why*, not just *what*.

4. **Performance**: 1.0 years/sec (6.3 min) beats 10-min goal with room to spare. Benchmarking framework enables data-driven optimization.

5. **AI interface**: `AiPlayer` trait with `VisibleWorldState` is forward-thinking. Easy to add fog-of-war later.

6. **UI polish**: Interactive speed control, pause with `Space`, multi-country observation—quality-of-life features that show user empathy.

---

## Verdict

**Needs revision** — Float determinism bug is blocking.

### Required for merge:
- [ ] Fix float arithmetic in `war_score.rs` (use `Fixed` throughout)
- [ ] Document monthly tick system ordering in `step.rs`

### Recommended post-merge:
- [ ] Add property tests for war score invariants
- [ ] Lock Command enum API with stub variants (prevents future refactoring)
- [ ] Profile AI loop allocations with flame graphs

**Estimated fix time**: 30 minutes for blocking issues.

---

**Final note**: This is high-quality work. Architecture decisions (persistent data structures, pluggable AI, metrics framework) are forward-thinking and will pay dividends when scaling to multiplayer. The float bug is the only serious flaw—everything else is optimization or polish.

---

## Files Changed

**Core additions**:
- ✅ `eu4sim-core/src/ai/mod.rs` — RandomAi implementation
- ✅ `eu4sim-core/src/metrics.rs` — Performance measurement
- ✅ `eu4sim-core/src/systems/mana.rs` — Monarch point generation
- ✅ `eu4sim-core/src/systems/war_score.rs` — War score calculation (⚠️ needs float fix)
- ✅ `eu4data/src/terrain.rs` — Terrain data loading

**Architecture docs**:
- ✅ `docs/architecture/state-management.md` — Functional core pattern
- ✅ `docs/development/performance.md` — Benchmarking workflow
- ✅ `docs/design/simulation/complete-game-target.md` — Updated milestones

**UI improvements**:
- ✅ Interactive speed control (1-5 keys)
- ✅ Pause toggle (Space bar)
- ✅ Multi-country observation (--tags)
- ✅ Self-updating status bar

**Test coverage**:
- ✅ Unit tests for all new systems
- ✅ Property tests for mana generation
- ⚠️ Missing property tests for war scores

---

**Commits reviewed**: 16 commits from `bc17873` to `efe0ca8`
