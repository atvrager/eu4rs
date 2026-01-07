# Task-Based Parallelism Design

**Status**: Design Phase
**Last Updated**: 2026-01-07
**Related**: [performance.md](performance.md), [type_system.md](../type_system.md)

## Problem Statement

The SIMD taxation optimization (commit ae227c5) achieved 25× speedup by replacing Rayon parallelism with sequential SIMD batching. However, this eliminated thread-level parallelism entirely. The root cause: **Rayon's overhead dominated for small task counts** (10 chunks of 256 provinces each).

**Key Findings**:
- Rayon overhead > computation time for taxation (~2450 provinces)
- Thread wakeup + synchronization costs are too high for monthly systems
- But we have many CPU cores sitting idle during simulation

**Goal**: Restore multi-core parallelism without reintroducing overhead that kills performance.

---

## Current Architecture Recap

### System Execution Model (Sequential)

```rust
pub fn step_world(state: &mut WorldState, date: Date) {
    // Daily systems (every day)
    run_movement_tick(state);      // Uses Rayon internally
    run_combat_tick(state);
    run_siege_tick(state);

    // Monthly systems (1st of month)
    if date.day == 1 {
        run_taxation_tick(state);  // SIMD, sequential
        run_production_tick(state);
        run_trade_value_tick(state);
        run_manpower_tick(state);
        // ... 15+ more systems
    }
}
```

**Problems**:
1. Systems run sequentially even when independent
2. `&mut WorldState` prevents concurrent access
3. Each system hogs all cores or none (movement uses Rayon, taxation doesn't)
4. No work distribution across systems

### Why Rayon Failed

| Factor | Impact | Evidence |
|--------|--------|----------|
| Extract cost | High | Building province SoA requires allocation |
| Scheduler overhead | High | Thread wakeup for 10 chunks |
| Work per chunk | Low | 256 provinces × SIMD = 32 AVX2 ops (~400ns) |
| Sync cost | High | Barrier + result collection |
| **Total** | **Overhead > Work** | Sequential SIMD was faster |

---

## Design Principles

### 1. **Zero-Overhead When Not Parallel**
If a system runs sequentially, it should have identical performance to current implementation. No trait objects, no atomics, no allocations.

### 2. **Work-Stealing Over Work-Sharing**
Rayon uses work-stealing but with high granularity. We need:
- Fiber-like tasks (lightweight context switches)
- CPU-local queues (reduce contention)
- Steal only when idle (preserve cache locality)

### 3. **Preserve SIMD**
Task parallelism must not conflict with SIMD:
- Tasks operate on SoA chunks (not per-province)
- SIMD remains sequential within a task
- Tasks are coarse-grained enough to justify overhead

### 4. **Explicit Dependencies**
Systems declare read/write dependencies:
- Read-only systems can run in parallel
- Write systems have exclusive access to their data
- Scheduler resolves dependencies at runtime

### 5. **Hybrid Coarse/Fine Parallelism**
- **Coarse**: Independent systems run concurrently (taxation || production || manpower)
- **Fine**: Within a system, spawn sub-tasks for large data sets (movement already does this)

---

## Proposed Architecture

### Overview

```
┌──────────────────────────────────────────────────────────┐
│                  Game Loop (step_world)                  │
└────────────────────────┬─────────────────────────────────┘
                         │
                ┌────────▼─────────┐
                │  Task Scheduler  │
                │  (work-stealing) │
                └────────┬─────────┘
                         │
        ┌────────────────┼────────────────┐
        │                │                │
   ┌────▼────┐      ┌────▼────┐     ┌────▼────┐
   │ Worker  │      │ Worker  │ ... │ Worker  │
   │ Thread  │      │ Thread  │     │ Thread  │
   └────┬────┘      └────┬────┘     └────┬────┘
        │                │                │
   ┌────▼─────────────────▼────────────────▼────┐
   │         Execute Tasks (Fibers/Jobs)        │
   │  ┌─────────┐  ┌──────────┐  ┌──────────┐  │
   │  │ Taxation│  │Production│  │ Manpower │  │
   │  └─────────┘  └──────────┘  └──────────┘  │
   └────────────────────────────────────────────┘
```

### Core Abstractions

#### 1. **Task** (Lightweight Work Unit)

```rust
/// A unit of work that can be scheduled on any worker thread.
/// Sized for stack allocation (no Box<dyn Fn>).
pub struct Task {
    /// Function pointer to the work
    work: fn(ctx: &mut TaskContext),

    /// Opaque data pointer (e.g., province range, country list)
    data: *mut u8,

    /// Resource access pattern (for dependency tracking)
    access: AccessPattern,

    /// Task name (for Tracy instrumentation)
    name: &'static str,
}

/// What data does this task read/write?
pub struct AccessPattern {
    reads: &'static [Resource],
    writes: &'static [Resource],
}

/// Coarse-grained resource categories
#[derive(Copy, Clone, Hash, Eq, PartialEq)]
pub enum Resource {
    Provinces,
    Countries,
    Armies,
    Navies,
    TradeNodes,
    Diplomacy,
    // ...
}
```

**Key Design Choices**:
- `fn(ctx)` instead of `Box<dyn Fn>`: No allocations, cache-friendly
- `*mut u8` data: Type-erased context (cast to `&mut ProvinceRange` in task body)
- `AccessPattern`: Declare dependencies at construction time, not runtime
- `name`: Tracy integration for profiling

#### 2. **TaskContext** (Execution Environment)

```rust
/// Execution context for a task (fiber-like).
pub struct TaskContext {
    /// Reference to shared world state (immutable parts)
    world: &WorldState,

    /// Mutable partition this task owns
    partition: Partition,

    /// Task spawner (for spawning child tasks)
    spawner: &TaskSpawner,

    /// Worker thread ID (for debugging/profiling)
    worker_id: u8,
}

/// A disjoint subset of WorldState that can be mutated.
pub enum Partition {
    Provinces(ProvinceRange),
    Countries(CountrySlice),
    Global(GlobalResources),
}
```

**Key Design Choices**:
- `world`: Shared reference for read-only data (modifiers, static game data)
- `partition`: Exclusive ownership of a mutable slice
- `spawner`: Spawn child tasks (e.g., taxation spawns 10 chunk tasks)
- No `&mut WorldState`: Instead, tasks get disjoint partitions

#### 3. **TaskScheduler** (Work-Stealing Executor)

```rust
/// Global scheduler for executing tasks across worker threads.
pub struct TaskScheduler {
    /// Per-thread work queues
    workers: Vec<Worker>,

    /// Dependency graph for systems
    graph: DependencyGraph,

    /// Thread pool handle
    pool: ThreadPool,
}

impl TaskScheduler {
    /// Submit a task to be scheduled (non-blocking).
    pub fn submit(&self, task: Task);

    /// Wait for all pending tasks to complete.
    pub fn wait(&self);

    /// Submit multiple tasks with dependencies.
    pub fn submit_batch(&self, tasks: &[Task], deps: &[TaskId]);
}

/// Per-worker state (one per CPU core).
struct Worker {
    /// Local queue (lock-free, single-producer)
    queue: LocalQueue<Task>,

    /// Random victim for work stealing
    rng: SmallRng,

    /// Currently executing task (for Tracy)
    current: Option<&'static str>,
}
```

**Key Design Choices**:
- `LocalQueue`: Lock-free SPSC queue (Chase-Lev deque)
- `submit()`: Push to calling thread's queue (LIFO, cache-friendly)
- `wait()`: Park until all tasks complete (futex on Linux, event on Windows)
- `rng`: Randomized work stealing reduces contention

#### 4. **DependencyGraph** (System Ordering)

```rust
/// Tracks read/write dependencies between systems.
pub struct DependencyGraph {
    /// Adjacency list (system_id -> depends_on)
    edges: Vec<Vec<SystemId>>,

    /// Access patterns per system
    access: Vec<AccessPattern>,
}

impl DependencyGraph {
    /// Add a system with its access pattern.
    pub fn add_system(&mut self, name: &'static str, access: AccessPattern) -> SystemId;

    /// Compute which systems can run in parallel.
    pub fn schedule(&self) -> Vec<TaskBatch>;
}

/// A group of systems that can run concurrently.
pub struct TaskBatch {
    systems: Vec<SystemId>,
}
```

**Key Design Choices**:
- Computed at compile-time or startup (not per-tick)
- Systems with disjoint access patterns batch together
- Read-only systems can run concurrently with other readers

---

## System Refactor Pattern

### Before (Current Sequential)

```rust
pub fn run_taxation_tick(state: &mut WorldState) {
    // Prepare: Build SoA cache
    let cache = &state.owned_provinces_cache;

    // Compute: SIMD batch processing
    let taxes = calculate_taxes_batch32(cache, &state.modifiers);

    // Apply: Update treasuries
    for (tag, amount) in taxes {
        state.countries[tag].treasury += amount;
    }
}
```

### After (Task-Based)

```rust
pub fn run_taxation_tick(ctx: &TaskContext) {
    // Prepare: Build SoA cache (no change)
    let cache = &ctx.world.owned_provinces_cache;

    // Compute: Spawn parallel chunk tasks
    let chunk_size = 256;
    let num_chunks = cache.len() / chunk_size;

    let mut results = Vec::with_capacity(num_chunks);
    for i in 0..num_chunks {
        let range = i * chunk_size..(i + 1) * chunk_size;
        let task = Task::new(
            taxation_chunk_task,
            ChunkData { cache, range, modifiers: &ctx.world.modifiers },
            AccessPattern::read_only(Resource::Provinces),
            "taxation_chunk",
        );
        ctx.spawner.submit(task);
    }

    // Wait for all chunks to complete
    ctx.spawner.wait();

    // Aggregate: Collect results (sequential, low overhead)
    for result in results {
        ctx.partition.countries[result.tag].treasury += result.amount;
    }
}

fn taxation_chunk_task(ctx: &mut TaskContext) {
    let data = unsafe { &*(ctx.data as *const ChunkData) };
    let taxes = calculate_taxes_batch32(
        data.cache.slice(data.range),
        data.modifiers,
    );
    ctx.write_result(taxes);
}
```

**Key Changes**:
1. **Spawn sub-tasks**: Each chunk becomes a task (amortize overhead across larger chunks)
2. **Wait barrier**: `ctx.spawner.wait()` blocks until all chunks done
3. **Result aggregation**: Still sequential (overhead negligible for ~10 chunks)

**Overhead Analysis**:
- Old Rayon: Thread wakeup + scheduler + barrier per tick
- New Task: Fiber yield + queue push per chunk (100× faster)
- Critical: Chunks remain 256 provinces (don't make them smaller!)

---

## Parallelism Opportunities

### Monthly Systems (1st of Month)

```rust
pub fn run_monthly_tick(ctx: &TaskContext) {
    // Phase 1: Independent systems (can run in parallel)
    ctx.submit_parallel(&[
        Task::new(run_taxation_tick, ...),
        Task::new(run_production_tick, ...),
        Task::new(run_manpower_tick, ...),
        Task::new(run_attrition_tick, ...),
    ]);

    // Phase 2: Trade systems (sequential dependencies)
    run_trade_power_tick(ctx);
    run_trade_value_tick(ctx);
    run_trade_income_tick(ctx);

    // Phase 3: Expenses (depends on income)
    ctx.submit_parallel(&[
        Task::new(run_expenses_tick, ...),
        Task::new(run_advisor_cost_tick, ...),
    ]);
}
```

**Batching Rules**:
1. **Independent**: Taxation, production, manpower touch disjoint data → parallel
2. **Sequential**: Trade systems have pipeline dependencies → sequential
3. **Read-heavy**: Stats, colonization read country state → parallel with other readers

**Expected Speedup**:
- Phase 1: 4× speedup (4 systems → 4 cores)
- Phase 2: 1× (unavoidable sequential)
- Phase 3: 2× speedup (2 systems → 2 cores)
- **Total**: ~2.5× for monthly tick

### Daily Systems

```rust
pub fn run_daily_tick(ctx: &TaskContext) {
    // Movement is already parallel (keep Rayon internally)
    run_movement_tick(ctx);

    // Combat systems can run in parallel (separate army sets)
    ctx.submit_parallel(&[
        Task::new(run_combat_tick, ...),
        Task::new(run_naval_combat_tick, ...),
        Task::new(run_siege_tick, ...),
    ]);

    // Occupation depends on combat results
    run_occupation_tick(ctx);
}
```

**Expected Speedup**:
- Movement: No change (already parallelized)
- Combat: 3× speedup (3 systems → 3 cores)
- **Total**: ~1.5× for daily tick (movement dominates)

---

## Implementation Phases

### Phase 1: Core Infrastructure (MVP)
- [ ] Implement `Task`, `TaskContext`, `Partition`
- [ ] Implement `TaskScheduler` with basic work-stealing (single-threaded first)
- [ ] Add Tracy integration for task profiling
- [ ] Write unit tests for scheduler correctness

**Deliverable**: Tasks run sequentially with zero overhead.

### Phase 2: Work-Stealing Parallelism
- [ ] Implement `Worker` with lock-free queues (crossbeam-deque)
- [ ] Implement thread pool with work stealing
- [ ] Add `submit_parallel()` for batching tasks
- [ ] Benchmark overhead vs. current implementation

**Deliverable**: Tasks run in parallel with <1µs overhead per task.

### Phase 3: System Refactor (Taxation)
- [ ] Refactor `run_taxation_tick()` to spawn chunk tasks
- [ ] Compare performance: old Rayon vs. new task system
- [ ] Tune chunk size for optimal overhead/parallelism tradeoff

**Deliverable**: Taxation runs as fast as sequential SIMD or faster.

### Phase 4: Dependency Graph
- [ ] Implement `DependencyGraph` with access patterns
- [ ] Refactor monthly systems to declare dependencies
- [ ] Automatic batching of independent systems

**Deliverable**: Monthly tick runs with 2-3× speedup.

### Phase 5: Daily Systems
- [ ] Refactor combat systems to run in parallel
- [ ] Refactor movement to use task system (remove Rayon dependency)
- [ ] Optimize army/navy partitioning for parallel access

**Deliverable**: Daily tick runs with 1.5-2× speedup.

---

## Technical Deep Dives

### Work-Stealing Queue Implementation

Use **crossbeam-deque** (already in dependency graph via Rayon):

```rust
use crossbeam_deque::{Injector, Stealer, Worker as LocalQueue};

pub struct TaskScheduler {
    /// Global queue for new tasks
    injector: Injector<Task>,

    /// Per-worker local queues
    workers: Vec<LocalQueue<Task>>,

    /// Stealers for each worker (for work stealing)
    stealers: Vec<Stealer<Task>>,
}
```

**Why crossbeam-deque**:
- Lock-free Chase-Lev deque (proven algorithm)
- LIFO for local queue (cache-friendly)
- FIFO for stealing (work spread)
- Already tested and optimized

### Fiber vs. OS Threads

**Option A: OS Threads (Rayon-style)**
- Pro: Simple, proven, OS scheduler handles CPU affinity
- Con: Context switch overhead (~1-2µs), stack allocation (2MB per thread)

**Option B: Green Threads/Fibers**
- Pro: Ultra-lightweight (100ns context switch), stackless
- Con: Requires async/await or unsafe state machines

**Recommendation**: Start with OS threads (Phase 2), measure overhead, consider fibers if >10% overhead observed.

### Memory Partitioning Strategy

**Challenge**: Systems need `&mut WorldState` but we can't have multiple mutable references.

**Solution**: Split `WorldState` into independent partitions:

```rust
pub struct WorldState {
    // Immutable (shared across tasks)
    pub modifiers: GameModifiers,
    pub static_data: StaticGameData,

    // Mutable (partitioned)
    pub provinces: ProvincePartition,
    pub countries: CountryPartition,
    pub armies: ArmyPartition,
    pub navies: NavyPartition,
}

/// A partitionable resource.
pub trait Partitionable {
    fn split(&mut self, ranges: &[Range<usize>]) -> Vec<PartitionView>;
}
```

**Key Insight**: Rust's borrow checker allows splitting a `Vec` into disjoint `&mut [T]` slices:

```rust
let (left, right) = provinces.split_at_mut(1000);
// Can pass `left` to task A and `right` to task B
```

### Tracy Integration

```rust
impl TaskScheduler {
    fn execute_task(&self, task: Task) {
        tracy_client::span!(task.name);
        (task.work)(&mut TaskContext { ... });
    }
}
```

**Visualization**: Tracy will show:
- Task timeline (which tasks run when)
- Work stealing events (steals across threads)
- Idle time (workers waiting for work)

---

## Performance Targets

### Baseline (Current Implementation)

| System | Provinces | Time (seq.) | Time (SIMD) | Speedup |
|--------|-----------|-------------|-------------|---------|
| Taxation | 2450 | ~800µs | ~200µs | 4× |
| Production | 2450 | ~600µs | ~600µs | 1× |
| Manpower | 2450 | ~400µs | ~400µs | 1× |

### Target (Task-Based Parallelism)

| System | Time (8 cores) | Speedup vs. SIMD | Notes |
|--------|----------------|------------------|-------|
| Taxation | ~200µs | 1× | Already optimal (SIMD) |
| Production | ~150µs | 4× | Parallel with taxation |
| Manpower | ~100µs | 4× | Parallel with taxation |
| **Total** | **~200µs** | **6×** | Limited by longest task |

**Key Metric**: Monthly tick should complete in <2ms (currently ~5ms).

### Overhead Budget

| Operation | Target | Notes |
|-----------|--------|-------|
| Task submission | <100ns | Queue push + counter increment |
| Context switch | <1µs | OS thread switch (avoid if possible) |
| Work stealing | <500ns | Check queue, atomic CAS |
| Barrier wait | <10µs | Futex park/unpark |

**Critical**: Overhead must be <5% of shortest task (~400µs → 20µs budget).

---

## Open Questions

1. **Should we use fibers or OS threads?**
   - Recommendation: Start with OS threads, profile, revisit if overhead >5%

2. **How to handle borrow checker with partitions?**
   - Recommendation: Use `split_at_mut()` for Vec partitioning, unsafe for HashMap (validate at runtime)

3. **Can we make taxation even faster with task parallelism?**
   - Unlikely: SIMD already saturates memory bandwidth (not CPU-bound)
   - But: Taxation + production + manpower can run concurrently (different memory regions)

4. **What about Rayon in movement system?**
   - Keep it for now (proven, low overhead for 1000s of armies)
   - Phase 5: Migrate to task system for consistency

5. **How to handle dynamic task graphs?**
   - Example: Combat generates casualties → affects attrition → affects manpower
   - Recommendation: Pre-declare conservative dependencies, refine in Phase 4

---

## References

- **Rayon**: https://github.com/rayon-rs/rayon (current solution)
- **crossbeam-deque**: https://docs.rs/crossbeam-deque (work-stealing queues)
- **Tokio**: https://tokio.rs (async runtime, not suitable here due to blocking compute)
- **Tracy**: https://github.com/wolfpld/tracy (profiling integration)
- **Chase-Lev Deque**: "Dynamic Circular Work-Stealing Deque" (2005)

---

## Conclusion

This design aims to restore multi-core parallelism without reintroducing Rayon's overhead:

1. **Lightweight tasks**: Function pointers + data pointers (no allocations)
2. **Work-stealing**: CPU-local queues with low-contention stealing
3. **Explicit dependencies**: Systems declare read/write access upfront
4. **Preserve SIMD**: Tasks operate on coarse-grained chunks (256+ provinces)
5. **Incremental rollout**: Phase 1 has zero overhead, Phase 5 targets 2-3× speedup

**Next Steps**: Implement Phase 1 (core infrastructure) and benchmark against current sequential code.
