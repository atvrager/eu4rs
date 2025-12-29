# Holy Roman Empire Implementation

**Phase**: 8 - HRE & Political Systems
**Scope**: Core mechanics (emperor, electors, IA, reforms)
**Deferred**: Religious leagues, imperial incidents, dismantling

## Implementation Checklist

### Step 0: Prerequisites
- [x] Add `Gender` enum to `state.rs`
- [x] Add monarch fields to `CountryState` (`ruler_dynasty`, `ruler_gender`, `ruler_instated`)
- [x] Add `is_in_hre` field to `ProvinceState`
- [x] Update save hydration to populate dynasty from saves

### Step 1: Foundation
- [x] Create `HREState` struct with emperor, electors, free_cities, imperial_authority
- [x] Add `ReformId` type for imperial reforms
- [x] Add `HREState` to `GlobalState`
- [x] Add 10 HRE commands to `input.rs`
- [x] Add command stubs to `step.rs`

### Step 2: Imperial Authority
- [x] Create `systems/hre.rs` with `run_hre_tick()`
- [x] Implement monthly IA formula (corrected per wiki)
- [x] Add HRE defines module with constants
- [x] Wire into monthly tick in `step.rs`
- [x] Unit tests for IA mechanics (7 tests)

### Step 3: Elections
- [x] Create election module (`hre/election.rs`)
- [x] Implement eligibility checks (religion, male, independent, not at war)
- [x] Implement AI voting logic (+200 same religion, +100 alliance, +50 RM)
- [x] Re-election bonus (same dynasty)
- [x] Election resolution (most votes, tie-break by prestige)
- [x] Trigger elections on emperor death/ineligibility

### Step 4: Commands
- [x] Implement `AddProvinceToHRE` / `RemoveProvinceFromHRE`
- [x] Implement `JoinHRE` / `LeaveHRE`
- [x] Implement `GrantElectorate` / `RemoveElectorate`
- [x] Implement `GrantFreeCity` / `RevokeFreeCity` (validate OPM, max 12)
- [x] Implement `PassImperialReform`
- [x] Implement `ImperialBan`
- [x] Add to `available_commands()` validation (Ewiger Landfriede filter)

### Step 5: Reforms
- [x] Add well-known reform constants (`reforms::EWIGER_LANDFRIEDE`, etc.)
- [x] Add helper methods (`has_ewiger_landfriede()`, `has_revoke_privilegia()`)
- [ ] Create `eu4data/src/imperial_reforms.rs` parser (deferred)
- [ ] Create `ImperialReformRegistry` (deferred)
- [ ] Load reforms from `common/imperial_reforms/` (deferred)
- [ ] Apply reform modifiers via modifier system (deferred)
- [x] Special handling for Revoke Privilegia (vassalizes HRE members)

### Step 6: Integration
- [x] War system: Ewiger Landfriede blocks internal HRE wars
- [x] Subject system: Revoke Privilegia vassalizes members
- [ ] Save hydration: Parse HRE state from real saves (deferred)

### Step 7: Testing
- [x] HRE command tests (24 tests in `step.rs`)
- [x] Ewiger Landfriede war blocking tests (3 tests)
- [x] Revoke Privilegia vassalization tests (2 tests)
- [x] Reform helper and membership tests (7 tests in `hre.rs`)
- [x] IA mechanics tests (7 tests)
- [x] Election tests (8 tests)

---

## Key Mechanics

| Component | Description |
|-----------|-------------|
| **Emperor** | Elected for life. Must be Christian, male, independent, not at war with HRE |
| **Electors** | 7 princes who vote. Can be appointed/removed by emperor |
| **Imperial Authority** | 0-100, accumulates monthly. Spent to pass reforms (50 IA each) |
| **Reforms** | Sequential improvements. Ewiger Landfriede bans internal wars |
| **Free Cities** | Give +0.005 IA/month. Max 12. Must be OPM. Can't be electors |

## Imperial Authority Formula

```
Monthly IA = Base + Prince Bonus + Free City Bonus - Heretic Penalty - Elector Penalty

Base (at peace):         +0.10
Per prince (>25):        +0.003 × (member_count - 25)
Per free city:           +0.005 each
Heretic princes:         -0.01 each
Missing electors (<7):   -0.10 each
```

## Commands

| Command | Description |
|---------|-------------|
| `JoinHRE` | Country joins empire (capital added to HRE) |
| `LeaveHRE` | Country leaves empire (capital removed from HRE) |
| `AddProvinceToHRE` | Add province to HRE territory |
| `RemoveProvinceFromHRE` | Remove province from HRE |
| `GrantElectorate` | Emperor grants elector status |
| `RemoveElectorate` | Remove elector status |
| `GrantFreeCity` | Make nation a free city (must be OPM, max 12) |
| `RevokeFreeCity` | Remove free city status |
| `PassImperialReform` | Pass reform (50 IA + elector majority) |
| `ImperialBan` | Emperor bans nation (unlocks CB) |

## Data Structures

```rust
pub struct HREState {
    pub emperor: Option<Tag>,
    pub electors: Vec<Tag>,           // Up to 7
    pub free_cities: HashSet<Tag>,    // Subset of members, max 12
    pub imperial_authority: Fixed,    // 0-100
    pub reforms_passed: Vec<ReformId>,
    pub official_religion: String,    // "catholic" initially
    pub dismantled: bool,
}
```

## Out of Scope (Phase 9+)

- Religious Leagues
- Imperial Incidents (Burgundian Inheritance, Shadow Kingdom)
- Dismantling the HRE
- Reform branch selection (decentralization vs centralization)
- Celestial Empire (China)

## References

- [EU4 Wiki: Holy Roman Empire](https://eu4.paradoxwikis.com/Holy_Roman_Empire)
- Gemini 2B review: `.antigravity/frolicking-purring-otter-review.md`
