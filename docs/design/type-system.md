# Type System Design

This document defines the type philosophy for the `eu4rs` project, with a focus on supporting:

- **Deterministic simulation** (netcode, multiplayer sync)
- **Reproducible replays** (timeline view, logging, catch-up)
- **Forward compatibility** (old files work forever)
- **Performance** (SIMD-friendly, cache-efficient)
- **Generic extensibility** (support other games in the genre)

## Design Philosophy

> **Guiding principle**: Design for "a generic grand strategy game with non-specific mechanics."
> 
> While EU4 is the immediate target, the type system should be flexible enough to handle CK3, Victoria 3, HOI4, or hypothetical games with similar data patterns. This isn't about shipping multi-game support—it's about making architecturally sound decisions that don't paint us into EU4-specific corners. The pleasure is in the pure computer science.

This means:
- Prefer general patterns (key-value stores, modifier stacks) over EU4-specific structs
- Use data-driven schemas rather than hardcoded enums where sensible
- Keep game-specific semantics in configuration, not code

## Two-Layer Architecture

```
┌──────────────────────────────────────────────────────────────────┐
│                     Parse Layer (eu4data)                        │
│  • Permissive types (f32, String, Option<T>)                    │
│  • Handles messy game files, unknown fields, schema evolution   │
│  • Goal: Parse without crashes, capture all data                │
└──────────────────────────────────────────────────────────────────┘
                              ↓ convert
┌──────────────────────────────────────────────────────────────────┐
│                     Sim Layer (eu4sim-core)                      │
│  • Strict types (fixed-point, interned IDs)                     │
│  • Deterministic, no floating-point drift                       │
│  • Goal: Exact reproducibility across platforms                 │
└──────────────────────────────────────────────────────────────────┘
```

## Type Mapping

| EU4 Concept | Parse Layer | Sim Layer | Rationale |
|-------------|-------------|-----------|-----------|
| Modifier values (0.1, 0.25) | `f32` | `FixedI32` (scale 10000) | No rounding drift |
| Integer counts | `i32` | `i32` | Exact, SIMD-friendly |
| Boolean (yes/no) | `bool` | `bool` | Perfect as-is |
| String IDs (religion, culture) | `String` | `u32` interned ID | Fast comparison |
| Dates (1444.11.11) | `String` | `u32` ordinal days | Arithmetic |
| Color (RGB) | `[i32; 3]` | `[u8; 4]` (RGBA) | Aligned, SIMD |
| Lists of numbers | `Vec<f32>` | `Vec<FixedI32>` | Deterministic |

## Determinism Guidelines

1. **Avoid `f32` in simulation logic**  
   Floating-point can produce different results across platforms due to:
   - x87 vs SSE rounding differences
   - Compiler optimizations (fused multiply-add)
   - Fast-math flags

2. **Use fixed-point for game values**  
   ```rust
   // Example: store 0.25 as 2500 with scale 10000
   pub struct FixedMod(i32);
   impl FixedMod {
       pub const SCALE: i32 = 10000;
       pub fn from_f32(v: f32) -> Self { Self((v * Self::SCALE as f32) as i32) }
       pub fn to_f32(self) -> f32 { self.0 as f32 / Self::SCALE as f32 }
   }
   ```

3. **Seed RNG deterministically**  
   The `WorldState` includes an RNG seed; all randomness derives from it.

4. **Log inputs, not outputs**  
   For replay, log `PlayerInputs` per tick. Re-running `step_world` reproduces state.

## Forward Compatibility

1. **Use wider types when in doubt**  
   - `i32` over `i16` for counts
   - `u64` for IDs if scaling is uncertain

2. **Optional fields with defaults**  
   - New fields added as `Option<T>` with `#[serde(default)]`
   - Old files deserialize with `None`, code handles gracefully

3. **Versioned serialization**  
   - Schema version in save files
   - Migration functions for major changes

## SIMD Considerations

1. **Power-of-2 struct sizes**  
   Prefer 4, 8, 16, 32, 64 byte structs for cache alignment.

2. **Arrays over heterogeneous structs**  
   ```rust
   // Prefer this (SoA - Struct of Arrays):
   struct ProvinceData {
       populations: Vec<u32>,
       developments: Vec<u16>,
   }
   
   // Over this (AoS - Array of Structs):
   struct Province { population: u32, development: u16 }
   let provinces: Vec<Province>; // Less SIMD-friendly
   ```

3. **Aligned primitive arrays**  
   `[f32; 4]` for colors (SIMD vector), `[i32; 4]` for coordinates.

## Type Inference Rules (for code generation)

When the auto-generator infers types from EU4 data:

1. **Distinguish integers from floats**  
   - If sample parses as `i32` → `InferredType::Integer`  
   - If sample has decimal → `InferredType::Float`

2. **Prefer specific types**  
   - `"yes"/"no"` → `bool`
   - List of ints → `Vec<i32>` (not `Vec<f32>`)

3. **Flag ambiguous blocks**  
   - `{ ... }` with assignments → `Block` (generate nested struct)
   - `{ ... }` with values only → List
   - Unknown → `InferredType::Unknown`, flag for human review

4. **Document scale factors**  
   When generating fixed-point conversions, emit comments:
   ```rust
   /// Parsed from f32, scale 10000 for simulation layer
   pub modifier: Option<f32>,  // TODO: FixedI32 for sim
   ```

## Current Status

| Layer | Status |
|-------|--------|
| Parse Layer types | ✅ Implemented (InferredType enum) |
| Code generation | 🚧 In progress |
| Sim Layer types | ⏳ Future work |
| Fixed-point wrappers | ⏳ Future work |
| ID interning | ⏳ Future work |

## Open Questions

1. Should `InferredType` include a `Date` variant for `YYYY.MM.DD` patterns?
2. What fixed-point crate to use? (`fixed`, `rust_decimal`, or manual)
3. Should the sim layer use ECS (Entity-Component-System) for better data locality?
