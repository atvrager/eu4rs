# Game Lobby System Design

> **Status**: Draft
> **Author**: eu4rs Team
> **Date**: 2025-12-17
> **Prerequisites**: [Integrity System](simulation/integrity.md)

## Overview

The lobby system coordinates player connections, game setup, and the transition into synchronized simulation. It implements the **Unified Single-Player/Multiplayer Architecture** established in the integrity design: single-player is simply a lobby of one.

**Key Insight**: We're not building two systems (SP + MP). We're building *one* networked simulation with a degenerate case of N=1.

---

## Goals

1. **Unified Architecture**: Single-player and multiplayer share identical code paths
2. **Performance**: Sub-10ms round-trip for lockstep coordination (LAN), <100ms (Internet)
3. **Security**: Encrypted transport, manifest validation, no trust in client state
4. **Simplicity**: Minimal protocol, clear state machine, easy debugging
5. **NAT Traversal**: "It just works" for residential connections
6. **Hot Join**: Players can join/leave mid-game without requiring restart

## Non-Goals (Phase 1)

- **Matchmaking Service**: No centralized matchmaking; direct connect or LAN discovery
- **Spectator Mode**: Observers will come later
- **Anti-Cheat**: We detect desync, not malice (see integrity doc)
- **Cross-Play**: Same binary required; no heterogeneous builds

---

## Architecture Overview

```
┌─────────────────────────────────────────────────────────────────────┐
│                          LOBBY LIFECYCLE                            │
├─────────────────────────────────────────────────────────────────────┤
│                                                                     │
│   ┌─────────┐    ┌─────────┐    ┌─────────┐    ┌─────────────────┐ │
│   │  IDLE   │───▶│ HOSTING │───▶│  SETUP  │───▶│    IN-GAME      │ │
│   │         │    │         │    │         │    │  (Lockstep Sim) │ │
│   └─────────┘    └─────────┘    └─────────┘    └─────────────────┘ │
│        │              ▲              │                   │          │
│        │              │              │                   │          │
│        ▼              │              ▼                   ▼          │
│   ┌─────────┐         │         ┌─────────┐    ┌─────────────────┐ │
│   │ JOINING │─────────┘         │  READY  │───▶│    FINISHED     │ │
│   │ (Client)│                   │  CHECK  │    │  (Return to     │ │
│   └─────────┘                   └─────────┘    │   Lobby/Exit)   │ │
│                                                └─────────────────┘ │
└─────────────────────────────────────────────────────────────────────┘
```

### Roles

| Role | Description |
|------|-------------|
| **Host** | Creates lobby, authoritative for game setup, coordinates tick broadcasting |
| **Client** | Joins existing lobby, follows host's tick schedule |
| **Local** | Degenerate case: Host with no network (single-player optimization) |

**Important**: Host is *not* server-authoritative for simulation. All clients run identical `step_world()`. Host is authoritative for:
- Lobby membership (kick, accept)
- Tick pacing (when to advance)
- Initial state distribution
- Desync arbitration (their checksum wins ties)

---

## Network Topology

### P2P with Host Coordination (Recommended)

```
        ┌─────────┐
        │  HOST   │
        │ (P2P    │
        │  Hub)   │
        └────┬────┘
             │
    ┌────────┼────────┐
    │        │        │
    ▼        ▼        ▼
┌───────┐┌───────┐┌───────┐
│Client1││Client2││Client3│
└───────┘└───────┘└───────┘
```

**Topology**: Star topology with host as hub. Clients communicate *only* with host.

**Rationale**:
- Simplifies NAT traversal (only host needs public reachability OR relay)
- Single point of truth for tick coordination
- Scales to 8-12 players easily (EU4 typical max)
- Host can be a dedicated server for competitive play

**Trade-off**: Single point of failure. If host drops, game ends (or requires host migration—Phase 2+).

### Why Not Full Mesh P2P?

Full mesh (every client connects to every client) has O(N²) connections. For 8 players, that's 28 connections to maintain, each with NAT traversal challenges. Not worth the complexity.

### Why Not Pure Client-Server?

We *could* run a headless simulation server that broadcasts state. But:
- Requires dedicated infrastructure (cost, maintenance)
- Adds latency (all inputs round-trip through server)
- We already have deterministic lockstep; leverage it

**Decision**: P2P star topology for community games, optional dedicated server mode for competitive.

---

## Protocol Design

### Transport Layer: QUIC

**Library**: `quinn` (Rust QUIC implementation)

**Why QUIC?**
- **Encrypted by default**: TLS 1.3 built-in, no separate security layer needed
- **NAT-friendly**: UDP-based, better hole-punching than TCP
- **Stream multiplexing**: Multiple logical streams over one connection
- **0-RTT reconnection**: Fast resume after brief disconnects
- **Congestion control**: Modern algorithms (BBR) built-in

**Why not raw UDP?**
- We'd have to implement reliability, ordering, encryption ourselves
- `quinn` is mature, well-maintained, and battle-tested
- QUIC overhead is minimal (~20 bytes per packet)

**Why not TCP?**
- Head-of-line blocking kills lockstep performance
- NAT traversal is harder
- No native multiplexing

### Message Framing

Custom binary protocol over QUIC streams. We roll our own for performance; the security comes from QUIC's TLS.

```rust
/// All messages are prefixed with a header
#[repr(C, packed)]
pub struct MessageHeader {
    /// Message type discriminant
    pub msg_type: u8,
    /// Payload length (little-endian)
    pub length: u16,
}

/// Message types
#[repr(u8)]
pub enum MessageType {
    // Handshake
    Hello = 0x01,
    HelloAck = 0x02,
    Reject = 0x03,

    // Lobby
    LobbyState = 0x10,
    SlotAssign = 0x11,
    PlayerReady = 0x12,
    ChatMessage = 0x13,

    // Game
    TickStart = 0x20,
    PlayerInput = 0x21,
    Checksum = 0x22,
    DesyncAlert = 0x23,
    Pause = 0x24,
    Resume = 0x25,

    // Control
    Ping = 0xF0,
    Pong = 0xF1,
    Disconnect = 0xFF,
}
```

**Zero-Copy Parsing**: Use `zerocopy` crate for header parsing. Payloads use `rkyv` for zero-copy deserialization of complex types.

### Handshake Protocol

```
┌────────┐                              ┌────────┐
│ Client │                              │  Host  │
└───┬────┘                              └───┬────┘
    │                                       │
    │  ──────── QUIC Connect ───────────▶  │
    │                                       │
    │  ◀─────── QUIC Accept ────────────   │
    │                                       │
    │  ──────── Hello ──────────────────▶  │
    │  {                                    │
    │    manifest_hash: [u8; 32],           │
    │    sim_version: "0.1.4",              │
    │    git_commit: [u8; 20],              │ // SHA1 hash for strict matching
    │    client_token: [u8; 16],            │ // Persistent UUID for reconnection
    │    player_name: "Gustav II",          │
    │  }                                    │
    │                                       │
    │           ┌───────────────────────┐   │
    │           │ Validate:             │   │
    │           │ - manifest_hash match │   │
    │           │ - sim_version match   │   │
    │           │ - lobby not full      │   │
    │           └───────────────────────┘   │
    │                                       │
    │  ◀─────── HelloAck ───────────────   │
    │  {                                    │
    │    player_id: u8,                     │
    │    lobby_state: LobbyState,           │
    │  }                                    │
    │           OR                          │
    │  ◀─────── Reject ─────────────────   │
    │  {                                    │
    │    reason: RejectReason,              │
    │  }                                    │
    │                                       │
```

**Reject Reasons**:
```rust
pub enum RejectReason {
    ManifestMismatch { expected: [u8; 32], got: [u8; 32] },
    VersionMismatch { expected: String, got: String },
    LobbyFull,
    Banned,
    GameInProgress,  // Until hot-join implemented
}
```

### Lobby State Machine

```rust
pub struct LobbyState {
    /// Unique lobby identifier (internal)
    pub lobby_id: u64,

    /// Human-friendly join code (e.g. "ABCD4")
    pub join_code: String,

    /// All player slots (human + AI)
    pub slots: Vec<Slot>,

    /// Game settings
    pub settings: GameSettings,

    /// Current phase
    pub phase: LobbyPhase,
}

pub struct Slot {
    pub slot_id: u8,
    pub assignment: SlotAssignment,
    pub country: Option<Tag>,
    pub ready: bool,
}

pub enum SlotAssignment {
    Empty,
    Human { player_id: u8, name: String },
    AI { difficulty: AIDifficulty, assigned_to: Option<u8> },  // assigned_to = which client computes this AI
    Closed,
}

pub enum LobbyPhase {
    Setup,      // Players joining, picking countries
    Loading,    // Initial state being distributed
    InGame,     // Lockstep simulation active
    Paused,     // Simulation paused (player request or desync)
    Finished,   // Game ended, reviewing results
}
```

### Tick Synchronization Protocol

Once in-game, the host coordinates lockstep:

```
┌────────┐      ┌────────┐      ┌────────┐
│  Host  │      │Client 1│      │Client 2│
└───┬────┘      └───┬────┘      └───┬────┘
    │               │               │
    │ ◀── Input ────┤               │   Clients send inputs
    │ ◀── Input ────┼───────────────┤   for their controlled
    │               │               │   countries (human + AI)
    │               │               │
    │ ── TickStart ─┼───────────────┼▶  Host broadcasts:
    │ {             │               │   - All collected inputs
    │   tick: 1234, │               │   - Tick number
    │   inputs: [], │               │   - (Optional) host checksum
    │   checksum?,  │               │
    │ }             │               │
    │               │               │
    │               ▼               ▼
    │         ┌──────────┐   ┌──────────┐
    │         │step_world│   │step_world│  All clients simulate
    │         └──────────┘   └──────────┘  identically
    │               │               │
    │ ◀── Checksum ─┤               │   Clients report checksums
    │ ◀── Checksum ─┼───────────────┤   (frequency configurable)
    │               │               │
    │ Compare...    │               │
    │               │               │
    ▼               ▼               ▼
```

**Input Collection Window**: Host waits for all clients' inputs before broadcasting `TickStart`. Configurable timeout (50-200ms) determines tick rate ceiling.

**Pacing**: Host controls simulation speed:
- Speed 1: 1 tick/second
- Speed 5: 5 ticks/second
- Maximum: Limited by slowest client + network RTT

---

## NAT Traversal: "Connect Anywhere"

The network layer must "just work" for residential internet connections. **Zero configuration required** by the user. Port forwarding and manual IP entry are deprecated.

### Strategy: Aggressive Hole Punching + Relay Fallback

We prioritize direct P2P (low latency) but seamlessly fall back to Relay (guaranteed connectivity) if NAT fails.

```
1. Host binds to QUIC endpoint
2. Host & Client query STUN server to map Public IP:Port
3. Protocol manages "Hole Punching" (simultaneous connection attempt)
4. IF Direct Connect fails -> Automatically route via TURN Relay
```

**Architecture**:
- **STUN**: Mandatory for all clients. Resolves public reflexive address.
- **Hole Punching**: The `quinn` / `ice` layer attempts mutual connectivity.
- **Relay (TURN)**: Heavyweight fallback. If hole punching fails (Symmetric NAT), data routes through a relay server. To avoid "weird services", we will either:
    - Host a managed TURN cluster for the official game
    - Use a library that abstracts this (e.g. `libp2p`'s relay or Steam Datagram Relay if moving to Steam)
    - **Design Goal**: The user *never* installs a separate server executable for peer-to-peer play.

**Libraries**:
- `stun-rs` / `webrtc-ice` for negotiation
- `turn-rs` for relay logic

**Optimization**: For LAN play, skip STUN entirely—broadcast UDP discovery on local subnet.

### LAN Discovery

```rust
/// Broadcast on 255.255.255.255:EU4RS_DISCOVERY_PORT
pub struct DiscoveryBroadcast {
    pub magic: [u8; 4],  // "EU4R"
    pub lobby_id: u64,
    pub host_name: String,
    pub player_count: u8,
    pub max_players: u8,
    pub connect_port: u16,
}
```

Host broadcasts every 2 seconds. Clients listen and populate server browser.

---

## AI Distribution

Per the integrity doc, AI computation can be distributed. Here's how:

### Assignment Algorithm

```rust
/// Called when game starts or player joins/leaves
fn assign_ai_to_clients(lobby: &mut LobbyState, clients: &[ClientInfo]) {
    let ai_slots: Vec<_> = lobby.slots.iter_mut()
        .filter(|s| matches!(s.assignment, SlotAssignment::AI { .. }))
        .collect();

    if ai_slots.is_empty() {
        return;
    }

    // Sort clients by "horsepower score" (self-reported)
    let mut sorted_clients: Vec<_> = clients.iter()
        .map(|c| (c.player_id, c.horsepower_score))
        .collect();
    sorted_clients.sort_by_key(|(_, hp)| std::cmp::Reverse(*hp));

    // Round-robin assign AIs to clients, weighted by horsepower
    let total_hp: u32 = sorted_clients.iter().map(|(_, hp)| hp).sum();
    let mut assignments: HashMap<u8, u32> = HashMap::new();  // player_id -> assigned count

    for slot in ai_slots {
        // Find client with most remaining "budget"
        let target = sorted_clients.iter()
            .min_by_key(|(pid, hp)| {
                let assigned = assignments.get(pid).unwrap_or(&0);
                // Fewer assignments relative to horsepower = higher priority
                // TODO: Monitor latency; if client > 200ms RTT, deprioritize
                (*assigned * total_hp) / hp
            })
            .map(|(pid, _)| *pid)
            .unwrap();

        if let SlotAssignment::AI { assigned_to, .. } = &mut slot.assignment {
            *assigned_to = Some(target);
        }
        *assignments.entry(target).or_insert(0) += 1;
    }
}

/// **Host Takeover Guarantee**
/// If an AI-assigned client lags (misses input window consistently),
/// the Host MUST revoke their AI assignments and take them over immediately
/// to prevent stalling the entire lobby.
fn check_ai_latency(lobby: &mut LobbyState, host_perf: &PerfStats) {
   // Implementation needed in Phase 3
}
```

**Horsepower Score**: Self-reported by client at handshake. Could be:
- CPU benchmark (run a mini `step_world` benchmark)
- User-configured ("I have a potato" vs "I have a workstation")
- Dynamic (measured during game, redistributed on lag)

### Input Submission for AI

Each client submits inputs for *all* countries they control (human + assigned AI):

```rust
pub struct PlayerInput {
    /// Which client is submitting
    pub client_id: u8,

    /// Inputs for each controlled country
    pub country_inputs: Vec<CountryInput>,
}

pub struct CountryInput {
    pub country: Tag,
    pub commands: Vec<Command>,
}
```

Host validates that client is authorized to submit for claimed countries.

---

## Security Model

### What We Trust

| Component | Trust Level | Rationale |
|-----------|-------------|-----------|
| QUIC/TLS | High | Proven cryptography, use `rustls` |
| Client Checksums | Medium | Can lie, but detected by other clients |
| Client Inputs | Low | Validated server-side against game rules |
| Manifest Hash | High | Compile-time embedded, can't be spoofed without rebuild |

### What We Don't Trust

- **Client-reported state**: Never used; we compute locally
- **Input validity**: All commands run through `can_execute()` before `step_world()`
- **Timing claims**: Host is timekeeper; clients follow
- **Player identity**: No persistent identity in Phase 1; anyone can claim any name

### Threat Model

| Threat | Mitigation |
|--------|------------|
| Eavesdropping | TLS encryption (QUIC) |
| MITM | Certificate pinning (self-signed CA for private games) |
| Replay attacks | QUIC nonces, tick sequence numbers |
| Desync manipulation | N-of-M checksum voting (if >50% disagree, you're wrong) |
| DoS on host | Rate limiting, connection limits |
| Invalid commands | `can_execute()` validation |

### What We Explicitly Don't Handle (Phase 1)

- **Wallhacks**: Client has full state; we can't hide it without server authority
- **Aimbots**: Not applicable (no real-time aiming)
- **Speed hacks**: Host controls tick rate; can't go faster than host allows
- **Memory editing**: Detectable via checksum mismatch, but not preventable

---

## Data Structures

### Core Types

```rust
/// Unique identifier for a lobby
pub type LobbyId = u64;

/// Player ID within a lobby (0 = host, 1-255 = clients)
pub type PlayerId = u8;

/// Connection state for a remote player
pub struct RemotePlayer {
    pub player_id: PlayerId,
    pub connection: quinn::Connection,
    pub name: String,
    pub latency_ms: u16,
    pub last_seen: Instant,
    pub controlled_countries: Vec<Tag>,
    pub checksum_history: VecDeque<(u64, u64)>,  // (tick, checksum)
}

/// Host's view of the lobby
pub struct HostLobby {
    pub id: LobbyId,
    pub state: LobbyState,
    pub players: HashMap<PlayerId, RemotePlayer>,
    pub endpoint: quinn::Endpoint,

    /// Pending inputs for current tick
    pub input_buffer: HashMap<PlayerId, Vec<CountryInput>>,

    /// Current tick number
    pub current_tick: u64,
}

/// Client's view of the lobby
pub struct ClientLobby {
    pub id: LobbyId,
    pub state: LobbyState,
    pub my_player_id: PlayerId,
    pub host_connection: quinn::Connection,

    /// Countries I'm responsible for
    pub my_countries: Vec<Tag>,

    /// Current tick number
    pub current_tick: u64,
}
```

### Wire Formats

Using `rkyv` for zero-copy deserialization:

```rust
use rkyv::{Archive, Deserialize, Serialize};

#[derive(Archive, Deserialize, Serialize)]
pub struct TickStartMessage {
    pub tick: u64,
    pub inputs: Vec<CountryInput>,
    pub host_checksum: Option<u64>,
}

#[derive(Archive, Deserialize, Serialize)]
pub struct ChecksumMessage {
    pub tick: u64,
    pub checksum: u64,
}
```

**Why `rkyv`?**
- Zero-copy: Access fields directly from buffer without parsing
- Performance: 10-100x faster than serde for large payloads
- No allocation: Critical for high-frequency tick messages

---

## Implementation Phases

### Phase 1: Local Foundation *(~2 weeks work)*

Build the lobby state machine without networking. Single-player becomes a "local lobby."

**Tasks**:
1. [ ] Define `LobbyState`, `Slot`, `LobbyPhase` types
2. [ ] Implement state transitions (Setup → Loading → InGame → Finished)
3. [ ] Add AI slot assignment (single client owns all AI)
4. [ ] Integrate with existing `eu4sim` binary
5. [ ] Add lobby commands: `CreateLobby`, `AddAI`, `RemoveAI`, `StartGame`
6. [ ] Write tests for state machine transitions

**Deliverable**: Single-player works through lobby abstraction. No network code yet.

**Property Tests**:
- Lobby state transitions are deterministic
- Slot assignments are valid (no duplicate countries, no unassigned AI)
- Game can only start when all slots have countries and are ready

---

### Phase 2: Host Networking *(~3 weeks work)*

Host can accept connections and run a networked lobby.

**Tasks**:
1. [ ] Add `quinn` dependency, configure QUIC endpoint
2. [ ] Implement handshake protocol (Hello/HelloAck/Reject)
3. [ ] Broadcast `LobbyState` to connected clients
4. [ ] Handle `SlotAssign` requests from clients
5. [ ] Implement `PlayerReady` aggregation
6. [ ] Add LAN discovery broadcast
7. [ ] Write integration tests with multiple processes

**Deliverable**: Host can accept clients, clients see lobby state, but no game yet.

**Libraries**:
```toml
[dependencies]
quinn = "0.10"
rustls = "0.21"
rcgen = "0.11"  # Self-signed cert generation
rkyv = "0.7"
```

---

### Phase 3: Lockstep Integration *(~3 weeks work)*

Connect lobby to simulation with tick synchronization.

**Tasks**:
1. [ ] Implement `TickStart` / `PlayerInput` message flow
2. [ ] Add input collection with timeout
3. [ ] Integrate `WorldState::checksum()` (from integrity doc)
4. [ ] Implement `Checksum` message and comparison
5. [ ] Add `DesyncAlert` with diagnostic dump
6. [ ] Implement pause/resume
7. [ ] Add speed controls (1x, 2x, 5x)
8. [ ] Handle client disconnect mid-game (reassign AI? pause?)

**Deliverable**: Full networked game between 2+ players.

**Property Tests**:
- All clients compute identical checksums for identical inputs
- Tick ordering is consistent (no gaps, no duplicates)
- Pause/resume preserves determinism

---

### Phase 4: Robustness *(~2 weeks work)*

Handle real-world networking issues.

**Tasks**:
1. [ ] Implement STUN for NAT traversal
2. [ ] Add TURN relay fallback
3. [ ] Handle packet loss gracefully (QUIC handles most, but need application-level retry)
4. [ ] Implement reconnection (client drops, rejoins same game)
5. [ ] Add latency compensation (input buffering for high-RTT clients)
6. [ ] Stress test: 8 players, 100 AI, speed 5

**Deliverable**: Playable over internet, not just LAN.

**Libraries**:
```toml
[dependencies]
stun-rs = "0.1"
# Or use a TURN library if self-hosting relay
```

---

### Phase 5: Quality of Life *(~2 weeks work)*

Features for a complete lobby experience.

**Tasks**:
1. [ ] Save/load lobby configuration
2. [ ] In-game chat
3. [ ] Observer mode (read-only client)
4. [ ] Host migration (if host disconnects, promote client)
5. [ ] Replay recording (input log to file)
6. [ ] Desync debugging (dump divergent states)

**Deliverable**: Feature-complete lobby system.

---

## Performance Targets

| Metric | Target | Notes |
|--------|--------|-------|
| Handshake time | <500ms | Including manifest validation |
| Tick latency (LAN) | <10ms | Host broadcast → client receive |
| Tick latency (Internet) | <100ms | Assuming 50ms RTT |
| Max players | 32 | Limited by tick collection timeout |
| Checksum compute | <1ms | For 200 countries, 3000 provinces |
| Message overhead | <50 bytes | Per tick, excluding inputs |

---

## Open Questions

1. **Dedicated Server Mode**: Do we want a headless server binary that doesn't run AI, just coordinates? Useful for competitive play.

2. **Cheat Detection**: Should we implement statistical anomaly detection (e.g., player always knows enemy army positions)?

3. **Lobby Persistence**: Save unfinished games and resume? Requires serializing full state + checkpoints.

4. **Cross-Platform**: Windows + Linux? macOS? Affects QUIC implementation details.

5. **IPv6**: Quinn supports it, but do we test/guarantee it?

---

## References

- [Gaffer on Games: Networked Physics](https://gafferongames.com/post/introduction_to_networked_physics/)
- [Source Engine Networking](https://developer.valvesoftware.com/wiki/Source_Multiplayer_Networking)
- [Age of Empires Networking](https://www.gamedeveloper.com/programming/1500-archers-on-a-28-8-network-programming-in-age-of-empires)
- [Quinn Documentation](https://docs.rs/quinn)
- [QUIC RFC 9000](https://www.rfc-editor.org/rfc/rfc9000.html)

---

## Appendix A: Message Catalog

Complete list of protocol messages for reference.

| Type | Direction | Description |
|------|-----------|-------------|
| `Hello` | C→H | Client announces itself |
| `HelloAck` | H→C | Host accepts client |
| `Reject` | H→C | Host rejects client |
| `LobbyState` | H→C | Full lobby state sync |
| `SlotAssign` | C→H | Client requests country |
| `PlayerReady` | C→H | Client signals ready |
| `ChatMessage` | C↔H | Chat broadcast |
| `TickStart` | H→C | Begin tick with inputs |
| `PlayerInput` | C→H | Client's inputs for tick |
| `Checksum` | C→H | Client's state checksum |
| `DesyncAlert` | H→C | Checksum mismatch detected |
| `Pause` | C↔H | Request/notify pause |
| `Resume` | H→C | Resume simulation |
| `Ping` | C↔H | Latency measurement |
| `Pong` | C↔H | Latency response |
| `Disconnect` | C↔H | Graceful disconnect |

---

## Appendix B: State Machine Diagrams

### Lobby Phase Transitions

```
                    ┌─────────────────────────────────────┐
                    │                                     │
                    ▼                                     │
┌───────┐  create  ┌───────┐  all ready  ┌─────────┐     │
│ None  │─────────▶│ Setup │────────────▶│ Loading │     │
└───────┘          └───────┘             └────┬────┘     │
                        ▲                     │          │
                        │                     │ loaded   │
                        │ not ready           ▼          │
                        │                ┌─────────┐     │
                        └────────────────│ InGame  │     │
                                         └────┬────┘     │
                                              │          │
                              ┌───────────────┼──────────┤
                              │               │          │
                              ▼               ▼          │
                         ┌────────┐     ┌──────────┐     │
                         │ Paused │     │ Finished │─────┘
                         └────────┘     └──────────┘
                              │               return
                              │ resume        to lobby
                              ▼
                         ┌─────────┐
                         │ InGame  │
                         └─────────┘
```

### Client Connection State

```
┌──────────────┐
│ Disconnected │
└──────┬───────┘
       │ connect()
       ▼
┌──────────────┐
│ Connecting   │──── timeout ────▶ Disconnected
└──────┬───────┘
       │ QUIC established
       ▼
┌──────────────┐
│ Handshaking  │──── reject ─────▶ Disconnected
└──────┬───────┘
       │ HelloAck received
       ▼
┌──────────────┐
│  Connected   │──── error ──────▶ Disconnected
└──────┬───────┘
       │ game starts
       ▼
┌──────────────┐
│   InGame     │──── disconnect ─▶ Disconnected
└──────────────┘                   (or Reconnecting)
```

---

*Last updated: 2025-12-17*
