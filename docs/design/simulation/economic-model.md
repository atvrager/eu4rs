# Simulation Economic Model

## Overview
The simulation uses a deterministic fixed-point arithmetic system (`Fixed`) to model the economies of all countries.

## Phase 1: Production & Taxation
*(Implemented in 0.1.0)*

### Production
- **Base Formula**: `Production = (Base Production * 0.2) + Trade Value`
- **Output**: Monthly income to treasury.

### Taxation
- **Base Formula**: `Tax = (Base Tax + Efficiency) * (1 - Autonomy)`
- **Output**: Monthly income.

## Phase 2: Military & Expenses
*(Implemented in 0.1.1)*

### Manpower
- **Pool**: Each country has a manpower pool capped by maximum manpower.
- **Recovery**: Manpower recovers over 10 years (120 months) when below max.
- **Development**: 1 Base Manpower Dev = ~1000 men max.

### Military Units
- **Regiments**: 1000 men per regiment. Types: Infantry, Cavalry, Artillery.
- **Initialization**: Automatically generated at start: 1 Regiment per 5,000 Manpower (Max).

### Expenses
Cost is deducted monthly from the treasury.

| Category | Cost Formula | Notes |
|----------|--------------|-------|
| **Army** | `Regiment Count * 0.2` | Base cost 0.2 ducats/month. |
| **Forts** | `Fort Count * 1.0` | 1.0 ducats/month per active fort. |

## Phase 3: Diplomacy & War
*(Implemented in 0.1.2)*

### Diplomatic Relations
Countries can have formal relationships with each other:
- **Alliance**: Mutual defense pact (not yet enforced)
- **Rival**: Competitive relationship (no mechanical effect yet)

### War System
- **Declaration**: Countries can declare war via `DeclareWar` command
- **War Structure**: Each war has attackers and defenders (coalitions)
- **Validation**: Cannot declare war on self or declare twice

### Combat Mechanics
Combat occurs daily when opposing armies occupy the same province.

#### Combat Power
Each regiment type has base combat power:
- **Infantry**: 1.0
- **Cavalry**: 1.5
- **Artillery**: 1.2

Total power scales with regiment strength (men count).

#### Casualties
- **Daily Rate**: 1% of strength per day (modified by power ratio)
- **Power Ratio**: Side with more power deals proportionally more damage
- **Formula**: `Casualties = Strength × 0.01 × (Enemy Power / Total Power)`
- **Destruction**: Regiments reduced to 0 strength are removed
- **Army Removal**: Armies with no regiments are removed from the map

### Technical Details

#### Fixed-Point Arithmetic

The simulation uses two fixed-point types for deterministic calculations:

| Type | Backing | Range | Precision | Use Case |
|------|---------|-------|-----------|----------|
| `Fixed` | i64 | ±922 trillion | 0.0001 | Treasury, mana pools, large aggregates |
| `Mod32` | i32 | ±214,000 | 0.0001 | Province stats, modifiers, SIMD batches |

**Why two types?**
- `Mod32` enables SIMD vectorization (8 values per AVX2 instruction)
- `Fixed` handles large accumulations without overflow
- Convert at boundaries: `mod32.to_fixed()` when adding to treasury

#### SIMD-Accelerated Taxation

The taxation system uses hybrid rayon + SIMD processing:

1. **Group by owner**: Provinces collected per country
2. **SIMD batch**: Each country's provinces processed with AVX2 (8 at a time)
3. **Rayon parallel**: Countries processed across CPU cores

See `eu4sim-core/src/simd/tax32.rs` and `eu4sim-core/src/systems/taxation.rs`.

**Performance**: ~2.16x faster than scalar i64 implementation.

#### Determinism
- All calculations use integer fixed-point (no floating-point)
- SIMD implementations validated against scalar golden implementations via proptest
- Strict bit-exact reproducibility for lockstep networking

#### Daily Combat
Combat resolution runs every simulation tick (daily)
