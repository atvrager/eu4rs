# Task Aggregation Strategies

**Status**: Design Phase
**Last Updated**: 2026-01-07
**Related**: [task-based-parallelism.md](task-based-parallelism.md), [performance.md](performance.md)

## Problem Statement

When tasks run in parallel and produce results, we need to collect and aggregate those results back into the main `WorldState`. The aggregation strategy determines:

1. **Performance**: How much overhead does result collection add?
2. **Correctness**: How do we prevent race conditions?
3. **Simplicity**: How easy is it to write and maintain?

---

## Current Aggregation Patterns (Analysis)

### Pattern 1: HashMap Accumulation (Taxation)

**File**: `eu4sim-core/src/systems/taxation.rs:96-140`

```rust
// Phase 2: SIMD batch processing (sequential)
let outputs: Vec<TaxOutput32> = calculate_taxes_batch32(&inputs);

// Phase 3: Aggregate by owner (sequential HashMap)
let mut country_totals: HashMap<TagId, Mod32> = HashMap::new();
for ((owner_id, _, _), output) in province_data.iter().zip(outputs.iter()) {
    *country_totals.entry(*owner_id).or_insert(Mod32::ZERO) +=
        Mod32::from_raw(output.monthly_income);
}

// Phase 4: Apply to state (sequential)
for (owner_id, total_tax) in country_totals {
    state.countries[tag].treasury += total_tax.to_fixed();
}
```

**Characteristics**:
- **Aggregation key**: `TagId` (u16, fast hash)
- **Aggregation operation**: `+= value` (associative, commutative)
- **Intermediate storage**: `HashMap<TagId, Mod32>`
- **Thread safety**: None needed (sequential)
- **Overhead**: ~2-5µs for ~2450 provinces → ~20 countries

**Why Sequential Works**:
- HashMap iteration is memory-bound (not CPU-bound)
- ~2450 items × (hash + insert) = ~5µs total
- Parallel overhead (atomic CAS, cache bouncing) would exceed 5µs

---

### Pattern 2: Vec Collection + Indexed Apply (Movement)

**File**: `eu4sim-core/src/systems/movement.rs:118-292`

```rust
// Phase 1: Extract with IDs
let fleet_inputs: Vec<(FleetId, LocationData)> = state.fleets.iter()
    .map(|(&id, fleet)| (id, extract_data(fleet)))
    .collect();

// Phase 2: Parallel processing (Rayon)
let results: Vec<MovementResult> = fleet_inputs
    .into_par_iter()
    .map(|(id, data)| MovementResult {
        unit_id: id,
        new_location: compute_new_location(data),
        new_progress: compute_progress(data),
        completed: check_completion(data),
    })
    .collect();

// Phase 3: Apply by ID (sequential)
for result in results {
    if let Some(fleet) = state.fleets.get_mut(&result.unit_id) {
        fleet.location = result.new_location;
        fleet.movement.progress = result.new_progress;
        if result.completed {
            fleet.movement = None;
        }
    }
}
```

**Characteristics**:
- **Aggregation key**: `unit_id` (carried in result struct)
- **Aggregation operation**: Direct replacement (not accumulation)
- **Intermediate storage**: `Vec<MovementResult>`
- **Thread safety**: Rayon's `par_iter().collect()` handles it
- **Overhead**: ~10-20µs for ~1000 units

**Why Parallel Works**:
- Work per unit is high (~100µs: path planning, speed calculation)
- Overhead ratio: 20µs / (1000 × 100µs) = 0.02% overhead
- Collection is lock-free (Rayon's Vec collector uses thread-local buffers)

---

### Pattern 3: Pre-Grouping + Direct Mutation (Combat)

**File**: `eu4sim-core/src/systems/combat.rs:124-450`

```rust
// Phase 1: Group by province (sequential HashMap)
let mut province_armies: HashMap<ProvinceId, Vec<ArmyId>> = HashMap::new();
for (&army_id, army) in &state.armies {
    province_armies.entry(army.location).or_default().push(army_id);
}

// Phase 2: Process battles (sequential)
for (&province_id, army_ids) in &province_armies {
    if should_create_battle(army_ids) {
        let battle_id = create_battle(state, army_ids);
        let damage = calculate_damage(state, battle_id);
        apply_damage(state, battle_id, damage); // Direct mutation
    }
}
```

**Characteristics**:
- **Aggregation key**: `ProvinceId` (grouping phase)
- **Aggregation operation**: No aggregation (direct mutation during compute)
- **Intermediate storage**: `HashMap<ProvinceId, Vec<ArmyId>>`
- **Thread safety**: None (fully sequential)
- **Overhead**: ~5µs grouping + 0µs aggregation (no separate phase)

**Why Sequential Works**:
- Combat has complex dependencies (casualties affect retreat, which affects positioning)
- Battles are infrequent (~10-50 active battles in typical game)
- Grouping overhead is negligible compared to combat computation

---

## Aggregation Strategies for Task-Based System

### Strategy A: Thread-Local Accumulators (Recommended)

**Concept**: Each worker thread maintains a thread-local result buffer. After all tasks complete, merge thread-local buffers sequentially.

```rust
pub struct TaskContext {
    /// Thread-local accumulator for this worker
    accumulator: &mut ThreadLocalResults,

    /// Other fields...
    world: &WorldState,
    partition: Partition,
}

/// Per-thread result storage
pub struct ThreadLocalResults {
    /// Accumulated values by key
    country_values: HashMap<TagId, Mod32>,

    /// List of entity updates
    unit_updates: Vec<UnitUpdate>,

    /// Batch events
    events: Vec<GameEvent>,
}

impl TaskContext {
    /// Accumulate a result (zero allocation, fast path)
    pub fn accumulate_country_value(&mut self, tag: TagId, value: Mod32) {
        *self.accumulator.country_values.entry(tag).or_insert(Mod32::ZERO) += value;
    }

    /// Record an entity update
    pub fn push_update(&mut self, update: UnitUpdate) {
        self.accumulator.unit_updates.push(update);
    }
}

/// Final aggregation (sequential)
impl TaskScheduler {
    pub fn collect_results(&self) -> AggregatedResults {
        let mut final_results = AggregatedResults::default();

        // Merge thread-local accumulators
        for worker in &self.workers {
            for (tag, value) in worker.results.country_values.drain() {
                *final_results.country_values.entry(tag).or_insert(Mod32::ZERO) += value;
            }

            final_results.unit_updates.extend(worker.results.unit_updates.drain(..));
        }

        final_results
    }
}
```

**Example: Taxation with Thread-Local Accumulation**

```rust
fn run_taxation_tick(ctx: &TaskContext) {
    let cache = &ctx.world.owned_provinces_cache;
    let chunk_size = 256;

    // Spawn chunk tasks
    for chunk_idx in 0..(cache.len() / chunk_size) {
        let range = chunk_idx * chunk_size..(chunk_idx + 1) * chunk_size;
        ctx.spawn(Task::new(
            taxation_chunk_task,
            ChunkData { cache, range },
            "taxation_chunk",
        ));
    }

    // Wait for all chunks (threads accumulate into their local buffers)
    ctx.wait();

    // Results are now in thread-local accumulators (automatic)
}

fn taxation_chunk_task(ctx: &mut TaskContext) {
    let data = unsafe { &*(ctx.data as *const ChunkData) };

    // Compute taxes for this chunk (SIMD)
    let taxes = calculate_taxes_batch32(data.cache.slice(data.range));

    // Accumulate into thread-local buffer (no locks!)
    for ((owner_id, _), output) in data.cache[data.range].iter().zip(taxes.iter()) {
        ctx.accumulate_country_value(*owner_id, Mod32::from_raw(output.monthly_income));
    }
}
```

**Advantages**:
- **Zero contention**: Each thread writes to its own HashMap
- **Cache-friendly**: Thread-local data stays in L1/L2 cache
- **Sequential merge**: Final aggregation is sequential (proven fast pattern)
- **Flexible**: Supports both accumulation (HashMap) and collection (Vec)

**Disadvantages**:
- **Memory overhead**: N workers × M countries = N HashMaps
- **Merge cost**: Must iterate all thread-local buffers

**Performance Analysis**:

| Metric | Value | Notes |
|--------|-------|-------|
| Thread-local accumulation | ~50ns/op | HashMap insert into hot cache |
| Per-chunk overhead | ~50ns × 256 = ~13µs | Negligible vs. 200µs SIMD compute |
| Final merge | ~5µs × 8 workers = ~40µs | Linear in number of workers |
| **Total overhead** | **~40µs** | <5% of 1ms monthly system budget |

---

### Strategy B: Lock-Free Concurrent HashMap

**Concept**: Use atomic operations to accumulate results into a shared concurrent HashMap (e.g., `dashmap`).

```rust
use dashmap::DashMap;

pub struct TaskScheduler {
    /// Shared concurrent result storage
    results: DashMap<TagId, AtomicI64>,
}

fn taxation_chunk_task(ctx: &mut TaskContext) {
    let taxes = calculate_taxes_batch32(data);

    for ((owner_id, _), output) in data.iter().zip(taxes.iter()) {
        // Atomic accumulation (lock-free but has CAS overhead)
        ctx.scheduler.results
            .entry(*owner_id)
            .or_insert(AtomicI64::new(0))
            .fetch_add(output.monthly_income as i64, Ordering::Relaxed);
    }
}
```

**Advantages**:
- **Simple API**: Single shared structure (no merge phase)
- **No duplication**: Only one result per key
- **Proven library**: `dashmap` is battle-tested

**Disadvantages**:
- **CAS contention**: Multiple threads updating same key causes cache bouncing
- **Atomic overhead**: `fetch_add` is ~50-100× slower than regular add
- **Memory ordering**: Relaxed ordering OK for commutative ops, but adds complexity

**Performance Analysis**:

| Metric | Value | Notes |
|--------|-------|-------|
| Atomic fetch_add | ~20-50ns | Uncontended case |
| Cache miss penalty | ~100-200ns | If another thread touched this cache line |
| Per-chunk overhead | 50ns × 256 = ~13µs | Comparable to thread-local |
| **Worst case** | **200ns × 256 = ~51µs** | High contention (multiple workers, same countries) |

**When to Use**:
- High contention scenarios (many workers, few keys)
- Non-associative operations (can't merge thread-local buffers)
- Real-time systems (need lock-free guarantees)

---

### Strategy C: Pre-Allocated Result Slots

**Concept**: Pre-allocate result slots per task, no aggregation needed.

```rust
pub struct TaskBatch {
    /// Pre-allocated output buffer (one slot per task)
    outputs: Vec<TaxOutput32>,

    /// Mapping of output slot → country
    slot_to_country: Vec<TagId>,
}

fn run_taxation_tick(ctx: &TaskContext) {
    let cache = &ctx.world.owned_provinces_cache;

    // Pre-allocate output buffer
    let mut outputs = vec![TaxOutput32::default(); cache.len()];

    // Spawn chunk tasks (write to pre-allocated slots)
    for chunk_idx in 0..(cache.len() / 256) {
        let range = chunk_idx * 256..(chunk_idx + 1) * 256;
        ctx.spawn(Task::new_with_output(
            taxation_chunk_task,
            ChunkData { cache, range },
            &mut outputs[range], // Each task writes to disjoint slice
            "taxation_chunk",
        ));
    }

    ctx.wait();

    // Aggregate sequentially (same as current implementation)
    let mut country_totals: HashMap<TagId, Mod32> = HashMap::new();
    for ((owner_id, _, _), output) in cache.iter().zip(outputs.iter()) {
        *country_totals.entry(*owner_id).or_insert(Mod32::ZERO) +=
            Mod32::from_raw(output.monthly_income);
    }

    // Apply
    for (tag, total) in country_totals {
        ctx.partition.countries[tag].treasury += total.to_fixed();
    }
}
```

**Advantages**:
- **Zero synchronization**: Each task writes to disjoint memory
- **Cache-friendly**: Sequential aggregation phase has perfect locality
- **Identical to current**: Final aggregation is exactly the current taxation code

**Disadvantages**:
- **Memory allocation**: Must allocate Vec (but can reuse across ticks)
- **Only works for dense outputs**: Requires N outputs for N inputs
- **Not suitable for sparse results**: Movement system can't use this (not all units move)

**Performance Analysis**:

| Metric | Value | Notes |
|--------|-------|-------|
| Allocation | ~10µs | 2450 × 4 bytes = 10KB, likely in cache |
| Per-chunk write | ~0ns | Writing to pre-allocated buffer (no overhead) |
| Sequential aggregate | ~5µs | Same as current HashMap aggregation |
| **Total overhead** | **~15µs** | Lowest overhead option |

**When to Use**:
- Dense results (every input produces one output)
- Simple aggregation (sum, max, etc.)
- Systems already using this pattern (taxation, production)

---

## Comparison Matrix

| Strategy | Overhead | Contention | Memory | Complexity | Best For |
|----------|----------|------------|--------|------------|----------|
| **Thread-Local** | ~40µs | None | 8× duplication | Medium | General purpose, associative ops |
| **Concurrent HashMap** | ~50µs | High | 1× | Low | Real-time, non-associative ops |
| **Pre-Allocated** | ~15µs | None | 1× | Low | Dense results, simple aggregation |

---

## Recommended Approach (Hybrid)

**Use different strategies for different system types:**

### Type 1: Dense Associative (Taxation, Production, Manpower)
→ **Pre-Allocated Result Slots**

```rust
// Current taxation pattern already does this!
let outputs = vec![TaxOutput32::default(); province_count];
// ... parallel SIMD compute ...
let country_totals = aggregate_by_owner(outputs); // Sequential HashMap
```

**Rationale**: Lowest overhead, preserves existing code structure.

---

### Type 2: Sparse Updates (Movement, Combat)
→ **Thread-Local Accumulators**

```rust
// Each worker collects its own updates
ctx.accumulator.push(MovementResult { unit_id: 123, new_location: ... });

// After wait(), merge all accumulators
let all_results = ctx.collect_thread_local_results();
for result in all_results {
    apply_to_state(result);
}
```

**Rationale**: No wasted allocation for units that don't move, natural Vec collection pattern.

---

### Type 3: Real-Time Critical (Future: AI Planning?)
→ **Lock-Free Concurrent HashMap** (dashmap)

```rust
// Tasks can update shared state without blocking
ctx.shared_results.entry(country_id).or_insert(0).fetch_add(value, Relaxed);
```

**Rationale**: Needed for systems that can't afford a sequential merge phase.

---

## Implementation Plan

### Phase 1: Add Result Collection to TaskContext

```rust
pub struct TaskContext {
    /// Pre-allocated output buffer (for dense results)
    output_buffer: Option<&mut [u8]>,

    /// Thread-local accumulator (for sparse results)
    accumulator: &mut ThreadLocalResults,

    /// Other fields...
}

impl TaskContext {
    /// Write result to pre-allocated slot (zero-copy)
    pub fn write_output<T>(&mut self, index: usize, value: T) {
        let ptr = self.output_buffer.as_mut().unwrap().as_mut_ptr();
        unsafe { ptr.cast::<T>().add(index).write(value); }
    }

    /// Accumulate into thread-local buffer
    pub fn accumulate<K, V>(&mut self, key: K, value: V)
    where
        K: Hash + Eq,
        V: Add<Output = V> + Copy,
    {
        // Generic accumulator (type-erased, use HashMap<TypeId, Box<dyn Any>>)
        self.accumulator.add(key, value);
    }

    /// Push to thread-local Vec
    pub fn push_result<T>(&mut self, value: T) {
        self.accumulator.push::<T>(value);
    }
}
```

### Phase 2: Refactor Taxation to Use Pre-Allocated

```rust
pub fn run_taxation_tick(ctx: &TaskContext) {
    let cache = &ctx.world.owned_provinces_cache;

    // Allocate output buffer
    let mut outputs = vec![TaxOutput32::default(); cache.len()];

    // Create batch with output buffer
    let batch = TaskBatch::with_output(
        taxation_chunk_task,
        cache.len(),
        &mut outputs,
    );

    // Spawn chunks
    for chunk_idx in 0..(cache.len() / 256) {
        batch.spawn_chunk(ctx, chunk_idx);
    }

    ctx.wait();

    // Aggregate (same as current code)
    let country_totals = aggregate_by_owner(cache, &outputs);

    // Apply
    apply_to_countries(ctx, country_totals);
}
```

### Phase 3: Refactor Movement to Use Thread-Local

```rust
pub fn run_movement_tick(ctx: &TaskContext) {
    // Spawn unit movement tasks
    for (&unit_id, unit) in &ctx.world.armies {
        ctx.spawn(Task::new(
            process_unit_movement,
            UnitData { unit_id, unit },
            "movement",
        ));
    }

    ctx.wait();

    // Collect thread-local results
    let all_results = ctx.collect::<MovementResult>();

    // Apply
    for result in all_results {
        apply_movement_result(ctx, result);
    }
}

fn process_unit_movement(ctx: &mut TaskContext) {
    let data = ctx.data::<UnitData>();
    let result = compute_movement(data);
    ctx.push_result(result); // Thread-local accumulation
}
```

---

## Open Questions

1. **Should we use type-erased accumulators or generic TaskContext?**
   - Type-erased: One `TaskContext` type, uses `TypeId` + `Box<dyn Any>`
   - Generic: `TaskContext<R: ResultType>`, compile-time dispatch
   - **Recommendation**: Start with type-erased for simplicity, optimize later if needed

2. **How to handle heterogeneous results (movement + combat in same batch)?**
   - Option A: Separate accumulators per result type (`Vec<MovementResult>`, `Vec<CombatResult>`)
   - Option B: Enum wrapper (`enum AnyResult { Movement(...), Combat(...) }`)
   - **Recommendation**: Separate accumulators (type-safe, zero-cost)

3. **Should aggregation be lazy or eager?**
   - Lazy: Accumulate into thread-local buffers, merge on `collect()`
   - Eager: Merge into shared buffer after each task completes
   - **Recommendation**: Lazy (proven pattern in Rayon, better cache locality)

4. **Can we reuse output buffers across ticks?**
   - Yes! Allocate once, reuse every tick (amortize allocation cost)
   - Store in `WorldState` as `taxation_output_buffer: Vec<TaxOutput32>`
   - **Recommendation**: Add buffer reuse in Phase 2 after validating correctness

---

## Conclusion

**Primary Recommendation**: **Hybrid approach with pre-allocated slots**

- **Taxation/Production/Manpower**: Pre-allocated output buffer (lowest overhead, ~15µs)
- **Movement/Combat**: Thread-local accumulators (natural pattern for sparse results, ~40µs)
- **Future systems**: Lock-free concurrent HashMap if needed (real-time guarantees, ~50µs)

**Critical Insight**: All three patterns **avoid locks** by ensuring:
1. Pre-allocated: Disjoint memory writes (no synchronization needed)
2. Thread-local: Per-thread buffers + sequential merge (no contention)
3. Lock-free: Atomic operations (CAS, no blocking)

This matches the proven pattern from current systems: **parallel compute + sequential aggregate**.

---

## References

- **Chase-Lev Deque**: Work-stealing queue algorithm (used by Rayon)
- **dashmap**: Lock-free concurrent HashMap (https://docs.rs/dashmap)
- **Rayon collect()**: Uses thread-local buffers + sequential merge (same as Strategy A)
- **Tracy Profiler**: Will visualize aggregation overhead (measure before optimizing)
