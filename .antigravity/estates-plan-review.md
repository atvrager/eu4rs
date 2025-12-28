# Estate System Plan - Review Request

## Review Focus

Please review the estate system implementation plan, with special attention to:

1. **Countries without estates** - Some government types disable estates entirely (Pirate Republics, Native Councils, etc.)
2. **Countries without all 3 base estates** - Theocracies might not have Clergy, some setups vary
3. **Special estates only** - Some countries have regional estates but not the 3 core ones
4. **Estate availability gating** - How to properly gate estates by government type, religion, DLC

## Current Plan Summary

### Data Structures

```rust
/// Per-estate runtime state
pub struct EstateState {
    pub loyalty: Fixed,           // 0-100
    pub influence: Fixed,         // 0-100
    pub privileges: Vec<PrivilegeId>,
    pub land_share: Fixed,
    pub disaster_progress: u8,
}

/// All estate state for a country
pub struct CountryEstateState {
    pub estates: HashMap<EstateTypeId, EstateState>,  // Only present estates
    pub crown_land: Fixed,
    pub estates_enabled: bool,  // <-- This flag for disabled estates
}
```

### Current Assumptions

1. **Core 3 (Nobles, Clergy, Burghers)** always available for most countries
2. **Special estates** gated by `allowed_government_types` and `allowed_religions` in `EstateTypeDef`
3. **estates_enabled** flag to completely disable for certain governments

## Questions for Review

1. **Is `estates_enabled: bool` sufficient?** Or do we need more granular control (e.g., `Vec<EstateTypeId>` of available estates)?

2. **How to determine initial estates?** At game start, should we:
   - Check government type against each estate's trigger conditions?
   - Use a hardcoded mapping?
   - Parse estate trigger scripts?

3. **What happens when government changes?** If you reform from Pirate Republic to normal, estates should appear. Current plan doesn't handle this.

4. **Edge cases in EU4:**
   - Tribal governments: May have different estate composition
   - Theocracies: May lack Nobility or have different setup
   - Merchant Republics: Special rules for Burghers
   - Native Councils: No estates until reform
   - Revolutionary governments: Different estate rules

## Proposed Refinements

### Option A: Static Estate Lists Per Government Type
```rust
pub struct GovernmentEstateConfig {
    pub available_estates: Vec<EstateTypeId>,
    pub disabled: bool,
}

// In WorldState
pub government_estate_map: HashMap<GovernmentTypeId, GovernmentEstateConfig>,
```

### Option B: Dynamic Availability Check
```rust
impl EstateTypeDef {
    pub fn is_available_for(&self, country: &CountryState) -> bool {
        // Check government type
        if !self.allowed_government_types.is_empty()
            && !self.allowed_government_types.contains(&country.government_type) {
            return false;
        }
        // Check religion
        if !self.allowed_religions.is_empty()
            && !self.allowed_religions.contains(&country.religion) {
            return false;
        }
        // Check DLC (stub - always true for now)
        true
    }
}
```

### Option C: Hybrid - Cache Available Estates
```rust
/// Computed once on load or government change
pub struct CountryEstateState {
    pub estates: HashMap<EstateTypeId, EstateState>,
    pub available_estates: Vec<EstateTypeId>,  // Cached availability
    pub estates_globally_disabled: bool,
}
```

## What Does EU4 Actually Do?

From the game files, estates have `trigger = { ... }` blocks that define when they're available:

```
estate_nobles = {
    trigger = {
        NOT = { has_government_attribute = disables_estate_nobles }
        NOT = { has_reform = pirate_republic_reform }
        NOT = { has_reform = cossacks_reform }
        # etc.
    }
}
```

So the game checks conditions dynamically. We could:
1. Parse these triggers (complex - requires script engine)
2. Hardcode the common cases (simpler, covers 95%+)
3. Use a simplified attribute system (`government.disables_nobles = true`)

## Recommendation

I think **Option C with hardcoded common cases** is most practical:

1. Store `available_estates: Vec<EstateTypeId>` per country
2. Compute on game load based on government type
3. Recompute on government reform
4. Hardcode the well-known disabled cases:
   - Pirate Republic: No estates
   - Native Council: No estates until reform
   - Revolutionary: Modified estate list
5. Default to core 3 + applicable special estates

This avoids script parsing while handling the main edge cases.

## Please Review

1. Is this approach sound?
2. Any edge cases I'm missing?
3. Should we handle estate availability changes mid-game? (Government reforms)
4. Any concerns about the overall architecture?
