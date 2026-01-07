# Building Parallel Systems

This document describes patterns for building performant, parallelizable systems in eu4sim. It covers profiling with Tracy, using immutable data structures, and the map-reduce pattern for parallel processing.

## Tracy Profiling

### Setup

Tracy profiling is enabled via the `tracy` feature flag:

```bash
cargo build -p eu4sim --features tracy --release
```

The feature enables:
- `tracing-tracy`: Routes `tracing` spans to Tracy
- `tracy-client`: Memory profiling via `ProfiledAllocator`
- Frame markers for daily/monthly tick boundaries

### Capturing Traces

Use the xtask command:

```bash
# Capture 60 ticks with GP-only AI
cargo xtask profile --ticks 60 --ai gp-only --output profile.tracy

# View in Tracy GUI
cargo xtask tracy profile.tracy
```

### CLI Analysis with tracy-csvexport

For quick analysis without the GUI:

```bash
tracy-csvexport profile.tracy | head -20
```

Output shows span statistics:
```
name,src_file,src_line,total_ns,total_perc,counts,mean_ns,...
movement,systems/movement.rs,117,24608977,0.035,100,246089,...
fleets_parallel{count=0},systems/movement.rs,140,231832,0.000,100,2318,...
```

### Instrumenting Systems

Use the `#[instrument]` attribute for system entry points:

```rust
use tracing::instrument;

#[instrument(skip_all, name = "movement")]
pub fn run_movement_tick(state: &mut WorldState, _graph: Option<&AdjacencyGraph>) {
    // System logic
}
```

Key attributes:
- `skip_all`: Don't capture function parameters (they'd serialize to Tracy)
- `name = "..."`: Custom span name for cleaner Tracy display

For inner spans (parallel phases, subsections):

```rust
let results = {
    let _span = tracing::info_span!("fleets_parallel", count = inputs.len()).entered();
    inputs.into_par_iter().map(process_one).collect()
};
```

The `count = inputs.len()` field shows workload size in Tracy.

### Span Levels

- Use default level (INFO) for important spans you always want to see
- Use `level = "debug"` for detailed spans that add overhead
- Use `level = "trace"` for per-item spans in hot loops

## Immutable Data Structures

### Why `im::HashMap`

The `WorldState` uses `im::HashMap` for collections like `armies` and `fleets`:

```rust
use im::HashMap;

pub struct WorldState {
    pub armies: HashMap<u32, Army>,
    pub fleets: HashMap<u32, Fleet>,
    // ...
}
```

Benefits:
1. **O(1) Clone**: Structural sharing means cloning is near-instant
2. **Safe Parallelism**: Multiple readers can access immutable snapshots
3. **Determinism**: Iteration order is consistent

Trade-offs:
- Slightly slower single-threaded mutation than `std::HashMap`
- No `values_mut()` - must extract, modify, and reinsert

### Efficient Cloning

With `im::HashMap`, cloning `WorldState` went from ~4ms to ~0.4ms per tick:

```rust
// This is cheap - just increments reference counts
let snapshot = state.clone();

// Can now read snapshot while mutating state
```

## Parallel Processing Pattern

### The Problem

Systems often need to:
1. Read state for many entities
2. Compute new values
3. Write back mutations

Direct mutation inside `par_iter` is unsafe and causes data races.

### Solution: Map-Reduce

Extract data → Process in parallel → Apply results sequentially.

```rust
use rayon::prelude::*;

// PHASE 1: Extract data (borrows state immutably)
let inputs: Vec<_> = state.armies
    .iter()
    .filter_map(|(&id, army)| {
        army.movement.as_ref().map(|m| (id, army.location, m.progress, ...))
    })
    .collect();

// PHASE 2: Process in parallel (pure functions, no mutation)
let results: Vec<MovementResult> = {
    let _span = tracing::info_span!("armies_parallel", count = inputs.len()).entered();
    inputs
        .into_par_iter()
        .map(|(id, loc, prog, ...)| process_army_movement(id, loc, prog, ...))
        .collect()
};

// PHASE 3: Apply results (sequential mutation)
for result in results {
    if let Some(army) = state.armies.get_mut(&result.unit_id) {
        army.location = result.new_location;
        // ...
    }
}
```

### Pure Processing Functions

The parallel work should be in pure functions:

```rust
/// Process a single army's movement (pure function, no mutation)
#[instrument(skip_all, name = "army_move")]
fn process_army_movement(
    army_id: u32,
    location: ProvinceId,
    movement_progress: Fixed,
    movement_required: Fixed,
    path_front: Option<ProvinceId>,
    path_next: Option<ProvinceId>,
    path_len: usize,
) -> MovementResult {
    // Pure computation - no side effects
    let new_progress = movement_progress + Fixed::from_int(BASE_SPEED);

    if new_progress >= movement_required {
        // Calculate state changes, return them
        MovementResult {
            unit_id: army_id,
            new_location: Some(next_province),
            // ...
        }
    } else {
        MovementResult {
            unit_id: army_id,
            new_progress,
            // ...
        }
    }
}
```

### Result Structs

Define a struct to carry computed changes:

```rust
struct MovementResult {
    unit_id: u32,
    new_location: Option<ProvinceId>,
    new_previous_location: Option<ProvinceId>,
    new_progress: Fixed,
    path_consumed: bool,
    completed: bool,
    cost_update: Option<(ProvinceId, ProvinceId)>,
}
```

This decouples "what changed" from "apply the change".

### Thread Distribution

Rayon automatically distributes work across its thread pool:
- Work-stealing scheduler balances load
- Each iteration runs on an available thread
- Tracy spans from `#[instrument]` appear on their executing thread

## Memory Profiling

Tracy can track allocations via a custom global allocator:

```rust
// In main.rs
#[cfg(feature = "tracy")]
#[global_allocator]
static ALLOC: tracy_client::ProfiledAllocator<std::alloc::System> =
    tracy_client::ProfiledAllocator::new(std::alloc::System, 100);
```

The second parameter (100) is the callstack depth. Lower values reduce overhead.

## Checklist for Parallelizing a System

1. **Profile first**: Use Tracy to confirm the system is a bottleneck
2. **Identify pure work**: What computation can be done without mutation?
3. **Define result struct**: What changes need to be applied?
4. **Extract inputs**: Collect data from state into a `Vec`
5. **Add tracing span**: Wrap par_iter with `info_span!` showing count
6. **Implement pure function**: Add `#[instrument]` for per-item tracing
7. **Apply results**: Sequential loop to mutate state
8. **Verify correctness**: Run existing tests
9. **Profile again**: Confirm speedup with Tracy

## Example: Movement System

See `eu4sim-core/src/systems/movement.rs` for a complete implementation:

- `run_movement_tick`: Entry point with `#[instrument]`
- `process_fleet_movement`: Pure function for fleet movement
- `process_army_movement`: Pure function for army movement
- `MovementResult`: Struct carrying computed changes
- Phase-based processing: Extract → Parallel → Apply

## SIMD Optimization

### Overview

SIMD (Single Instruction Multiple Data) processes multiple values in one CPU instruction. Modern x86-64 CPUs support:

| ISA | Width | Values per op (i32) | Values per op (i64) |
|-----|-------|---------------------|---------------------|
| SSE4.1 | 128-bit | 4 | 2 |
| AVX2 | 256-bit | 8 | 4 |
| AVX-512 | 512-bit | 16 | 8 |

### Fixed-Point Types for SIMD

We use two fixed-point types with different trade-offs:

| Type | Backing | Range | Precision | SIMD Lanes (AVX2) | Use Case |
|------|---------|-------|-----------|-------------------|----------|
| `Fixed` | i64 | ±922T | 0.0001 | 4 | Treasury, large aggregates |
| `Mod32` | i32 | ±214k | 0.0001 | **8** | Province stats, modifiers |

**Key insight**: `i32 × i32 → i64` fits in AVX2's `_mm256_mul_epi32`, while `i64 × i64 → i128` forces scalar fallback.

```rust
// Mod32: SIMD-friendly multiplication
impl Mul for Mod32 {
    fn mul(self, other: Self) -> Self {
        // i32 * i32 = i64, no i128 needed!
        let wide = self.0 as i64 * other.0 as i64;
        Mod32((wide / SCALE as i64) as i32)
    }
}
```

**Benchmark results** (3000 provinces, 1000 iterations):
- i64 (Fixed): 6.30 ns/province
- i32 (Mod32): 2.71 ns/province
- **Speedup: 2.16x**

### Runtime Dispatch with `multiversion`

The `multiversion` crate generates multiple function versions and selects at runtime:

```rust
use multiversion::multiversion;

#[multiversion(targets(
    "x86_64+avx2+fma",  // Haswell+, Zen1+
    "x86_64+avx2",      // AVX2 without FMA
    "x86_64+sse4.1",    // Nehalem+
))]
pub fn calculate_batch(inputs: &[Input], outputs: &mut [Output]) {
    // Same code, compiled for each target
    for (input, output) in inputs.iter().zip(outputs.iter_mut()) {
        *output = process(input);
    }
}
```

**Verifying dispatch at runtime:**

```rust
#[multiversion(targets("x86_64+avx2+fma", "x86_64+avx2", "x86_64+sse4.1",))]
pub fn selected_target() -> multiversion::target::Target {
    multiversion::target::selected_target!()
}

// In test:
let target = selected_target();
let features: Vec<&str> = target.features().map(|f| f.name()).collect();
println!("Dispatched to: {:?}", features);
// Output: ["avx", "avx2", "fma", ...]
```

### Validation Pattern

**Every SIMD implementation MUST have a scalar golden implementation:**

```rust
/// Scalar reference - source of truth
pub fn process_scalar(input: &Input) -> Output { ... }

/// SIMD batch - must match scalar exactly
#[multiversion(targets(...))]
pub fn process_batch(inputs: &[Input], outputs: &mut [Output]) { ... }

#[cfg(test)]
mod proptests {
    proptest! {
        #[test]
        fn simd_matches_scalar(input in any_input()) {
            let scalar = process_scalar(&input);
            let batch = process_batch(&[input])[0];
            prop_assert_eq!(scalar, batch); // Bit-exact!
        }
    }
}
```

### Autovectorization Limitations

LLVM autovectorization is fragile. It **cannot** vectorize:

| Pattern | Why | Workaround |
|---------|-----|------------|
| i128 operations | No SIMD support for 128-bit ints | Use `Mod32` (i32) instead of `Fixed` (i64) |
| Complex indexing | Can't prove bounds safety | Use iterators, `chunks_exact` |
| Early returns | Branches break vectorization | Hoist conditions outside loop |
| Function calls | Non-inlined calls are barriers | `#[inline(always)]` or LTO |

### Checking Vectorization

```bash
# Install cargo-show-asm
cargo install cargo-show-asm

# View generated assembly
cargo asm --lib eu4sim_core::simd::tax32::calculate_taxes_batch32

# Look for:
#   vmulps, vaddps  → Vectorized (good)
#   vmulss, vaddss  → Scalar (bad)
```

### SIMD Module Structure

See `eu4sim-core/src/simd/`:

```
simd/
├── mod.rs         # SimdFeatures detection, SimdLevel enum
├── tax.rs         # i64 (Fixed) batch - baseline reference
└── tax32.rs       # i32 (Mod32) batch - production SIMD
```

**tax32.rs exports:**
- `TaxInput32` / `TaxOutput32`: Packed batch data (i32 raw values)
- `calculate_tax_scalar32`: Golden reference implementation
- `calculate_taxes_batch32`: Multiversion dispatch (AVX2+FMA, AVX2, SSE4.1)
- `tax32_selected_target`: Runtime dispatch verification

**Benchmark:** `cargo test -p eu4sim-core --release bench_i32_vs_i64 -- --nocapture`

## Hybrid Pattern: Rayon + SIMD

### The Best of Both Worlds

For systems with many entities grouped by owner (countries), combine:
- **Rayon**: Parallel across groups (countries)
- **SIMD**: Vectorized within each group (provinces)

```
┌──────────────────────────────────────────────────┐
│              run_taxation_tick                    │
├──────────────────────────────────────────────────┤
│ Phase 1: Group provinces by owner                │
│          Pre-compute modifiers                   │
├──────────────────────────────────────────────────┤
│ Phase 2: Rayon par_iter over countries           │
│          ├─ FRA: [128 provinces] → SIMD batch    │
│          ├─ ENG: [64 provinces]  → SIMD batch    │
│          └─ ...                                  │
├──────────────────────────────────────────────────┤
│ Phase 3: Apply results to country state          │
└──────────────────────────────────────────────────┘
```

### Implementation Pattern

```rust
use rayon::prelude::*;
use crate::simd::tax32::{calculate_taxes_batch32, TaxInput32, TaxOutput32};

pub fn run_system_tick(state: &mut WorldState) {
    // PHASE 1: Group entities by owner, prepare SIMD inputs
    let mut entities_by_owner: HashMap<Tag, Vec<(EntityId, TaxInput32)>> = HashMap::new();

    for (&id, entity) in state.entities.iter() {
        let Some(owner) = entity.owner.as_ref() else { continue };

        // Pre-compute all modifiers, effective values
        let input = TaxInput32::new(
            entity.base_value,
            get_modifier(state, owner),
            get_local_mod(state, id),
            compute_effective_factor(entity, owner),
        );

        entities_by_owner.entry(owner.clone()).or_default().push((id, input));
    }

    // PHASE 2: Parallel + SIMD computation
    let results: Vec<(Tag, Mod32)> = {
        let _span = tracing::info_span!("system_simd", owners = entities_by_owner.len()).entered();

        entities_by_owner
            .into_par_iter()
            .map(|(tag, entity_data)| {
                // Per-owner tracing span
                let _span = tracing::trace_span!("owner_batch", owner = %tag, count = entity_data.len()).entered();

                // Extract SIMD inputs
                let inputs: Vec<TaxInput32> = entity_data.iter().map(|(_, i)| *i).collect();

                // SIMD batch calculation
                let mut outputs = vec![TaxOutput32::default(); inputs.len()];
                calculate_taxes_batch32(&inputs, &mut outputs);

                // Sum results
                let total: Mod32 = outputs.iter()
                    .map(|o| Mod32::from_raw(o.monthly_income))
                    .fold(Mod32::ZERO, |acc, x| acc + x);

                (tag, total)
            })
            .collect()
    };

    // PHASE 3: Apply to state (sequential)
    for (tag, value) in results {
        if let Some(owner_state) = state.owners.get_mut(&tag) {
            owner_state.accumulated += value.to_fixed();
        }
    }
}
```

### Key Benefits

1. **Scalability**: Rayon distributes ~200 countries across CPU cores
2. **Vectorization**: Each country's provinces processed 8-at-a-time (AVX2)
3. **Cache efficiency**: Provinces grouped by owner have better locality
4. **Observability**: Tracing spans at both country and batch level

### When to Use Hybrid Pattern

Use hybrid rayon+SIMD when:
- You have **grouped entities** (provinces by country, units by army)
- **Batch size per group** is >32 (enough for SIMD benefit)
- The **computation is arithmetic-heavy** (multiplications, divisions)
- You need **per-group tracing** for debugging

Use pure rayon when:
- Entities are independent (no grouping)
- Computation involves complex branching
- I/O or allocation dominates

Use pure SIMD when:
- All entities can be processed in one batch
- No per-entity tracing needed
- Maximum throughput is critical

## Designing SIMD-First Systems

When building new economic/simulation systems, follow this pattern:

### 1. Choose the Right Fixed-Point Type

```rust
// For province-level stats (base_tax, production, manpower)
pub base_tax: Mod32,    // i32, ±214k range, 8 SIMD lanes

// For country-level aggregates (treasury, total income)
pub treasury: Fixed,     // i64, ±922T range, 4 SIMD lanes
```

### 2. Define Packed Input/Output Structs

```rust
#[derive(Debug, Clone, Copy, Default)]
#[repr(C, align(16))]  // Align for efficient SIMD loads
pub struct SystemInput32 {
    pub base_value: i32,     // raw Mod32
    pub modifier_a: i32,
    pub modifier_b: i32,
    pub factor: i32,
}

#[derive(Debug, Clone, Copy, Default)]
#[repr(C)]
pub struct SystemOutput32 {
    pub result: i32,
}
```

### 3. Implement Golden Scalar First

```rust
pub fn calculate_scalar(input: &SystemInput32) -> SystemOutput32 {
    const SCALE: i32 = 10000;
    // ... pure arithmetic with i64 intermediates
}
```

### 4. Add Multiversion Batch Function

```rust
#[multiversion(targets("x86_64+avx2+fma", "x86_64+avx2", "x86_64+sse4.1",))]
pub fn calculate_batch(inputs: &[SystemInput32], outputs: &mut [SystemOutput32]) {
    for (input, output) in inputs.iter().zip(outputs.iter_mut()) {
        // Same logic as scalar - compiler will vectorize
    }
}
```

### 5. Add Proptest Validation

```rust
proptest! {
    #[test]
    fn batch_matches_scalar(input in any_input()) {
        let scalar = calculate_scalar(&input);
        let batch = calculate_batch(&[input])[0];
        prop_assert_eq!(scalar, batch);
    }
}
```

### 6. Integrate with Hybrid Pattern

Use grouping + rayon + SIMD batch as shown above.

---

*Last updated: 2026-01-06*
