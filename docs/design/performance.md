# Performance & Optimization Guide

This document outlines the patterns and techniques used in `eu4sim-core` to achieve high-performance simulation ticks (target: <2ms/month).

## core Principles

1.  **Memory Layout is King**: Data locality often matters more than algorithm complexity.
2.  **Batch Processing**: Group similar operations to leverage instruction pipelining and SIMD.
3.  **Lazy Evaluation**: Rebuild acceleration structures only when necessary.
4.  **Avoid Allocations**: Hot paths should never heap allocate.

## Optimization Patterns

### 1. Structure of Arrays (SoA)

For systems processing thousands of entities (e.g., Taxation, Production), standard `Vec<Struct>` or `HashMap<Id, Struct>` layouts cause cache thrashing. We prefer **Structure of Arrays (SoA)** optimization for hot paths.

**Example: `OwnedProvinceSoA`**
Instead of iterating `WorldState.provinces` (an `im::HashMap`), we maintain a dense cache:

```rust
#[derive(Default, Clone)]
pub struct OwnedProvinceSoA {
    pub ids: Vec<ProvinceId>,       // Contiguous IDs
    pub owners: Vec<TagId>,         // Contiguous Owner IDs (interned)
    pub base_tax: Vec<Mod32>,       // Contiguous Data for calculation
    pub autonomy_floor: Vec<Mod32>, // Pre-calculated values
}
```

**Benefits:**
- **SIMD-Friendly**: Data for vectorized operations (e.g., `base_tax`) is loaded into registers linearly.
- **Pre-calculation**: Complex logic (e.g., `effective_autonomy` which checks cores) is done once during cache rebuild, not every tick.
- **Interning**: Replaces strings with `u16` indices (`TagId`), removing hashing from the hot path.

### 2. Lazy Cache Rebuilding

Caches are maintained via a dirty flag pattern on `WorldState`:

```rust
pub struct WorldState {
    // ...
    pub owned_provinces_cache: OwnedProvinceSoA,
    pub owned_provinces_cache_valid: bool,
}

impl WorldState {
    pub fn ensure_owned_provinces_valid(&mut self) {
        if !self.owned_provinces_cache_valid {
            // Rebuild logic...
            self.owned_provinces_cache_valid = true;
        }
    }
    
    pub fn step_modifier(&mut self) {
        // Invalidate when source of truth changes
        self.owned_provinces_cache_valid = false;
    }
}
```

### 3. Batched Lookups & Sorting

To minimize random access into `HashMap` lookups (e.g., Country Modifiers), sort the SoA cache by the lookup key (e.g., Owner).

**Technique:**
1.  Sort SoA arrays by `Owner`.
2.  Iterate linearly.
3.  Only lookup country modifiers when `Owner` changes (simple `if current != last` check).

This reduces N hash lookups (where N = provinces) to M hash lookups (where M = countries). For EU4, this is ~20,000 -> ~800, a **25x reduction** in hash overhead.

### 4. SIMD Integration

We use explicit SIMD via the `simd` module (using `wide` or `std::simd` abstraction layers).

- **Data Alignment**: Ensure input data is compatible with vector loads.
- **Chunking**: Process data in chunks (e.g., groups of 8 or 16) to map to AVX2/SSE registers.
- **Fallback**: Always provide a scalar fallback for unsupported architectures.

## Profiling

Use `cargo xtask profile` to capture Tracy traces.
- **Goal**: Hot path systems should execute in <0.5ms.
- **Metrics**: Look for "Mean Time" in `tracy-csvexport` output.
