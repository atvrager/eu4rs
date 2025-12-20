# Simulation Integrity System

> **Status**: Phase 3 Complete (Cache Validation)  
> **Author**: eu4rs Team  
> **Date**: 2025-12-17

## Overview

This document describes the integrity system for `eu4rs`, ensuring deterministic simulation across multiple clients and protecting against data corruption in multiplayer scenarios.

## Unified Single-Player/Multiplayer Architecture

**Core Principle**: Single-player and multiplayer are the same system.

- **Single-player** = Lobby with 1 human player + N AI players
- **Multiplayer** = Lobby with M human players + N AI players

**AI Load Balancing**: AI computation can be distributed across clients based on available resources:
- Beefier PCs handle more AI players
- Load balancing negotiated during lobby setup
- Each client runs `step_world()` for their assigned countries (human + AI)
- Deterministic checksums ensure all clients stay in sync

**Benefits**:
- No separate code paths for single-player vs multiplayer
- AI can be offloaded from weaker clients
- Replay system works identically for both modes
- Testing multiplayer = testing single-player
- **Verified Core**: (Vision) Using formal methods (Lean 4/Coq) to prove simulation invariants. See [formal-methods.md](./formal-methods.md).

---

## Goals

1. **Determinism**: Identical inputs produce identical outputs on all clients.
2. **Early Desync Detection**: Detect state divergence within ticks, not hours.
3. **Perfect Replay**: Reconstruct any game from initial state + input log.
4. **Data Integrity**: Ensure all clients use compatible game data.
5. **Zero Trust Caching**: Caches auto-invalidate when sources change.

## Non-Goals (for now)

- **Anti-Cheat**: We don't aim to prevent determined attackers who recompile.
- **Encrypted State**: State is not encrypted; observers can inspect it.
- **Byzantine Fault Tolerance**: We assume clients are cooperative, not malicious.

---

## Architecture

### 1. Game Data Manifest (Build-Time)

At compile time, we generate a manifest of all game data files used:

```
┌──────────────────────────────────────────────────────────────┐
│                     build.rs (eu4data)                        │
├──────────────────────────────────────────────────────────────┤
│  1. Scan game data files (definition.csv, provinces.bmp...)  │
│  2. Compute SHA256 of each file                               │
│  3. Generate combined manifest_hash                           │
│  4. Embed as const GAME_MANIFEST in binary                    │
└──────────────────────────────────────────────────────────────┘
```

**Manifest Structure**:
```rust
pub struct GameDataManifest {
    /// Simulation library version
    pub sim_version: &'static str,
    
    /// Git commit hash (if available)
    pub git_commit: Option<&'static str>,
    
    /// Individual file hashes
    pub file_hashes: &'static [FileHash],
    
    /// Combined hash of all file hashes (deterministic order)
    pub manifest_hash: [u8; 32],
}

pub struct FileHash {
    pub path: &'static str,
    pub sha256: [u8; 32],
}
```

**Use Case**: On multiplayer connect, clients exchange `manifest_hash`. Any mismatch = incompatible builds.

---

### 2. Cache Integrity

Caches (adjacency graph, parsed data, etc.) must match the game data they were generated from.

**Cache Metadata**:
```rust
pub struct CacheMetadata {
    /// Hash of source files at generation time
    pub source_hashes: HashMap<PathBuf, [u8; 32]>,
    
    /// Modification times (faster validation for local use)
    pub source_mtimes: HashMap<PathBuf, SystemTime>,
    
    /// Manifest hash this cache was built against
    pub manifest_hash: [u8; 32],
    
    /// Cache generation timestamp
    pub generated_at: SystemTime,
    
    /// Hash of the cached data itself
    pub data_hash: [u8; 32],
}
```

**Validation Flow**:
```
1. Load cache metadata
2. Compare manifest_hash with GAME_MANIFEST.manifest_hash
   → Mismatch? Regenerate.
3. Compare source_hashes with current file hashes
   → Mismatch? Regenerate.
4. Load cached data
5. Verify data_hash matches loaded content
   → Mismatch? Corrupt cache, regenerate.
```

**Cache Location**: `~/.cache/eu4rs/{name}.json` + `{name}.meta.json`

---

### 2.1. Text Encoding & Character Preservation

Paradox game data traditionally uses legacy encodings that are incompatible with default UTF-8 loaders. To maintain visual integrity and prevent "mojibake" (corrupted text), we enforce strict rules:

- **Mandatory Encoding**: All script (`.txt`) and CSV (`.csv`) files MUST be loaded using **`WINDOWS-1252`** (CP1252).
- **Zero Loss Promise**: Regional characters (e.g., Swedish `å`, German `ö`) must be perfectly preserved. We use `encoding_rs` to decode bytes before parsing.
- **Verification**: All loaders must have unit tests verifying preservation of `WINDOWS-1252` specific characters.

**Status: ✅ IMPLEMENTED** in `eu4txt` and `eu4data` loaders.

---

### 3. Runtime State Checksums

During simulation, we compute checksums of `WorldState` to detect desync.

**Checksum Scope** (deterministic fields only):
```rust
impl WorldState {
    pub fn checksum(&self) -> u64 {
        use std::hash::{Hash, Hasher};
        use std::collections::hash_map::DefaultHasher;
        
        let mut hasher = DefaultHasher::new();
        
        // Date
        self.date.hash(&mut hasher);
        
        // Countries (sorted by tag for determinism)
        let mut tags: Vec<_> = self.countries.keys().collect();
        tags.sort();
        for tag in tags {
            let c = &self.countries[tag];
            tag.hash(&mut hasher);
            c.treasury.0.hash(&mut hasher);  // Fixed backing type
            c.manpower.0.hash(&mut hasher);
        }
        
        // Provinces (sorted by ID)
        let mut ids: Vec<_> = self.provinces.keys().collect();
        ids.sort();
        for id in ids {
            let p = &self.provinces[id];
            id.hash(&mut hasher);
            p.owner.hash(&mut hasher);
            // ... other deterministic fields
        }
        
        // Armies, Fleets, Wars...
        
        hasher.finish()
    }
}

**Optimization**: For large states, sorting HashMaps every tick is expensive. Consider:
- Use `BTreeMap` instead of `HashMap` for deterministic iteration without sorting
- Cache checksums with state versioning to skip recomputation
- Compute checksums incrementally (only hash changed fields)

**Floating-Point Determinism**: The `Fixed` type ensures deterministic arithmetic, but watch for:
- Division operations (ensure rounding mode is consistent)
- Avoid transcendental functions (sin/cos/sqrt) or use lookup tables
- Document `Fixed` precision guarantees (currently i64 with 16 fractional bits)
```

**Checksum Frequency**: Configurable via `SimConfig`:
```rust
pub struct SimConfig {
    /// Compute checksum every N ticks (0 = disabled)
    pub checksum_frequency: u32,
}
```

**Guidelines**:
- `1`: Every tick (safest, ~0.5ms overhead per tick)
- `30`: Every month (~1 tick/day for 30 days)
- `365`: Every year (lowest overhead, slowest detection)

**RNG State Management**:
```rust
pub struct WorldState {
    pub rng_seed: u64,
    /// Current RNG state (must be deterministic)
    pub rng_state: u64,
}
```

> [!IMPORTANT]
> All randomness MUST go through `WorldState`'s RNG, never `thread_rng()` or `rand::random()`. This ensures replay determinism.

---

### 4. Multiplayer Protocol

**Handshake**:
```
Client → Host: { manifest_hash, sim_version }
Host:
  if manifest_hash != GAME_MANIFEST.manifest_hash:
    Reject("Game data version mismatch")
  if sim_version != env!("CARGO_PKG_VERSION"):
    Reject("Simulation version mismatch")
  else:
    Accept, send initial world state
```

**Runtime Sync**:
```
Each tick:
  1. Host broadcasts: { tick, checksum, inputs[] }
  2. Clients apply inputs, compute local checksum
  3. Client compares: local_checksum == host_checksum?
     → Match: continue
     → Mismatch: request resync or disconnect
```

**Resync Strategy** (future):
- Full state transfer (simple, expensive)
- Delta compression (complex, efficient)
- Checkpoint + replay from last good state

---

### 5. Replay System

Replays store inputs, not states:

```rust
pub struct Replay {
    /// Initial world state hash (for validation)
    pub initial_state_hash: [u8; 32],
    
    /// Manifest hash (game data version)
    pub manifest_hash: [u8; 32],
    
    /// Simulation version
    pub sim_version: String,
    
    /// Ordered list of all inputs
    pub inputs: Vec<TickInputs>,
}

pub struct TickInputs {
    pub tick: u64,
    pub player_inputs: Vec<PlayerInputs>,
}
```

**Replay Validation**:
```
1. Check manifest_hash matches current GAME_MANIFEST
2. Check sim_version matches current binary
3. Load initial state, verify hash
4. Apply all inputs sequentially
5. Final state should be reproducible

**Replay Checkpoints** (optimization for long games):
```rust
pub struct Replay {
    // ...
    /// Periodic state snapshots for fast-forward (every 365 ticks)
    pub checkpoints: Vec<(u64, WorldState)>,
}
```
Allows seeking to year 1500 without replaying from 1444.
```

---

## Implementation Phases

### Phase 1: Foundation
- [ ] Add `WorldState::checksum()` method
- [ ] Add `checksum_frequency` to `SimConfig`
- [ ] Integrate checksum logging in simulation loop

### Phase 2: Build-Time Manifest
- [x] Create `build.rs` in `eu4data` to hash source files
- [x] Generate `GAME_MANIFEST` constant
- [x] Add `manifest_hash` to `CacheMetadata`

### Phase 3: Cache Validation
- [x] Enhance cache loader to verify `manifest_hash`
- [x] Add `data_hash` field for integrity check
- [x] Implement auto-regeneration on mismatch
- [x] Use mtimes for fast-path, hashes for multiplayer validation

### Phase 4: Multiplayer Integration
*(Note: Distinct from "Phase 4: Movement" in `military.md`)*
- [ ] Define handshake protocol

- [ ] Implement checksum comparison per tick
- [ ] Add desync detection and logging

### Phase 5: Replay System
- [ ] Define replay format
- [ ] Implement replay recording
- [ ] Implement replay playback with validation
- [ ] Add periodic checkpoints for fast-forward

---

## Trade-offs

| Decision | Trade-off |
|----------|-----------|
| SHA256 for hashes | Slower than xxHash, but standard and collision-resistant |
| Per-tick checksums | 0.5ms overhead, but instant desync detection |
| Manifest at build time | Requires rebuild for game data changes, but zero runtime cost |
| Replay = inputs only | Small file size, but requires exact same binary to replay |

---

## Security Considerations

This system provides **integrity**, not **security**:

- ✅ Detects accidental corruption
- ✅ Detects version mismatches
- ✅ Ensures reproducibility
- ❌ Does NOT prevent a malicious client from sending false checksums
- ❌ Does NOT encrypt data in transit

For competitive multiplayer, a server-authoritative model with state broadcasting would be required.

---

## References

- [Gaffer On Games: Deterministic Lockstep](https://gafferongames.com/post/deterministic_lockstep/)
- [Age of Empires Networking](https://www.gamedeveloper.com/programming/1500-archers-on-a-28-8-network-programming-in-age-of-empires)
- [Factorio Friday Facts #302: The Multiplayer Story](https://www.factorio.com/blog/post/fff-302)
