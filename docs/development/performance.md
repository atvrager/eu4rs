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

### 1. Tracy Profiling (Recommended)
Real-time profiler with frame-aware analysis, perfect for game/GUI applications.

```bash
# Quick start - see .agent/workflows/profile.md for full guide
cargo xtask profile --duration 60
```

**Features:**
- Real-time CPU + GPU profiling
- Frame time analysis and FPS tracking
- Zero overhead when not enabled
- Automatic report generation

**Output:** Markdown report in `profiling/<timestamp>/report.md` that I can analyze directly.

See [Profiling Workflow](../../.agent/workflows/profile.md) for complete guide.

### 2. Sampling Profilers (Ad-hoc Analysis)
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

### 3. Micro-benchmarking (Criterion)
For critical algorithms like pathfinding or CAS calculations, use `criterion` to measure performance in isolation and detect regressions.

### 3. CPU Cache Optimization
- **Data Locality**: Monitor `Occupation` and `Combat` costs. If they grow, consider moving to a more ECS-like (entity-component-system) layout for `WorldState` to improve cache hit rates.
- **Parallelism**: Most systems are currently sequential. While determinism is easier to maintain sequentially, monthly economy ticks across 600+ countries are a prime candidate for `rayon` if total speed becomes a bottleneck.

---

## Visualization Performance (eu4viz)

### Map Regeneration Bottleneck

The political map must be regenerated whenever the timeline tick changes (scrubbing, playback). This operation processes ~11.5M pixels (5632×2048) and was a major performance bottleneck.

**Timeline Scrub Benchmarks (Dec 20, 2025)**:

| Optimization | CPU Time | Status |
|--------------|----------|--------|
| Sequential (baseline) | 5.87s | ❌ Unusable |
| Rayon row parallelization | 605ms | ⚠️ Sluggish |
| + Pre-computed province ID buffer | 367ms | ✅ Acceptable |

**Target**: <100ms for responsive scrubbing, <16ms for 60 FPS playback.

### Current Optimizations

1. **Rayon Parallelization**: `regenerate_political_map` uses `par_chunks_mut` to process rows in parallel across all CPU cores. ~10x speedup.

2. **Pre-computed Province ID Buffer**: A `Vec<Option<u32>>` mapping each pixel to its province ID is computed once at load time. Eliminates per-pixel HashMap lookup during regeneration.

3. **FPS Counter**: Window title displays real-time FPS for profiling.

4. **Timing Instrumentation**: Debug logs show CPU vs GPU upload breakdown.

### Future Optimization Opportunities

The following optimizations are documented for future implementation if current performance is insufficient:

#### 1. Pre-computed Country Color Buffer (CPU)
Instead of chaining owner→country→color HashMap lookups per pixel, maintain a `Vec<[u8; 3]>` where index = province_id → RGB color. Update only when ownership changes.

**Estimated improvement**: 2-5x (eliminates 3 HashMap lookups per province).

#### 2. GPU Compute Shader (Major Refactor)
Move map generation to the GPU entirely using wgpu compute shaders:

```wgsl
// Pseudo-code structure
@group(0) @binding(0) var<storage, read> province_ids: array<u32>;
@group(0) @binding(1) var<storage, read> province_colors: array<vec3<f32>>;
@group(0) @binding(2) var<storage, read_write> output_pixels: array<vec3<f32>>;

@compute @workgroup_size(256)
fn main(@builtin(global_invocation_id) id: vec3<u32>) {
    let pixel_idx = id.x;
    let prov_id = province_ids[pixel_idx];
    output_pixels[pixel_idx] = province_colors[prov_id];
}
```

**Requirements**:
- Upload province ID buffer to GPU once at load
- Upload province→color mapping when tick changes
- Single dispatch: 11.5M pixels / 256 workgroup = 44,921 workgroups

**Estimated improvement**: 50-100x (GPU parallelism + memory bandwidth).

#### 3. Delta Updates
For playback mode, only update provinces that changed ownership between ticks rather than regenerating the entire map. Requires tracking ownership deltas from the event log.

**Estimated improvement**: 10-100x for typical ticks (few provinces change).

#### 4. Lower Resolution Preview
During active scrubbing, render at 1/4 resolution (1408×512) and upscale. Switch to full resolution when scrubbing stops.

**Estimated improvement**: 16x during interaction.

### Profiling Commands

```powershell
# Run with verbose logging to see timing breakdown
cargo run -p eu4viz -- -v --event-log events.jsonl

# Watch for these log lines:
# [DEBUG] Map regen: CPU 85ms, Upload 12ms, Total 97ms
# [DEBUG] regenerate_political_map took 85.3ms
```
