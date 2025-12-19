# Performance Measurement & Benchmarking

This document describes how we measure and optimize the simulation's performance to meet the mid-term goal of a 1444-1821 run in under 10 minutes.

## Measurement Framework

### SimMetrics Struct
The `eu4sim-core` crate includes a `SimMetrics` struct (in `src/metrics.rs`) that accumulates timing data for each major phase of the simulation tick.

- **Movement**: Advanced army/fleet paths, pathfinding resets.
- **Combat**: Casualty calculations and regiment destruction.
- **Occupation**: Persistence-based controller updates.
- **Economy**: Monthly ticks for taxation, production, manpower, and expenses.

### Instrumentation
The `step_world` function in `step.rs` accepts an optional `Option<&mut SimMetrics>`. When provided, it uses `std::time::Instant` to measure the duration of each phase.

```rust
pub fn step_world(
    state: &WorldState,
    inputs: &[PlayerInputs],
    adjacency: Option<&AdjacencyGraph>,
    config: &SimConfig,
    mut metrics: Option<&mut SimMetrics>,
) -> WorldState {
    let tick_start = Instant::now();
    // ...
    let move_start = Instant::now();
    run_movement_tick(&mut new_state, adjacency);
    if let Some(m) = metrics.as_mut() { m.movement_time += move_start.elapsed(); }
    // ...
}
```

### CLI Benchmark Flag
The `eu4sim` application supports a `--benchmark` flag that initializes metrics and prints a summary report upon completion.

```powershell
cargo run -p eu4sim -- --benchmark --ticks 1000
```

## Benchmarking Results (Dec 19, 2025)

**Environment**: Developer Machine (Windows)
**Scenario**: Observer mode, 1444 start (Release Build).

| Metric | Value |
|--------|-------|
| Years Simulated | 3 |
| Total Time | 2.87s |
| **Speed** | **1.0 years/sec** |
| Average Tick | 2.87ms |

### Analysis
The simulation speed is now **1.0 years/sec**, which translates to a full game run (377 years) in approximately **6.3 minutes**. This exceeds the mid-term goal of 10 minutes.

- **AI Overhead**: ~80% of tick time is currently AI decision making (despite being random). This is the next target for parallelization.
- **State Cloning**: Reduced from **~4.0ms** to **~0.4ms** per tick by switching to `im::HashMap`.

## Future Optimization & Profiling

When simulation speed drops or sub-system costs rise unexpectedly, the following techniques should be used:

### 1. Sampling Profilers
Use `samply` (Web-based profiler for Rust/Firefox) or `flamegraph` to identify hotspots in the simulation loop.
- **Samply**: Excellent for Windows/Linux. `samply record cargo run -p eu4sim -- --ticks 1000`
- **Flamegraph**: Classic visualization. `cargo flamegraph -p eu4sim -- --ticks 1000`

### 2. Micro-benchmarking (Criterion)
For critical algorithms like pathfinding or CAS calculations, use `criterion` to measure performance in isolation and detect regressions.

### 3. CPU Cache Optimization
- **Data Locality**: Monitor `Occupation` and `Combat` costs. If they grow, consider moving to a more ECS-like (entity-component-system) layout for `WorldState` to improve cache hit rates.
- **Parallelism**: Most systems are currently sequential. While determinism is easier to maintain sequentially, monthly economy ticks across 600+ countries are a prime candidate for `rayon` if total speed becomes a bottleneck.
