# Complete Game Target

This document defines what "complete game" means for the eu4rs mid-term milestone: running a full 1444→1821 simulation with AI-controlled countries.

## Success Criteria

A successful run means:
- [x] Game ticks from 1444.11.11 to 1821.1.1 (377 years, ~137,000 days)
- [x] All countries have AI making decisions each tick
- [x] No crashes, hangs, or invalid states
- [x] Observable output (headless logs, or optional eu4viz connection)
- [x] Completes in reasonable wall-clock time (target: <10 minutes)

## System Tiers

Each game system is defined at three fidelity levels. The mid-term goal targets **Minimal** or **Medium** for each.

---

### Economy

**Target: Minimal**

| Tier | Description |
|------|-------------|
| **Minimal** | Treasury changes monthly (income - expenses). Countries can go bankrupt or accumulate wealth. No trade mechanics. |
| **Medium** | Trade nodes exist, goods flow downstream, trade power determines share. Merchants can steer/collect. |
| **Full** | Trade companies, trade conflicts, privateering, mercantilism, full modifier stacking. |

**Current Status**: Minimal implemented (production + taxation + expenses).

---

### Military

**Target: Minimal+** (Minimal, but wars must end)

| Tier | Description |
|------|-------------|
| **Minimal** | Units exist, move, fight, die. Wars declared. Armies have strength. Combat resolves daily. |
| **Medium** | Sieges work, morale affects combat, attrition exists, leaders provide bonuses, terrain matters. |
| **Full** | Full combat width, dice rolls, discipline/tactics, professionalism, mercenaries, condottieri. |

**Current Status**: Minimal implemented. Missing: war termination.

**Army Movement & Occupation**:

Movement rules:
- Armies can move to **adjacent provinces** (land adjacency for armies, sea for fleets)
- During war, armies **can enter enemy territory** (no access needed vs. enemies)
- Movement to neutral territory requires **military access** (or violate → stability hit)
- `available_commands()` includes `MoveArmy { army_id, destination }` for all valid adjacent provinces

Combat triggering:
- When hostile armies occupy the same province, **combat happens automatically** (daily tick)
- Combat continues until one side is destroyed or retreats (retreat not in minimal - fight to death)
- Existing combat system handles casualties (power-based, daily resolution)

Occupation flow:
1. Army issues `MoveArmy` to enemy province
2. If enemy army present → combat until one side destroyed
3. Surviving army **occupies** the province (no siege in minimal)
4. Occupation contributes to war score
5. On peace, occupied provinces can be transferred

Random AI behavior:
- Will sometimes randomly pick `MoveArmy` into enemy territory
- Chaotic but eventually armies collide
- Stronger side wins, occupies, gains war score
- Over 5-10 years, wars resolve through attrition

---

### War Resolution

**Target: Medium** (required for wars to end)

| Tier | Description |
|------|-------------|
| **Minimal** | Wars end after fixed duration or when one side is fully occupied. Simple binary outcome (win/lose). |
| **Medium** | War score from battles/occupation/blockades. Peace deals transfer provinces. White peace possible. |
| **Full** | Ticking war score, call to arms, separate peace, coalition mechanics, war exhaustion, aggressive expansion. |

**Current Status**: Not implemented. **BLOCKING for mid-term goal.**

**Mid-term Implementation**:
- **War score**: 0-100%, from battles won and provinces occupied
  - Battles: +5% per battle won (capped at 40% total from battles)
  - Occupation: +X% per province occupied, where X = province_dev / total_enemy_dev * 60
  - (So occupying half their development = 30% war score from occupation)
  - Total cap: 100%
- **White peace**: Accepted at ≥50% war score by either side
- **Province taking**: Above 50%, cost scales by province development (high-dev provinces cost more war score)
- **Full annexation**: Requires 100% war score
- **AI acceptance**: AI accepts any peace deal favorable to them immediately
- **Stalemate prevention**:
  - After 5 years: both sides *willing* to accept white peace offers
  - After 10 years: war auto-ends in white peace
- **Country death**: Fully annexed countries are eliminated permanently (no return)

---

### Colonization

**Target: Minimal**

| Tier | Description |
|------|-------------|
| **Minimal** | Uncolonized provinces can be targeted. Colony grows at fixed rate (~1000 settlers/year). At 1000 pop → becomes regular province owned by colonizer. Map is revealed (no terra incognita). No natives. |
| **Medium** | Terra incognita exists, explorers/conquistadors reveal it. Natives occupy provinces (can be attacked/coexisted). Colonial nations form at 5 provinces in a colonial region. Colonial maintenance cost. |
| **Full** | Native policies, native trading, native uprisings. Colonial nations have liberty desire, can declare independence. Tariffs, treasure fleets. Trade companies in Asia/Africa. Treaty of Tordesillas. |

**Current Status**: Not implemented.

**Mid-term Implementation**:
- **Colonial range**: Distance-based from nearest owned coastal province. Tech increases range (game data has colonial range values per tech level).
- **Targeting**: Country can start colony in any uncolonized province within range.
- **Command model**: `StartColony { province }` is a **standing order**. Once issued, colony grows automatically until completion. AI only needs to pick it once.
- **Growth**: Fixed rate, ~1000 settlers/year. No native attacks, no disease, no events.
- **Completion**: At 1000 population → province becomes fully owned.
- **No colonial nations**: Provinces stay directly owned (skip CN formation for minimal).

---

### AI

**Target: Minimal** (with architectural emphasis on pluggability)

| Tier | Description |
|------|-------------|
| **Minimal** | Random valid commands. Each country picks from available legal actions uniformly. Chaotic but functional. |
| **Medium** | Reactive heuristics. Defend when attacked, build when rich, declare war on weak neighbors, accept reasonable peace. |
| **Full** | Trained neural network / reinforcement learning. Strategic planning, multi-front coordination, economy optimization. Goal: beat Florryworry. |

**Current Status**: Not implemented.

**Architecture Note**: AI interface must be pluggable from day one. See [AI Visibility Architecture](#ai-visibility-architecture) below.

**Minimal Path → Final**:
- **Phase 1 (Mid-Term)**: Simple trait with full state access
  ```rust
  pub trait AI {
      fn decide(&self, state: &WorldState) -> Vec<Command>;
  }
  ```
- **Phase 2**: Add visibility parameter (always pass `Omniscient` initially)
  ```rust
  fn decide(&self, state: &WorldState, mode: VisibilityMode) -> Vec<Command>;
  ```
- **Phase 3**: Implement `Realistic` filtering for production AI training

---

### Diplomacy

**Target: Minimal** (with stability consequences)

| Tier | Description |
|------|-------------|
| **Minimal** | Alliances with call-to-arms. Royal marriages exist. Military access. Truces after wars. **Stability hits** for betrayal (attacking royal marriage partner, violating military access). |
| **Medium** | Opinion system, vassals/subjects, coalition formation, rivalry bonuses, diplomatic reputation, separate peace in wars. |
| **Full** | Favors, trust, great power mechanics, guarantee independence, threaten war, support independence, full subject types (PU, march, tributary). |

**Current Status**: Alliance/Rival tracking exists. War declaration exists.

**Design Note**: Stability consequences for diplomatic betrayal are prioritized because they're often the source of confusing war declaration UI in the real game. Getting this right is a concrete UX improvement opportunity.

---

### Technology & Institutions

**Target: Minimal** (with mana infrastructure)

| Tier | Description |
|------|-------------|
| **Minimal** | Tech levels exist (1-32). Monarch points (ADM/DIP/MIL) exist and accumulate. Tech costs mana. Institutions exist and spread based on development (grow from cities). Random AI unlikely to save enough to tech up. Skip unit pips. |
| **Medium** | Institutions spread geographically with proper mechanics. Ahead/behind of time penalties. Tech affects unit stats. Advisors generate extra mana. |
| **Full** | Full MP system, idea groups, policies, national ideas, embracement mechanics, institution origin events, tech groups. |

**Current Status**: Not implemented.

**Design Note**: Mana system is fundamental to EU4 and should exist even if underutilized at Minimal tier. Property tests should verify mana accumulation/spending. Unit pips (Ottoman early advantage) deliberately skipped—balance comes from other factors.

---

### Events

**Target: SKIP** (not in mid-term scope)

| Tier | Description |
|------|-------------|
| **Minimal** | No events. History unfolds purely from AI decisions and mechanics. No scripted country formations. |
| **Medium** | Milestone events only (Reformation fires, Colonial nations form, major formables like Germany/Italy available as decisions). |
| **Full** | Full event system with triggers, MTTH, random events, flavor events, disasters, decisions, mission trees. |

**Current Status**: Not implemented. **Deliberately skipped for mid-term.**

**Design Note**: Events are content-heavy. The simulation can be "complete" without them—it just produces alternate history. Pluggable event system can be added later.

---

### Rebels

**Target: SKIP** (not in mid-term scope)

| Tier | Description |
|------|-------------|
| **Minimal** | No rebels. Internal stability is a number but has no spawn consequences. |
| **Medium** | Unrest accumulates from war exhaustion/overextension. Rebels spawn as hostile armies. If they occupy capital/enough provinces, demands enforced. |
| **Full** | Rebel types (pretenders, separatists, religious, particularists), rebel progress bar, autonomy changes, country break-up, disaster triggers. |

**Current Status**: Not implemented. **Deliberately skipped for mid-term.**

---

### Religion

**Target: Minimal** (upgrade to Medium post-launch)

| Tier | Description |
|------|-------------|
| **Minimal** | Provinces and countries have a religion tag. No mechanical effects. Everyone stays 1444 religions forever. |
| **Medium** | Religious unity affects stability/unrest. Reformation fires ~1517, spreads to provinces (simplified spread logic). Missionaries convert provinces at flat rate. Holy war CB available. |
| **Full** | Papal mechanics, Protestant/Reformed leagues, defender of faith, religious ideas, Curia controller, Orthodox patriarchs, Hindu deity swapping, Shinto isolationism, etc. |

**Current Status**: Not implemented.

**Design Note**: Religion is core to EU4 identity (1444-1821 spans the Reformation), but static religions are sufficient for mid-term "complete game" testing. Medium tier can be added post-launch for historical flavor and simulation variety.

**Minimal Path → Medium**:
- **Phase 1 (Mid-Term)**: Static `religion: ReligionId` field, no conversion
- **Phase 2**: Add `Missionary` system (simple conversion at fixed rate)
- **Phase 3**: Add Reformation event (fires 1517, spreads via adjacency + dev weight)

---

### Rulers & Succession

**Target: SKIP** (rulers may never exist)

| Tier | Description |
|------|-------------|
| **Minimal** | No rulers. Mana generates at flat rate (e.g., 3/3/3 per month). No succession, no regencies, no RNG deaths. |
| **Medium** | Rulers exist with ADM/DIP/MIL stats affecting mana rate. Random lifespan, simple succession (random new ruler on death). |
| **Full** | Heirs, regency councils, personal unions, legitimacy, elective monarchies, republics, theocracies, consorts, disinheriting. |

**Current Status**: Not implemented. **Deliberately skipped for mid-term.**

**Design Note**: Controversial simplification—rulers are a major EU4 mechanic. But they add RNG frustration (bad heir, regency council) without strategic depth. Flat mana generation is predictable and lets players/AI plan. Consider: maybe the game is *better* without rulers? This is an intentional design experiment.

---

### Development & Buildings

**Target: Minimal** (with dev purchasing)

| Tier | Description |
|------|-------------|
| **Minimal** | Provinces have static development from 1444. Dev can be increased by spending mana (50 ADM/DIP/MIL per click). Buildings are static (forts exist for military). Gives random AI something to spend mana on. |
| **Medium** | Buildings can be constructed (cost, build time, bonuses). Core building types affect income/manpower/trade. Development cost modifiers (terrain, climate). |
| **Full** | Great projects, monuments, manufactories, state buildings, dev efficiency, centralization, governing capacity. |

**Current Status**: Not implemented.

**Design Note**: Dev purchasing gives AI a mana sink and lets provinces grow over 377 years. Without it, accumulated mana has nowhere to go (especially with no tech-up from random AI). Simple implementation: click → spend 50 mana → +1 dev.

---

## Visualization

**Target: Optional but desirable**

| Approach | Description |
|----------|-------------|
| **Headless** | Console logging only. Monthly summary: "1445-01: France at war with England, treasury 523 ducats" |
| **File Output** | Periodic state dumps (JSON/binary) that eu4viz can load as snapshots |
| **Live Connection** | IPC/socket streaming state to eu4viz for real-time visualization |
| **In-Process** | eu4viz embeds eu4sim-core, runs simulation inline with rendering |

**Current Status**: eu4viz exists but not connected to simulation.

---

## Performance Target

- **137,000 ticks** (days from 1444 to 1821)
- **~200 countries** (varies as countries form/die)
- **~3,000 provinces**
- **Target**: <10 minutes wall-clock for full run
- **Implies**: ~230 ticks/second, or ~4.3ms per tick budget

---

## Summary Table

| System | Target Tier | Blocking? | Notes |
|--------|-------------|-----------|-------|
| Economy | Minimal | ✅ Done | Production + tax + expenses |
| Military | Minimal+ | 🔄 Partial | Combat works, wars can't end |
| War Resolution | Medium | ❌ **BLOCKING** | Peace deals needed |
| Colonization | Minimal | ❌ Not started | Fixed growth, no exploration |
| AI | Minimal | ❌ Not started | Random valid commands |
| Diplomacy | Minimal | 🔄 Partial | Need stability consequences |
| Tech & Institutions | Minimal | ❌ Not started | Mana exists, institutions spread by dev |
| Events | SKIP | — | Alternate history is fine |
| Rebels | SKIP | — | No internal instability |
| Religion | Minimal | ❌ Not started | Static religions (upgrade to Medium post-launch) |
| Rulers | SKIP | — | Flat mana, no rulers |
| Development | Minimal | ❌ Not started | Static + dev buying |

---

## Shared Infrastructure

### AI Visibility Architecture

The AI does **not** receive raw `WorldState`. It receives a **filtered view** based on what that country can legitimately observe.

**Core Principle**: Same visibility rules for AI and human players. The UI filters state the same way AI does.

```rust
/// What information is visible to an observer
pub enum VisibilityMode {
    /// Fog of war, intel requirements, realistic constraints
    Realistic,
    /// See everything (testing, debugging, cheating AI)
    Omniscient,
}

/// Filtered view of world state from one country's perspective
pub struct VisibleWorldState {
    pub date: Date,
    pub observer: Tag,

    // Always visible
    pub own_country: CountryState,       // Full info about self
    pub known_countries: Vec<Tag>,        // Countries we know exist
    pub public_wars: Vec<WarSummary>,     // Who's at war with whom
    pub province_ownership: HashMap<ProvinceId, Tag>,  // Political map

    // Visibility-dependent
    pub visible_armies: Vec<VisibleArmy>, // Only armies we can see
    pub province_details: HashMap<ProvinceId, ProvinceDetails>, // Dev, buildings, etc.
    pub diplomatic_intel: HashMap<Tag, DiplomaticIntel>, // What we know about others
}

/// AI interface - receives filtered state only
pub trait AI: Send + Sync {
    /// Decide what commands to issue this tick
    fn decide(
        &self,
        visible_state: &VisibleWorldState,
        available_commands: &[Command],
    ) -> Vec<Command>;
}
```

**Visibility Rules** (examples, not exhaustive):

| Information | When Visible |
|-------------|--------------|
| Province ownership | Always |
| Province development | Own provinces, neighbors, or intel |
| Enemy army location | In owned/allied territory, or adjacent |
| Enemy army composition | Only in battle or with spy network |
| Foreign treasury | Never (or high intel) |
| Foreign diplomacy | Alliances public, other relations hidden |
| War participants | Always |
| War score | Only for wars you're in |

**Why This Matters**:

1. **Fair AI**: A trained AI that beats Florryworry using only player-visible info is *actually impressive*
2. **Cheating AI**: Omniscient mode for testing, balance analysis, or "hard mode"
3. **Fog of War**: Same code filters UI rendering and AI input
4. **Replay/Observer**: Can watch with full visibility or player perspective
5. **Multiplayer-ready**: Visibility rules already enforced, not trusted

**Implementation Strategy**:

```rust
// The simulation exposes this, not raw WorldState
impl Simulation {
    /// Get filtered state for a specific observer
    pub fn visible_state(&self, observer: Tag, mode: VisibilityMode) -> VisibleWorldState {
        match mode {
            VisibilityMode::Realistic => self.filter_realistic(observer),
            VisibilityMode::Omniscient => self.filter_omniscient(observer),
        }
    }

    /// Get legal commands for a country (also uses visibility!)
    pub fn available_commands(&self, country: Tag, mode: VisibilityMode) -> Vec<Command> {
        // Can't declare war on a country you don't know exists (in fog)
        // Can't move army to province you haven't discovered
        // etc.
    }
}
```

**For Mid-Term**: Start with `Omniscient` mode (simpler). The interface is designed so `Realistic` can be added without changing AI implementations.

**Implementation Phases**:
1. **Phase 1 (Mid-Term)**: AI trait takes `&WorldState` directly, no visibility filtering
2. **Phase 2**: Add `VisibilityMode` parameter to API, always pass `Omniscient`
3. **Phase 3**: Implement `visible_state()` filtering function for `Realistic` mode
4. **Phase 4**: Train production AI using only `Realistic` mode

**Key**: Design the final interface in Phase 2, but defer expensive filtering implementation until needed.

---

### Command Enumeration

Commands are organized by system. Each system at its chosen tier contributes commands to `available_commands()`.

#### Military (Minimal+)

| Command | Parameters | Notes |
|---------|------------|-------|
| `MoveArmy` | army_id, destination | Pathfinding happens server-side |
| `MoveFleet` | fleet_id, destination | Naval movement |
| `MergeArmies` | army_ids[] | Combine into one stack |
| `SplitArmy` | army_id, regiment_ids[] | Detach units |
| `EmbarkArmy` | army_id, fleet_id | Load onto ships |
| `DisembarkArmy` | army_id, province | Unload from ships |

*Future (Medium+): RecruitRegiment, DisbandRegiment, AssignGeneral*

#### War Resolution (Medium)

| Command | Parameters | Notes |
|---------|------------|-------|
| `DeclareWar` | target, casus_belli | Starts war, calls allies |
| `OfferPeace` | war_id, terms | White peace, take provinces, or full annex |
| `AcceptPeace` | war_id | Accept incoming offer |
| `RejectPeace` | war_id | Decline incoming offer |

*Peace terms structure:*
```rust
pub enum PeaceTerms {
    WhitePeace,
    TakeProvinces { provinces: Vec<ProvinceId> },
    FullAnnexation,
}
```

#### Colonization (Minimal)

| Command | Parameters | Notes |
|---------|------------|-------|
| `StartColony` | province | Standing order, grows until complete |
| `AbandonColony` | province | Cancel in-progress colony |

*Future (Medium+): SetNativePolicy, SendExplorer*

#### Diplomacy (Minimal)

| Command | Parameters | Notes |
|---------|------------|-------|
| `OfferAlliance` | target | Propose alliance |
| `BreakAlliance` | target | End alliance (stability hit?) |
| `OfferRoyalMarriage` | target | Propose RM |
| `BreakRoyalMarriage` | target | End RM (stability hit if at war) |
| `RequestMilitaryAccess` | target | Ask for access |
| `CancelMilitaryAccess` | target | Revoke access we granted |
| `SetRival` | target | Declare rival |
| `RemoveRival` | target | Un-rival |

*Diplomacy responses (queued like EU4 - offer sits until recipient decides):*
| Command | Parameters | Notes |
|---------|------------|-------|
| `AcceptAlliance` | from | Accept incoming offer |
| `RejectAlliance` | from | Decline |
| `AcceptRoyalMarriage` | from | Accept RM offer |
| `RejectRoyalMarriage` | from | Decline |
| `GrantMilitaryAccess` | to | Allow passage |
| `DenyMilitaryAccess` | to | Refuse |

*Response model: Offers are queued in `PendingDiplomacy`. They appear in `available_commands()` for the recipient until accepted/rejected or expired.*

#### Tech & Institutions (Minimal)

| Command | Parameters | Notes |
|---------|------------|-------|
| `BuyTech` | tech_type | ADM/DIP/MIL, costs mana |
| `EmbraceInstitution` | institution | Costs mana based on dev |

*Note: Random AI unlikely to accumulate enough mana to use these.*

#### Religion (Medium)

| Command | Parameters | Notes |
|---------|------------|-------|
| `AssignMissionary` | province | Convert province religion |
| `RecallMissionary` | province | Stop conversion |
| `ConvertCountryReligion` | religion | Switch state religion (stability hit) |

*Future (Full): DefendFaith, ExcommunicateRuler*

#### Development (Minimal)

| Command | Parameters | Notes |
|---------|------------|-------|
| `DevelopProvince` | province, type | ADM→tax, DIP→production, MIL→manpower. Costs 50 mana. |

*Future (Medium+): BuildBuilding, DestroyBuilding*

#### Control / Misc

| Command | Parameters | Notes |
|---------|------------|-------|
| `MoveCapital` | province | Change capital (costs ADM?) |
| `Pass` | — | Do nothing this tick (explicit no-op) |

---

#### Command Count Summary

| System | Commands | Notes |
|--------|----------|-------|
| Military | 6 | Movement, merging, embarking |
| War Resolution | 4 | Declare, offer, accept, reject |
| Colonization | 2 | Start, abandon |
| Diplomacy (outgoing) | 8 | Alliances, RM, access, rivals |
| Diplomacy (responses) | 6 | Accept/reject incoming |
| Tech & Institutions | 2 | Buy tech, embrace |
| Religion | 3 | Missionary, convert country |
| Development | 1 | Develop province |
| Control | 2 | Move capital, pass |

**Total: ~34 command types** (at mid-term tier)

*Random AI picks from this set uniformly. Many will be invalid at any given moment (can't declare war if already at war, can't colonize if no valid targets), so `available_commands()` filters to legal subset.*

**Implementation Strategy: Define All, Implement Incrementally**

Define the full `Command` enum with all 34 variants NOW (cheap), but implement execution in phases:

```rust
pub enum Command {
    // PHASE 1: Core loop (implement for mid-term)
    MoveArmy { army_id, destination },
    DeclareWar { target, cb },
    OfferPeace { war_id, terms },
    AcceptPeace { war_id },
    Pass,

    // PHASE 2: Essential systems (implement post mid-term)
    StartColony { province },
    DevelopProvince { province, mana_type },
    OfferAlliance { target },
    AcceptAlliance { from },
    // ... (all defined but stubbed)
}

impl Command {
    pub fn execute(&self, world: &mut WorldState) -> Result<()> {
        match self {
            // Implemented
            Command::MoveArmy { .. } => { /* working */ },

            // Stubbed (graceful no-op)
            Command::StartColony { .. } => {
                log::warn!("Colonization not implemented yet");
                Ok(())
            },
        }
    }
}
```

**Benefits**:
- Command API locked early → no refactoring when adding networking
- `available_commands()` can return unimplemented commands → AI sees future features
- Graceful degradation → game doesn't crash on stub commands

---

### Bounded Range Library

Many EU4 values share a common pattern: bounded numeric ranges with clamping, decay, and modifiers. Rather than implementing each separately, create a reusable `BoundedValue` type.

**Examples of bounded ranges in EU4:**
| Value | Min | Max | Decay? | Notes |
|-------|-----|-----|--------|-------|
| Stability | -3 | +3 | No | Discrete integers |
| Prestige | -100 | +100 | Yes (yearly) | Continuous |
| Legitimacy | 0 | 100 | No | |
| Republican Tradition | 0 | 100 | Yes | |
| Power Projection | 0 | 100 | Yes | |
| Army Tradition | 0 | 100 | Yes | |
| Religious Unity | 0% | 100% | No | Calculated |
| Overextension | 0% | ∞ | No | Calculated |
| War Score | 0 | 100 | No | Per-war |

**Proposed API:**
```rust
pub struct BoundedValue<T> {
    value: T,
    min: T,
    max: T,
    // Optional: decay_rate, monthly_change, etc.
}

impl BoundedValue<i32> {
    pub fn add(&mut self, delta: i32) { /* clamps */ }
    pub fn set(&mut self, value: i32) { /* clamps */ }
    pub fn ratio(&self) -> f32 { /* 0.0 to 1.0 */ }
}
```

**Benefits:**
- Consistent behavior across all bounded values
- Property tests can verify clamping invariants once
- Easy to add new bounded values later

---

## Open Questions

~~1. **Peace deals**: What triggers AI to accept peace?~~ → Resolved: 50% for white peace, scaled by dev for provinces, 5yr/10yr stalemate timers

~~2. **Country death**: Eliminated permanently~~ → Resolved: Dead is dead

~~3. **Colonial range**: Distance-based~~ → Resolved: From coastal provinces, tech increases range

~~4. **Stability range**: EU4's -3 to +3, configurable~~ → Resolved: Use bounded range library

**Remaining:**

~~1. **Occupation mechanics**~~ → Resolved: Army standing in enemy province = occupied (no siege for minimal)

~~2. **War score from battles**~~ → Resolved: Flat 5% per battle, capped at 40% (superiority wars may have different cap as war goal parameter)

~~3. **Colonist assignment**~~ → TBD: See discussion below

~~4. **Mana generation rate**~~ → Resolved: Flat 3/3/3 per month (EU4's base rate, no rulers)

~~**Colonist model**~~ → Resolved: Standing order. `StartColony { province }` persists until colony completes. AI picks once.

**All major questions resolved for mid-term scope.**

---

## Open Planning Work

*Areas that need deeper design discussion in future sessions. These are "handwave" decisions that work for now but may benefit from dedicated exploration (like we did for AI visibility).*

### Needs Design Session

| Topic | Current Handwave | Why It Might Need More |
|-------|------------------|------------------------|
| **Casus Belli System** | "Holy war CB available" | What CBs exist? How do they affect war score demands? Conquest CB? No-CB penalty? |
| **Alliance Call-to-Arms** | "Auto-join or queued request" | Does defender choose? Can you decline? Honor penalty? Separate peace? |
| **Stability Triggers** | "Betrayal causes stability hit" | Full list of triggers? How much per trigger? Positive stability events? |
| **Reformation Spread** | "Simplified spread logic" | What's the algorithm? Province adjacency? Dev-weighted? Random element? |
| **Institution Spread** | "Spread by dev" | Origin points? Monthly spread chance? Embrace cost formula? |
| **Tech Effects** | "Mana exists, tech can be bought" | What does tech DO in minimal? Unit strength? Colonial range? Nothing? |
| **Province Cost Formula** | "Scaled by dev" | Exact formula: `dev * X`? Diminishing returns? Capital bonus? |
| ~~**Truce Duration**~~ | ~~"X years after peace"~~ | ~~Resolved: 5 years flat. See [`truce-system.md`](./truce-system.md)~~ |

### Architectural Sketches Needed

| Topic | Notes |
|-------|-------|
| **AI Decision Frequency** | Every tick? Monthly? On-demand when state changes? Batched? |
| **Performance Architecture** | ECS? Plain structs? Component arrays? Batched updates? |
| **Save/Load System** | Needed for debugging, replays, checkpoints. Serde? Custom format? |
| **Determinism Testing** | How do we verify same input → same output across runs? |
| **Event Bus Architecture** | For future events system. Pub/sub? Direct calls? |

### Deliberately Deferred

| Topic | Reason |
|-------|--------|
| **Detailed Combat** | Minimal tier is fine. Combat width, dice, discipline can wait. |
| **Trade System** | Not in mid-term scope. Design when tackling Phase 7 (roadmap). |
| **Buildings** | Static forts work for mid-term. Full system is Phase 7. |
| **Personal Unions** | Rulers are skipped. No PUs without rulers. |
| **Naval Mechanics** | Transports work. Naval combat/blockades can wait. |

---

## Critical Path

Implementation order based on dependencies. Each phase unblocks the next.

### Phase 0: Infrastructure (Foundation)

| Task | Dependencies | Notes |
|------|--------------|-------|
| Bounded range library | None | Stability, prestige, war score all use this |
| Pending diplomacy queue | None | For queued offers/responses |
| Command enum definition | None | All 34 command types as Rust enum (define all, implement ~10 initially) |

*These are small, self-contained pieces that other systems build on.*

**Implementation Note**: Command enum should define ALL 34 variants now (cheap, locks API), but `execute()` only implements Phase 1 commands. Unimplemented commands log a warning and return `Ok(())`. This prevents refactoring when adding networking.

### Phase 1: Core Loop (Blockers)

| Task | Dependencies | Notes |
|------|--------------|-------|
| Occupation tracking | Movement system | Mark provinces as occupied when army present |
| War score calculation | Occupation tracking | 5% per battle + occupation contribution |
| Peace offer system | War score | OfferPeace, AcceptPeace, RejectPeace commands |
| Province transfer | Peace system | Change ownership on peace acceptance |
| Country elimination | Province transfer | Remove country when 0 provinces |
| Random AI | Command enum | Pick uniformly from available_commands() |
| available_commands() | All systems | Filters legal commands per country |

*Note: Movement system already exists (Phase 4 roadmap). Occupation tracking connects it to war score.*

**Milestone**: After Phase 1, simulation can run 1444→1821 with wars starting, ending, and countries dying. Chaotic but functional.

### Phase 2: Essential Systems

| Task | Dependencies | Notes |
|------|--------------|-------|
| Mana generation | None | Flat 3/3/3 per month to CountryState |
| Stability system | Bounded range | -3 to +3, triggers from diplomacy betrayal |
| Stability triggers | Stability | Define: RM break at war, access violation, etc. |
| Alliance call-to-arms | Diplomacy exists | Auto-join or queued request on defensive war |
| Truce system | Peace system | 5 years flat, blocks re-declaration. **Design complete**: [`truce-system.md`](./truce-system.md) |

**Milestone**: After Phase 2, diplomacy has consequences and alliances matter.

### Phase 3: Content Systems

| Task | Dependencies | Notes |
|------|--------------|-------|
| Colonization | Province data | StartColony command, growth tick, completion |
| Development | Mana system | DevelopProvince command, costs 50 mana |
| Tech system | Mana system | BuyTech command (rarely used by random AI) |
| Institution spread | Tech system | Monthly spread based on dev |
| ~~Reformation~~ | ~~Date system, religion data~~ | **DEFERRED** (Religion is Minimal for mid-term) |
| ~~Missionary system~~ | ~~Religion~~ | **DEFERRED** (Religion is Minimal for mid-term) |

**Milestone**: After Phase 3, the world changes over 377 years - colonies appear, development grows. (Religion remains static for mid-term.)

### Phase 4: Polish & Observation

| Task | Dependencies | Notes |
|------|--------------|-------|
| Headless logging | Core loop | Monthly summary to console |
| Statistics tracking | Core loop | Country count, war count, avg dev, etc. |
| Performance profiling | Full system | Verify <10 min for full run |
| eu4viz connection | Optional | IPC or in-process state sharing |

**Milestone**: After Phase 4, we can observe and analyze simulation runs.

---

### Dependency Graph

```
Phase 0 (Infrastructure)
    │
    ▼
Phase 1 (Core Loop) ◄── BLOCKING: Nothing runs without this
    │
    ├──────────────────┐
    ▼                  ▼
Phase 2 (Essential)  Phase 3 (Content)
    │                  │
    └────────┬─────────┘
             ▼
       Phase 4 (Polish)
```

*Phases 2 and 3 can be developed in parallel after Phase 1 is complete.*

---

### Estimated Scope

| Phase | Systems | Rough Size |
|-------|---------|------------|
| Phase 0 | 3 | Small - utility code |
| Phase 1 | 6 | **Large** - core game loop |
| Phase 2 | 5 | Medium - mechanics |
| Phase 3 | 4 | Small - content (Religion deferred) |
| Phase 4 | 4 | Small - tooling |

**Phase 1 is the critical work.** Once that's done, the rest can be parallelized or deferred.

---

## Beyond Mid-Term: Multiplayer & Lobby

The simulation design is **multiplayer-ready from day one**, but the networking layer is deliberately deferred.

**Single-Player as "Local Lobby"**:

Mid-term implementation treats single-player as a degenerate lobby case:

```rust
pub struct LocalLobby {
    pub slots: Vec<Slot>,        // 1 human, N AI
    pub settings: GameSettings,
    pub phase: LobbyPhase,       // Setup → Loading → InGame
}

// No networking code
impl LocalLobby {
    pub fn start_game(&self) -> WorldState { /* initialize */ }
}
```

**Progression to Networked Multiplayer**:

1. **Phase 1 (Mid-Term)**: Local lobby only, single-player
2. **Phase 2**: Extract `trait LobbyBackend` with `Local` impl
3. **Phase 3**: Add `NetworkedLobby` impl (QUIC/STUN/TURN)
4. **Phase 4**: Polish (hot-join, host migration, observer mode)

See [`docs/design/lobby.md`](./lobby.md) for full multiplayer design. The lobby state machine (`LobbyPhase`, `Slot`, `GameSettings`) is identical for local and networked backends—only the transport changes.

**Key Insight**: By designing the lobby abstraction NOW (even for single-player), we avoid refactoring when adding networking. The same `Command` enum, `WorldState` checksum, and deterministic `step_world()` work for both local and networked games.

---

*Created: 2025-12-18*
