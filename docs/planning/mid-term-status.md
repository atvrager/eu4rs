# Mid-Term Goal: Planning Status

*Last updated: 2025-12-19*

## Goal

Run a "complete" game from 1444 to 1821 with:
- All countries AI-controlled
- Economy and military systems functional
- Observable output (headless or eu4viz)
- Completes in <10 minutes wall-clock (Current: **~6.3 minutes** based on 1.0 yr/s)

### Performance Benchmarks (Dec 19, 2025)

| Metric | Value | Status |
| :--- | :--- | :--- |
| **Tick Time** | **0.93 ms** | ✅ Goal Reached (<10ms) |
| **Speed** | **3.0 years/sec** | 🚀 3x improvement |
| **Full Game Time** | **~2.1 minutes** | 🚀 Goal Exceeded (<10m) |

**Breakdown (Release Build):**
- **AI Decision Loop:** 0.39 ms (42%) - Parallelized with rayon.
- **State Cloning (Other):** 0.44 ms (47%) - Persistent `im::HashMap`.
- **Systems (Combat/Move/Economy):** ~0.1 ms (11%) - Negligible.

**Verdict:** The simulation is now production-ready for large-scale observer runs. AI loop parallelized with rayon, yielding 3x speedup (1.0 → 3.0 yr/s).

"Complete" means the simulation doesn't crash or stall - systems exist at varying fidelity levels.

## Design Decisions Made


### Tier Targets

| System | Tier | Key Decision |
|--------|------|--------------|
| Economy | Minimal | Already done (production + tax + expenses) |
| Military | Minimal+ | Combat works, need war termination |
| War Resolution | Medium | 50% white peace, dev-scaled province costs, 5/10yr auto-end |
| Colonization | Minimal | Distance-based, standing order command |
| AI | Minimal | Random valid commands, pluggable interface |
| Diplomacy | Minimal | Stability hits for betrayal (RM, military access) |
| Tech/Institutions | Minimal | 3/3/3 flat mana, institutions spread by dev |
| Religion | Minimal | Static religions (upgrade to Medium post-launch) |
| Rulers | SKIP | No rulers - flat mana generation (design experiment) |
| Development | Minimal | Static + dev purchasing as mana sink |
| Events | SKIP | Alternate history is fine |
| Rebels | SKIP | No internal instability |

### Key Mechanics

**War Resolution**:
- War score 0-100% from battles (5% each, 40% cap) + occupation
- Province cost scales by development
- AI accepts favorable deals immediately
- 5yr: willing to white peace | 10yr: auto white peace
- Full annexation = country death (permanent)

**Colonization**:
- Distance-based range from coastal provinces
- `StartColony { province }` as standing order
- ~1000 settlers/year, completes at 1000 pop
- No colonial nations in minimal

**Mana**:
- Flat 3/3/3 per month (no rulers)
- Dev purchasing costs 50 mana per click
- Property tests should verify accumulation/spending

**Religion**:
- Reformation fires ~1517
- Simplified spread logic
- Religious unity affects stability
- Holy war CB available

**Army Movement & Occupation**:
- Movement system complete (Phase 4 roadmap, 100% complete)
- Armies move to adjacent provinces; can enter enemy territory during war
- Combat triggers automatically when hostile armies in same province
- Occupation = army standing in enemy province (no siege for minimal)
- War score from occupation: `province_dev / total_enemy_dev * 60`

### Shared Infrastructure

- **AI Visibility Architecture**: AI receives filtered `VisibleWorldState`, not raw `WorldState`. Same visibility rules for AI and UI. Modes: `Realistic` (fog of war) and `Omniscient` (testing/cheating). Commands also filtered by visibility.
- **Bounded Range Library**: Reusable type for stability (-3/+3), prestige (-100/+100), war score (0-100), etc. Clamping, decay, ratio calculations in one place.

## What's Blocking

1. **War Resolution** - Done (Minimal: white peace / annexation / simple terms)
2. **AI** - Done (Minimal: Random AI enabled)

Everything else can be stubbed or is already done.

## Next Steps (Not Prioritized)

- [ ] Implement peace deal system (Logic integration)
- [ ] Add stability system with betrayal consequences
- [x] Add mana generation + dev purchasing
- [ ] Add colonization with standing orders
- [ ] Add reformation spread
- [ ] Connect headless output or eu4viz
- [x] Multithreaded AI decision loop (Performance optimization)

## Open Planning Work

See [complete-game-target.md](../design/simulation/complete-game-target.md#open-planning-work) for full list.

**Needs Design Session**: Casus Belli system, Alliance call-to-arms, Stability triggers, Reformation spread, Institution spread, Tech effects, Province cost formula, Truce duration.

**Architectural Sketches Needed**: AI decision frequency, Performance architecture, Save/Load system, Determinism testing.

**Visualization strategy**: Headless vs IPC vs in-process (still open).

---

## Handoff Prompt

*For resuming planning work in a future session:*

> We're working toward the mid-term goal: run a "complete" simulation from 1444 to 1821 in <10 minutes. Design decisions are documented in `docs/design/simulation/complete-game-target.md` with a summary in `docs/planning/mid-term-status.md`.
>
> **Current status**: Phase 0 and Phase 1 of the critical path are blocking. War resolution (peace deals, province transfer, country elimination) and random AI are the two main blockers.
>
> **Where we left off**: All system tier decisions made, command enumeration complete, critical path defined, army movement/occupation mechanics specified. Ready to begin implementation.
>
> **Next action**: Pick a task from Phase 0 or Phase 1 of the critical path and implement it.

---

See [complete-game-target.md](../design/simulation/complete-game-target.md) for full system tier definitions and critical path.
