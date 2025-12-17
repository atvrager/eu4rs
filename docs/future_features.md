# Future Features

This document tracks planned features that are deferred from the MVP implementation.

## Fog of War

### Overview

Fog of War is a game mechanic where players have limited visibility into enemy territories, positions, and activities. In EU4, you can only see:
- Your own provinces and armies
- Provinces/armies of allied nations
- Enemy provinces/armies in regions where you have visibility (through your units, forts, or allied vision)

### Current State (MVP)

The MVP implementation assumes **perfect information**:
- All players can see all provinces, armies, and state information
- Movement validation is done server-side with full map knowledge
- Combat resolution is visible to all participants

### Proposed Implementation

#### 1. Vision System

Each country maintains a `VisionState` that tracks what they can currently see:

```rust
pub struct VisionState {
    /// Provinces that are currently visible
    pub visible_provinces: HashSet<ProvinceId>,
    /// Last known state of armies (may be outdated)
    pub known_armies: HashMap<ArmyId, KnownArmyState>,
    /// Last seen timestamp for each province
    pub province_last_seen: HashMap<ProvinceId, Date>,
}

pub struct KnownArmyState {
    pub location: ProvinceId,
    pub owner: Tag,
    pub estimated_strength: Option<u32>,
    pub last_seen: Date,
}
```

#### 2. Vision Sources

Provinces become visible through:
1. **Own territories**: All owned provinces are always visible
2. **Adjacent to own provinces**: Provinces bordering owned land are visible
3. **Army presence**: Provinces containing own armies grant vision
4. **Allied vision**: Vision shared from allies (if diplomatic setting enabled)
5. **Forts**: High-level forts extend vision range
6. **Naval vision**: Fleets grant vision in sea provinces and adjacent coastal provinces

#### 3. State Filtering

When serving game state to clients, filter based on vision:
- Only send visible province states
- Hide army compositions unless in same province or recently engaged in combat
- Redact country treasury/manpower for enemies
- Show approximate army strength as range (e.g., "5,000-10,000") when partially visible

#### 4. Validation Changes

Movement validation becomes more complex:
- Players issue movement orders without seeing full enemy positions
- Server validates against real state (including hidden armies)
- Combat triggers even if player didn't know enemy was present
- Client receives "army discovered" events when entering provinces with unexpected enemies

#### 5. Multiplayer Considerations

Fog of War is critical for multiplayer to prevent:
- Perfect information giving unfair strategic advantages
- Players coordinating against others using out-of-game communication
- Deterministic replay exploits (players could replay to "scout" fog)

However, it adds complexity:
- Increased network traffic (per-player state filtering)
- Potential for desyncs if vision calculations differ
- Cheating detection: players could modify client to reveal fog

### Implementation Phases

**Phase 1: Basic Vision (Post-MVP)**
- Implement VisionState per country
- Add vision calculation based on owned provinces + armies
- Filter WorldState before sending to clients

**Phase 2: Advanced Vision**
- Fort-based extended vision
- Allied vision sharing
- Naval vision mechanics
- Terrain-based vision modifiers (e.g., mountains block vision)

**Phase 3: Historical Information**
- Track "last known" state for provinces outside vision
- UI shows outdated information with timestamp
- Scouts/spies can refresh information without requiring army presence

**Phase 4: Anti-Cheat**
- Server-authoritative movement validation
- Encrypted state transmission
- Replay validation with server-side recording

### Open Questions

1. **Determinism**: How do we maintain deterministic simulation if vision affects behavior?
   - Solution: Vision is derived state, not part of core simulation. Core sim runs with perfect info, vision only affects what players *see*, not game logic.

2. **Replay compatibility**: If replays show perfect information, does that break fog of war?
   - Solution: Replays can have two modes: "as played" (fog applied) vs "observer" (full visibility).

3. **Performance**: Calculating vision for 10+ players every tick could be expensive.
   - Solution: Cache vision state, only recalculate when provinces change ownership or armies move.

4. **Test coverage**: How do we test fog of war without game UI?
   - Solution: Unit tests can verify VisionState calculation. Integration tests can simulate two-player scenarios and verify correct filtering.

### Related Systems

- **Espionage**: Future spy mechanics could allow temporary vision into enemy territory
- **Diplomacy**: Military access should grant vision in allied provinces
- **Trade**: Trade nodes might grant limited economic intel without full vision
- **Casus Belli**: Certain war goals might require having seen the target province recently

### References

- EU4 Wiki: https://eu4.paradoxwikis.com/Fog_of_war
- Terra Invicta vision system: Similar grand strategy game with fog mechanics
- Age of Empires 2 fog implementation: RTS reference for real-time vision updates
