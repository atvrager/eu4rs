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
- **Fixed Point**: Uses `i64` scaled by 10,000 for precision (1.0000).
- **Determinism**: All calculations are strictly deterministic for lockstep networking.
- **Daily Combat**: Combat resolution runs every simulation tick (daily)
