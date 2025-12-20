# Truce System Design

*Created: 2025-12-19*
*Status: Ready for Implementation*
*Target Tier: Minimal*

## Overview

After a peace deal, the warring parties cannot immediately re-declare war. A **truce** enforces a cooling-off period, preventing endless war cycles and giving the simulation more realistic diplomatic rhythms.

## Design Decisions

### Duration: 5 Years (Flat)

For the minimal tier, all truces last exactly **5 years** from the peace date.

**Rationale:**
- EU4 uses 5-15 years scaled by war score taken (complex)
- Flat 5 years is simple, testable, and sufficient for mid-term goal
- Can be upgraded to scaled duration in Medium tier

### Storage: Bilateral Truce Map

Add to `DiplomacyState`:

```rust
/// Active truces: (Tag1, Tag2) -> expiry date
/// Keys stored in sorted order (smaller tag first) to avoid duplication
pub truces: HashMap<(Tag, Tag), Date>,
```

**Key format:** Same as `relations` - alphabetically sorted pair ensures `(A, B)` and `(B, A)` resolve to the same key.

### Scope: Cross-Side Truces Only

When a war ends, truces are created between each attacker and each defender:

```
War: Attackers [A, B] vs Defenders [X, Y, Z]

Truces created:
  A ↔ X, A ↔ Y, A ↔ Z
  B ↔ X, B ↔ Y, B ↔ Z

NOT created:
  A ↔ B (same side)
  X ↔ Y, X ↔ Z, Y ↔ Z (same side)
```

### Enforcement: Block DeclareWar

The `DeclareWar` command handler must check for active truces:

```rust
// In execute_command for DeclareWar
if state.diplomacy.has_active_truce(country_tag, target, state.date) {
    return Err(ActionError::TruceActive {
        target: target.clone(),
        expires: truce_date,
    });
}
```

### Creation Points

Truces must be created at both peace resolution paths:

1. **AcceptPeace command** (step.rs ~line 705-710)
2. **Auto-end stale wars** (step.rs ~line 220-232)

### No Active Cleanup Needed

Truces naturally expire when `expiry_date <= current_date`. The check function handles this:

```rust
impl DiplomacyState {
    pub fn has_active_truce(&self, tag1: &str, tag2: &str, current_date: Date) -> bool {
        let key = Self::sorted_pair(tag1, tag2);
        self.truces
            .get(&key)
            .map(|expiry| *expiry > current_date)
            .unwrap_or(false)
    }

    fn sorted_pair(a: &str, b: &str) -> (String, String) {
        if a < b {
            (a.to_string(), b.to_string())
        } else {
            (b.to_string(), a.to_string())
        }
    }
}
```

Expired truces can remain in the map indefinitely (they're just ignored). Periodic cleanup is optional optimization.

---

## Implementation Checklist

### Phase 1: Data Structure

- [ ] Add `truces: HashMap<(Tag, Tag), Date>` to `DiplomacyState` in `state.rs`
- [ ] Add `#[derive(Default)]` compatible initialization (empty HashMap)
- [ ] Add `has_active_truce(&self, tag1, tag2, current_date) -> bool` method
- [ ] Add `create_truce(&mut self, tag1, tag2, expiry_date)` method
- [ ] Add `TruceActive { target, expires }` variant to `ActionError` enum

### Phase 2: Truce Creation

- [ ] Create helper function `create_war_truces(state, war, current_date)`
  ```rust
  fn create_war_truces(state: &mut WorldState, war: &War, current_date: Date) {
      let expiry = current_date.add_years(5);
      for attacker in &war.attackers {
          for defender in &war.defenders {
              state.diplomacy.create_truce(attacker, defender, expiry);
          }
      }
  }
  ```
- [ ] Call from `AcceptPeace` handler (after `execute_peace_terms`, before `wars.remove`)
- [ ] Call from `auto_end_stale_wars` (before `wars.remove`)

### Phase 3: Enforcement

- [ ] Add truce check to `DeclareWar` command handler
- [ ] Return `ActionError::TruceActive` if truce exists
- [ ] Test: declare war, peace, immediate re-declare should fail
- [ ] Test: declare war, peace, wait 5 years, re-declare should succeed

### Phase 4: AI Integration

- [ ] In `eu4sim/src/main.rs`, filter `DeclareWar` commands by truce status
- [ ] Only add `DeclareWar` to available commands if no active truce

### Phase 5: Checksum Integration

- [ ] Add truce data to `WorldState::compute_checksum()` for determinism
- [ ] Sort truces by key before hashing (HashMap iteration order is unstable)

---

## Test Cases

### Unit Tests (eu4sim-core)

```rust
#[test]
fn test_truce_blocks_war_declaration() {
    let mut state = WorldStateBuilder::new()
        .with_country("A")
        .with_country("B")
        .build();

    // Create truce expiring in 5 years
    let expiry = state.date.add_years(5);
    state.diplomacy.create_truce("A", "B", expiry);

    // Declare war should fail
    let result = execute_command(
        &mut state,
        "A",
        &Command::DeclareWar { target: "B".into(), cb: None }
    );
    assert!(matches!(result, Err(ActionError::TruceActive { .. })));
}

#[test]
fn test_truce_expires() {
    let mut state = WorldStateBuilder::new()
        .with_country("A")
        .with_country("B")
        .build();

    // Truce expired yesterday
    let yesterday = state.date.add_days(-1);
    state.diplomacy.create_truce("A", "B", yesterday);

    // Should not be active
    assert!(!state.diplomacy.has_active_truce("A", "B", state.date));
}

#[test]
fn test_peace_creates_truces() {
    // Setup war between A and B
    // Execute AcceptPeace
    // Verify truce exists between A and B
}
```

### Property Tests

```rust
proptest! {
    #[test]
    fn prop_truce_symmetric(a in "[A-Z]{3}", b in "[A-Z]{3}") {
        let mut diplomacy = DiplomacyState::default();
        let expiry = Date::new(1450, 1, 1);
        diplomacy.create_truce(&a, &b, expiry);

        let current = Date::new(1445, 1, 1);
        // Truce should be visible from both sides
        prop_assert_eq!(
            diplomacy.has_active_truce(&a, &b, current),
            diplomacy.has_active_truce(&b, &a, current)
        );
    }
}
```

---

## Edge Cases

### Full Annexation

When a country is fully annexed:
- Truces are still created with the dead country
- Since dead countries can't be revived in minimal tier, these truces are harmless
- No special handling needed

### Multi-Party Wars

Alliance members on the same side do NOT get truces with each other:
- `[A, B]` attacking `[X]` → truces: A-X, B-X
- A and B remain free to fight each other (if not allied)

### Overlapping Truces

If countries have multiple wars and peace separately:
- Later peace overwrites truce expiry with the later date
- `create_truce` should use `insert` (replaces existing)

---

## Future Enhancements (Medium Tier)

These are NOT in scope for the current implementation:

1. **Scaled Duration**: 5 + (war_score_taken / 10) years
2. **Truce-Breaking Penalty**: Stability hit, coalition risk
3. **Truce Display**: UI showing active truces and expiry dates
4. **Truce Transfer**: When country is vassalized, truces transfer to overlord

---

## Files to Modify

| File | Changes |
|------|---------|
| `eu4sim-core/src/state.rs` | Add `truces` field, helper methods |
| `eu4sim-core/src/step.rs` | Add `TruceActive` error, enforcement, creation |
| `eu4sim/src/main.rs` | Filter AI DeclareWar by truce status |

---

## Handoff Notes

This design is ready for implementation by a Tier 1.5/2 model (Gemini Flash or Sonnet).

**Key files to read first:**
1. `eu4sim-core/src/state.rs:287-327` - DiplomacyState structure
2. `eu4sim-core/src/step.rs:340-395` - DeclareWar handler
3. `eu4sim-core/src/step.rs:685-711` - AcceptPeace handler
4. `eu4sim-core/src/step.rs:204-233` - auto_end_stale_wars

**Implementation order:**
1. Data structure (Phase 1) - smallest, testable in isolation
2. Creation (Phase 2) - depends on Phase 1
3. Enforcement (Phase 3) - depends on Phase 1
4. AI integration (Phase 4) - depends on Phase 3
5. Checksum (Phase 5) - can be done anytime after Phase 1

**Estimated scope:** ~100-150 lines of new code, 5-6 files touched.
