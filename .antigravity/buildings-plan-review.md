# Antigravity Handoff: Buildings Implementation Plan Review

**Task**: Review the buildings implementation plan for the EU4 simulation project
**Requested Reviewer**: Gemini (large context analysis)
**Return To**: Claude Code session

---

## Context

This is an EU4 (Europa Universalis IV) simulation project in Rust. We're adding a buildings system that allows provinces to construct buildings that provide economic/military bonuses.

## Design Decisions (Already Locked)

| Decision | Value |
|----------|-------|
| Construction | 1 at a time per province |
| Slot limits | 1 of each type; total slots dev-based |
| Manufactories | 1 per province; eligibility by trade good |
| Destruction | Yes, can demolish |
| Cost | Gold only |
| Conquest | Buildings survive |

---

## Implementation Plan Summary

### Phase 0: Codegen Enhancement
Update schema inference to parse `cost`, `time`, and `manufactory` fields from `common/buildings/*.txt`.

### Phase 1: Core Data Structures

**New types**:
- `BuildingId(u8)` - Type-safe ID
- `BuildingDef` - Static definition with cost, time, modifiers, manufactory goods
- `BuildingConstruction` - Progress tracking

**ProvinceState additions**:
```rust
pub buildings: HashSet<BuildingId>,
pub building_construction: Option<BuildingConstruction>,
pub has_port: bool,
```

### Phase 2: Construction System

- `can_build()` - Validate slot limits, gold, manufactory eligibility
- `start_construction()` - Deduct gold, set progress
- `tick_buildings()` - Monthly progress, completion
- `DemolishBuilding` command

### Phase 3: Modifier Integration

Buildings contribute to existing `GameModifiers`:
- `province_tax_modifier`
- `province_production_efficiency`
- Trade power, manpower, etc.

### Phase 4: AI Integration

GreedyAI scoring by building type:
- Manufactories: 400 (high ROI)
- Production buildings: 250
- Tax buildings: 200
- Trade buildings: 180

### Phase 5: Save Hydration

Hydrate `buildings` from save files via eu4sim-verify, already extracts `Vec<String>`.

---

## Review Questions for Gemini

1. **Data Structure Efficiency**: Is `HashSet<BuildingId>` the right choice for storing completed buildings per province? (~70 possible buildings, typically 5-10 per province)

2. **Modifier Accumulation**: The plan recalculates all province modifiers from buildings on completion. Should we instead incrementally update modifiers when buildings complete/demolish?

3. **AI Scoring Balance**: The hardcoded scores (manufactories=400, workshops=250, etc.) - any concerns about this approach vs. calculating actual ROI?

4. **Manufactory Eligibility Edge Cases**:
   - Province changes trade good after manufactory built - keep or invalidate?
   - Multiple manufactories become eligible (trade good change) - how to prioritize?

5. **Fort Building Migration**: Currently `fort_level: u8` is separate from buildings. The plan keeps them separate with forts updating the level. Any issues with this hybrid approach?

6. **Construction Cancellation**: What should happen if:
   - Province is conquered mid-construction?
   - Province owner changes via event?
   - Player manually cancels?

7. **Slot Calculation**: `(dev / 10) + 1, max 12` - does this match EU4's actual formula?

8. **Missing Considerations**: Any critical aspects of EU4 buildings we're not addressing?

---

## File Structure Reference

```
eu4data/src/generated/types/buildings.rs  - Auto-generated from game files
eu4sim-core/src/
  modifiers.rs                             - BuildingId type
  buildings.rs                             - BuildingDef (NEW)
  state.rs                                 - ProvinceState, WorldState
  systems/buildings.rs                     - Construction logic (NEW)
  step.rs                                  - Command execution
eu4sim/src/loader.rs                       - Load building defs
eu4sim-verify/src/hydrate.rs               - Save hydration
eu4sim-ai/src/ai/greedy.rs                 - AI scoring
```

---

## Expected Output

Please provide:
1. Answers/recommendations for the review questions
2. Any architectural concerns
3. Suggestions for improvements
4. Potential edge cases we missed

After review, return findings to Claude Code for implementation.
