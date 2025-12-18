# Property-Based Testing Guide

## What is Property-Based Testing?

Property-based testing (PBT) is a testing methodology where you define **invariants** (properties that should always hold true) rather than specific input-output examples. The testing framework generates hundreds or thousands of randomized test cases to try to violate those invariants.

**Example-based test** (traditional):
```rust
#[test]
fn test_manpower_recovery() {
    let mut state = setup();
    state.manpower = 0;
    state.max_manpower = 11000;

    run_manpower_tick(&mut state);

    assert_eq!(state.manpower, 91.6666); // 11000 / 120
}
```

**Property-based test** (invariant-focused):
```rust
#[test]
fn prop_manpower_never_exceeds_max() {
    proptest!(|(initial_manpower: i32, max_manpower in 1000..100000)| {
        let mut state = setup();
        state.manpower = Fixed::from_int(initial_manpower);
        state.max_manpower = Fixed::from_int(max_manpower);

        run_manpower_tick(&mut state);

        // Property: manpower should NEVER exceed max, regardless of input
        prop_assert!(state.manpower <= state.max_manpower);
    });
}
```

The framework tries **thousands** of combinations: negative manpower, max=0, max < current, etc. It finds edge cases you'd never think to test manually.

---

## Why Use Property-Based Testing for EU4 Simulation?

### 1. **Catch Boundary Bugs**
- What happens when autonomy = 1.5? (negative income)
- What if a province has 0 base tax? (division by zero?)
- What if there are 1000 armies in one province? (performance regression)

PBT systematically explores the input space.

### 2. **Ensure Determinism**
Simulation must be deterministic for netcode and replays. PBT can verify this:

```rust
proptest!(|(seed: u64)| {
    let state1 = run_simulation_from_seed(seed);
    let state2 = run_simulation_from_seed(seed);

    prop_assert_eq!(state1.checksum(), state2.checksum());
});
```

Run this with 1000+ random seeds. If even one fails, you've found a determinism bug.

### 3. **Test Invariants, Not Implementation**
Instead of testing "income = base × 0.2 × price", test:
- "Income is always non-negative"
- "Total wealth in the system can only decrease by expenses or increase by production"
- "No country can have more manpower than their max"

These properties survive refactoring. If you change the formula, the property still holds.

### 4. **Find Exploits**
PBT can uncover game-breaking edge cases:
- Can a player bankrupt the simulation by spamming commands?
- Can combat result in negative regiment strength?
- Can moving an army cause it to teleport due to integer overflow?

---

## Tools: `proptest` vs `quickcheck`

### `proptest` (Recommended)
- **Rust-native**, well-maintained, excellent ergonomics
- Integrates seamlessly with `cargo test`
- Supports shrinking (when a test fails, it minimizes the input to find the simplest failing case)
- Good for custom generators (e.g., generating valid EU4 game states)

**Installation:**
```toml
[dev-dependencies]
proptest = "1.4"
```

### `quickcheck`
- Older, inspired by Haskell's QuickCheck
- Simpler API but less flexible
- Adequate for basic cases, but `proptest` is more powerful

**Verdict:** Use `proptest` for this project.

---

## Example: Testing Combat Invariants

### Property 1: Total Strength Decreases Monotonically

Combat should only destroy units, never create them.

```rust
use proptest::prelude::*;

proptest! {
    #[test]
    fn prop_combat_total_strength_decreases(
        side1_strength in 100..10000u32,
        side2_strength in 100..10000u32,
        days in 1..100usize,
    ) {
        let mut state = WorldStateBuilder::new()
            .with_country("SWE")
            .with_country("DEN")
            .build();

        // Create two armies
        add_army(&mut state, "SWE", 1, side1_strength);
        add_army(&mut state, "DEN", 1, side2_strength);

        // Declare war
        state.diplomacy.declare_war("SWE", "DEN");

        let total_before = total_strength(&state);

        // Run combat for N days
        for _ in 0..days {
            run_combat_tick(&mut state);
        }

        let total_after = total_strength(&state);

        // Invariant: Total strength must decrease (or stay same if no combat)
        prop_assert!(total_after <= total_before);
    }
}
```

**What this finds:**
- Bug where casualties are added instead of subtracted
- Integer overflow causing wraparound
- Regiments being duplicated during combat

### Property 2: No Negative Strength

```rust
proptest! {
    #[test]
    fn prop_combat_no_negative_strength(
        side1_strength in 1..1000u32,
        side2_strength in 1..10000u32, // Asymmetric strengths
    ) {
        let mut state = setup_combat(side1_strength, side2_strength);

        // Run combat for many ticks (might destroy weaker side)
        for _ in 0..100 {
            run_combat_tick(&mut state);
        }

        // Invariant: No regiment can have negative strength
        for army in state.armies.values() {
            for regiment in &army.regiments {
                prop_assert!(regiment.strength >= Fixed::ZERO);
            }
        }
    }
}
```

---

## Example: Testing Economic Invariants

### Property 3: Treasury Conservation (Closed System)

In a closed system (no external income/expenses), total wealth should be conserved.

```rust
proptest! {
    #[test]
    fn prop_treasury_conservation(
        num_countries in 1..10usize,
        initial_treasury in 0..100000i32,
    ) {
        let mut state = setup_world_with_countries(num_countries, initial_treasury);

        let total_before = total_treasury(&state);

        // Run a full month of ticks (but disable income/expense systems)
        for _ in 0..30 {
            run_movement_tick(&mut state); // No money involved
            run_combat_tick(&mut state);   // No money involved
        }

        let total_after = total_treasury(&state);

        // Invariant: Total treasury unchanged (conservation)
        prop_assert_eq!(total_before, total_after);
    }
}
```

### Property 4: Income is Non-Negative

```rust
proptest! {
    #[test]
    fn prop_production_income_non_negative(
        base_production in 0..100u32,
        autonomy in 0.0..1.0f32,
        efficiency in -0.5..2.0f32,
    ) {
        let mut state = setup_province(base_production, autonomy, efficiency);

        let treasury_before = state.countries["SWE"].treasury;

        run_production_tick(&mut state, &EconomyConfig::default());

        let treasury_after = state.countries["SWE"].treasury;

        // Invariant: Production income should never go negative
        prop_assert!(treasury_after >= treasury_before);
    }
}
```

**What this finds:**
- The bug where autonomy > 1.0 causes negative income (flagged in review!)
- Edge case where efficiency = -1.0 exactly causes division by zero
- Floating-point rounding issues that accumulate over time

---

## Example: Testing Movement Invariants

### Property 5: Armies Don't Teleport

```rust
proptest! {
    #[test]
    fn prop_movement_is_continuous(
        path_length in 2..20usize,
    ) {
        let mut state = setup_world_with_path(path_length);
        let army = &state.armies[&1];
        let initial_location = army.location;
        let mut prev_location = initial_location;

        // Run movement for path_length days
        for _ in 0..path_length {
            run_movement_tick(&mut state);
            let current_location = state.armies[&1].location;

            // Invariant: Army must move to an adjacent province (or stay put)
            prop_assert!(
                current_location == prev_location ||
                adjacency_graph.are_adjacent(prev_location, current_location)
            );

            prev_location = current_location;
        }
    }
}
```

---

## Generating Complex Test Data

### Custom Strategies for EU4 States

`proptest` lets you define custom generators for complex types like `WorldState`.

```rust
use proptest::prelude::*;

fn arb_world_state() -> impl Strategy<Value = WorldState> {
    (
        1..10usize,  // num_countries
        1..100usize, // num_provinces
        any::<u64>(), // rng_seed
    ).prop_map(|(num_countries, num_provinces, seed)| {
        let mut state = WorldState::default();
        state.rng_seed = seed;

        // Generate countries
        for i in 0..num_countries {
            state.countries.insert(
                format!("C{}", i),
                CountryState {
                    treasury: Fixed::from_int((seed % 10000) as i32),
                    manpower: Fixed::from_int((seed % 50000) as i32),
                    ..Default::default()
                },
            );
        }

        // Generate provinces
        for i in 0..num_provinces {
            state.provinces.insert(
                i as u32,
                ProvinceState {
                    owner: Some(format!("C{}", i % num_countries)),
                    base_tax: Fixed::from_int((seed % 20) as i32),
                    ..Default::default()
                },
            );
        }

        state
    })
}

proptest! {
    #[test]
    fn prop_any_state_has_valid_checksum(state in arb_world_state()) {
        let checksum1 = state.checksum();
        let checksum2 = state.checksum();

        // Invariant: Checksums are deterministic
        prop_assert_eq!(checksum1, checksum2);
    }
}
```

---

## Integration with Existing Tests

You don't need to replace your example-based tests. Use both:

1. **Example-based tests**: Verify specific known cases (e.g., "Sweden with 12 base tax generates 1.0 monthly income")
2. **Property-based tests**: Verify invariants across all possible inputs

**Directory structure:**
```
eu4sim-core/
├── src/
│   └── systems/
│       ├── production.rs
│       │   ├── Unit tests (examples)
│       │   └── Property tests (invariants)
```

**Example:**
```rust
#[cfg(test)]
mod tests {
    use super::*;

    // Example-based tests
    #[test]
    fn test_production_generates_income() { /* ... */ }

    #[test]
    fn test_efficiency_modifier() { /* ... */ }

    // Property-based tests
    proptest! {
        #[test]
        fn prop_income_never_negative(
            base_production in 0..100u32,
            autonomy in 0.0..1.5f32,
        ) { /* ... */ }
    }
}
```

---

## How This Increases Awesomeness

### Before PBT:
- Write 5 tests, catch 5 bugs
- Edge cases discovered in production
- "It worked in my test!" → "But not with 0 manpower..."

### After PBT:
- Write 1 property, test 1000 cases
- Edge cases discovered at test time
- "This property holds for all valid inputs" → Ship with confidence

### Specific Wins for EU4 Simulation:

1. **Determinism Guarantee**: Run 10,000 random seeds, verify checksum matches every time
2. **No Exploits**: Fuzz the input space to find game-breaking edge cases before players do
3. **Refactor Safely**: Change the formula, properties still validate correctness
4. **Document Intent**: Properties are living documentation ("Treasury can never go below -1000 debt limit")
5. **Catch Regressions**: If a future change violates a property, CI fails immediately

---

## Getting Started

### Step 1: Add `proptest` to `eu4sim-core`

```toml
# eu4sim-core/Cargo.toml
[dev-dependencies]
proptest = "1.4"
```

### Step 2: Write Your First Property Test

Pick a simple invariant from the review TODOs:

```rust
// eu4sim-core/src/systems/manpower.rs

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;

    proptest! {
        #[test]
        fn prop_manpower_never_exceeds_max(
            initial in 0..200000i32,
            max in 1000..100000i32,
        ) {
            let mut state = WorldStateBuilder::new()
                .with_country("SWE")
                .build();

            state.countries.get_mut("SWE").unwrap().manpower = Fixed::from_int(initial);
            // Setup max based on provinces (simplified)

            run_manpower_tick(&mut state);

            let swe = state.countries.get("SWE").unwrap();
            prop_assert!(swe.manpower <= /* calculated max */);
        }
    }
}
```

### Step 3: Run It

```bash
cargo test -p eu4sim-core prop_manpower_never_exceeds_max
```

`proptest` will generate 256 test cases by default. You can increase this:

```rust
proptest! {
    #![proptest_config(ProptestConfig::with_cases(10000))]

    #[test]
    fn prop_exhaustive_test(...) { /* ... */ }
}
```

---

## Recommended Properties to Test

Based on the code review, here are high-value properties to implement:

### High Priority

1. **Manpower never exceeds max** (`manpower.rs`)
2. **Combat total strength decreases monotonically** (`combat.rs`)
3. **No regiment has negative strength** (`combat.rs`)
4. **Treasury change = income - expenses** (conservation law)
5. **Autonomy ∈ [0, 1] doesn't cause negative income** (`production.rs`, `taxation.rs`)

### Medium Priority

6. **Movement is continuous** (no teleportation)
7. **Determinism: Same seed = same result**
8. **Checksums are deterministic** (already partially tested)
9. **Date arithmetic wraps correctly** (month 13 → year+1)

### Low Priority (Nice to Have)

10. **Performance: N provinces scales linearly** (property test for algorithmic complexity)
11. **Serialization round-trip: `WorldState` → JSON → `WorldState` is lossless**

---

## Further Reading

- [Proptest Book](https://altsysrq.github.io/proptest-book/)
- [QuickCheck (Haskell original)](https://www.cse.chalmers.se/~rjmh/QuickCheck/)
- [Hypothesis (Python PBT)](https://hypothesis.readthedocs.io/) - Great examples of shrinking
- [John Hughes - Don't Write Tests](https://www.youtube.com/watch?v=hXnS_Xjwk2Y) - Classic PBT talk

---

## Summary

Property-based testing transforms how you think about correctness:

- **Old way**: "Does this specific input produce this specific output?"
- **New way**: "Does this invariant hold for ALL valid inputs?"

For a deterministic simulation like EU4, PBT is a force multiplier:
- Catches edge cases you'd never manually test
- Provides confidence for refactoring
- Documents intended behavior as executable properties
- Finds bugs before players do

**Next step**: Add `proptest` to `eu4sim-core/Cargo.toml` and convert one of the review TODOs into a property test. See what breaks!
