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
| Years Simulated | 11 |
| Total Time | 4.90s |
| **Speed** | **2.2 years/sec** |
| Average Tick | 1.342ms |

### Breakdown

| System | Time per Tick | % of Total |
|--------|---------------|------------|
| AI | 0.812ms | 60.5% |
| Other | 0.419ms | 31.2% |
| Economy | 0.055ms | 4.1% |
| Combat | 0.023ms | 1.7% |
| Movement | 0.021ms | 1.6% |
| Occupation | 0.012ms | 0.9% |

### Analysis
The simulation speed is now **3.0 years/sec**, which translates to a full game run (377 years) in approximately **2.1 minutes**. This significantly exceeds the mid-term goal of 10 minutes.

**Recent Improvements**:
- **AI Parallelization (Dec 19)**: Rayon-based parallel AI loop reduced AI overhead from 81% to 42% of tick time, yielding a **3x speedup** (1.0 → 3.0 years/sec).
- **State Cloning**: Reduced from **~4.0ms** to **~0.4ms** per tick by switching to `im::HashMap`.

## Future Optimization & Profiling

When simulation speed drops or sub-system costs rise unexpectedly, the following techniques should be used:

### 1. Sampling Profilers
Use `samply` (Web-based profiler for Rust/Firefox) or `flamegraph` to identify hotspots in the simulation loop.

**Recommended: Samply** (Easy interactive traces)
```bash
cargo install samply
# Run observer mode for 1000 ticks
samply record cargo run -p eu4sim --release -- --observer --ticks 1000
```
This opens a local server (firefox Profiler compatible) to explore call stacks.

**Alternative: Flamegraph** (Classic visualization)
```bash
cargo install flamegraph
cargo flamegraph -p eu4sim -- --observer --ticks 1000
```
Generates `flamegraph.svg` in current directory.

*Note: Always use `--release` for accurate bottlenecks (debug builds are dominated by non-inlined method calls).*

### 2. Micro-benchmarking (Criterion)
For critical algorithms like pathfinding or CAS calculations, use `criterion` to measure performance in isolation and detect regressions.

### 3. CPU Cache Optimization
- **Data Locality**: Monitor `Occupation` and `Combat` costs. If they grow, consider moving to a more ECS-like (entity-component-system) layout for `WorldState` to improve cache hit rates.
- **Parallelism**: Most systems are currently sequential. While determinism is easier to maintain sequentially, monthly economy ticks across 600+ countries are a prime candidate for `rayon` if total speed becomes a bottleneck.
