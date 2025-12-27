# Buildings Implementation Plan Review Findings

**Reviewer**: Gemini 3 Pro (High)
**Date**: 2025-12-25

## 1. Data Structure Efficiency
**Question**: Is `HashSet<BuildingId>` the right choice?
**Finding**: **Suboptimal**.
While `HashSet` is functionally correct, it has high overhead for small sets (typically <10 items) replicated across thousands of provinces.
**Recommendation**:
- **Best**: Use a `u64` (or `u128`) **bitmask** if we can guarantee building IDs are sequential and < 64 (or 128). This is zero-allocation and O(1) for all checks.
- **Good**: `Vec<BuildingId>` (kept sorted or just linear scan). Linear scan over 10 items is faster than hashing.
- **Acceptable**: `HashSet` (easiest to write, acceptable performance for prototype, but heavier on memory).
Notes from atv: bitmasks are fucking sexy.

## 2. Modifier Accumulation
**Question**: Recompute vs Incremental?
**Finding**: **Recompute Local State**.
Incrementally updating global modifiers is error-prone (drift, missed de-allocations).
**Recommendation**:
Implement a `recompute_province_modifiers(province_id)` function.
- When a building completes/destroys:
  1. Clear modifiers *originating from this province*.
  2. Iterate this province's buildings.
  3. Sum their effects.
  4. Update the global/cached state.
This isolates the cost to the single affected province, keeping it O(1) relative to world size, while ensuring correctness.

## 3. AI Scoring Balance
**Question**: Hardcoded scores vs ROI?
**Finding**: **Stick to Hardcoded for Phase 4**.
Calculating true ROI requires complex contextual data (trade node flows, tax efficiency chains, etc.) which is likely premature.
**Recommendation**:
- Use the proposed hardcoded scores to match existing `GreedyAI` patterns.
- **Refinement**: Add a simple multiplier for "Main Trade Port" or "Center of Trade" for trade buildings.
- **Refinement**: Ensure military buildings (Manpower) strictly check `base_manpower` to avoid building barracks in 3-dev deserts.

## 4. Manufactory Eligibility Edge Cases
**Question**: Trade good changes?
**Finding**: **Invalidation Required**.
**Recommendation**:
- **Rule**: If a province's trade good changes (event/script), trigger a validator.
- **Action**: If the existing manufactory is no longer valid for the new good, **destroy it instantly**.
- **Conflict**: If multiple manufactories become valid, the player/AI chooses. If automated (e.g., history file load), prioritizing the one with higher raw value is a safe default, but usually this is a player decision.

## 5. Fort Building Migration
**Question**: Hybrid `fort_level` vs Buildings?
**Finding**: **Risk of Desync**.
Storing `fort_level` separately from the Fort Building creates two sources of truth.
**Recommendation**:
- Treat `fort_level` in `ProvinceState` as a **Cached Property**, not a Source of Truth.
- **Getter**: `completion_of_fort_building` -> triggers `recompute_fort_level()`.
- `recompute_fort_level()` = `Capital(1) + Max(Building_Fort_Levels)`.
- **Note**: Modifiers (maintenance) should look at the *building*, while combat/ZOC looks at the *level*.

## 6. Construction Cancellation
**Question**: Conquest/Owner Change/Manual?
**Finding**: Standard EU4 Rules apply.
**Recommendation**:
- **Conquest**: **Cancel immediately**. No refund. The invader does not inherit the construction queue.
- **Diplomatic Annex**: **Inherit**. If specific building is invalid for new owner (tech/religion unique), cancel + refund.
- **Manual Cancel**: **100% Refund**.

## 7. Slot Calculation
**Question**: `(dev / 10) + 1`?
**Finding**: **Authentic Approximation**.
EU4 formula is indeed `Base + Floor(Total_Dev / 10)`.
- Base is usually 2, not 1, but depends on terrain.
- **Correction**: Terrain drastically affects this (Mountains -1, Farmlands +1).
- **Recommendation**: Add a `terrain_slot_modifier` lookup. Without it, mountains will have too many slots.

## 8. Missing Considerations

### A. Technology Requirements
**Critical**: The plan mentions cost/time parsing but misses **Tech Requirements**.
- Buildings must enforce `adm_tech`, `dip_tech`, or `mil_tech` triggers.
- Without this, AI will build Temples at Tech 3 (Year 1444), which breaks balance.

### B. Building Upgrades
**Mechanism**: EU4 buildings often form chains (Church -> Cathedral).
- **Rule**: You cannot build a Church if you have a Cathedral.
- **Rule**: Building a Cathedral *replaces* the Church (cost is difference? or full cost? In EU4 it's "upgrade" price, usually full price but replaces slot).
- **Impl**: `BuildingDef` needs `replaces_building: Option<BuildingId>`.

### C. One-Way Upgrades
- Ensure `can_build` checks `!has_building(self) && !has_better_version(self)`.

### D. Macro Builder Support
- Future-proofing: Data structures should support "Sort by ROI" eventually, even if AI is greedy now.

---

**Next Steps**:
Return to coding session to implement Phase 0 (Codegen) with added `tech_required` fields.
