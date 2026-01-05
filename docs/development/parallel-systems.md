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

---

*Last updated: 2026-01-05*
