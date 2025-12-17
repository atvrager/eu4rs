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

### Technical Details
- **Fixed Point**: Uses `i64` scaled by 10,000 for precision (1.0000).
- **Determinism**: All calculations are strictly deterministic for lockstep networking.
